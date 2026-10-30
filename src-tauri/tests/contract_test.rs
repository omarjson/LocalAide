use app_lib::app::error::AppError;
use app_lib::app::events::ChatStreamEvent;
use app_lib::app::jobs::{JobRegistry, JobState};
use app_lib::domain::chat::SummarizeResponse;
use app_lib::domain::document::{IndexFolderResponse, IndexerStatsResponse, SearchResultDoc};
use app_lib::domain::model::{ModelInfo, ModelPreset};
use app_lib::domain::system::SystemProbeResult;

#[test]
fn test_app_error_serialization() {
    let err = AppError::model_not_installed("qwen3-0.6b-q4")
        .with_detail("File missing from disk")
        .retryable();

    let json = serde_json::to_string(&err).expect("Serialization must succeed");
    let v: serde_json::Value = serde_json::from_str(&json).expect("JSON parsing must succeed");

    assert_eq!(v["code"], "MODEL_NOT_INSTALLED");
    assert!(v["message"].as_str().unwrap().contains("qwen3-0.6b-q4"));
    assert_eq!(v["detail"], "File missing from disk");
    assert_eq!(v["retryable"], true);

    let roundtrip: AppError = serde_json::from_str(&json).expect("Deserialization must succeed");
    assert_eq!(roundtrip.code, "MODEL_NOT_INSTALLED");
    assert!(roundtrip.retryable);
}

#[test]
fn test_system_probe_serialization() {
    let probe = SystemProbeResult {
        total_ram_gb: 16.0,
        available_ram_gb: 12.0,
        gpu_vendor: Some("Apple".to_string()),
        recommended_preset: "balanced".to_string(),
        budget_gb: 9.8,
    };

    let json = serde_json::to_string(&probe).expect("Serialization must succeed");
    let roundtrip: SystemProbeResult = serde_json::from_str(&json).expect("Deserialization");
    assert_eq!(roundtrip, probe);
}

#[test]
fn test_model_info_serialization() {
    let model = ModelInfo {
        id: "qwen3-0.6b-q4".to_string(),
        name: "Qwen 3 0.6B".to_string(),
        filename: "qwen3-0.6b-q4.bin".to_string(),
        size_bytes: 400_000_000,
        ram_estimate_gb: 0.8,
        sha256: "abcdef123456".to_string(),
        download_url: "https://example.com/model.bin".to_string(),
        installed: true,
        verified: true,
        active: false,
        download_progress: Some(100.0),
    };

    let json = serde_json::to_string(&model).expect("Serialization must succeed");
    let roundtrip: ModelInfo = serde_json::from_str(&json).expect("Deserialization");
    assert_eq!(roundtrip, model);
}

#[test]
fn test_document_and_preset_serialization() {
    let preset = ModelPreset {
        id: "balanced".to_string(),
        name: "Balanced".to_string(),
        description: "Fits 8-12 GB".to_string(),
        models: vec!["qwen3-4b-q4".to_string()],
        ram_gb: 4.3,
    };
    let json_preset = serde_json::to_string(&preset).expect("Serialization");
    let roundtrip_preset: ModelPreset =
        serde_json::from_str(&json_preset).expect("Deserialization");
    assert_eq!(roundtrip_preset, preset);

    let doc = SearchResultDoc {
        path: "/path/to/doc.md".to_string(),
        title: "Test Note".to_string(),
        snippet: "This is a snippet...".to_string(),
        score: 0.95,
        modified: 1700000000,
        size: 1024,
    };
    let json_doc = serde_json::to_string(&doc).expect("Serialization");
    let roundtrip_doc: SearchResultDoc = serde_json::from_str(&json_doc).expect("Deserialization");
    assert_eq!(roundtrip_doc.path, doc.path);

    let stats = IndexerStatsResponse {
        doc_count: 10,
        total_bytes: 40960,
        storage_path: "/tmp/index".to_string(),
    };
    let json_stats = serde_json::to_string(&stats).expect("Serialization");
    let roundtrip_stats: IndexerStatsResponse =
        serde_json::from_str(&json_stats).expect("Deserialization");
    assert_eq!(roundtrip_stats.doc_count, 10);

    let sum = SummarizeResponse {
        summary: "This is a summary".to_string(),
    };
    let json_sum = serde_json::to_string(&sum).expect("Serialization");
    let roundtrip_sum: SummarizeResponse =
        serde_json::from_str(&json_sum).expect("Deserialization");
    assert_eq!(roundtrip_sum.summary, sum.summary);

    let ifr = IndexFolderResponse {
        indexed: 5,
        skipped: 1,
    };
    let json_ifr = serde_json::to_string(&ifr).expect("Serialization");
    let roundtrip_ifr: IndexFolderResponse =
        serde_json::from_str(&json_ifr).expect("Deserialization");
    assert_eq!(roundtrip_ifr.indexed, 5);
}

#[test]
fn test_chat_stream_event_serialization() {
    let token_evt = ChatStreamEvent::Token {
        token: "Hello".to_string(),
    };
    let json = serde_json::to_string(&token_evt).expect("Serialization");
    assert!(json.contains(r#""type":"token""#));
    assert!(json.contains(r#""token":"Hello""#));

    let done_evt = ChatStreamEvent::Done {
        finish_reason: "stop".to_string(),
        total_tokens: 42,
    };
    let json_done = serde_json::to_string(&done_evt).expect("Serialization");
    assert!(json_done.contains(r#""type":"done""#));
    assert!(json_done.contains(r#""total_tokens":42"#));
}

#[test]
fn test_job_registry_cancellation_and_progress() {
    let registry = JobRegistry::new();
    let (id, cancel_flag) = registry.create_job("download_test");

    assert!(!cancel_flag.load(std::sync::atomic::Ordering::Relaxed));
    let job = registry.get_job(&id).expect("Job should exist");
    assert_eq!(job.state, JobState::Running);
    assert_eq!(job.progress, 0.0);

    registry.update_progress(&id, 45.5, Some("Downloading...".to_string()));
    let job_updated = registry.get_job(&id).unwrap();
    assert_eq!(job_updated.progress, 45.5);
    assert_eq!(job_updated.message.as_deref(), Some("Downloading..."));

    assert!(registry.cancel_job(&id));
    assert!(cancel_flag.load(std::sync::atomic::Ordering::Relaxed));
    assert!(registry.is_cancelled(&id));

    let cancelled_job = registry.get_job(&id).unwrap();
    assert_eq!(cancelled_job.state, JobState::Cancelled);
}

#[test]
fn test_chat_thread_serialization() {
    let thread = app_lib::domain::chat::ChatThread {
        id: "t_1700000000".to_string(),
        title: "Release Planning".to_string(),
        system_prompt: Some("You are an expert project manager.".to_string()),
        created_at: 1700000000,
        updated_at: 1700000100,
        message_count: 4,
    };

    let json = serde_json::to_string(&thread).expect("Serialization must succeed");
    let roundtrip: app_lib::domain::chat::ChatThread =
        serde_json::from_str(&json).expect("Deserialization");
    assert_eq!(roundtrip, thread);
}
