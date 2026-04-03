use std::fs;
use std::path::Path;
use std::process::Command;

use chrono::{Duration, Utc};
use tempfile::{TempDir, tempdir};

pub struct TmpGitRepo {
    temp_dir: TempDir,
}

impl TmpGitRepo {
    pub fn new() -> Self {
        let temp_dir = tempdir().expect("temp dir should be created");
        run_git(&["init"], temp_dir.path());
        run_git(&["config", "user.email", "graveyard@example.com"], temp_dir.path());
        run_git(&["config", "user.name", "graveyard"], temp_dir.path());

        Self { temp_dir }
    }

    pub fn commit_file(&self, path: &str, content: &str, days_ago: i64) {
        let file_path = self.temp_dir.path().join(path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).expect("parent directory should be created");
        }

        fs::write(&file_path, content).expect("fixture file should be written");
        run_git(&["add", path], self.temp_dir.path());

        let commit_time = (Utc::now() - Duration::days(days_ago)).to_rfc3339();
        let output = Command::new("git")
            .args(["commit", "-m", &format!("add {path}")])
            .current_dir(self.temp_dir.path())
            .env("GIT_AUTHOR_DATE", &commit_time)
            .env("GIT_COMMITTER_DATE", &commit_time)
            .output()
            .expect("git commit should execute");

        assert!(
            output.status.success(),
            "git commit failed: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    pub fn path(&self) -> &Path {
        self.temp_dir.path()
    }
}

fn run_git(args: &[&str], cwd: &Path) {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("git command should execute");

    assert!(
        output.status.success(),
        "git {:?} failed: {}\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
