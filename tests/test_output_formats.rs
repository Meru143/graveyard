mod common;

use std::path::Path;

use assert_cmd::Command;
use common::TmpGitRepo;
use insta::{assert_json_snapshot, assert_snapshot};
use path_slash::PathExt;
use serde_json::Value;

fn scan_json(repo: &TmpGitRepo) -> Value {
    let output = run_graveyard(repo.path(), &["scan", "--no-cache", "--format", "json"]);
    assert!(
        output.status.success(),
        "scan should succeed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "scan output should be json: {error}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn finding_by_fqn<'a>(json: &'a Value, fqn: &str) -> Option<&'a Value> {
    json["findings"]
        .as_array()?
        .iter()
        .find(|finding| finding["symbol_fqn"] == fqn)
}

fn run_graveyard(cwd: &Path, args: &[&str]) -> std::process::Output {
    Command::cargo_bin("graveyard")
        .expect("binary should build")
        .current_dir(cwd)
        .args(args)
        .output()
        .expect("graveyard command should execute")
}

fn fixture_repo() -> TmpGitRepo {
    let repo = TmpGitRepo::new();
    repo.commit_file(
        "Cargo.toml",
        r#"[package]
name = "fixture"
version = "0.1.0"
edition = "2021"
"#,
        400,
    );
    repo.commit_file(
        "src/main.rs",
        r#"
pub fn dead_public() {}

fn live_helper() {}

fn main() {
    live_helper();
}
"#,
        400,
    );
    repo
}

fn sanitized_json_output(repo: &TmpGitRepo, format: &str) -> Value {
    let output = run_graveyard(repo.path(), &["scan", "--no-cache", "--format", format]);
    assert!(
        output.status.success(),
        "scan should succeed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let repo_root = repo.path().to_slash_lossy().to_string();
    let sanitized = String::from_utf8(output.stdout)
        .expect("output should be utf-8")
        .replace(&repo_root, "<repo>");
    let mut value: Value = serde_json::from_str(&sanitized).unwrap_or_else(|error| {
        panic!("output should be valid json: {error}\noutput:\n{sanitized}")
    });

    if let Some(scanned_at) = value.get_mut("scanned_at") {
        *scanned_at = Value::String("[timestamp]".to_string());
    }

    value
}

#[test]
fn json_output_matches_snapshot() {
    let repo = fixture_repo();
    let json = scan_json(&repo);
    assert!(finding_by_fqn(&json, "src/main.rs::dead_public").is_some());
    let value = sanitized_json_output(&repo, "json");

    assert_json_snapshot!("scan_json_output", value);
}

#[test]
fn sarif_output_matches_snapshot() {
    let repo = fixture_repo();
    let value = sanitized_json_output(&repo, "sarif");

    assert_json_snapshot!("scan_sarif_output", value);
}

#[test]
fn csv_output_matches_snapshot() {
    let repo = fixture_repo();
    let output = run_graveyard(repo.path(), &["scan", "--no-cache", "--format", "csv"]);
    assert!(
        output.status.success(),
        "scan should succeed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let repo_root = repo.path().to_slash_lossy().to_string();
    let sanitized = String::from_utf8(output.stdout)
        .expect("output should be utf-8")
        .replace(&repo_root, "<repo>");

    assert_snapshot!("scan_csv_output", sanitized);
}
