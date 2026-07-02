use crate::utils::data::utils::DataUtils;

// ==================== Base64 Tests ====================

#[test]
fn test_base64_operations() {
    let original = "Hello, World!";
    let encoded = DataUtils::encode_base64(original);
    assert!(DataUtils::is_base64_encoded(&encoded));

    let decoded = DataUtils::decode_base64(&encoded).unwrap();
    assert_eq!(decoded, original);

    assert!(!DataUtils::is_base64_encoded("not base64!"));
}

#[test]
fn test_base64_empty_string() {
    let empty = "";
    let encoded = DataUtils::encode_base64(empty);
    let decoded = DataUtils::decode_base64(&encoded).unwrap();
    assert_eq!(decoded, empty);
}

#[test]
fn test_base64_special_characters() {
    let special = "Hello\nWorld\t🌍";
    let encoded = DataUtils::encode_base64(special);
    let decoded = DataUtils::decode_base64(&encoded).unwrap();
    assert_eq!(decoded, special);
}

#[test]
fn test_get_base64_string() {
    let plain = "Hello";
    let result = DataUtils::get_base64_string(plain);
    assert_eq!(result, "SGVsbG8=");
}

#[test]
fn test_get_base64_string_with_valid_base64() {
    let encoded = DataUtils::encode_base64("test");
    let result = DataUtils::get_base64_string(&encoded);
    assert_eq!(result, encoded);
}

#[test]
fn test_decode_base64_invalid() {
    let invalid = "not-valid-base64!!!";
    let result = DataUtils::decode_base64(invalid);
    assert!(result.is_err());
}

#[test]
fn test_base64_binary_data() {
    let binary = "\x00\x01\x02\x7F";
    let encoded = DataUtils::encode_base64(binary);
    let decoded = DataUtils::decode_base64(&encoded).unwrap();
    assert_eq!(decoded, binary);
}

#[test]
fn test_base64_long_string() {
    let long_string = "A".repeat(10000);
    let encoded = DataUtils::encode_base64(&long_string);
    assert!(DataUtils::is_base64_encoded(&encoded));
    let decoded = DataUtils::decode_base64(&encoded).unwrap();
    assert_eq!(decoded, long_string);
}

#[test]
fn test_is_base64_edge_cases() {
    // Very short strings
    assert!(!DataUtils::is_base64_encoded("a"));
    assert!(!DataUtils::is_base64_encoded("ab"));

    // Valid base64 padding
    let valid = DataUtils::encode_base64("a"); // Should be "YQ=="
    assert!(DataUtils::is_base64_encoded(&valid));
}
