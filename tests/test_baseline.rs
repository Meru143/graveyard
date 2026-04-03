use std::fs;

use assert_cmd::Command;
use serde_json::Value;
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

    assert!(output.status.success(), "command should succeed: {output:?}");
    assert!(baseline_path.exists(), "baseline file should be written");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Baseline saved:"),
        "stderr should confirm save: {stderr}"
    );

    let content = fs::read_to_string(&baseline_path).expect("baseline file should be readable");
    let json: Value = serde_json::from_str(&content).expect("baseline file should be json");
    assert_eq!(json["total_findings"], 1);
    assert_eq!(json["findings"][0]["symbol_fqn"], "src/main.rs::stale_helper");
}
