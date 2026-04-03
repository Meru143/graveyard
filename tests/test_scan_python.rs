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
fn scan_python_reports_old_dead_function_with_high_confidence() {
    let repo = TmpGitRepo::new();
    repo.commit_file(
        "pyproject.toml",
        "[project]\nname = \"fixture\"\nversion = \"0.1.0\"\n",
        400,
    );
    repo.commit_file(
        "app.py",
        r#"
def dead_helper():
    return 0

def live_helper():
    return 1

def main():
    return live_helper()
"#,
        400,
    );

    let json = scan_json(&repo);
    let finding = finding_by_fqn(&json, "app.py::dead_helper")
        .expect("dead python helper should be reported");

    assert!(
        finding["confidence"].as_f64().unwrap_or_default() > 0.7,
        "confidence should be > 0.7: {finding}"
    );
    assert!(finding_by_fqn(&json, "app.py::main").is_none());
    assert!(finding_by_fqn(&json, "app.py::live_helper").is_none());
}
