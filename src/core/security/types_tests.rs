use super::*;
use chrono::Utc;

// ====================================================================================
// PIIPattern Tests
// ====================================================================================

#[test]
fn test_pii_pattern_creation() {
    let pattern = PIIPattern {
        name: "email".to_string(),
        pattern: Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").unwrap(),
        replacement: PIIReplacement::Redact,
        confidence: 0.95,
    };
    assert_eq!(pattern.name, "email");
    assert_eq!(pattern.confidence, 0.95);
}

#[test]
fn test_pii_pattern_with_placeholder_replacement() {
    let pattern = PIIPattern {
        name: "ssn".to_string(),
        pattern: Regex::new(r"\d{3}-\d{2}-\d{4}").unwrap(),
        replacement: PIIReplacement::Placeholder("[SSN REDACTED]".to_string()),
        confidence: 0.99,
    };
    assert!(matches!(
        pattern.replacement,
        PIIReplacement::Placeholder(_)
    ));
}

#[test]
fn test_pii_pattern_with_partial_mask() {
    let pattern = PIIPattern {
        name: "credit_card".to_string(),
        pattern: Regex::new(r"\d{4}[- ]?\d{4}[- ]?\d{4}[- ]?\d{4}").unwrap(),
        replacement: PIIReplacement::PartialMask {
            keep_start: 4,
            keep_end: 4,
        },
        confidence: 0.98,
    };
    if let PIIReplacement::PartialMask {
        keep_start,
        keep_end,
    } = pattern.replacement
    {
        assert_eq!(keep_start, 4);
        assert_eq!(keep_end, 4);
    } else {
        panic!("Expected PartialMask");
    }
}

#[test]
fn test_pii_pattern_clone() {
    let pattern = PIIPattern {
        name: "phone".to_string(),
        pattern: Regex::new(r"\d{3}-\d{3}-\d{4}").unwrap(),
        replacement: PIIReplacement::Hash,
        confidence: 0.85,
    };
    let cloned = pattern.clone();
    assert_eq!(pattern.name, cloned.name);
    assert_eq!(pattern.confidence, cloned.confidence);
}

#[test]
fn test_pii_pattern_regex_match() {
    let pattern = PIIPattern {
        name: "email".to_string(),
        pattern: Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").unwrap(),
        replacement: PIIReplacement::Redact,
        confidence: 0.95,
    };
    assert!(pattern.pattern.is_match("test@example.com"));
    assert!(!pattern.pattern.is_match("not an email"));
}

// ====================================================================================
// PIIReplacement Tests
// ====================================================================================

#[test]
fn test_pii_replacement_redact() {
    let replacement = PIIReplacement::Redact;
    assert!(matches!(replacement, PIIReplacement::Redact));
}

#[test]
fn test_pii_replacement_placeholder() {
    let replacement = PIIReplacement::Placeholder("[HIDDEN]".to_string());
    if let PIIReplacement::Placeholder(text) = replacement {
        assert_eq!(text, "[HIDDEN]");
    } else {
        panic!("Expected Placeholder");
    }
}

#[test]
fn test_pii_replacement_hash() {
    let replacement = PIIReplacement::Hash;
    assert!(matches!(replacement, PIIReplacement::Hash));
}

#[test]
fn test_pii_replacement_remove() {
    let replacement = PIIReplacement::Remove;
    assert!(matches!(replacement, PIIReplacement::Remove));
}

#[test]
fn test_pii_replacement_partial_mask() {
    let replacement = PIIReplacement::PartialMask {
        keep_start: 2,
        keep_end: 3,
    };
    if let PIIReplacement::PartialMask {
        keep_start,
        keep_end,
    } = replacement
    {
        assert_eq!(keep_start, 2);
        assert_eq!(keep_end, 3);
    } else {
        panic!("Expected PartialMask");
    }
}

#[test]
fn test_pii_replacement_clone() {
    let replacement = PIIReplacement::Placeholder("test".to_string());
    let cloned = replacement.clone();
    if let (PIIReplacement::Placeholder(a), PIIReplacement::Placeholder(b)) = (replacement, cloned)
    {
        assert_eq!(a, b);
    }
}

// ====================================================================================
// ModerationRule Tests
// ====================================================================================

#[test]
fn test_moderation_rule_creation() {
    let rule = ModerationRule {
        name: "hate_speech_filter".to_string(),
        rule_type: ModerationType::HateSpeech,
        action: ModerationAction::Block,
        severity: ModerationSeverity::Critical,
    };
    assert_eq!(rule.name, "hate_speech_filter");
    assert!(matches!(rule.rule_type, ModerationType::HateSpeech));
    assert!(matches!(rule.action, ModerationAction::Block));
    assert!(matches!(rule.severity, ModerationSeverity::Critical));
}

#[test]
fn test_moderation_rule_clone() {
    let rule = ModerationRule {
        name: "test_rule".to_string(),
        rule_type: ModerationType::Violence,
        action: ModerationAction::Warn,
        severity: ModerationSeverity::Medium,
    };
    let cloned = rule.clone();
    assert_eq!(rule.name, cloned.name);
}

// ====================================================================================
// ModerationType Tests
// ====================================================================================

#[test]
fn test_moderation_type_hate_speech() {
    let mt = ModerationType::HateSpeech;
    assert!(matches!(mt, ModerationType::HateSpeech));
}

#[test]
fn test_moderation_type_violence() {
    let mt = ModerationType::Violence;
    assert!(matches!(mt, ModerationType::Violence));
}

#[test]
fn test_moderation_type_sexual() {
    let mt = ModerationType::Sexual;
    assert!(matches!(mt, ModerationType::Sexual));
}

#[test]
fn test_moderation_type_self_harm() {
    let mt = ModerationType::SelfHarm;
    assert!(matches!(mt, ModerationType::SelfHarm));
}

#[test]
fn test_moderation_type_harassment() {
    let mt = ModerationType::Harassment;
    assert!(matches!(mt, ModerationType::Harassment));
}

#[test]
fn test_moderation_type_illegal_activity() {
    let mt = ModerationType::IllegalActivity;
    assert!(matches!(mt, ModerationType::IllegalActivity));
}

#[test]
fn test_moderation_type_custom() {
    let mt = ModerationType::Custom("custom_category".to_string());
    if let ModerationType::Custom(category) = mt {
        assert_eq!(category, "custom_category");
    } else {
        panic!("Expected Custom");
    }
}

#[test]
fn test_moderation_type_clone() {
    let mt = ModerationType::Custom("test".to_string());
    let cloned = mt.clone();
    if let (ModerationType::Custom(a), ModerationType::Custom(b)) = (mt, cloned) {
        assert_eq!(a, b);
    }
}

// ====================================================================================
// ModerationAction Tests
// ====================================================================================

#[test]
fn test_moderation_action_block() {
    let action = ModerationAction::Block;
    assert!(matches!(action, ModerationAction::Block));
}

#[test]
fn test_moderation_action_warn() {
    let action = ModerationAction::Warn;
    assert!(matches!(action, ModerationAction::Warn));
}

#[test]
fn test_moderation_action_log() {
    let action = ModerationAction::Log;
    assert!(matches!(action, ModerationAction::Log));
}

#[test]
fn test_moderation_action_modify() {
    let action = ModerationAction::Modify;
    assert!(matches!(action, ModerationAction::Modify));
}

#[test]
fn test_moderation_action_human_review() {
    let action = ModerationAction::HumanReview;
    assert!(matches!(action, ModerationAction::HumanReview));
}

#[test]
fn test_moderation_action_clone() {
    let action = ModerationAction::Block;
    let cloned = action.clone();
    assert!(matches!(cloned, ModerationAction::Block));
}

// ====================================================================================
// ModerationSeverity Tests
// ====================================================================================

#[test]
fn test_moderation_severity_low() {
    let severity = ModerationSeverity::Low;
    assert!(matches!(severity, ModerationSeverity::Low));
}

#[test]
fn test_moderation_severity_medium() {
    let severity = ModerationSeverity::Medium;
    assert!(matches!(severity, ModerationSeverity::Medium));
}

#[test]
fn test_moderation_severity_high() {
    let severity = ModerationSeverity::High;
    assert!(matches!(severity, ModerationSeverity::High));
}

#[test]
fn test_moderation_severity_critical() {
    let severity = ModerationSeverity::Critical;
    assert!(matches!(severity, ModerationSeverity::Critical));
}

#[test]
fn test_moderation_severity_ordering() {
    assert!(ModerationSeverity::Low < ModerationSeverity::Medium);
    assert!(ModerationSeverity::Medium < ModerationSeverity::High);
    assert!(ModerationSeverity::High < ModerationSeverity::Critical);
}

#[test]
fn test_moderation_severity_equality() {
    assert_eq!(ModerationSeverity::Low, ModerationSeverity::Low);
    assert_ne!(ModerationSeverity::Low, ModerationSeverity::High);
}

#[test]
fn test_moderation_severity_clone() {
    let severity = ModerationSeverity::Critical;
    let cloned = severity.clone();
    assert_eq!(severity, cloned);
}

// ====================================================================================
// CustomFilter Tests
// ====================================================================================

#[test]
fn test_custom_filter_creation() {
    let filter = CustomFilter {
        name: "profanity_filter".to_string(),
        pattern: Regex::new(r"badword").unwrap(),
        action: ModerationAction::Block,
    };
    assert_eq!(filter.name, "profanity_filter");
    assert!(matches!(filter.action, ModerationAction::Block));
}

#[test]
fn test_custom_filter_regex_match() {
    let filter = CustomFilter {
        name: "test_filter".to_string(),
        pattern: Regex::new(r"secret\d+").unwrap(),
        action: ModerationAction::Log,
    };
    assert!(filter.pattern.is_match("secret123"));
    assert!(!filter.pattern.is_match("public"));
}

#[test]
fn test_custom_filter_clone() {
    let filter = CustomFilter {
        name: "test".to_string(),
        pattern: Regex::new(r"test").unwrap(),
        action: ModerationAction::Warn,
    };
    let cloned = filter.clone();
    assert_eq!(filter.name, cloned.name);
}

// ====================================================================================
// FilterResult Tests
// ====================================================================================

#[test]
fn test_filter_result_blocked() {
    let result = FilterResult {
        blocked: true,
        issues: vec![],
        modified_content: None,
        confidence: 0.99,
    };
    assert!(result.blocked);
    assert!(result.issues.is_empty());
}

#[test]
fn test_filter_result_with_issues() {
    let issue = ContentIssue {
        issue_type: "hate_speech".to_string(),
        description: "Detected hate speech".to_string(),
        severity: ModerationSeverity::High,
        location: Some((10, 20)),
        confidence: 0.95,
    };
    let result = FilterResult {
        blocked: true,
        issues: vec![issue],
        modified_content: None,
        confidence: 0.95,
    };
    assert_eq!(result.issues.len(), 1);
    assert_eq!(result.issues[0].issue_type, "hate_speech");
}

#[test]
fn test_filter_result_with_modified_content() {
    let result = FilterResult {
        blocked: false,
        issues: vec![],
        modified_content: Some("Cleaned content".to_string()),
        confidence: 0.8,
    };
    assert!(!result.blocked);
    assert_eq!(result.modified_content, Some("Cleaned content".to_string()));
}

#[test]
fn test_filter_result_clone() {
    let result = FilterResult {
        blocked: true,
        issues: vec![],
        modified_content: None,
        confidence: 0.9,
    };
    let cloned = result.clone();
    assert_eq!(result.blocked, cloned.blocked);
    assert_eq!(result.confidence, cloned.confidence);
}

// ====================================================================================
// ContentIssue Tests
// ====================================================================================

#[test]
fn test_content_issue_creation() {
    let issue = ContentIssue {
        issue_type: "violence".to_string(),
        description: "Contains violent content".to_string(),
        severity: ModerationSeverity::High,
        location: Some((0, 50)),
        confidence: 0.88,
    };
    assert_eq!(issue.issue_type, "violence");
    assert_eq!(issue.location, Some((0, 50)));
}

#[test]
fn test_content_issue_without_location() {
    let issue = ContentIssue {
        issue_type: "general".to_string(),
        description: "General issue".to_string(),
        severity: ModerationSeverity::Low,
        location: None,
        confidence: 0.5,
    };
    assert!(issue.location.is_none());
}

#[test]
fn test_content_issue_clone() {
    let issue = ContentIssue {
        issue_type: "test".to_string(),
        description: "Test issue".to_string(),
        severity: ModerationSeverity::Medium,
        location: Some((1, 2)),
        confidence: 0.7,
    };
    let cloned = issue.clone();
    assert_eq!(issue.issue_type, cloned.issue_type);
    assert_eq!(issue.location, cloned.location);
}

// ====================================================================================
// RetentionPolicy Tests
// ====================================================================================

#[test]
fn test_retention_policy_creation() {
    let policy = RetentionPolicy {
        data_type: "logs".to_string(),
        retention_days: 30,
        auto_delete: true,
        anonymization: None,
    };
    assert_eq!(policy.data_type, "logs");
    assert_eq!(policy.retention_days, 30);
    assert!(policy.auto_delete);
}

#[test]
fn test_retention_policy_with_anonymization() {
    let rule = AnonymizationRule {
        fields: vec!["email".to_string(), "name".to_string()],
        method: AnonymizationMethod::Hash,
    };
    let policy = RetentionPolicy {
        data_type: "user_data".to_string(),
        retention_days: 365,
        auto_delete: false,
        anonymization: Some(rule),
    };
    assert!(policy.anonymization.is_some());
}

#[test]
fn test_retention_policy_clone() {
    let policy = RetentionPolicy {
        data_type: "test".to_string(),
        retention_days: 7,
        auto_delete: true,
        anonymization: None,
    };
    let cloned = policy.clone();
    assert_eq!(policy.data_type, cloned.data_type);
    assert_eq!(policy.retention_days, cloned.retention_days);
}

// ====================================================================================
// ConsentManager Tests
// ====================================================================================

#[test]
fn test_consent_manager_creation() {
    let manager = ConsentManager {
        consents: HashMap::new(),
    };
    assert!(manager.consents.is_empty());
}

#[test]
fn test_consent_manager_with_consents() {
    let mut consents = HashMap::new();
    consents.insert(
        "user1".to_string(),
        UserConsent {
            user_id: "user1".to_string(),
            consented: true,
            timestamp: Utc::now(),
            version: "1.0".to_string(),
            permissions: vec!["data_collection".to_string()],
        },
    );
    let manager = ConsentManager { consents };
    assert_eq!(manager.consents.len(), 1);
    assert!(manager.consents.contains_key("user1"));
}

#[test]
fn test_consent_manager_clone() {
    let manager = ConsentManager {
        consents: HashMap::new(),
    };
    let cloned = manager.clone();
    assert_eq!(manager.consents.len(), cloned.consents.len());
}

// ====================================================================================
// UserConsent Tests
// ====================================================================================

#[test]
fn test_user_consent_creation() {
    let consent = UserConsent {
        user_id: "user123".to_string(),
        consented: true,
        timestamp: Utc::now(),
        version: "2.0".to_string(),
        permissions: vec!["analytics".to_string(), "marketing".to_string()],
    };
    assert_eq!(consent.user_id, "user123");
    assert!(consent.consented);
    assert_eq!(consent.permissions.len(), 2);
}

#[test]
fn test_user_consent_not_consented() {
    let consent = UserConsent {
        user_id: "user456".to_string(),
        consented: false,
        timestamp: Utc::now(),
        version: "1.0".to_string(),
        permissions: vec![],
    };
    assert!(!consent.consented);
    assert!(consent.permissions.is_empty());
}

#[test]
fn test_user_consent_clone() {
    let consent = UserConsent {
        user_id: "test".to_string(),
        consented: true,
        timestamp: Utc::now(),
        version: "1.0".to_string(),
        permissions: vec!["test".to_string()],
    };
    let cloned = consent.clone();
    assert_eq!(consent.user_id, cloned.user_id);
    assert_eq!(consent.consented, cloned.consented);
}

// ====================================================================================
// DataExportTools Tests
// ====================================================================================

#[test]
fn test_data_export_tools_creation() {
    let tools = DataExportTools {
        formats: vec![ExportFormat::Json, ExportFormat::Csv],
    };
    assert_eq!(tools.formats.len(), 2);
}

#[test]
fn test_data_export_tools_all_formats() {
    let tools = DataExportTools {
        formats: vec![
            ExportFormat::Json,
            ExportFormat::Csv,
            ExportFormat::Xml,
            ExportFormat::Pdf,
        ],
    };
    assert_eq!(tools.formats.len(), 4);
}

#[test]
fn test_data_export_tools_clone() {
    let tools = DataExportTools {
        formats: vec![ExportFormat::Json],
    };
    let cloned = tools.clone();
    assert_eq!(tools.formats.len(), cloned.formats.len());
}

// ====================================================================================
// ExportFormat Tests
// ====================================================================================

#[test]
fn test_export_format_json() {
    let format = ExportFormat::Json;
    assert!(matches!(format, ExportFormat::Json));
}

#[test]
fn test_export_format_csv() {
    let format = ExportFormat::Csv;
    assert!(matches!(format, ExportFormat::Csv));
}

#[test]
fn test_export_format_xml() {
    let format = ExportFormat::Xml;
    assert!(matches!(format, ExportFormat::Xml));
}

#[test]
fn test_export_format_pdf() {
    let format = ExportFormat::Pdf;
    assert!(matches!(format, ExportFormat::Pdf));
}

#[test]
fn test_export_format_clone() {
    let format = ExportFormat::Json;
    let cloned = format.clone();
    assert!(matches!(cloned, ExportFormat::Json));
}

// ====================================================================================
// AnonymizationRule Tests
// ====================================================================================

#[test]
fn test_anonymization_rule_creation() {
    let rule = AnonymizationRule {
        fields: vec!["email".to_string(), "phone".to_string()],
        method: AnonymizationMethod::Hash,
    };
    assert_eq!(rule.fields.len(), 2);
    assert!(matches!(rule.method, AnonymizationMethod::Hash));
}

#[test]
fn test_anonymization_rule_single_field() {
    let rule = AnonymizationRule {
        fields: vec!["ssn".to_string()],
        method: AnonymizationMethod::Remove,
    };
    assert_eq!(rule.fields.len(), 1);
}

#[test]
fn test_anonymization_rule_clone() {
    let rule = AnonymizationRule {
        fields: vec!["test".to_string()],
        method: AnonymizationMethod::Randomize,
    };
    let cloned = rule.clone();
    assert_eq!(rule.fields, cloned.fields);
}

// ====================================================================================
// AnonymizationMethod Tests
// ====================================================================================

#[test]
fn test_anonymization_method_randomize() {
    let method = AnonymizationMethod::Randomize;
    assert!(matches!(method, AnonymizationMethod::Randomize));
}

#[test]
fn test_anonymization_method_hash() {
    let method = AnonymizationMethod::Hash;
    assert!(matches!(method, AnonymizationMethod::Hash));
}

#[test]
fn test_anonymization_method_remove() {
    let method = AnonymizationMethod::Remove;
    assert!(matches!(method, AnonymizationMethod::Remove));
}

#[test]
fn test_anonymization_method_generalize() {
    let method = AnonymizationMethod::Generalize;
    assert!(matches!(method, AnonymizationMethod::Generalize));
}

#[test]
fn test_anonymization_method_clone() {
    let method = AnonymizationMethod::Hash;
    let cloned = method.clone();
    assert!(matches!(cloned, AnonymizationMethod::Hash));
}
