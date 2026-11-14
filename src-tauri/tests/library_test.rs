use app_lib::services::library::LibraryService;
use app_lib::services::library::db::{DbChunk, DbDocument, LibraryDb};
use app_lib::services::library::ingest::FileIngester;
use app_lib::services::library::search::{HybridSearcher, SearchMode};
use std::fs::File;
use std::io::Write;
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
fn test_sqlite_library_db_crud_and_fts() {
    let db = LibraryDb::open_in_memory().expect("In-memory SQLite should open");

    let doc = DbDocument {
        id: "doc_test_1".to_string(),
        path: "/tmp/sample.md".to_string(),
        title: "Rust Guide".to_string(),
        content_hash: "hash123".to_string(),
        size: 1024,
        modified: 1700000000,
        indexed_at: 1700000010,
        kind: "markdown".to_string(),
    };

    let chunks = vec![
        DbChunk {
            id: "doc_test_1_chunk_0".to_string(),
            document_id: "doc_test_1".to_string(),
            chunk_index: 0,
            text:
                "Rust is a systems programming language focused on safety, speed, and concurrency."
                    .to_string(),
            token_count: 12,
            start_char: 0,
            end_char: 81,
            embedding: None,
        },
        DbChunk {
            id: "doc_test_1_chunk_1".to_string(),
            document_id: "doc_test_1".to_string(),
            chunk_index: 1,
            text: "Ownership and borrowing guarantee memory safety without garbage collection."
                .to_string(),
            token_count: 9,
            start_char: 82,
            end_char: 157,
            embedding: None,
        },
    ];

    db.insert_or_replace_document(&doc, &chunks)
        .expect("Insert should succeed");

    // Verify stats
    let stats = db.get_stats().expect("Stats should succeed");
    assert_eq!(stats.doc_count, 1);
    assert_eq!(stats.chunk_count, 2);
    assert_eq!(stats.total_bytes, 1024);

    // Verify FTS search
    let hits = db
        .fts_search("ownership", 10)
        .expect("FTS search should succeed");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].title, "Rust Guide");
    assert!(hits[0].snippet.to_lowercase().contains("ownership"));

    // Verify delete
    let deleted = db
        .delete_document_by_path("/tmp/sample.md")
        .expect("Delete should succeed");
    assert!(deleted);
    let stats_after = db.get_stats().expect("Stats after delete should succeed");
    assert_eq!(stats_after.doc_count, 0);
    assert_eq!(stats_after.chunk_count, 0);
}

#[test]
fn test_text_chunking_with_overlap_and_boundaries() {
    let paragraph_1 =
        "First paragraph discussing system architectures and local-first computing principles.";
    let paragraph_2 = "Second paragraph elaborating on transactional storage engines, WAL mode, and fast retrieval.";
    let full_text = format!("{paragraph_1}\n\n{paragraph_2}");

    let chunks = FileIngester::chunk_text("doc_chunk_test", &full_text);
    assert!(!chunks.is_empty());
    assert_eq!(chunks[0].document_id, "doc_chunk_test");
    assert!(chunks[0].text.contains(paragraph_1));
}

#[test]
fn test_robustness_against_empty_and_binary_files() {
    let temp_dir = TempTestDir::new("robustness");

    // Empty file
    let empty_path = temp_dir.path().join("empty.txt");
    File::create(&empty_path).expect("Create empty file");
    let err_empty = FileIngester::ingest_file(&empty_path);
    assert!(err_empty.is_err(), "Empty file must be rejected");

    // Binary file
    let binary_path = temp_dir.path().join("binary.dat");
    let mut bin_file = File::create(&binary_path).expect("Create binary file");
    bin_file
        .write_all(&[0x7f, 0x45, 0x4c, 0x46, 0x00, 0x01, 0x01, 0x00])
        .expect("Write binary bytes");
    let err_bin = FileIngester::ingest_file(&binary_path);
    assert!(
        err_bin.is_err(),
        "Binary file with null bytes must be rejected"
    );
}

#[test]
fn test_hybrid_search_reciprocal_rank_fusion() {
    let db = Arc::new(LibraryDb::open_in_memory().expect("In-memory SQLite should open"));

    let doc = DbDocument {
        id: "doc_ai_1".to_string(),
        path: "/docs/local_ai.md".to_string(),
        title: "Local AI Principles".to_string(),
        content_hash: "hash_ai_1".to_string(),
        size: 512,
        modified: 1700001000,
        indexed_at: 1700001010,
        kind: "markdown".to_string(),
    };

    // Normalized 4-dimensional embedding vectors for testing
    let emb = vec![0.5, 0.5, 0.5, 0.5];
    let chunks = vec![DbChunk {
        id: "doc_ai_1_chunk_0".to_string(),
        document_id: "doc_ai_1".to_string(),
        chunk_index: 0,
        text: "Local inference provides complete privacy and zero network dependency.".to_string(),
        token_count: 9,
        start_char: 0,
        end_char: 70,
        embedding: Some(emb),
    }];

    db.insert_or_replace_document(&doc, &chunks)
        .expect("Insert doc");

    let searcher = HybridSearcher::new(db);

    // Keyword search
    let kw_results = searcher
        .search("privacy inference", None, SearchMode::Keyword, 5)
        .expect("Keyword search");
    assert_eq!(kw_results.len(), 1);
    assert_eq!(kw_results[0].title, "Local AI Principles");

    // Vector search
    let query_vec = vec![0.5, 0.5, 0.5, 0.5];
    let vec_results = searcher
        .search("", Some(&query_vec), SearchMode::Semantic, 5)
        .expect("Semantic search");
    assert_eq!(vec_results.len(), 1);
    assert!((vec_results[0].score - 1.0).abs() < 1e-4);

    // Hybrid search
    let start_time = std::time::Instant::now();
    let hybrid_results = searcher
        .search("privacy", Some(&query_vec), SearchMode::Hybrid, 5)
        .expect("Hybrid search");
    let latency = start_time.elapsed();
    assert_eq!(hybrid_results.len(), 1);
    assert!(
        latency.as_millis() < 50,
        "Search latency must be under 50ms"
    );
}

#[test]
fn test_folder_recursive_crawler_and_ignore_patterns() {
    let temp_dir = TempTestDir::new("crawler");
    let base_path = temp_dir.path();

    // Create supported text files
    let file1 = base_path.join("doc1.txt");
    let mut f1 = File::create(&file1).expect("create file 1");
    writeln!(f1, "Document one talking about databases and indexes.").expect("write");

    let sub_dir = base_path.join("sub");
    std::fs::create_dir_all(&sub_dir).expect("create subdir");
    let file2 = sub_dir.join("code.rs");
    let mut f2 = File::create(&file2).expect("create file 2");
    writeln!(f2, "pub fn add(a: i32, b: i32) -> i32 {{ a + b }}").expect("write");

    // Create ignored folder .git and node_modules
    let git_dir = base_path.join(".git");
    std::fs::create_dir_all(&git_dir).expect("create .git");
    let git_file = git_dir.join("config");
    let mut f_git = File::create(&git_file).expect("create git config");
    writeln!(f_git, "[core] repositoryformatversion = 0").expect("write");

    let service = LibraryService::open_in_memory().expect("open service");
    let (indexed, _skipped) = service.index_folder(base_path, None).expect("index folder");

    assert_eq!(indexed, 2, "Must index exactly doc1.txt and code.rs");

    // Search index
    let search_res = service
        .search("databases", None, SearchMode::Keyword, 10)
        .expect("search");
    assert_eq!(search_res.len(), 1);
    assert_eq!(search_res[0].title, "doc1.txt");
}
