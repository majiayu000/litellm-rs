//! Utility functions for streaming

use super::types::Event;
use serde_json::json;

/// Parse SSE data line
pub fn parse_sse_line(line: &str) -> Option<String> {
    line.strip_prefix("data: ")
        .map(|stripped| stripped.to_string())
}

/// Check if SSE line indicates end of stream
pub fn is_done_marker(data: &str) -> bool {
    data == "[DONE]"
}

/// Check if SSE line indicates end of stream
pub fn is_done_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed == "data: [DONE]" || is_done_marker(trimmed)
}

/// Create an error event for SSE
pub fn create_error_event(error: &str) -> Event {
    Event::default()
        .event("error")
        .data(&json!({"error": error}).to_string())
}

/// Create a heartbeat event for SSE
pub fn create_heartbeat_event() -> Event {
    Event::default().event("heartbeat").data("ping")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== parse_sse_line Tests ====================

    #[test]
    fn test_parse_sse_line_valid() {
        let result = parse_sse_line("data: hello world");
        assert_eq!(result, Some("hello world".to_string()));
    }

    #[test]
    fn test_parse_sse_line_empty_data() {
        let result = parse_sse_line("data: ");
        assert_eq!(result, Some("".to_string()));
    }

    #[test]
    fn test_parse_sse_line_json_data() {
        let result = parse_sse_line("data: {\"key\": \"value\"}");
        assert_eq!(result, Some("{\"key\": \"value\"}".to_string()));
    }

    #[test]
    fn test_parse_sse_line_no_prefix() {
        let result = parse_sse_line("hello world");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_sse_line_wrong_prefix() {
        let result = parse_sse_line("event: message");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_sse_line_partial_prefix() {
        let result = parse_sse_line("data:no space");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_sse_line_done_signal() {
        let result = parse_sse_line("data: [DONE]");
        assert_eq!(result, Some("[DONE]".to_string()));
    }

    #[test]
    fn test_parse_sse_line_with_extra_spaces() {
        // "data: " prefix followed by spaces - parses successfully
        let result = parse_sse_line("data:   multiple spaces");
        // Actually matches "data: " and returns "  multiple spaces"
        assert_eq!(result, Some("  multiple spaces".to_string()));
    }

    #[test]
    fn test_parse_sse_line_no_space_after_colon() {
        // Without the space after "data:", it doesn't match
        let result = parse_sse_line("data:no_space");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_sse_line_complex_json() {
        let json_data = r#"{"choices":[{"delta":{"content":"Hello"},"index":0}]}"#;
        let line = format!("data: {}", json_data);
        let result = parse_sse_line(&line);
        assert_eq!(result, Some(json_data.to_string()));
    }

    // ==================== is_done_line Tests ====================

    #[test]
    fn test_is_done_marker_exact_match() {
        assert!(is_done_marker("[DONE]"));
    }

    #[test]
    fn test_is_done_marker_non_match() {
        assert!(!is_done_marker(" [DONE] "));
        assert!(!is_done_marker("done"));
    }

    #[test]
    fn test_is_done_line_with_data_prefix() {
        assert!(is_done_line("data: [DONE]"));
    }

    #[test]
    fn test_is_done_line_without_prefix() {
        assert!(is_done_line("[DONE]"));
    }

    #[test]
    fn test_is_done_line_with_whitespace() {
        assert!(is_done_line("  data: [DONE]  "));
        assert!(is_done_line("  [DONE]  "));
    }

    #[test]
    fn test_is_done_line_not_done() {
        assert!(!is_done_line("data: some content"));
        assert!(!is_done_line("hello"));
        assert!(!is_done_line(""));
    }

    #[test]
    fn test_is_done_line_case_sensitive() {
        // Note: The current implementation is case-sensitive
        assert!(!is_done_line("data: [done]"));
        assert!(!is_done_line("[Done]"));
    }

    #[test]
    fn test_is_done_line_partial_match() {
        assert!(!is_done_line("data: [DONE] extra"));
        assert!(!is_done_line("[DONE]extra"));
    }

    // ==================== create_error_event Tests ====================

    #[test]
    fn test_create_error_event_simple() {
        let event = create_error_event("Something went wrong");
        assert_eq!(event.event, Some("error".to_string()));
        assert!(event.data.contains("Something went wrong"));
    }

    #[test]
    fn test_create_error_event_json_structure() {
        let event = create_error_event("Test error");
        let parsed: serde_json::Value = serde_json::from_str(&event.data).unwrap();
        assert_eq!(parsed["error"], "Test error");
    }

    #[test]
    fn test_create_error_event_empty_message() {
        let event = create_error_event("");
        let parsed: serde_json::Value = serde_json::from_str(&event.data).unwrap();
        assert_eq!(parsed["error"], "");
    }

    #[test]
    fn test_create_error_event_special_characters() {
        let event = create_error_event("Error: \"quotes\" and 'apostrophes'");
        let parsed: serde_json::Value = serde_json::from_str(&event.data).unwrap();
        assert_eq!(parsed["error"], "Error: \"quotes\" and 'apostrophes'");
    }

    #[test]
    fn test_create_error_event_unicode() {
        let event = create_error_event("错误: エラー");
        let parsed: serde_json::Value = serde_json::from_str(&event.data).unwrap();
        assert_eq!(parsed["error"], "错误: エラー");
    }

    #[test]
    fn test_create_error_event_to_bytes() {
        let event = create_error_event("test");
        let bytes = event.to_bytes();
        let result = String::from_utf8_lossy(&bytes);
        assert!(result.contains("event: error"));
        assert!(result.contains("data: "));
    }

    // ==================== create_heartbeat_event Tests ====================

    #[test]
    fn test_create_heartbeat_event() {
        let event = create_heartbeat_event();
        assert_eq!(event.event, Some("heartbeat".to_string()));
        assert_eq!(event.data, "ping");
    }

    #[test]
    fn test_create_heartbeat_event_to_bytes() {
        let event = create_heartbeat_event();
        let bytes = event.to_bytes();
        let result = String::from_utf8_lossy(&bytes);
        assert_eq!(result, "event: heartbeat\ndata: ping\n\n");
    }

    #[test]
    fn test_create_heartbeat_event_multiple() {
        // Ensure each call creates a fresh event
        let event1 = create_heartbeat_event();
        let event2 = create_heartbeat_event();
        assert_eq!(event1.event, event2.event);
        assert_eq!(event1.data, event2.data);
    }

    // ==================== Integration Tests ====================

    #[test]
    fn test_parse_and_check_done() {
        let line = "data: [DONE]";
        let parsed = parse_sse_line(line);
        assert_eq!(parsed, Some("[DONE]".to_string()));
        assert!(is_done_line(line));
    }

    #[test]
    fn test_error_event_roundtrip() {
        let error_msg = "Connection timeout";
        let event = create_error_event(error_msg);

        // Verify event structure
        assert_eq!(event.event, Some("error".to_string()));

        // Verify data can be parsed back
        let parsed: serde_json::Value = serde_json::from_str(&event.data).unwrap();
        assert_eq!(parsed["error"].as_str().unwrap(), error_msg);
    }

    #[test]
    fn test_streaming_workflow() {
        // Simulate a streaming workflow
        let lines = [
            "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}",
            "data: {\"choices\":[{\"delta\":{\"content\":\" world\"}}]}",
            "data: [DONE]",
        ];

        for (i, line) in lines.iter().enumerate() {
            let parsed = parse_sse_line(line);
            assert!(parsed.is_some());

            if i == 2 {
                assert!(is_done_line(line));
            } else {
                assert!(!is_done_line(line));
            }
        }
    }
}
