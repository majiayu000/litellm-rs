use super::*;

// ==================== TranscriptionRequest Tests ====================

#[test]
fn test_transcription_request_basic() {
    let request = TranscriptionRequest {
        file: vec![1, 2, 3, 4],
        filename: "audio.mp3".to_string(),
        model: "whisper-1".to_string(),
        language: None,
        prompt: None,
        response_format: None,
        temperature: None,
        timestamp_granularities: None,
    };

    assert_eq!(request.model, "whisper-1");
    assert_eq!(request.filename, "audio.mp3");
    assert_eq!(request.file.len(), 4);
}

#[test]
fn test_transcription_request_with_options() {
    let request = TranscriptionRequest {
        file: vec![],
        filename: "speech.wav".to_string(),
        model: "whisper-large-v3".to_string(),
        language: Some("en".to_string()),
        prompt: Some("Meeting transcript".to_string()),
        response_format: Some("verbose_json".to_string()),
        temperature: Some(0.2),
        timestamp_granularities: Some(vec!["word".to_string(), "segment".to_string()]),
    };

    assert_eq!(request.language.as_deref(), Some("en"));
    assert_eq!(request.temperature, Some(0.2));
    assert_eq!(request.timestamp_granularities.as_ref().unwrap().len(), 2);
}

#[test]
fn test_transcription_request_clone() {
    let request = TranscriptionRequest {
        file: vec![1, 2, 3],
        filename: "test.mp3".to_string(),
        model: "whisper-1".to_string(),
        language: Some("fr".to_string()),
        prompt: None,
        response_format: None,
        temperature: None,
        timestamp_granularities: None,
    };

    let cloned = request.clone();
    assert_eq!(request.model, cloned.model);
    assert_eq!(request.file, cloned.file);
    assert_eq!(request.language, cloned.language);
}

#[test]
fn test_transcription_request_serialization_skips_file() {
    let request = TranscriptionRequest {
        file: vec![1, 2, 3, 4, 5],
        filename: "test.mp3".to_string(),
        model: "whisper-1".to_string(),
        language: None,
        prompt: None,
        response_format: None,
        temperature: None,
        timestamp_granularities: None,
    };

    let json = serde_json::to_value(&request).unwrap();
    // File should be skipped in serialization
    assert!(!json.as_object().unwrap().contains_key("file"));
    assert!(!json.as_object().unwrap().contains_key("filename"));
    assert_eq!(json["model"], "whisper-1");
}

// ==================== TranscriptionResponse Tests ====================

#[test]
fn test_transcription_response_basic() {
    let response = TranscriptionResponse {
        text: "Hello, world!".to_string(),
        task: None,
        language: None,
        duration: None,
        words: None,
        segments: None,
    };

    assert_eq!(response.text, "Hello, world!");
    assert!(response.task.is_none());
}

#[test]
fn test_transcription_response_verbose() {
    let response = TranscriptionResponse {
        text: "Hello, world!".to_string(),
        task: Some("transcribe".to_string()),
        language: Some("en".to_string()),
        duration: Some(5.5),
        words: Some(vec![
            WordInfo {
                word: "Hello".to_string(),
                start: 0.0,
                end: 0.5,
            },
            WordInfo {
                word: "world".to_string(),
                start: 0.6,
                end: 1.0,
            },
        ]),
        segments: None,
    };

    assert_eq!(response.task.as_deref(), Some("transcribe"));
    assert_eq!(response.duration, Some(5.5));
    assert_eq!(response.words.as_ref().unwrap().len(), 2);
}

#[test]
fn test_transcription_response_serialization() {
    let response = TranscriptionResponse {
        text: "Test".to_string(),
        task: Some("transcribe".to_string()),
        language: Some("en".to_string()),
        duration: Some(1.0),
        words: None,
        segments: None,
    };

    let json = serde_json::to_value(&response).unwrap();
    assert_eq!(json["text"], "Test");
    assert_eq!(json["task"], "transcribe");
    assert_eq!(json["language"], "en");
}

#[test]
fn test_transcription_response_deserialization() {
    let json = r#"{
            "text": "Hello from JSON",
            "task": "transcribe",
            "language": "en",
            "duration": 2.5
        }"#;

    let response: TranscriptionResponse = serde_json::from_str(json).unwrap();
    assert_eq!(response.text, "Hello from JSON");
    assert_eq!(response.duration, Some(2.5));
}

// ==================== WordInfo Tests ====================

#[test]
fn test_word_info_structure() {
    let word = WordInfo {
        word: "hello".to_string(),
        start: 0.0,
        end: 0.5,
    };

    assert_eq!(word.word, "hello");
    assert_eq!(word.start, 0.0);
    assert_eq!(word.end, 0.5);
}

#[test]
fn test_word_info_clone() {
    let word = WordInfo {
        word: "test".to_string(),
        start: 1.0,
        end: 1.5,
    };

    let cloned = word.clone();
    assert_eq!(word.word, cloned.word);
    assert_eq!(word.start, cloned.start);
    assert_eq!(word.end, cloned.end);
}

#[test]
fn test_word_info_serialization() {
    let word = WordInfo {
        word: "world".to_string(),
        start: 0.5,
        end: 1.0,
    };

    let json = serde_json::to_value(&word).unwrap();
    assert_eq!(json["word"], "world");
    assert_eq!(json["start"], 0.5);
    assert_eq!(json["end"], 1.0);
}

// ==================== SegmentInfo Tests ====================

#[test]
fn test_segment_info_structure() {
    let segment = SegmentInfo {
        id: 0,
        start: 0.0,
        end: 5.0,
        text: "First segment".to_string(),
    };

    assert_eq!(segment.id, 0);
    assert_eq!(segment.text, "First segment");
}

#[test]
fn test_segment_info_clone() {
    let segment = SegmentInfo {
        id: 1,
        start: 5.0,
        end: 10.0,
        text: "Second segment".to_string(),
    };

    let cloned = segment.clone();
    assert_eq!(segment.id, cloned.id);
    assert_eq!(segment.text, cloned.text);
}

#[test]
fn test_segment_info_serialization() {
    let segment = SegmentInfo {
        id: 2,
        start: 10.0,
        end: 15.0,
        text: "Third segment".to_string(),
    };

    let json = serde_json::to_value(&segment).unwrap();
    assert_eq!(json["id"], 2);
    assert_eq!(json["start"], 10.0);
    assert_eq!(json["text"], "Third segment");
}

// ==================== TranslationRequest Tests ====================

#[test]
fn test_translation_request_basic() {
    let request = TranslationRequest {
        file: vec![1, 2, 3],
        filename: "french.mp3".to_string(),
        model: "whisper-1".to_string(),
        prompt: None,
        response_format: None,
        temperature: None,
    };

    assert_eq!(request.model, "whisper-1");
    assert_eq!(request.filename, "french.mp3");
}

#[test]
fn test_translation_request_with_options() {
    let request = TranslationRequest {
        file: vec![],
        filename: "audio.wav".to_string(),
        model: "whisper-large-v3".to_string(),
        prompt: Some("Technical translation".to_string()),
        response_format: Some("json".to_string()),
        temperature: Some(0.3),
    };

    assert!(request.prompt.is_some());
    assert_eq!(request.temperature, Some(0.3));
}

#[test]
fn test_translation_request_clone() {
    let request = TranslationRequest {
        file: vec![1, 2],
        filename: "test.mp3".to_string(),
        model: "whisper-1".to_string(),
        prompt: Some("test".to_string()),
        response_format: None,
        temperature: None,
    };

    let cloned = request.clone();
    assert_eq!(request.model, cloned.model);
    assert_eq!(request.prompt, cloned.prompt);
}

// ==================== TranslationResponse Tests ====================

#[test]
fn test_translation_response_basic() {
    let response = TranslationResponse {
        text: "Hello in English".to_string(),
        task: None,
        language: None,
        duration: None,
        segments: None,
    };

    assert_eq!(response.text, "Hello in English");
}

#[test]
fn test_translation_response_verbose() {
    let response = TranslationResponse {
        text: "Translated text".to_string(),
        task: Some("translate".to_string()),
        language: Some("fr".to_string()),
        duration: Some(3.5),
        segments: Some(vec![SegmentInfo {
            id: 0,
            start: 0.0,
            end: 3.5,
            text: "Translated text".to_string(),
        }]),
    };

    assert_eq!(response.task.as_deref(), Some("translate"));
    assert_eq!(response.language.as_deref(), Some("fr"));
    assert_eq!(response.segments.as_ref().unwrap().len(), 1);
}

#[test]
fn test_translation_response_serialization() {
    let response = TranslationResponse {
        text: "Test translation".to_string(),
        task: Some("translate".to_string()),
        language: Some("de".to_string()),
        duration: Some(2.0),
        segments: None,
    };

    let json = serde_json::to_value(&response).unwrap();
    assert_eq!(json["text"], "Test translation");
    assert_eq!(json["task"], "translate");
}

// ==================== SpeechRequest Tests ====================

#[test]
fn test_speech_request_basic() {
    let request = SpeechRequest {
        input: "Hello, world!".to_string(),
        model: "tts-1".to_string(),
        voice: "alloy".to_string(),
        response_format: None,
        speed: None,
    };

    assert_eq!(request.input, "Hello, world!");
    assert_eq!(request.model, "tts-1");
    assert_eq!(request.voice, "alloy");
}

#[test]
fn test_speech_request_with_options() {
    let request = SpeechRequest {
        input: "Test speech synthesis".to_string(),
        model: "tts-1-hd".to_string(),
        voice: "nova".to_string(),
        response_format: Some("opus".to_string()),
        speed: Some(1.5),
    };

    assert_eq!(request.response_format.as_deref(), Some("opus"));
    assert_eq!(request.speed, Some(1.5));
}

#[test]
fn test_speech_request_clone() {
    let request = SpeechRequest {
        input: "Clone test".to_string(),
        model: "tts-1".to_string(),
        voice: "echo".to_string(),
        response_format: Some("mp3".to_string()),
        speed: Some(1.0),
    };

    let cloned = request.clone();
    assert_eq!(request.input, cloned.input);
    assert_eq!(request.voice, cloned.voice);
}

#[test]
fn test_speech_request_serialization() {
    let request = SpeechRequest {
        input: "Serialize test".to_string(),
        model: "tts-1".to_string(),
        voice: "fable".to_string(),
        response_format: None,
        speed: None,
    };

    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["input"], "Serialize test");
    assert_eq!(json["model"], "tts-1");
    assert_eq!(json["voice"], "fable");
    // Optional None fields should not be present
    assert!(!json.as_object().unwrap().contains_key("response_format"));
    assert!(!json.as_object().unwrap().contains_key("speed"));
}

#[test]
fn test_speech_request_deserialization() {
    let json = r#"{
            "input": "Test input",
            "model": "tts-1-hd",
            "voice": "shimmer",
            "response_format": "flac",
            "speed": 0.8
        }"#;

    let request: SpeechRequest = serde_json::from_str(json).unwrap();
    assert_eq!(request.input, "Test input");
    assert_eq!(request.model, "tts-1-hd");
    assert_eq!(request.voice, "shimmer");
    assert_eq!(request.response_format.as_deref(), Some("flac"));
    assert_eq!(request.speed, Some(0.8));
}

#[test]
fn test_speech_request_all_voices() {
    let voices = vec!["alloy", "echo", "fable", "onyx", "nova", "shimmer"];

    for voice in voices {
        let request = SpeechRequest {
            input: "Test".to_string(),
            model: "tts-1".to_string(),
            voice: voice.to_string(),
            response_format: None,
            speed: None,
        };
        assert_eq!(request.voice, voice);
    }
}

// ==================== SpeechResponse Tests ====================

#[test]
fn test_speech_response_structure() {
    let response = SpeechResponse {
        audio: vec![1, 2, 3, 4, 5],
        content_type: "audio/mpeg".to_string(),
    };

    assert_eq!(response.audio.len(), 5);
    assert_eq!(response.content_type, "audio/mpeg");
}

#[test]
fn test_speech_response_various_formats() {
    let formats = vec![
        ("audio/mpeg", "mp3"),
        ("audio/opus", "opus"),
        ("audio/flac", "flac"),
        ("audio/wav", "wav"),
    ];

    for (content_type, _format) in formats {
        let response = SpeechResponse {
            audio: vec![],
            content_type: content_type.to_string(),
        };
        assert_eq!(response.content_type, content_type);
    }
}

// ==================== supported_audio_formats Tests ====================

#[test]
fn test_supported_audio_formats_contains_common() {
    let formats = supported_audio_formats();

    assert!(formats.contains(&"mp3"));
    assert!(formats.contains(&"wav"));
    assert!(formats.contains(&"flac"));
    assert!(formats.contains(&"ogg"));
}

#[test]
fn test_supported_audio_formats_count() {
    let formats = supported_audio_formats();
    assert_eq!(formats.len(), 10);
}

#[test]
fn test_supported_audio_formats_all() {
    let formats = supported_audio_formats();
    let expected = vec![
        "flac", "m4a", "mp3", "mp4", "mpeg", "mpga", "oga", "ogg", "wav", "webm",
    ];

    for expected_format in expected {
        assert!(
            formats.contains(&expected_format),
            "Missing format: {}",
            expected_format
        );
    }
}

// ==================== format_to_content_type Tests ====================

#[test]
fn test_format_to_content_type_mp3() {
    assert_eq!(format_to_content_type("mp3"), "audio/mpeg");
    assert_eq!(format_to_content_type("MP3"), "audio/mpeg");
    assert_eq!(format_to_content_type("Mp3"), "audio/mpeg");
}

#[test]
fn test_format_to_content_type_opus() {
    assert_eq!(format_to_content_type("opus"), "audio/opus");
    assert_eq!(format_to_content_type("OPUS"), "audio/opus");
}

#[test]
fn test_format_to_content_type_aac() {
    assert_eq!(format_to_content_type("aac"), "audio/aac");
    assert_eq!(format_to_content_type("AAC"), "audio/aac");
}

#[test]
fn test_format_to_content_type_flac() {
    assert_eq!(format_to_content_type("flac"), "audio/flac");
    assert_eq!(format_to_content_type("FLAC"), "audio/flac");
}

#[test]
fn test_format_to_content_type_wav() {
    assert_eq!(format_to_content_type("wav"), "audio/wav");
    assert_eq!(format_to_content_type("WAV"), "audio/wav");
}

#[test]
fn test_format_to_content_type_pcm() {
    assert_eq!(format_to_content_type("pcm"), "audio/pcm");
    assert_eq!(format_to_content_type("PCM"), "audio/pcm");
}

#[test]
fn test_format_to_content_type_unknown() {
    // Unknown formats should default to audio/mpeg
    assert_eq!(format_to_content_type("xyz"), "audio/mpeg");
    assert_eq!(format_to_content_type("unknown"), "audio/mpeg");
    assert_eq!(format_to_content_type(""), "audio/mpeg");
}

#[test]
fn test_format_to_content_type_case_insensitive() {
    let formats = vec!["mp3", "opus", "aac", "flac", "wav", "pcm"];
    for format in formats {
        let lowercase = format_to_content_type(format);
        let uppercase = format_to_content_type(&format.to_uppercase());
        let mixed = format_to_content_type(
            &format
                .chars()
                .enumerate()
                .map(|(i, c)| {
                    if i % 2 == 0 {
                        c.to_uppercase().next().unwrap()
                    } else {
                        c
                    }
                })
                .collect::<String>(),
        );

        assert_eq!(lowercase, uppercase, "Case mismatch for {}", format);
        assert_eq!(lowercase, mixed, "Mixed case mismatch for {}", format);
    }
}

// ==================== Integration Tests ====================

#[test]
fn test_transcription_workflow() {
    // Create a request
    let request = TranscriptionRequest {
        file: vec![0; 1024], // Fake audio data
        filename: "meeting.mp3".to_string(),
        model: "whisper-1".to_string(),
        language: Some("en".to_string()),
        prompt: None,
        response_format: Some("verbose_json".to_string()),
        temperature: Some(0.0),
        timestamp_granularities: Some(vec!["word".to_string()]),
    };

    assert_eq!(request.file.len(), 1024);
    assert_eq!(request.response_format.as_deref(), Some("verbose_json"));

    // Simulate a response
    let response = TranscriptionResponse {
        text: "This is the meeting transcript.".to_string(),
        task: Some("transcribe".to_string()),
        language: Some("en".to_string()),
        duration: Some(60.0),
        words: Some(vec![
            WordInfo {
                word: "This".to_string(),
                start: 0.0,
                end: 0.2,
            },
            WordInfo {
                word: "is".to_string(),
                start: 0.2,
                end: 0.3,
            },
        ]),
        segments: None,
    };

    assert_eq!(response.text, "This is the meeting transcript.");
    assert!(response.words.is_some());
}

#[test]
fn test_tts_workflow() {
    // Create a request
    let request = SpeechRequest {
        input: "Welcome to the audio demo.".to_string(),
        model: "tts-1-hd".to_string(),
        voice: "nova".to_string(),
        response_format: Some("opus".to_string()),
        speed: Some(1.0),
    };

    // Get expected content type
    let content_type = format_to_content_type(request.response_format.as_deref().unwrap_or("mp3"));
    assert_eq!(content_type, "audio/opus");

    // Simulate a response
    let response = SpeechResponse {
        audio: vec![0; 2048], // Fake audio data
        content_type: content_type.to_string(),
    };

    assert_eq!(response.audio.len(), 2048);
    assert_eq!(response.content_type, "audio/opus");
}
