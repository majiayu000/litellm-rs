use serde_json::Value;

pub struct StringOps;

impl StringOps {
    pub fn sanitize_for_json(input: &str) -> String {
        input
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
    }

    pub fn extract_json_from_string(input: &str) -> Option<Value> {
        let trimmed = input.trim();

        if let Some(start) = trimmed.find('{')
            && let Some(end) = trimmed.rfind('}')
        {
            let json_str = &trimmed[start..=end];
            return serde_json::from_str(json_str).ok();
        }

        if let Some(start) = trimmed.find('[')
            && let Some(end) = trimmed.rfind(']')
        {
            let json_str = &trimmed[start..=end];
            return serde_json::from_str(json_str).ok();
        }

        serde_json::from_str(trimmed).ok()
    }

    pub fn truncate_string(input: &str, max_length: usize) -> String {
        if input.len() <= max_length {
            input.to_string()
        } else {
            let mut truncated = input
                .chars()
                .take(max_length.saturating_sub(3))
                .collect::<String>();
            truncated.push_str("...");
            truncated
        }
    }

    pub fn extract_urls_from_text(text: &str) -> Vec<String> {
        let url_pattern = regex::Regex::new(
            r"https?://(?:[-\w.])+(?::[0-9]+)?(?:/(?:[\w/_.])*(?:\?(?:[\w&=%.])*)?(?:#(?:[\w.])*)?)?")
            .unwrap();

        url_pattern
            .find_iter(text)
            .map(|m| m.as_str().to_string())
            .collect()
    }

    pub fn clean_whitespace(text: &str) -> String {
        text.split_whitespace().collect::<Vec<&str>>().join(" ")
    }

    pub fn word_count(text: &str) -> usize {
        text.split_whitespace().count()
    }

    pub fn character_count(text: &str) -> usize {
        text.chars().count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== sanitize_for_json Tests ====================

    #[test]
    fn test_sanitize_for_json_backslash() {
        assert_eq!(
            StringOps::sanitize_for_json(r"path\to\file"),
            r"path\\to\\file"
        );
    }

    #[test]
    fn test_sanitize_for_json_quotes() {
        assert_eq!(
            StringOps::sanitize_for_json(r#"say "hello""#),
            r#"say \"hello\""#
        );
    }

    #[test]
    fn test_sanitize_for_json_newline() {
        assert_eq!(
            StringOps::sanitize_for_json("line1\nline2"),
            "line1\\nline2"
        );
    }

    #[test]
    fn test_sanitize_for_json_carriage_return() {
        assert_eq!(
            StringOps::sanitize_for_json("line1\rline2"),
            "line1\\rline2"
        );
    }

    #[test]
    fn test_sanitize_for_json_tab() {
        assert_eq!(StringOps::sanitize_for_json("col1\tcol2"), "col1\\tcol2");
    }

    #[test]
    fn test_sanitize_for_json_combined() {
        let input = "line1\nline2\t\"quoted\"\r\\end";
        let expected = "line1\\nline2\\t\\\"quoted\\\"\\r\\\\end";
        assert_eq!(StringOps::sanitize_for_json(input), expected);
    }

    #[test]
    fn test_sanitize_for_json_empty() {
        assert_eq!(StringOps::sanitize_for_json(""), "");
    }

    #[test]
    fn test_sanitize_for_json_no_escapes() {
        assert_eq!(StringOps::sanitize_for_json("hello world"), "hello world");
    }

    // ==================== extract_json_from_string Tests ====================

    #[test]
    fn test_extract_json_object() {
        let input = r#"Some text {"key": "value"} more text"#;
        let result = StringOps::extract_json_from_string(input);
        assert!(result.is_some());
        assert_eq!(result.unwrap()["key"], "value");
    }

    #[test]
    fn test_extract_json_array() {
        let input = r#"Before [1, 2, 3] after"#;
        let result = StringOps::extract_json_from_string(input);
        assert!(result.is_some());
        let arr = result.unwrap();
        assert!(arr.is_array());
        assert_eq!(arr.as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_extract_json_nested() {
        let input = r#"{"outer": {"inner": "value"}}"#;
        let result = StringOps::extract_json_from_string(input);
        assert!(result.is_some());
        assert_eq!(result.unwrap()["outer"]["inner"], "value");
    }

    #[test]
    fn test_extract_json_invalid() {
        let input = "This is not JSON at all";
        let result = StringOps::extract_json_from_string(input);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_json_malformed() {
        let input = "{incomplete";
        let result = StringOps::extract_json_from_string(input);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_json_empty_object() {
        let input = "{}";
        let result = StringOps::extract_json_from_string(input);
        assert!(result.is_some());
    }

    #[test]
    fn test_extract_json_with_whitespace() {
        let input = "   { \"key\": \"value\" }   ";
        let result = StringOps::extract_json_from_string(input);
        assert!(result.is_some());
    }

    // ==================== truncate_string Tests ====================

    #[test]
    fn test_truncate_string_shorter() {
        assert_eq!(StringOps::truncate_string("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_string_exact() {
        assert_eq!(StringOps::truncate_string("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_string_longer() {
        let result = StringOps::truncate_string("hello world", 8);
        assert_eq!(result, "hello...");
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_truncate_string_empty() {
        assert_eq!(StringOps::truncate_string("", 10), "");
    }

    #[test]
    fn test_truncate_string_very_short_max() {
        let result = StringOps::truncate_string("hello world", 4);
        assert_eq!(result, "h...");
    }

    #[test]
    fn test_truncate_string_unicode() {
        let result = StringOps::truncate_string("你好世界test", 5);
        assert!(result.ends_with("..."));
    }

    // ==================== extract_urls_from_text Tests ====================

    #[test]
    fn test_extract_urls_single() {
        let text = "Visit https://example.com for more";
        let urls = StringOps::extract_urls_from_text(text);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0], "https://example.com");
    }

    #[test]
    fn test_extract_urls_multiple() {
        let text = "Check https://example.com and http://test.org";
        let urls = StringOps::extract_urls_from_text(text);
        assert_eq!(urls.len(), 2);
    }

    #[test]
    fn test_extract_urls_with_path() {
        let text = "See https://example.com/path/to/page";
        let urls = StringOps::extract_urls_from_text(text);
        assert_eq!(urls.len(), 1);
        assert!(urls[0].contains("/path/to/page"));
    }

    #[test]
    fn test_extract_urls_none() {
        let text = "No URLs here";
        let urls = StringOps::extract_urls_from_text(text);
        assert!(urls.is_empty());
    }

    #[test]
    fn test_extract_urls_empty() {
        let urls = StringOps::extract_urls_from_text("");
        assert!(urls.is_empty());
    }

    // ==================== clean_whitespace Tests ====================

    #[test]
    fn test_clean_whitespace_multiple_spaces() {
        assert_eq!(StringOps::clean_whitespace("hello    world"), "hello world");
    }

    #[test]
    fn test_clean_whitespace_tabs_newlines() {
        assert_eq!(StringOps::clean_whitespace("hello\t\nworld"), "hello world");
    }

    #[test]
    fn test_clean_whitespace_leading_trailing() {
        assert_eq!(
            StringOps::clean_whitespace("  hello world  "),
            "hello world"
        );
    }

    #[test]
    fn test_clean_whitespace_empty() {
        assert_eq!(StringOps::clean_whitespace(""), "");
    }

    #[test]
    fn test_clean_whitespace_only_spaces() {
        assert_eq!(StringOps::clean_whitespace("     "), "");
    }

    // ==================== word_count Tests ====================

    #[test]
    fn test_word_count_simple() {
        assert_eq!(StringOps::word_count("hello world"), 2);
    }

    #[test]
    fn test_word_count_multiple_spaces() {
        assert_eq!(StringOps::word_count("hello    world    test"), 3);
    }

    #[test]
    fn test_word_count_empty() {
        assert_eq!(StringOps::word_count(""), 0);
    }

    #[test]
    fn test_word_count_single_word() {
        assert_eq!(StringOps::word_count("hello"), 1);
    }

    // ==================== character_count Tests ====================

    #[test]
    fn test_character_count_ascii() {
        assert_eq!(StringOps::character_count("hello"), 5);
    }

    #[test]
    fn test_character_count_unicode() {
        assert_eq!(StringOps::character_count("你好"), 2);
    }

    #[test]
    fn test_character_count_emoji() {
        assert_eq!(StringOps::character_count("hello 👋"), 7);
    }

    #[test]
    fn test_character_count_empty() {
        assert_eq!(StringOps::character_count(""), 0);
    }

    #[test]
    fn test_character_count_with_spaces() {
        assert_eq!(StringOps::character_count("hello world"), 11);
    }
}
