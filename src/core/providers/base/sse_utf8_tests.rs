//! Regression tests: multi-byte UTF-8 sequences split by network chunk
//! boundaries must survive UnifiedSSEParser without U+FFFD corruption.
//!
//! The parser previously lossy-decoded each network chunk independently;
//! the fix carries a trailing incomplete sequence into the next chunk.

use super::sse::{OpenAICompatibleTransformer, UnifiedSSEParser};

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

#[test]
fn multibyte_split_across_every_byte_boundary_survives() {
    let content = "中文emoji🚀✨";
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
    let got = collect(&mut parser, &payload);
    assert_eq!(got, content);
}
