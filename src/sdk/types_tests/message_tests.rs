use crate::sdk::types::*;

// ==================== Role Tests ====================

#[test]
fn test_role_variants() {
    let system = Role::System;
    let user = Role::User;
    let assistant = Role::Assistant;
    let tool = Role::Tool;

    assert_eq!(system, Role::System);
    assert_eq!(user, Role::User);
    assert_eq!(assistant, Role::Assistant);
    assert_eq!(tool, Role::Tool);
}

#[test]
fn test_role_clone() {
    let role = Role::User;
    let cloned = role.clone();
    assert_eq!(role, cloned);
}

#[test]
fn test_role_serialization() {
    let role = Role::User;
    let json = serde_json::to_string(&role).unwrap();
    assert_eq!(json, "\"user\"");

    let system = Role::System;
    let system_json = serde_json::to_string(&system).unwrap();
    assert_eq!(system_json, "\"system\"");

    let assistant = Role::Assistant;
    let assistant_json = serde_json::to_string(&assistant).unwrap();
    assert_eq!(assistant_json, "\"assistant\"");

    let tool = Role::Tool;
    let tool_json = serde_json::to_string(&tool).unwrap();
    assert_eq!(tool_json, "\"tool\"");
}

#[test]
fn test_role_deserialization() {
    let user: Role = serde_json::from_str("\"user\"").unwrap();
    assert_eq!(user, Role::User);

    let system: Role = serde_json::from_str("\"system\"").unwrap();
    assert_eq!(system, Role::System);

    let assistant: Role = serde_json::from_str("\"assistant\"").unwrap();
    assert_eq!(assistant, Role::Assistant);

    let tool: Role = serde_json::from_str("\"tool\"").unwrap();
    assert_eq!(tool, Role::Tool);
}

#[test]
fn test_role_roundtrip() {
    let roles = vec![Role::System, Role::User, Role::Assistant, Role::Tool];
    for role in roles {
        let json = serde_json::to_string(&role).unwrap();
        let deserialized: Role = serde_json::from_str(&json).unwrap();
        assert_eq!(role, deserialized);
    }
}

// ==================== Content Tests ====================

#[test]
fn test_content_text() {
    let content = Content::Text("Hello, world!".to_string());
    if let Content::Text(text) = content {
        assert_eq!(text, "Hello, world!");
    } else {
        panic!("Expected Text content");
    }
}

#[test]
fn test_content_multimodal() {
    let parts = vec![ContentPart::Text {
        text: "Describe this image".to_string(),
    }];
    let content = Content::Multimodal(parts);
    if let Content::Multimodal(parts) = content {
        assert_eq!(parts.len(), 1);
    } else {
        panic!("Expected Multimodal content");
    }
}

#[test]
fn test_content_text_serialization() {
    let content = Content::Text("Hello".to_string());
    let json = serde_json::to_string(&content).unwrap();
    assert_eq!(json, "\"Hello\"");
}

#[test]
fn test_content_clone() {
    let content = Content::Text("test".to_string());
    let cloned = content.clone();
    if let (Content::Text(a), Content::Text(b)) = (&content, &cloned) {
        assert_eq!(a, b);
    }
}

// ==================== ContentPart Tests ====================

#[test]
fn test_content_part_text() {
    let part = ContentPart::Text {
        text: "Hello".to_string(),
    };
    if let ContentPart::Text { text } = part {
        assert_eq!(text, "Hello");
    } else {
        panic!("Expected Text part");
    }
}

#[test]
fn test_content_part_image() {
    let part = ContentPart::Image {
        image_url: ImageUrl {
            url: "https://example.com/image.png".to_string(),
            detail: Some("high".to_string()),
        },
    };
    if let ContentPart::Image { image_url } = part {
        assert_eq!(image_url.url, "https://example.com/image.png");
        assert_eq!(image_url.detail, Some("high".to_string()));
    } else {
        panic!("Expected Image part");
    }
}

#[test]
fn test_content_part_audio() {
    let part = ContentPart::Audio {
        audio: AudioData {
            data: "base64data".to_string(),
            format: Some("mp3".to_string()),
        },
    };
    if let ContentPart::Audio { audio } = part {
        assert_eq!(audio.data, "base64data");
        assert_eq!(audio.format, Some("mp3".to_string()));
    } else {
        panic!("Expected Audio part");
    }
}

#[test]
fn test_content_part_text_serialization() {
    let part = ContentPart::Text {
        text: "Hello".to_string(),
    };
    let json = serde_json::to_string(&part).unwrap();
    assert!(json.contains("\"type\":\"text\""));
    assert!(json.contains("\"text\":\"Hello\""));
}

#[test]
fn test_content_part_image_serialization() {
    let part = ContentPart::Image {
        image_url: ImageUrl {
            url: "https://example.com/img.png".to_string(),
            detail: None,
        },
    };
    let json = serde_json::to_string(&part).unwrap();
    assert!(json.contains("\"type\":\"image_url\""));
    assert!(json.contains("\"url\":\"https://example.com/img.png\""));
}

// ==================== ImageUrl Tests ====================

#[test]
fn test_image_url_creation() {
    let img = ImageUrl {
        url: "https://example.com/image.jpg".to_string(),
        detail: Some("low".to_string()),
    };
    assert_eq!(img.url, "https://example.com/image.jpg");
    assert_eq!(img.detail, Some("low".to_string()));
}

#[test]
fn test_image_url_no_detail() {
    let img = ImageUrl {
        url: "data:image/png;base64,abc123".to_string(),
        detail: None,
    };
    assert!(img.url.starts_with("data:image"));
    assert!(img.detail.is_none());
}

#[test]
fn test_image_url_clone() {
    let img = ImageUrl {
        url: "test.png".to_string(),
        detail: Some("auto".to_string()),
    };
    let cloned = img.clone();
    assert_eq!(img.url, cloned.url);
    assert_eq!(img.detail, cloned.detail);
}

// ==================== AudioData Tests ====================

#[test]
fn test_audio_data_creation() {
    let audio = AudioData {
        data: "base64encoded".to_string(),
        format: Some("wav".to_string()),
    };
    assert_eq!(audio.data, "base64encoded");
    assert_eq!(audio.format, Some("wav".to_string()));
}

#[test]
fn test_audio_data_no_format() {
    let audio = AudioData {
        data: "audiodata".to_string(),
        format: None,
    };
    assert_eq!(audio.data, "audiodata");
    assert!(audio.format.is_none());
}

#[test]
fn test_audio_data_clone() {
    let audio = AudioData {
        data: "data".to_string(),
        format: Some("mp3".to_string()),
    };
    let cloned = audio.clone();
    assert_eq!(audio.data, cloned.data);
    assert_eq!(audio.format, cloned.format);
}

// ==================== Message Tests ====================

#[test]
fn test_message_creation() {
    let msg = Message {
        role: Role::User,
        content: Some(Content::Text("Hello".to_string())),
        name: None,
        tool_calls: None,
    };
    assert_eq!(msg.role, Role::User);
    assert!(msg.content.is_some());
    assert!(msg.name.is_none());
    assert!(msg.tool_calls.is_none());
}

#[test]
fn test_message_with_name() {
    let msg = Message {
        role: Role::User,
        content: Some(Content::Text("Hi".to_string())),
        name: Some("John".to_string()),
        tool_calls: None,
    };
    assert_eq!(msg.name, Some("John".to_string()));
}

#[test]
fn test_message_system() {
    let msg = Message {
        role: Role::System,
        content: Some(Content::Text("You are a helpful assistant.".to_string())),
        name: None,
        tool_calls: None,
    };
    assert_eq!(msg.role, Role::System);
}

#[test]
fn test_message_clone() {
    let msg = Message {
        role: Role::Assistant,
        content: Some(Content::Text("Response".to_string())),
        name: None,
        tool_calls: None,
    };
    let cloned = msg.clone();
    assert_eq!(msg.role, cloned.role);
}

#[test]
fn test_message_serialization() {
    let msg = Message {
        role: Role::User,
        content: Some(Content::Text("Hello".to_string())),
        name: None,
        tool_calls: None,
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("\"role\":\"user\""));
    assert!(json.contains("\"content\":\"Hello\""));
}
