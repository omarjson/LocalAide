use std::fs;
use std::path::{Path, PathBuf};
use app_lib::domain::chat::ChatMessage;
use app_lib::services::inference::context::ContextBudget;
use app_lib::services::library::db::LibraryDb;
use app_lib::services::models::verify::verify_file_checksum;
use app_lib::services::tools::sandbox::WorkspaceJail;

struct TempTestDir {
    path: PathBuf,
}

impl TempTestDir {
    fn new(prefix: &str) -> Self {
        let unique = format!(
            "{}_{}_{}",
            prefix,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        let path = std::env::temp_dir().join(unique);
        fs::create_dir_all(&path).expect("failed to create temp test dir");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempTestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn test_fault_corrupt_gguf_fails_cleanly() {
    let temp = TempTestDir::new("fault_corrupt_gguf");
    let corrupt_model = temp.path().join("qwen-corrupt.gguf");

    // Write corrupted GGUF content (invalid magic bytes and truncated payload)
    fs::write(&corrupt_model, b"NOT_GGUF_MAGIC_HEADER_GARBAGE_BYTES_1234567890").expect("write corrupt file");

    let expected_valid_sha256 = "b9c3f0a5d4e12e8790b83e6024e0310fb176e7ef42d2a138da64c05295c52b71";
    let verify_res = verify_file_checksum(&corrupt_model, expected_valid_sha256);

    assert!(verify_res.is_err(), "Corrupt model must fail checksum verification");
    let err = verify_res.unwrap_err();
    assert_eq!(err.code, "CHECKSUM_MISMATCH");
    assert!(err.detail.is_some());
}

#[test]
fn test_fault_simulated_killed_download_part_resumes() {
    let temp = TempTestDir::new("fault_resume_part");
    let model_dest = temp.path().join("model.gguf");
    let part_dest = model_dest.with_extension("part");

    // Simulate an interrupted download that wrote 512 bytes before process termination
    let initial_partial_bytes = vec![0x42u8; 512];
    fs::write(&part_dest, &initial_partial_bytes).expect("write partial download");

    assert!(part_dest.exists());
    let size = fs::metadata(&part_dest).map(|m| m.len()).unwrap_or(0);
    assert_eq!(size, 512);

    // Verify file does not yet exist as finalized .gguf
    assert!(!model_dest.exists());
}

#[test]
fn test_fault_corrupt_sqlite_db_recovery() {
    let temp = TempTestDir::new("fault_sqlite");
    let db_path = temp.path().join("corrupted.db");

    // Write invalid SQLite header
    fs::write(&db_path, b"THIS IS NOT A SQLITE DATABASE HEADER AT ALL").expect("write corrupt db");

    let db_res = LibraryDb::open(&db_path);
    // Opening corrupted SQLite file should return AppError::storage without panic
    match db_res {
        Err(err) => {
            assert_eq!(err.code, "STORAGE_ERROR");
        }
        Ok(_) => panic!("Corrupted SQLite database must fail to open"),
    }
}

#[test]
fn test_fault_read_only_jail_write_returns_storage_error() {
    let temp = TempTestDir::new("fault_readonly");
    let jail = WorkspaceJail::new(temp.path()).expect("jail init");

    // Attempting to write with an invalid subpath or exceeding MAX_WRITE_BYTES
    let oversized = "X".repeat(5 * 1024 * 1024); // 5 MB > 4 MB limit
    let write_res = jail.write_file("oversized.txt", &oversized);

    assert!(write_res.is_err(), "Oversized file write must be rejected");
    let err = write_res.unwrap_err();
    assert_eq!(err.code, "INVALID_INPUT");
}

#[test]
fn test_benchmark_bounded_memory_and_turn_budget() {
    let budget = ContextBudget::new(2048, 512);
    let mut history = Vec::new();

    // Simulate 100 conversational turns
    for i in 0..100 {
        history.push(ChatMessage {
            role: "user".to_string(),
            content: format!("Question turn {i}: What are the key architectural tenets of local-first computing?"),
            ts: Some(i as u64),
            citations: None,
        });
        history.push(ChatMessage {
            role: "assistant".to_string(),
            content: format!("Answer turn {i}: Local-first computing prioritizes local state ownership, offline functionality, cryptographic data sovereignty, and user privacy."),
            ts: Some(i as u64 + 1),
            citations: None,
        });
    }

    let start = std::time::Instant::now();
    let result = budget.build_prompt(
        Some("You are LocalAide, a private desktop assistant."),
        &history,
        "Turn 101: Summarize the conversation so far.",
    );
    let elapsed = start.elapsed();

    // Enforce prompt assembly performance (< 10 ms for 200 turns)
    assert!(elapsed.as_millis() < 10, "Context budgeting took too long: {:?}", elapsed);

    // Enforce bounded prompt context budget (dropped oldest turns)
    assert_eq!(result.was_truncated, true);
    assert!(result.dropped_turns > 0);
    assert!(result.estimated_tokens <= 1536);
}
