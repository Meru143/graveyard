use std::fs;
use std::process::Command as ProcessCommand;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;

mod common;

use common::TmpGitRepo;

fn write_rust_fixture_repo(source: &str) -> TempDir {
    let temp_dir = TempDir::new().expect("temp dir should exist");
    fs::create_dir_all(temp_dir.path().join("src")).expect("src dir should exist");
    fs::write(
        temp_dir.path().join("Cargo.toml"),
        r#"[package]
name = "fixture"
version = "0.1.0"
edition = "2021"
"#,
    )
    .expect("cargo manifest should exist");
    fs::write(temp_dir.path().join("src/main.rs"), source).expect("main file should exist");
    temp_dir
}

#[test]
fn baseline_save_writes_file_and_reports_confirmation() {
    let repo = write_rust_fixture_repo(
        r#"
fn stale_helper() {}

fn main() {}
"#,
    );
    let baseline_path = repo.path().join(".graveyard-baseline.json");

    let output = Command::cargo_bin("graveyard")
        .expect("binary should build")
        .current_dir(repo.path())
        .args([
            "baseline",
            "save",
            "--output",
            baseline_path.to_string_lossy().as_ref(),
        ])
        .output()
        .expect("baseline save should execute");

    assert!(
        output.status.success(),
        "command should succeed: {output:?}"
    );
    assert!(baseline_path.exists(), "baseline file should be written");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Baseline saved:"),
        "stderr should confirm save: {stderr}"
    );

    let content = fs::read_to_string(&baseline_path).expect("baseline file should be readable");
    let json: Value = serde_json::from_str(&content).expect("baseline file should be json");
    assert_eq!(json["total_findings"], 1);
    assert_eq!(
        json["findings"][0]["symbol_fqn"],
        "src/main.rs::stale_helper"
    );
}

#[test]
fn baseline_diff_reports_only_new_findings_and_honors_ci() {
    let repo = write_rust_fixture_repo(
        r#"
fn old_dead() {}

fn main() {}
"#,
    );
    let baseline_path = repo.path().join(".graveyard-baseline.json");

    let save_output = Command::cargo_bin("graveyard")
        .expect("binary should build")
        .current_dir(repo.path())
        .args([
            "baseline",
            "save",
            "--output",
            baseline_path.to_string_lossy().as_ref(),
        ])
        .output()
        .expect("baseline save should execute");
    assert!(
        save_output.status.success(),
        "baseline save should succeed: {save_output:?}"
    );

    fs::write(
        repo.path().join("src/main.rs"),
        r#"
fn old_dead() {}
fn brand_new() {}

fn main() {}
"#,
    )
    .expect("updated main file should be written");

    let diff_output = Command::cargo_bin("graveyard")
        .expect("binary should build")
        .current_dir(repo.path())
        .args([
            "baseline",
            "diff",
            "--baseline",
            baseline_path.to_string_lossy().as_ref(),
            "--ci",
        ])
        .output()
        .expect("baseline diff should execute");

    assert_eq!(
        diff_output.status.code(),
        Some(1),
        "baseline diff should fail CI for new findings: {diff_output:?}"
    );

    let stdout = String::from_utf8_lossy(&diff_output.stdout);
    assert!(
        stdout.contains("brand_new"),
        "diff output should include the new finding: {stdout}"
    );
    assert!(
        !stdout.contains("old_dead"),
        "diff output should suppress baseline findings: {stdout}"
    );
}

#[test]
fn tmp_git_repo_commits_backdated_files() {
    let repo = TmpGitRepo::new();
    repo.commit_file("src/main.rs", "fn main() {}\n", 30);

    let output = ProcessCommand::new("git")
        .args(["log", "--format=%ad", "--date=short", "-1"])
        .current_dir(repo.path())
        .output()
        .expect("git log should execute");

    assert!(
        output.status.success(),
        "git log should succeed: {output:?}"
    );
    assert!(repo.path().join("src/main.rs").exists());
    assert!(!String::from_utf8_lossy(&output.stdout).trim().is_empty());
}
