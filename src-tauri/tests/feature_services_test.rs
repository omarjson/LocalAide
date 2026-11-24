//! Pure unit tests for the feature-services stream: reference parsing and
//! doctor report serialization. Nothing here touches the network or user
//! state; these are deterministic and safe to run in CI.

use app_lib::services::context_refs::parse_reference;
use app_lib::services::doctor::{DoctorCheck, DoctorReport};

#[test]
fn parses_reference_with_line_range() {
    let parsed = parse_reference("@src/main.rs:10-20").expect("valid reference");
    assert_eq!(parsed.path, "src/main.rs");
    assert_eq!(parsed.start, Some(10));
    assert_eq!(parsed.end, Some(20));
}

#[test]
fn parses_reference_without_line_range() {
    let parsed = parse_reference("@docs/guide.md").expect("valid reference");
    assert_eq!(parsed.path, "docs/guide.md");
    assert_eq!(parsed.start, None);
    assert_eq!(parsed.end, None);
}

#[test]
fn rejects_malformed_references() {
    assert!(parse_reference("src/main.rs").is_err(), "missing '@'");
    assert!(parse_reference("@").is_err(), "empty reference");
    assert!(parse_reference("@a.txt:5-2").is_err(), "reversed range");
    assert!(parse_reference("@a.txt:0-3").is_err(), "zero start line");
    assert!(parse_reference("@a.txt:x-y").is_err(), "non-numeric range");
}

#[test]
fn doctor_report_serializes_and_computes_ok() {
    let report = DoctorReport::from_checks(vec![
        DoctorCheck::new("config", true, "config.json parses"),
        DoctorCheck::new("library_db", true, "library.db opens"),
    ]);
    assert!(report.ok);

    let json = serde_json::to_string(&report).expect("serialization must succeed");
    let value: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    assert_eq!(value["ok"], true);
    assert_eq!(value["checks"].as_array().map(|a| a.len()), Some(2));
    assert_eq!(value["checks"][0]["name"], "config");
    assert_eq!(value["checks"][0]["ok"], true);

    let roundtrip: DoctorReport =
        serde_json::from_str(&json).expect("deserialization must succeed");
    assert_eq!(roundtrip, report);
}

#[test]
fn doctor_report_reflects_failed_required_check() {
    let report = DoctorReport::from_checks(vec![
        DoctorCheck::new("config", true, "ok"),
        DoctorCheck::new("library_db", false, "missing"),
    ]);
    assert!(!report.ok);
}

#[test]
fn keychain_is_best_effort_and_does_not_fail_the_report() {
    let report = DoctorReport::from_checks(vec![
        DoctorCheck::new("config", true, "ok"),
        DoctorCheck::new("keychain", false, "no secret service"),
    ]);
    assert!(
        report.ok,
        "the keychain probe must not fail the overall report"
    );
}
