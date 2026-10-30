use app_lib::domain::tool::{ToolAction, ToolActionType, ToolExecutionRequest};
use app_lib::services::library::db::LibraryDb;
use app_lib::services::models::catalog::CatalogManifest;
use app_lib::services::models::verify::verify_file_checksum;
use app_lib::services::tools::ToolService;
use app_lib::services::tools::sandbox::WorkspaceJail;
use sha2::Digest;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

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
fn test_workspace_jail_blocks_traversal_escapes() {
    let temp = TempTestDir::new("jail_traversal");
    let jail = WorkspaceJail::new(temp.path()).expect("jail init");

    // Traversal attempts
    let res1 = jail.resolve_path("../escaped.txt");
    assert!(res1.is_err());
    let err1 = res1.unwrap_err();
    assert_eq!(err1.code, "SANDBOX_DENIED");

    let res2 = jail.resolve_path("subfolder/../../escaped.txt");
    assert!(res2.is_err());
    assert_eq!(res2.unwrap_err().code, "SANDBOX_DENIED");
}

#[test]
fn test_workspace_jail_blocks_absolute_paths_outside_root() {
    let temp = TempTestDir::new("jail_absolute");
    let jail = WorkspaceJail::new(temp.path()).expect("jail init");

    let res = jail.resolve_path("/etc/passwd");
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().code, "SANDBOX_DENIED");
}

#[test]
fn test_workspace_jail_blocks_symlinks_pointing_outside() {
    let outside_dir = TempTestDir::new("jail_outside");
    let secret_file = outside_dir.path().join("secret.txt");
    fs::write(&secret_file, "super_secret_credentials").expect("write secret");

    let jail_dir = TempTestDir::new("jail_inside");
    let symlink_path = jail_dir.path().join("leak_link.txt");

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&secret_file, &symlink_path).expect("create symlink");
        let jail = WorkspaceJail::new(jail_dir.path()).expect("jail init");

        let res = jail.read_file("leak_link.txt");
        assert!(
            res.is_err(),
            "Symlink pointing outside jail must be blocked"
        );
        let err = res.unwrap_err();
        assert_eq!(err.code, "SANDBOX_DENIED");
    }
}

#[test]
fn test_workspace_jail_detects_symlink_loops() {
    let jail_dir = TempTestDir::new("jail_loop");
    let link_a = jail_dir.path().join("loop_a");
    let link_b = jail_dir.path().join("loop_b");

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&link_b, &link_a).expect("create link a");
        std::os::unix::fs::symlink(&link_a, &link_b).expect("create link b");

        let jail = WorkspaceJail::new(jail_dir.path()).expect("jail init");
        let res = jail.resolve_path("loop_a");
        assert!(res.is_err(), "Symlink loop must be caught and rejected");
        let err = res.unwrap_err();
        assert_eq!(err.code, "SANDBOX_DENIED");
    }
}

#[test]
fn test_workspace_jail_permits_valid_read_and_atomic_write() {
    let temp = TempTestDir::new("jail_valid");
    let jail = WorkspaceJail::new(temp.path()).expect("jail init");

    let write_res = jail.write_file("sub/note.txt", "Hello secure sandbox");
    assert!(write_res.is_ok());
    assert_eq!(write_res.unwrap(), 20);

    let (content, truncated) = jail.read_file("sub/note.txt").expect("read file");
    assert_eq!(content, "Hello secure sandbox");
    assert!(!truncated);

    let list = jail.list_dir(Some("sub")).expect("list dir");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "note.txt");
}

#[test]
fn test_tool_execution_approval_workflow_and_audit_logging() {
    let temp = TempTestDir::new("jail_tool_svc");
    let jail = Arc::new(WorkspaceJail::new(temp.path()).expect("jail init"));
    let db = Arc::new(LibraryDb::open_in_memory().expect("db init"));
    let service = ToolService::new(jail, db);

    // 1. Unapproved write file -> requires_approval: true
    let req_unapproved = ToolExecutionRequest {
        tool_name: "fs_tool".to_string(),
        action: ToolAction {
            action_type: ToolActionType::WriteFile,
            path: "test.txt".to_string(),
            content: Some("Write without consent".to_string()),
            query: None,
        },
        approved: None,
        session_token: None,
    };

    let res_unapproved = service.execute(req_unapproved).expect("execute");
    assert!(res_unapproved.requires_approval);
    assert!(!res_unapproved.success);

    // 2. Approved write file -> succeeds and writes file
    let req_approved = ToolExecutionRequest {
        tool_name: "fs_tool".to_string(),
        action: ToolAction {
            action_type: ToolActionType::WriteFile,
            path: "test.txt".to_string(),
            content: Some("Approved write content".to_string()),
            query: None,
        },
        approved: Some(true),
        session_token: None,
    };

    let res_approved = service.execute(req_approved).expect("execute");
    assert!(!res_approved.requires_approval);
    assert!(res_approved.success);

    // 3. Read file -> safe and auto-approved
    let req_read = ToolExecutionRequest {
        tool_name: "fs_tool".to_string(),
        action: ToolAction {
            action_type: ToolActionType::ReadFile,
            path: "test.txt".to_string(),
            content: None,
            query: None,
        },
        approved: None,
        session_token: None,
    };

    let res_read = service.execute(req_read).expect("read execute");
    assert!(res_read.success);
    assert!(res_read.output.contains("Approved write content"));

    // 4. Verify audit entries in SQLite
    let audits = service.list_audits(10).expect("list audits");
    assert!(audits.len() >= 2);
    assert_eq!(audits[0].action, "read_file");
    assert_eq!(audits[0].status, "success");
}

#[test]
fn test_model_catalog_signature_and_checksum_verification() {
    let manifest = CatalogManifest::default_manifest();
    assert!(manifest.verify().is_ok());

    // Corrupt signature
    let mut corrupt_manifest = manifest.clone();
    corrupt_manifest.signature = "invalid_signature_hex".to_string();
    assert!(corrupt_manifest.verify().is_err());

    // Checksum verification
    let temp = TempTestDir::new("verify_checksum");
    let test_file = temp.path().join("model.bin");
    fs::write(&test_file, b"test model binary content").expect("write file");

    // Correct SHA-256
    let actual_sha = sha2::Sha256::digest(b"test model binary content");
    let actual_hex = format!("{:x}", actual_sha);

    assert!(verify_file_checksum(&test_file, &actual_hex).is_ok());

    // Mismatched SHA-256
    let mismatch_err = verify_file_checksum(&test_file, "00000000000000000000000000000000");
    assert!(mismatch_err.is_err());
    assert_eq!(mismatch_err.unwrap_err().code, "CHECKSUM_MISMATCH");
}
