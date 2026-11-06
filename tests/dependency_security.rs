#![cfg(feature = "gateway")]

use actix_http::{
    encoding::Decoder as ContentDecoder,
    error::PayloadError,
    h1::{Codec, Message},
    header::ContentEncoding,
};
use actix_web::{App, HttpResponse, http::StatusCode, test as web_test, web};
use bytes::{Bytes, BytesMut};
use futures::{StreamExt, stream};
use tokio_util::codec::Decoder;

// gzip.compress(b'x' * 4096, mtime=0): small input, substantially larger decoded body.
const GZIP_BODY: &[u8] = &[
    31, 139, 8, 0, 0, 0, 0, 0, 2, 19, 237, 193, 1, 13, 0, 0, 0, 194, 160, 218, 143, 111, 15, 7, 20,
    0, 0, 0, 240, 110, 193, 119, 16, 62, 0, 16, 0, 0,
];

fn chunked_request(tail: &[u8]) -> BytesMut {
    let mut bytes = BytesMut::from(
        &b"POST /upload HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n"[..],
    );
    bytes.extend_from_slice(tail);
    bytes
}

#[actix_web::test]
async fn malformed_chunked_trailer_cannot_consume_a_pipelined_request() {
    let mut bytes = chunked_request(b"0\r\n\nGET /next HTTP/1.1\r\nHost: localhost\r\n\r\n");
    let mut codec = Codec::default();
    assert!(matches!(
        codec.decode(&mut bytes),
        Ok(Some(Message::Item(_)))
    ));
    assert!(codec.decode(&mut bytes).is_err());
}

#[actix_web::test]
async fn valid_chunked_terminator_preserves_the_next_request() {
    let mut bytes = chunked_request(b"0\r\n\r\nGET /next HTTP/1.1\r\nHost: localhost\r\n\r\n");
    let mut codec = Codec::default();
    assert!(matches!(
        codec.decode(&mut bytes),
        Ok(Some(Message::Item(_)))
    ));
    assert!(matches!(
        codec.decode(&mut bytes),
        Ok(Some(Message::Chunk(None)))
    ));
    match codec.decode(&mut bytes).unwrap() {
        Some(Message::Item(request)) => assert_eq!(request.path(), "/next"),
        _ => panic!("valid chunked framing must preserve the following request"),
    }
}

#[actix_web::test]
async fn decompression_stops_at_the_decoded_byte_limit() {
    let input = stream::iter([Ok(Bytes::from_static(GZIP_BODY))]);
    let mut decoder = ContentDecoder::new(input, ContentEncoding::Gzip).limit(64);
    assert!(matches!(
        decoder.next().await,
        Some(Err(PayloadError::Overflow))
    ));
    assert!(decoder.next().await.is_none());
}

#[actix_web::test]
async fn compressed_payload_respects_extractor_limit_and_accepts_valid_body() {
    for (limit, expected_status) in [(64, StatusCode::PAYLOAD_TOO_LARGE), (4096, StatusCode::OK)] {
        let app =
            web_test::init_service(App::new().app_data(web::PayloadConfig::new(limit)).route(
                "/",
                web::post().to(|body: web::Bytes| async move {
                    HttpResponse::Ok().body(body.len().to_string())
                }),
            ))
            .await;
        let request = web_test::TestRequest::post()
            .uri("/")
            .insert_header(("content-encoding", "gzip"))
            .set_payload(GZIP_BODY)
            .to_request();
        let response = web_test::call_service(&app, request).await;
        assert_eq!(response.status(), expected_status);
        if expected_status == StatusCode::OK {
            assert_eq!(web_test::read_body(response).await, "4096");
        }
    }
}
