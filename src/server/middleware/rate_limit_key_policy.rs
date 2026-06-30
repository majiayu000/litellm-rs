use crate::core::models::ApiKey;
use actix_web::HttpMessage;
use actix_web::dev::ServiceRequest;

pub(super) fn effective_requests_per_minute(
    req: &ServiceRequest,
    default_rpm: Option<u32>,
) -> Option<u32> {
    req.extensions()
        .get::<ApiKey>()
        .and_then(|api_key| api_key.rate_limits.as_ref())
        .and_then(|limits| limits.rpm)
        .or(default_rpm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::models::{ApiKey, Metadata, RateLimits, UsageStats};
    use actix_web::test::TestRequest;

    fn api_key_with_rpm(rpm: Option<u32>) -> ApiKey {
        ApiKey {
            metadata: Metadata::new(),
            name: "test-key".to_string(),
            key_hash: "hash".to_string(),
            key_prefix: "sk-test".to_string(),
            user_id: None,
            team_id: None,
            permissions: vec![],
            rate_limits: Some(RateLimits {
                rpm,
                tpm: None,
                rpd: None,
                tpd: None,
                concurrent: None,
            }),
            expires_at: None,
            is_active: true,
            last_used_at: None,
            usage_stats: UsageStats::default(),
        }
    }

    #[test]
    fn uses_api_key_rpm_before_default() {
        let req = TestRequest::default().to_srv_request();
        req.extensions_mut().insert(api_key_with_rpm(Some(2)));

        assert_eq!(effective_requests_per_minute(&req, Some(60)), Some(2));
    }

    #[test]
    fn falls_back_to_default_when_key_has_no_rpm() {
        let req = TestRequest::default().to_srv_request();
        req.extensions_mut().insert(api_key_with_rpm(None));

        assert_eq!(effective_requests_per_minute(&req, Some(60)), Some(60));
    }

    #[test]
    fn returns_none_without_key_rpm_or_default() {
        let req = TestRequest::default().to_srv_request();

        assert_eq!(effective_requests_per_minute(&req, None), None);
    }
}
