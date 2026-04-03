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
fn scan_mixed_repo_reports_findings_from_python_js_and_go() {
    let repo = TmpGitRepo::new();
    repo.commit_file(
        "pyproject.toml",
        "[project]\nname = \"fixture\"\nversion = \"0.1.0\"\n",
        400,
    );
    repo.commit_file(
        "package.json",
        "{\n  \"name\": \"fixture\",\n  \"version\": \"0.1.0\"\n}\n",
        400,
    );
    repo.commit_file("go.mod", "module fixture\n\ngo 1.22\n", 400);
    repo.commit_file("python/app.py", "def dead_python():\n    return 0\n", 400);
    repo.commit_file("web/app.js", "function deadJs() { return 0; }\n", 400);
    repo.commit_file("cmd/main.go", "package main\n\nfunc DeadGo() {}\n", 400);

    let json = scan_json(&repo);

    assert!(finding_by_fqn(&json, "python/app.py::dead_python").is_some());
    assert!(finding_by_fqn(&json, "web/app.js::deadJs").is_some());
    assert!(finding_by_fqn(&json, "cmd/main.go::DeadGo").is_some());
}
