//! Regression tests: multi-byte UTF-8 sequences split by network chunk
//! boundaries must survive UnifiedSSEParser without U+FFFD corruption.
//!
//! The parser previously lossy-decoded each network chunk independently;
//! the fix carries a trailing incomplete sequence into the next chunk.

use std::sync::{Arc, Mutex};

use bytes::Bytes;
use futures::StreamExt;

use super::sse::{OpenAICompatibleTransformer, SSETransformer, UnifiedSSEParser, UnifiedSSEStream};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::responses::ChatChunk;

fn frame(content: &str) -> String {
    format!(
        "data: {{\"id\":\"c1\",\"object\":\"chat.completion.chunk\",\"model\":\"m\",\"choices\":[{{\"index\":0,\"delta\":{{\"content\":\"{content}\"}}}}]}}\n\n"
    )
}

fn collect(parser: &mut UnifiedSSEParser<OpenAICompatibleTransformer>, bytes: &[u8]) -> String {
    let mut out = String::new();
    for b in bytes {
        // One byte per call exercises every possible chunk boundary.
        let chunks = parser.process_bytes(std::slice::from_ref(b)).unwrap();
        for c in chunks {
            if let Some(delta) = c.choices.first().and_then(|ch| ch.delta.content.clone()) {
                out.push_str(&delta);
            }
        }
    }
    out
}

fn collect_chunks(chunks: Vec<ChatChunk>) -> String {
    chunks
        .into_iter()
        .filter_map(|chunk| chunk.choices.first()?.delta.content.clone())
        .collect()
}

#[derive(Clone, Default)]
struct RecordingTransformer {
    calls: Arc<Mutex<Vec<String>>>,
    emit_event_chunk: bool,
    emit_finish_once: bool,
}

impl RecordingTransformer {
    fn emitting_event_chunk() -> Self {
        Self {
            emit_event_chunk: true,
            ..Self::default()
        }
    }

    fn emitting_finish_once() -> Self {
        Self {
            emit_finish_once: true,
            ..Self::default()
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    fn record(&self, kind: &str, data: &str) {
        self.calls.lock().unwrap().push(format!("{kind}:{data}"));
    }

    fn chunk(content: &str) -> Result<Option<ChatChunk>, ProviderError> {
        OpenAICompatibleTransformer::new("test").transform_chunk(&format!(
            r#"{{"id":"c1","object":"chat.completion.chunk","model":"m","choices":[{{"index":0,"delta":{{"content":"{content}"}}}}]}}"#
        ))
    }
}

impl SSETransformer for RecordingTransformer {
    fn provider_name(&self) -> &'static str {
        "test"
    }

    fn transform_chunk(&self, data: &str) -> Result<Option<ChatChunk>, ProviderError> {
        self.record("chunk", data);
        Ok(None)
    }

    fn transform_stream_chunk(&self, data: &str) -> Result<Option<ChatChunk>, ProviderError> {
        self.record("stream", data);
        if self.emit_event_chunk {
            Self::chunk("flushed")
        } else {
            Ok(None)
        }
    }

    fn finish_stream(&self) -> Result<Option<ChatChunk>, ProviderError> {
        let first_finish = {
            let mut calls = self.calls.lock().unwrap();
            let first = !calls.iter().any(|call| call == "finish");
            calls.push("finish".to_string());
            first
        };
        if self.emit_finish_once && first_finish {
            Self::chunk("finished")
        } else {
            Ok(None)
        }
    }
}

#[test]
fn multibyte_split_across_every_byte_boundary_survives() {
    let content = "é中文emoji🚀✨";
    let payload = frame(content).into_bytes();
    let mut parser = UnifiedSSEParser::new(OpenAICompatibleTransformer::new("test"));
    let got = collect(&mut parser, &payload);
    assert_eq!(got, content);
}

#[test]
fn whole_frame_in_one_chunk_still_works() {
    let content = "hello 你好";
    let payload = frame(content).into_bytes();
    let mut parser = UnifiedSSEParser::new(OpenAICompatibleTransformer::new("test"));
    let got = collect_chunks(parser.process_bytes(&payload).unwrap());
    assert_eq!(got, content);
}

#[test]
fn every_boundary_of_two_three_and_four_byte_characters_survives() {
    for content in ["é", "中", "🚀"] {
        let payload = frame(content).into_bytes();
        let character_start = payload
            .windows(content.len())
            .position(|window| window == content.as_bytes())
            .unwrap();

        for split in 1..content.len() {
            let split_at = character_start + split;
            let mut parser = UnifiedSSEParser::new(OpenAICompatibleTransformer::new("test"));
            assert!(
                parser
                    .process_bytes(&payload[..split_at])
                    .unwrap()
                    .is_empty()
            );
            let got = collect_chunks(parser.process_bytes(&payload[split_at..]).unwrap());
            assert_eq!(got, content, "failed {content:?} at byte {split}");
        }
    }
}

#[test]
fn earlier_invalid_byte_does_not_mask_split_trailing_sequence() {
    let transformer = RecordingTransformer::default();
    let mut parser = UnifiedSSEParser::new(transformer.clone());
    let mut first = b"data: earlier ".to_vec();
    first.extend_from_slice(&[0xff, 0xf0, 0x9f]);

    assert!(parser.process_bytes(&first).unwrap().is_empty());
    assert!(parser.process_bytes(b"\x9a\x80\n\n").unwrap().is_empty());
    assert_eq!(transformer.calls(), ["chunk:earlier �🚀"]);
}

#[test]
fn invalid_continuations_are_lossy_decoded_instead_of_carried() {
    for (first, second, expected) in [
        (
            &b"data: bad \xe2"[..],
            &b"(tail\n\n"[..],
            "chunk:bad �(tail",
        ),
        (
            &b"data: bad \x80"[..],
            &b" tail\n\n"[..],
            "chunk:bad � tail",
        ),
    ] {
        let transformer = RecordingTransformer::default();
        let mut parser = UnifiedSSEParser::new(transformer.clone());
        assert!(parser.process_bytes(first).unwrap().is_empty());
        assert!(parser.process_bytes(second).unwrap().is_empty());
        assert_eq!(transformer.calls(), [expected]);
    }
}

#[test]
fn invalid_scalar_encodings_flush_without_swallowing_a_valid_split_character() {
    for invalid in [
        &b"\xc0\x80"[..],
        &b"\xed\xa0\x80"[..],
        &b"\xf4\x90\x80\x80"[..],
    ] {
        let transformer = RecordingTransformer::default();
        let mut parser = UnifiedSSEParser::new(transformer.clone());
        let mut first = b"data: bad ".to_vec();
        first.extend_from_slice(invalid);
        first.extend_from_slice(b"\n\ndata: valid \xf0\x9f");

        assert!(parser.process_bytes(&first).unwrap().is_empty());
        assert_eq!(
            transformer.calls(),
            [format!("chunk:bad {}", String::from_utf8_lossy(invalid))]
        );
        assert!(parser.process_bytes(b"\x9a\x80\n\n").unwrap().is_empty());
        assert_eq!(
            transformer.calls().last().map(String::as_str),
            Some("chunk:valid 🚀")
        );
    }
}

#[test]
fn empty_chunk_preserves_a_split_sequence() {
    let transformer = RecordingTransformer::default();
    let mut parser = UnifiedSSEParser::new(transformer.clone());

    assert!(
        parser
            .process_bytes(b"data: empty \xf0\x9f")
            .unwrap()
            .is_empty()
    );
    assert!(parser.process_bytes(b"").unwrap().is_empty());
    assert!(parser.process_bytes(b"\x9a\x80\n\n").unwrap().is_empty());
    assert_eq!(transformer.calls(), ["chunk:empty 🚀"]);
}

#[tokio::test]
async fn done_and_eof_emit_an_idempotent_terminal_chunk_once() {
    let source = futures::stream::iter([Ok::<Bytes, reqwest::Error>(Bytes::from_static(
        b"data: [DONE]\n\n",
    ))]);
    let transformer = RecordingTransformer::emitting_finish_once();
    let mut stream = UnifiedSSEStream::new(source, transformer);

    let chunk = stream.next().await.unwrap().unwrap();
    assert_eq!(collect_chunks(vec![chunk]), "finished");
    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn eof_lossy_flushes_a_truncated_final_sequence_before_finishing() {
    let source = futures::stream::iter([Ok::<Bytes, reqwest::Error>(Bytes::from_static(
        b"data: truncated \xe2\x82",
    ))]);
    let transformer = RecordingTransformer::default();
    let mut stream = UnifiedSSEStream::new(source, transformer.clone());

    assert!(stream.next().await.is_none());
    assert_eq!(transformer.calls(), ["stream:truncated �", "finish"]);
}

#[tokio::test]
async fn stream_error_flushes_pending_utf8_before_returning_the_error() {
    let request_error = reqwest::Client::new()
        .get("://invalid")
        .build()
        .unwrap_err();
    let source = futures::stream::iter([
        Ok(Bytes::from_static(b"data: truncated \xf0\x9f\x92")),
        Err(request_error),
    ]);
    let transformer = RecordingTransformer::emitting_event_chunk();
    let mut stream = UnifiedSSEStream::new(source, transformer.clone());

    let chunk = stream.next().await.unwrap().unwrap();
    assert_eq!(collect_chunks(vec![chunk]), "flushed");
    let error = stream.next().await.unwrap().unwrap_err();
    assert!(error.to_string().contains("Stream error"));
    assert!(stream.next().await.is_none());
    assert_eq!(transformer.calls(), ["stream:truncated �", "finish"]);
}
