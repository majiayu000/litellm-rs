use super::*;
use std::time::Duration;

// ==================== EnvUtils Tests ====================

#[test]
fn test_env_utils_get_or_default() {
    EnvUtils::set_env("TEST_VAR_1", "test_value");
    assert_eq!(
        EnvUtils::get_env_or_default("TEST_VAR_1", "default"),
        "test_value"
    );
    assert_eq!(
        EnvUtils::get_env_or_default("NON_EXISTENT_VAR", "default"),
        "default"
    );
    EnvUtils::remove_env("TEST_VAR_1");
}

#[test]
fn test_env_utils_get_required_env() {
    EnvUtils::set_env("REQUIRED_VAR", "required_value");
    let result = EnvUtils::get_required_env("REQUIRED_VAR");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "required_value");

    let missing = EnvUtils::get_required_env("MISSING_REQUIRED_VAR");
    assert!(missing.is_err());

    EnvUtils::remove_env("REQUIRED_VAR");
}

#[test]
fn test_env_utils_get_as_int() {
    EnvUtils::set_env("INT_VAR", "42");
    assert_eq!(EnvUtils::get_env_as_int("INT_VAR", 0).unwrap(), 42);

    EnvUtils::set_env("INVALID_INT", "not_a_number");
    assert!(EnvUtils::get_env_as_int("INVALID_INT", 0).is_err());

    assert_eq!(EnvUtils::get_env_as_int("MISSING_INT", 99).unwrap(), 99);

    EnvUtils::remove_env("INT_VAR");
    EnvUtils::remove_env("INVALID_INT");
}

#[test]
fn test_env_utils_get_as_int_negative() {
    EnvUtils::set_env("NEG_INT", "-42");
    assert_eq!(EnvUtils::get_env_as_int("NEG_INT", 0).unwrap(), -42);
    EnvUtils::remove_env("NEG_INT");
}

#[test]
fn test_env_utils_get_as_bool() {
    let true_values = ["true", "1", "yes", "on", "TRUE", "YES", "ON"];
    for (i, val) in true_values.iter().enumerate() {
        let key = format!("BOOL_VAR_{}", i);
        EnvUtils::set_env(&key, val);
        assert!(
            EnvUtils::get_env_as_bool(&key, false),
            "Failed for: {}",
            val
        );
        EnvUtils::remove_env(&key);
    }

    let false_values = ["false", "0", "no", "off", "random"];
    for (i, val) in false_values.iter().enumerate() {
        let key = format!("BOOL_FALSE_VAR_{}", i);
        EnvUtils::set_env(&key, val);
        assert!(
            !EnvUtils::get_env_as_bool(&key, true),
            "Failed for: {}",
            val
        );
        EnvUtils::remove_env(&key);
    }

    assert!(!EnvUtils::get_env_as_bool("MISSING_BOOL", false));
    assert!(EnvUtils::get_env_as_bool("MISSING_BOOL", true));
}

#[test]
fn test_env_utils_get_as_float() {
    EnvUtils::set_env("FLOAT_VAR", "1.234");
    let result = EnvUtils::get_env_as_float("FLOAT_VAR", 0.0).unwrap();
    assert!((result - 1.234).abs() < 0.001);

    EnvUtils::set_env("INVALID_FLOAT", "not_a_float");
    assert!(EnvUtils::get_env_as_float("INVALID_FLOAT", 0.0).is_err());

    let default = EnvUtils::get_env_as_float("MISSING_FLOAT", 1.5).unwrap();
    assert!((default - 1.5).abs() < 0.001);

    EnvUtils::remove_env("FLOAT_VAR");
    EnvUtils::remove_env("INVALID_FLOAT");
}

#[test]
fn test_env_utils_get_as_list() {
    EnvUtils::set_env("LIST_VAR", "a,b,c");
    assert_eq!(
        EnvUtils::get_env_as_list("LIST_VAR", vec![]),
        vec!["a", "b", "c"]
    );

    EnvUtils::set_env("LIST_WITH_SPACES", "a, b, c");
    assert_eq!(
        EnvUtils::get_env_as_list("LIST_WITH_SPACES", vec![]),
        vec!["a", "b", "c"]
    );

    EnvUtils::set_env("SINGLE_ITEM", "single");
    assert_eq!(
        EnvUtils::get_env_as_list("SINGLE_ITEM", vec![]),
        vec!["single"]
    );

    EnvUtils::set_env("EMPTY_ITEMS", "a,,b");
    let result = EnvUtils::get_env_as_list("EMPTY_ITEMS", vec![]);
    assert_eq!(result, vec!["a", "b"]);

    assert_eq!(
        EnvUtils::get_env_as_list("MISSING_LIST", vec!["default".to_string()]),
        vec!["default"]
    );

    EnvUtils::remove_env("LIST_VAR");
    EnvUtils::remove_env("LIST_WITH_SPACES");
    EnvUtils::remove_env("SINGLE_ITEM");
    EnvUtils::remove_env("EMPTY_ITEMS");
}

#[test]
fn test_env_utils_get_with_prefix() {
    EnvUtils::set_env("PREFIX_VAR1", "value1");
    EnvUtils::set_env("PREFIX_VAR2", "value2");
    EnvUtils::set_env("OTHER_VAR", "other");

    let prefixed = EnvUtils::get_env_with_prefix("PREFIX_");
    assert!(prefixed.contains_key("VAR1"));
    assert!(prefixed.contains_key("VAR2"));
    assert!(!prefixed.contains_key("OTHER_VAR"));

    EnvUtils::remove_env("PREFIX_VAR1");
    EnvUtils::remove_env("PREFIX_VAR2");
    EnvUtils::remove_env("OTHER_VAR");
}

// ==================== ConfigFileUtils Tests ====================

#[test]
fn test_file_exists() {
    assert!(ConfigFileUtils::file_exists("Cargo.toml"));
    assert!(!ConfigFileUtils::file_exists("nonexistent_file.txt"));
}

#[test]
fn test_get_file_extension() {
    assert_eq!(
        ConfigFileUtils::get_file_extension("config.yaml"),
        Some("yaml".to_string())
    );
    assert_eq!(
        ConfigFileUtils::get_file_extension("config.JSON"),
        Some("json".to_string())
    );
    assert_eq!(
        ConfigFileUtils::get_file_extension("config.YML"),
        Some("yml".to_string())
    );
    assert_eq!(ConfigFileUtils::get_file_extension("noextension"), None);
    assert_eq!(
        ConfigFileUtils::get_file_extension("path/to/file.toml"),
        Some("toml".to_string())
    );
}

#[test]
fn test_find_config_file() {
    // This file exists in the repo
    let found = ConfigFileUtils::find_config_file("Cargo.toml");
    assert!(found.is_some());

    let not_found = ConfigFileUtils::find_config_file("nonexistent_config.yaml");
    assert!(not_found.is_none());
}

// ==================== ConfigValidator Tests ====================

#[test]
fn test_validate_url() {
    assert!(ConfigValidator::validate_url("https://api.openai.com").is_ok());
    assert!(ConfigValidator::validate_url("http://localhost:8080").is_ok());
    assert!(ConfigValidator::validate_url("ftp://files.example.com").is_ok());
    assert!(ConfigValidator::validate_url("invalid-url").is_err());
    assert!(ConfigValidator::validate_url("").is_err());
}

#[test]
fn test_validate_email() {
    assert!(ConfigValidator::validate_email("test@example.com").is_ok());
    assert!(ConfigValidator::validate_email("user.name@domain.co.uk").is_ok());
    assert!(ConfigValidator::validate_email("user+tag@example.com").is_ok());
    assert!(ConfigValidator::validate_email("invalid-email").is_err());
    assert!(ConfigValidator::validate_email("@nodomain.com").is_err());
    assert!(ConfigValidator::validate_email("noat.com").is_err());
}

#[test]
fn test_validate_port() {
    assert!(ConfigValidator::validate_port(8080).is_ok());
    assert!(ConfigValidator::validate_port(1).is_ok());
    assert!(ConfigValidator::validate_port(65535).is_ok());
    assert!(ConfigValidator::validate_port(0).is_err());
}

#[test]
fn test_validate_positive_int() {
    assert!(ConfigValidator::validate_positive_int(1, "count").is_ok());
    assert!(ConfigValidator::validate_positive_int(100, "count").is_ok());
    assert!(ConfigValidator::validate_positive_int(0, "count").is_err());
    assert!(ConfigValidator::validate_positive_int(-1, "count").is_err());
}

#[test]
fn test_validate_range() {
    assert!(ConfigValidator::validate_range(5, 1, 10, "value").is_ok());
    assert!(ConfigValidator::validate_range(1, 1, 10, "value").is_ok());
    assert!(ConfigValidator::validate_range(10, 1, 10, "value").is_ok());
    assert!(ConfigValidator::validate_range(0, 1, 10, "value").is_err());
    assert!(ConfigValidator::validate_range(11, 1, 10, "value").is_err());

    // Test with floats
    assert!(ConfigValidator::validate_range(0.5, 0.0, 1.0, "ratio").is_ok());
    assert!(ConfigValidator::validate_range(1.5, 0.0, 1.0, "ratio").is_err());
}

#[test]
fn test_validate_string_length() {
    assert!(ConfigValidator::validate_string_length("hello", 1, 10, "name").is_ok());
    assert!(ConfigValidator::validate_string_length("a", 1, 10, "name").is_ok());
    assert!(ConfigValidator::validate_string_length("", 1, 10, "name").is_err());
    assert!(ConfigValidator::validate_string_length("this is too long", 1, 10, "name").is_err());
}

#[test]
fn test_validate_required() {
    let some_value: Option<i32> = Some(42);
    let none_value: Option<i32> = None;

    assert!(ConfigValidator::validate_required(&some_value, "field").is_ok());
    assert!(ConfigValidator::validate_required(&none_value, "field").is_err());
}

#[test]
fn test_validate_non_empty() {
    assert!(ConfigValidator::validate_non_empty("hello", "field").is_ok());
    assert!(ConfigValidator::validate_non_empty("  content  ", "field").is_ok());
    assert!(ConfigValidator::validate_non_empty("", "field").is_err());
    assert!(ConfigValidator::validate_non_empty("   ", "field").is_err());
}

#[test]
fn test_validate_alphanumeric() {
    assert!(ConfigValidator::validate_alphanumeric("hello123", "id").is_ok());
    assert!(ConfigValidator::validate_alphanumeric("hello_world", "id").is_ok());
    assert!(ConfigValidator::validate_alphanumeric("hello-world", "id").is_ok());
    assert!(ConfigValidator::validate_alphanumeric("hello_world-123", "id").is_ok());
    assert!(ConfigValidator::validate_alphanumeric("hello@world", "id").is_err());
    assert!(ConfigValidator::validate_alphanumeric("hello world", "id").is_err());
    assert!(ConfigValidator::validate_alphanumeric("hello.world", "id").is_err());
}

#[test]
fn test_validate_json() {
    assert!(ConfigValidator::validate_json(r#"{"key": "value"}"#).is_ok());
    assert!(ConfigValidator::validate_json(r#"[1, 2, 3]"#).is_ok());
    assert!(ConfigValidator::validate_json(r#"null"#).is_ok());
    assert!(ConfigValidator::validate_json(r#""string""#).is_ok());
    assert!(ConfigValidator::validate_json(r#"123"#).is_ok());
    assert!(ConfigValidator::validate_json("invalid json").is_err());
    assert!(ConfigValidator::validate_json(r#"{"key": }"#).is_err());
}

#[test]
fn test_validate_duration_string_seconds() {
    let result = ConfigValidator::validate_duration_string("30s").unwrap();
    assert_eq!(result, Duration::from_secs(30));

    let result = ConfigValidator::validate_duration_string("1s").unwrap();
    assert_eq!(result, Duration::from_secs(1));
}

#[test]
fn test_validate_duration_string_minutes() {
    let result = ConfigValidator::validate_duration_string("5m").unwrap();
    assert_eq!(result, Duration::from_secs(300));

    let result = ConfigValidator::validate_duration_string("1m").unwrap();
    assert_eq!(result, Duration::from_secs(60));
}

#[test]
fn test_validate_duration_string_hours() {
    let result = ConfigValidator::validate_duration_string("1h").unwrap();
    assert_eq!(result, Duration::from_secs(3600));

    let result = ConfigValidator::validate_duration_string("24h").unwrap();
    assert_eq!(result, Duration::from_secs(86400));
}

#[test]
fn test_validate_duration_string_days() {
    let result = ConfigValidator::validate_duration_string("1d").unwrap();
    assert_eq!(result, Duration::from_secs(86400));

    let result = ConfigValidator::validate_duration_string("7d").unwrap();
    assert_eq!(result, Duration::from_secs(604800));
}

#[test]
fn test_validate_duration_string_invalid() {
    assert!(ConfigValidator::validate_duration_string("invalid").is_err());
    assert!(ConfigValidator::validate_duration_string("30").is_err());
    assert!(ConfigValidator::validate_duration_string("s30").is_err());
    assert!(ConfigValidator::validate_duration_string("30x").is_err());
    assert!(ConfigValidator::validate_duration_string("").is_err());
}

#[test]
fn test_validate_duration_string_zero() {
    let result = ConfigValidator::validate_duration_string("0s").unwrap();
    assert_eq!(result, Duration::from_secs(0));
}

// ==================== ConfigFileUtils Async Tests ====================

#[tokio::test]
async fn test_read_file() {
    // Read an existing file
    let result = ConfigFileUtils::read_file("Cargo.toml").await;
    assert!(result.is_ok());
    let content = result.unwrap();
    assert!(content.contains("[package]"));
}

#[tokio::test]
async fn test_read_file_not_found() {
    let result = ConfigFileUtils::read_file("nonexistent_file.txt").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_write_and_read_file() {
    let temp_path = "/tmp/test_config_write.txt";
    let content = "test content\nline 2";

    let write_result = ConfigFileUtils::write_file(temp_path, content).await;
    assert!(write_result.is_ok());

    let read_result = ConfigFileUtils::read_file(temp_path).await;
    assert!(read_result.is_ok());
    assert_eq!(read_result.unwrap(), content);

    // Cleanup
    let _ = tokio::fs::remove_file(temp_path).await;
}

#[tokio::test]
async fn test_write_file_creates_directories() {
    let temp_path = "/tmp/test_nested/dir/config.txt";
    let content = "nested content";

    let write_result = ConfigFileUtils::write_file(temp_path, content).await;
    assert!(write_result.is_ok());

    let read_result = ConfigFileUtils::read_file(temp_path).await;
    assert!(read_result.is_ok());

    // Cleanup
    let _ = tokio::fs::remove_dir_all("/tmp/test_nested").await;
}

#[tokio::test]
async fn test_parse_json_file() {
    let temp_path = "/tmp/test_config.json";
    let json_content = r#"{"name": "test", "value": 42}"#;

    ConfigFileUtils::write_file(temp_path, json_content)
        .await
        .unwrap();

    #[derive(serde::Deserialize)]
    struct TestConfig {
        name: String,
        value: i32,
    }

    let result: Result<TestConfig> = ConfigFileUtils::parse_json_file(temp_path).await;
    assert!(result.is_ok());
    let config = result.unwrap();
    assert_eq!(config.name, "test");
    assert_eq!(config.value, 42);

    // Cleanup
    let _ = tokio::fs::remove_file(temp_path).await;
}

#[tokio::test]
async fn test_parse_yaml_file() {
    let temp_path = "/tmp/test_config.yaml";
    let yaml_content = "name: test\nvalue: 42";

    ConfigFileUtils::write_file(temp_path, yaml_content)
        .await
        .unwrap();

    #[derive(serde::Deserialize)]
    struct TestConfig {
        name: String,
        value: i32,
    }

    let result: Result<TestConfig> = ConfigFileUtils::parse_yaml_file(temp_path).await;
    assert!(result.is_ok());
    let config = result.unwrap();
    assert_eq!(config.name, "test");
    assert_eq!(config.value, 42);

    // Cleanup
    let _ = tokio::fs::remove_file(temp_path).await;
}

#[tokio::test]
async fn test_write_json_file() {
    let temp_path = "/tmp/test_write_config.json";

    #[derive(serde::Serialize)]
    struct TestConfig {
        name: String,
        value: i32,
    }

    let config = TestConfig {
        name: "test".to_string(),
        value: 123,
    };

    let result = ConfigFileUtils::write_json_file(temp_path, &config).await;
    assert!(result.is_ok());

    let content = ConfigFileUtils::read_file(temp_path).await.unwrap();
    assert!(content.contains("\"name\""));
    assert!(content.contains("test"));

    // Cleanup
    let _ = tokio::fs::remove_file(temp_path).await;
}

#[tokio::test]
async fn test_write_yaml_file() {
    let temp_path = "/tmp/test_write_config.yaml";

    #[derive(serde::Serialize)]
    struct TestConfig {
        name: String,
        value: i32,
    }

    let config = TestConfig {
        name: "test".to_string(),
        value: 456,
    };

    let result = ConfigFileUtils::write_yaml_file(temp_path, &config).await;
    assert!(result.is_ok());

    let content = ConfigFileUtils::read_file(temp_path).await.unwrap();
    assert!(content.contains("name:"));
    assert!(content.contains("test"));

    // Cleanup
    let _ = tokio::fs::remove_file(temp_path).await;
}
