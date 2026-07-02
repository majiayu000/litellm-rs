use crate::utils::data::utils::DataUtils;

// ==================== String Utilities Tests ====================

#[test]
fn test_string_utilities() {
    assert_eq!(
        DataUtils::truncate_string("Hello, World!", 10),
        "Hello, ..."
    );
    assert_eq!(DataUtils::truncate_string("Short", 10), "Short");

    assert_eq!(
        DataUtils::clean_whitespace("  Hello   world  "),
        "Hello world"
    );

    assert_eq!(DataUtils::word_count("Hello world test"), 3);
    assert_eq!(DataUtils::character_count("Hello 🌍"), 7);
}

#[test]
fn test_truncate_string_exact_length() {
    assert_eq!(DataUtils::truncate_string("Hello", 5), "Hello");
}

#[test]
fn test_truncate_string_one_over() {
    assert_eq!(DataUtils::truncate_string("Hello!", 5), "He...");
}

#[test]
fn test_truncate_string_empty() {
    assert_eq!(DataUtils::truncate_string("", 10), "");
}

#[test]
fn test_truncate_string_very_short_limit() {
    assert_eq!(DataUtils::truncate_string("Hello", 3), "...");
}

#[test]
fn test_clean_whitespace_multiple_spaces() {
    assert_eq!(DataUtils::clean_whitespace("  a    b     c  "), "a b c");
}

#[test]
fn test_clean_whitespace_tabs_and_newlines() {
    assert_eq!(
        DataUtils::clean_whitespace("  hello\t\nworld  "),
        "hello world"
    );
}

#[test]
fn test_word_count_empty() {
    assert_eq!(DataUtils::word_count(""), 0);
}

#[test]
fn test_word_count_only_spaces() {
    assert_eq!(DataUtils::word_count("     "), 0);
}

#[test]
fn test_word_count_single_word() {
    assert_eq!(DataUtils::word_count("hello"), 1);
}

#[test]
fn test_character_count_empty() {
    assert_eq!(DataUtils::character_count(""), 0);
}

#[test]
fn test_character_count_unicode() {
    assert_eq!(DataUtils::character_count("你好世界"), 4);
    assert_eq!(DataUtils::character_count("🎉🎊🎁"), 3);
}

#[test]
fn test_sanitize_for_json() {
    let input = "Hello\n\"World\"\t\\test";
    let sanitized = DataUtils::sanitize_for_json(input);
    assert!(!sanitized.contains('\n'));
    assert!(!sanitized.contains('\t'));
}

#[test]
fn test_sanitize_for_json_empty() {
    assert_eq!(DataUtils::sanitize_for_json(""), "");
}

#[test]
fn test_sanitize_for_json_already_clean() {
    let input = "Hello World";
    assert_eq!(DataUtils::sanitize_for_json(input), input);
}

// ==================== URL Extraction Tests ====================

#[test]
fn test_url_extraction() {
    let text = "Check out https://example.com and http://test.org/path?query=1";
    let urls = DataUtils::extract_urls_from_text(text);
    assert_eq!(urls.len(), 2);
    assert!(urls.contains(&"https://example.com".to_string()));
    assert!(urls.contains(&"http://test.org/path?query=1".to_string()));
}

#[test]
fn test_url_extraction_no_urls() {
    let text = "This text has no URLs";
    let urls = DataUtils::extract_urls_from_text(text);
    assert!(urls.is_empty());
}

#[test]
fn test_url_extraction_empty_text() {
    let urls = DataUtils::extract_urls_from_text("");
    assert!(urls.is_empty());
}

#[test]
fn test_url_extraction_multiple_same() {
    let text = "Visit https://example.com and https://example.com again";
    let urls = DataUtils::extract_urls_from_text(text);
    // May contain duplicates depending on implementation
    assert!(urls.contains(&"https://example.com".to_string()));
}
