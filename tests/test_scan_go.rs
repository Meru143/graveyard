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
fn scan_go_reports_old_dead_exported_function_with_high_confidence() {
    let repo = TmpGitRepo::new();
    repo.commit_file("go.mod", "module fixture\n\ngo 1.22\n", 400);
    repo.commit_file(
        "main.go",
        r#"
package main

func DeadExported() {}

func liveHelper() {}

func main() {
    liveHelper()
}
"#,
        400,
    );

    let json = scan_json(&repo);
    let finding =
        finding_by_fqn(&json, "main.go::DeadExported").expect("dead go export should be reported");

    assert!(
        finding["confidence"].as_f64().unwrap_or_default() > 0.7,
        "confidence should be > 0.7: {finding}"
    );
    assert_eq!(finding["tag"], "exported_unused");
    assert!(finding_by_fqn(&json, "main.go::main").is_none());
    assert!(finding_by_fqn(&json, "main.go::liveHelper").is_none());
}
