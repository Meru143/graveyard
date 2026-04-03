use std::fs;

use assert_cmd::Command;
use tempfile::TempDir;

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
fn scan_ci_exits_one_and_reports_duration_when_findings_exist() {
    let repo = write_rust_fixture_repo(
        r#"
fn stale_helper() {}

fn main() {}
"#,
    );

    let output = Command::cargo_bin("graveyard")
        .expect("binary should build")
        .current_dir(repo.path())
        .args(["scan", "--ci"])
        .output()
        .expect("scan should execute");

    assert_eq!(
        output.status.code(),
        Some(1),
        "scan --ci should fail on findings: {output:?}"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Scan completed in"),
        "stderr should report scan duration: {stderr}"
    );
}
