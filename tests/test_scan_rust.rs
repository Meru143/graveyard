mod common;

use std::path::Path;

use assert_cmd::Command;
use common::TmpGitRepo;
use serde_json::Value;

fn scan_json(repo: &TmpGitRepo) -> Value {
    let output = run_graveyard(repo.path(), &["scan", "--format", "json"]);
    assert!(
        output.status.success(),
        "scan should succeed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("scan output should be json")
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

#[test]
fn scan_rust_reports_old_dead_public_function_with_high_confidence() {
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

    let json = scan_json(&repo);
    let finding = finding_by_fqn(&json, "src/main.rs::dead_public")
        .expect("dead rust export should be reported");

    assert!(
        finding["confidence"].as_f64().unwrap_or_default() > 0.7,
        "confidence should be > 0.7: {finding}"
    );
    assert_eq!(finding["tag"], "exported_unused");
    assert!(finding_by_fqn(&json, "src/main.rs::main").is_none());
    assert!(finding_by_fqn(&json, "src/main.rs::live_helper").is_none());
}
