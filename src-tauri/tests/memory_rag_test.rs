use app_lib::services::library::LibraryService;
use app_lib::services::memory::MemoryService;
use app_lib::services::memory::rag::MIN_RELEVANCE_SCORE;
use std::path::{Path, PathBuf};
use std::sync::Arc;

struct TempTestDir {
    path: PathBuf,
}

impl TempTestDir {
    fn new(prefix: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("localaide_{prefix}_{nanos}"));
        std::fs::create_dir_all(&path).expect("Create test dir");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempTestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[test]
fn test_memory_crud_and_disk_sync() {
    let temp_dir = TempTestDir::new("mem_crud");
    let library = Arc::new(LibraryService::open_in_memory().expect("Open library"));
    let memory = MemoryService::new(library.db(), library.clone(), temp_dir.path().to_path_buf());

    // 1. Create Note
    let note = memory
        .create_note(
            "Project Venus Architecture",
            "Project Venus utilizes SQLite WAL mode and a single state owner.",
            Some(vec!["architecture".to_string(), "database".to_string()]),
        )
        .expect("Create note");

    assert_eq!(note.title, "Project Venus Architecture");
    assert!(note.tags.contains(&"architecture".to_string()));
    assert!(
        Path::new(&note.path).exists(),
        "Markdown file must exist on disk"
    );

    // 2. Read Note
    let content = memory.read_note(&note.path).expect("Read note");
    assert!(content.contains("Project Venus utilizes SQLite WAL mode"));

    // 3. Update Note
    memory
        .update_note(
            &note.path,
            "Updated content with expanded memory recall algorithms.",
            Some(vec!["architecture".to_string(), "v2".to_string()]),
        )
        .expect("Update note");

    let updated_content = memory.read_note(&note.path).expect("Read updated note");
    assert!(updated_content.contains("expanded memory recall algorithms"));

    // 4. List Notes
    let list = memory.list_notes().expect("List notes");
    assert_eq!(list.len(), 1);
    assert_eq!(
        list[0].tags,
        vec!["architecture".to_string(), "v2".to_string()]
    );

    // 5. Delete Note
    let deleted = memory.delete_note(&note.path).expect("Delete note");
    assert!(deleted);
    assert!(
        !Path::new(&note.path).exists(),
        "File on disk must be removed"
    );
    let list_after = memory.list_notes().expect("List after delete");
    assert_eq!(list_after.len(), 0);
}

#[test]
fn test_memory_forget_all_cleans_disk_and_sqlite() {
    let temp_dir = TempTestDir::new("forget_all");
    let library = Arc::new(LibraryService::open_in_memory().expect("Open library"));
    let memory = MemoryService::new(library.db(), library.clone(), temp_dir.path().to_path_buf());

    memory
        .create_note("Secret Note 1", "Super secret contents 1", None)
        .expect("Create 1");
    memory
        .create_note("Secret Note 2", "Super secret contents 2", None)
        .expect("Create 2");

    let list_before = memory.list_notes().expect("List before");
    assert_eq!(list_before.len(), 2);

    let deleted_count = memory.forget_all().expect("Forget all");
    assert_eq!(deleted_count, 2);

    let list_after = memory.list_notes().expect("List after forget all");
    assert_eq!(
        list_after.len(),
        0,
        "SQLite notes table must be completely empty"
    );

    // Ensure zero markdown files remain on disk
    let disk_entries: Vec<_> = std::fs::read_dir(temp_dir.path())
        .expect("Read dir")
        .flatten()
        .collect();
    assert_eq!(disk_entries.len(), 0, "Disk folder must contain zero files");

    // Ensure library search returns zero documents
    let search_res = library.list().expect("Library list");
    assert_eq!(
        search_res.len(),
        0,
        "Library index must have zero documents"
    );
}

#[test]
fn test_rag_relevance_gate_and_citation_generation() {
    let temp_dir = TempTestDir::new("rag_citations");
    let library = Arc::new(LibraryService::open_in_memory().expect("Open library"));
    let memory = MemoryService::new(library.db(), library.clone(), temp_dir.path().to_path_buf());

    memory
        .create_note(
            "Rust Lifetime Rules",
            "Lifetimes ensure references do not outlive the data they point to in Rust.",
            Some(vec!["rust".to_string()]),
        )
        .expect("Create note");

    // Recall matching query
    let recall_res = memory
        .recall("references outlive Rust lifetimes", None, Some(512))
        .expect("Recall");
    assert!(
        !recall_res.citations.is_empty(),
        "Must recall relevant note"
    );
    assert_eq!(recall_res.citations[0].index, 1);
    assert!(recall_res.citations[0].snippet.contains("references"));
    assert!(recall_res.citations[0].snippet.contains("outlive"));
    assert!(recall_res.citations[0].score >= MIN_RELEVANCE_SCORE);
    assert!(recall_res.context_prompt.contains("[1] (Source:"));

    // Query completely unrelated to note (testing relevance gate)
    let unrelated_recall = memory
        .recall("quantum astrophysics dark matter galaxy", Some(512))
        .expect("Recall unrelated");
    assert_eq!(
        unrelated_recall.citations.len(),
        0,
        "Irrelevant query must produce zero citations"
    );
    assert!(
        unrelated_recall.context_prompt.is_empty(),
        "Irrelevant query must not inject context"
    );
}

#[test]
fn test_rag_strictly_respects_token_budget() {
    let temp_dir = TempTestDir::new("rag_budget");
    let library = Arc::new(LibraryService::open_in_memory().expect("Open library"));
    let memory = MemoryService::new(library.db(), library.clone(), temp_dir.path().to_path_buf());

    // Create a large note with 200 words
    let mut large_text = String::new();
    for i in 0..200 {
        large_text.push_str(&format!("word{} testing budget limits ", i));
    }
    memory
        .create_note("Budget Benchmark Note", &large_text, None)
        .expect("Create note");

    // Recall with tight token budget of 50 tokens
    let budget_res = memory
        .recall("testing budget limits", Some(50))
        .expect("Recall");
    assert!(
        budget_res.total_tokens <= 50,
        "Total tokens must not exceed budget: got {}",
        budget_res.total_tokens
    );
}
