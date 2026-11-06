//! Native Responses usage settlement within one admission window.

use super::*;

#[tokio::test]
async fn native_unknown_usage_retains_admission_but_known_zero_does_not() {
    use actix_web::body::MessageBody;
    use litellm_rs::core::router::DeploymentConfig;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    let mut backends = vec![None];
    if let Ok(url) = std::env::var("REDIS_URL") {
        backends.push(Some(url));
    }
    for redis_url in backends {
        for (streaming, terminal, output, known_zero) in [
            (true, false, true, false),
            (true, false, false, false),
            (true, true, true, false),
            (true, true, true, true),
            (false, true, true, false),
            (false, true, true, true),
        ] {
            let (state, upstream, handle) = fixture(StatusCode::OK, |config| {
                config.gateway.providers[0].name =
                    format!("native-admission-{}", uuid::Uuid::new_v4());
                if let Some(url) = &redis_url {
                    config.gateway.storage.redis.enabled = true;
                    config.gateway.storage.redis.url = url.clone();
                }
            })
            .await;
            let router = state.unified_router();
            let id = router
                .get_deployments_for_model("gpt-4o-mini")
                .pop()
                .unwrap();
            let deployment = router.get_deployment(&id).unwrap();
            router.add_deployment(deployment.as_ref().clone().with_config(DeploymentConfig {
                tpm_limit: Some(1000),
                ..deployment.config.clone()
            }));
            let deployment = router.get_deployment(&id).unwrap();
            let initial_health = deployment.state.health.load(Ordering::Relaxed);
            {
                let mut response = upstream.output.lock().unwrap();
                response["usage"] = if known_zero {
                    json!({"input_tokens":0,"output_tokens":0,"total_tokens":0})
                } else {
                    Value::Null
                };
                response["test_stall_stream"] = json!(!terminal);
                response["test_output_before_stall"] = json!(output);
            }
            let app = test::init_service(
                App::new()
                    .app_data(web::Data::new(state.clone()))
                    .configure(litellm_rs::server::routes::ai::configure_routes),
            )
            .await;
            // One positive input/output estimate fits 1000; two 700-output
            // reservations cannot fit. This tests retained quota through the route.
            let mut redis = match &redis_url {
                Some(url) => Some(
                    redis::Client::open(url.as_str())
                        .unwrap()
                        .get_multiplexed_async_connection()
                        .await
                        .unwrap(),
                ),
                None => None,
            };
            // Settled admission records expire at the end of the backend's
            // minute. Keep both requests and their assertions in that window.
            let second = if let Some(conn) = &mut redis {
                let (seconds, _micros): (u64, u64) =
                    redis::cmd("TIME").query_async(conn).await.unwrap();
                seconds % 60
            } else {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    % 60
            };
            if second >= 40 {
                tokio::time::sleep(Duration::from_secs(61 - second)).await;
            }
            let body = json!({"model":"gpt-4o-mini","stream":streaming,"store":false,"input":"x","max_output_tokens":700});
            let response = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/v1/responses")
                    .set_json(&body)
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
            if terminal {
                let _ = test::read_body(response).await;
            } else {
                let mut response_body = Box::pin(response.into_body());
                tokio::time::timeout(Duration::from_secs(5), async {
                    let mut wire = String::new();
                    loop {
                        let chunk =
                            futures::future::poll_fn(|cx| response_body.as_mut().poll_next(cx))
                                .await
                                .expect("stream must expose creation/output before cancellation")
                                .unwrap();
                        wire.push_str(&String::from_utf8_lossy(&chunk));
                        if wire.contains(if output {
                            "generated"
                        } else {
                            "response.created"
                        }) {
                            break;
                        }
                    }
                })
                .await
                .expect("native stream must expose the expected prefix promptly");
                drop(response_body);
            }
            tokio::time::timeout(Duration::from_secs(5), async {
                while deployment.state.active_requests.load(Ordering::Relaxed) != 0 {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .expect("native completion must release the original lease");
            // Acceptance is known even when only response.created reached the client.
            // Retain its estimate without inventing observed tokens or health.
            let retained = !known_zero;
            assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 1);
            assert_eq!(deployment.state.tpm_current.load(Ordering::Relaxed), 0);
            assert_eq!(
                deployment.state.success_requests.load(Ordering::Relaxed),
                u64::from(terminal)
            );
            assert_eq!(deployment.state.fail_requests.load(Ordering::Relaxed), 0);
            if !terminal {
                assert_eq!(
                    deployment.state.health.load(Ordering::Relaxed),
                    initial_health
                );
            }
            if let Some(conn) = &mut redis {
                // The gateway binds admission to its captured resource identity.
                // This fixture has a unique provider; inspect that exact namespace.
                let mut cursor = 0_u64;
                let mut keys = Vec::<String>::new();
                loop {
                    let (next, found): (u64, Vec<String>) = redis::cmd("SCAN")
                        .arg(cursor)
                        .arg("MATCH")
                        .arg(format!("litellm-rs:admission:v1:{id}:*"))
                        .arg("COUNT")
                        .arg(100)
                        .query_async(&mut *conn)
                        .await
                        .unwrap();
                    keys.extend(found);
                    cursor = next;
                    if cursor == 0 {
                        break;
                    }
                }
                keys.sort();
                keys.dedup();
                assert_eq!(keys.len(), 1, "one captured admission identity per fixture");
                let key = &keys[0];
                tokio::time::timeout(Duration::from_secs(5), async {
                    loop {
                        let counts: Vec<Option<i64>> = redis::cmd("HMGET")
                            .arg(key).arg(&["p", "r", "t"])
                            .query_async(&mut *conn).await.unwrap();
                        if counts[0] == Some(0) {
                            assert_eq!(counts[1], Some(1));
                            let tokens = counts[2].unwrap();
                            assert!(if retained { tokens > 700 && tokens < 1000 } else { tokens == 0 });
                            let fields: Vec<String> = redis::cmd("HKEYS").arg(key).query_async(&mut *conn).await.unwrap();
                            assert!(!fields.iter().any(|field| field.starts_with("l:") || field.starts_with("p:")));
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                }).await.expect("shared admission must retain/refund the original hold");
            }
            // The existing selection retry policy waits 60s after a quota
            // denial. Bound/cancel this waiter before the minute can roll over.
            let next = tokio::time::timeout(
                Duration::from_secs(if retained { 1 } else { 5 }),
                test::call_service(
                    &app,
                    test::TestRequest::post()
                        .uri("/v1/responses")
                        .set_json(&body)
                        .to_request(),
                ),
            )
            .await;
            if retained {
                assert!(
                    next.is_err(),
                    "retained estimate must deny the next admission"
                );
            } else {
                let next = next.expect("known-zero request must remain admissible");
                assert_eq!(next.status(), StatusCode::OK);
                drop(next);
            }
            assert_eq!(
                upstream.seen.lock().unwrap().len(),
                if retained { 1 } else { 2 }
            );
            tokio::time::timeout(Duration::from_secs(5), async {
                while deployment.state.active_requests.load(Ordering::Relaxed) != 0 {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .expect("follow-up must leave no active lease");
            handle.stop(false).await;
        }
    }
}
