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

#[test]
fn scan_with_invalid_config_exits_two_and_reports_toml_error() {
    let repo = write_rust_fixture_repo(
        r#"
fn stale_helper() {}

fn main() {}
"#,
    );
    fs::write(
        repo.path().join(".graveyard.toml"),
        "[graveyard]\nmin_confidence =\n",
    )
    .expect("invalid config should be written");

    let output = Command::cargo_bin("graveyard")
        .expect("binary should build")
        .current_dir(repo.path())
        .args(["scan"])
        .output()
        .expect("scan should execute");

    assert_eq!(
        output.status.code(),
        Some(2),
        "invalid config should exit 2: {output:?}"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("[ERROR] .graveyard.toml: invalid TOML at line 2:"),
        "stderr should report formatted toml error: {stderr}"
    );
}

#[test]
fn scan_in_non_git_repo_warns_and_continues_in_static_only_mode() {
    let repo = write_rust_fixture_repo(
        r#"
fn stale_helper() {}

fn main() {}
"#,
    );

    let output = Command::cargo_bin("graveyard")
        .expect("binary should build")
        .current_dir(repo.path())
        .args(["scan"])
        .output()
        .expect("scan should execute");

    assert!(
        output.status.success(),
        "non-git scan should still succeed: {output:?}"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(
            "[WARN]  Not a git repository; running in static-only mode (--no-git to suppress)"
        ),
        "stderr should report static-only warning: {stderr}"
    );
}

#[test]
fn scan_with_no_supported_files_reports_info_and_skips_dead_code_output() {
    let repo = TempDir::new().expect("temp dir should exist");

    let output = Command::cargo_bin("graveyard")
        .expect("binary should build")
        .current_dir(repo.path())
        .args(["scan"])
        .output()
        .expect("scan should execute");

    assert!(
        output.status.success(),
        "empty scan should exit successfully: {output:?}"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("[INFO]  No Python/JS/TS/Go/Rust files found under"),
        "stderr should report no supported files: {stderr}"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("No dead code found above confidence"),
        "empty scan should not render normal dead-code summary: {stdout}"
    );
}

#[test]
fn scan_with_invalid_min_age_exits_two_and_formats_error() {
    let output = Command::cargo_bin("graveyard")
        .expect("binary should build")
        .args(["scan", "--min-age", "fortnight"])
        .output()
        .expect("scan should execute");

    assert_eq!(
        output.status.code(),
        Some(2),
        "invalid min-age should exit 2: {output:?}"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(
            "[ERROR] --min-age: invalid duration \"fortnight\" — use formats like 7d, 30d, 6mo, 1y"
        ),
        "stderr should report normalized min-age error: {stderr}"
    );
}

#[test]
fn baseline_diff_with_missing_file_exits_three_with_error_prefix() {
    let repo = write_rust_fixture_repo(
        r#"
fn stale_helper() {}

fn main() {}
"#,
    );

    let output = Command::cargo_bin("graveyard")
        .expect("binary should build")
        .current_dir(repo.path())
        .args(["baseline", "diff", "--baseline", "missing-baseline.json"])
        .output()
        .expect("baseline diff should execute");

    assert_eq!(
        output.status.code(),
        Some(3),
        "missing baseline should use fatal exit code: {output:?}"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("[ERROR] Baseline file not found:"),
        "stderr should include top-level error prefix: {stderr}"
    );
}
