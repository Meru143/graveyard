use std::collections::HashMap;
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use anyhow::Context;
use chrono::Utc;
use git2::{Commit, DiffDelta, DiffHunk, ErrorCode, Repository, Sort};
use path_slash::PathExt;

use crate::parse::types::Symbol;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GitScore {
    pub age_days: f64,
    pub commits_90d: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LineRange {
    start: u32,
    end: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileTouch {
    timestamp: i64,
    ranges: Vec<LineRange>,
}

pub fn open_repo(root: &Path) -> Option<Repository> {
    match Repository::discover(root) {
        Ok(repo) => Some(repo),
        Err(_) => {
            eprintln!("[WARN]  Not a git repository; running in static-only mode (--no-git to suppress)");
            None
        }
    }
}

pub fn get_head_sha(repo: &Repository) -> String {
    match repo.head() {
        Ok(head) if head.is_branch() => head
            .peel_to_commit()
            .map(|commit| commit.id().to_string())
            .unwrap_or_else(|_| "UNKNOWN".to_string()),
        _ => "UNKNOWN".to_string(),
    }
}

pub fn deadness_age_days(symbol: &Symbol, repo: &Repository, repo_root: &Path) -> f64 {
    match collect_file_history(&symbol.file, repo, repo_root, None)
        .map_err(anyhow::Error::from)
        .context("git history walk")
    {
        Ok(history) => age_days_from_history(symbol, &history, Utc::now().timestamp()),
        Err(error) => {
            log_git_fallback(&symbol.file, &error);
            0.0
        }
    }
}

pub fn commit_count_90d(file: &Path, repo: &Repository, repo_root: &Path) -> usize {
    let head_sha = get_head_sha(repo);
    let cache_key = file_cache_key(file, repo, repo_root, &head_sha);

    if let Some(count) = load_cached_commit_count(&cache_key) {
        return count;
    }

    match collect_file_history(file, repo, repo_root, None)
        .map_err(anyhow::Error::from)
        .context("git history walk")
    {
        Ok(history) => cache_commit_count(cache_key, commit_count_from_history(&history)),
        Err(error) => {
            log_git_fallback(file, &error);
            0
        }
    }
}

pub fn score_all_git(symbols: &[Symbol], repo: &Repository, root: &Path) -> HashMap<String, GitScore> {
    let head_sha = get_head_sha(repo);
    let mut grouped: HashMap<PathBuf, Vec<&Symbol>> = HashMap::new();
    let mut scores = HashMap::new();

    for symbol in symbols {
        grouped.entry(symbol.file.clone()).or_default().push(symbol);
    }

    for (file, file_symbols) in grouped {
        let history = match collect_file_history(&file, repo, root, Some(500))
            .map_err(anyhow::Error::from)
            .context("git history walk")
        {
            Ok(history) => history,
            Err(error) => {
                log_git_fallback(&file, &error);
                Vec::new()
            }
        };

        let cache_key = file_cache_key(&file, repo, root, &head_sha);
        let commits_90d = load_cached_commit_count(&cache_key)
            .unwrap_or_else(|| cache_commit_count(cache_key, commit_count_from_history(&history)));

        for symbol in file_symbols {
            scores.insert(
                symbol.fqn.clone(),
                GitScore {
                    age_days: age_days_from_history(symbol, &history, Utc::now().timestamp()),
                    commits_90d,
                },
            );
        }
    }

    scores
}

fn commit_count_cache() -> &'static Mutex<HashMap<String, usize>> {
    static CACHE: OnceLock<Mutex<HashMap<String, usize>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn load_cached_commit_count(cache_key: &str) -> Option<usize> {
    commit_count_cache()
        .lock()
        .expect("commit count cache lock should not be poisoned")
        .get(cache_key)
        .copied()
}

fn cache_commit_count(cache_key: String, count: usize) -> usize {
    commit_count_cache()
        .lock()
        .expect("commit count cache lock should not be poisoned")
        .insert(cache_key, count);
    count
}

fn collect_file_history(
    file: &Path,
    repo: &Repository,
    repo_root: &Path,
    max_commits: Option<usize>,
) -> Result<Vec<FileTouch>, git2::Error> {
    let rel_path = relative_path(file, repo, repo_root).to_path_buf();
    let mut walk = repo.revwalk()?;
    walk.push_head()?;
    walk.set_sorting(Sort::TIME)?;
    walk.simplify_first_parent()?;

    let mut touches = Vec::new();
    for (walked, oid) in walk.enumerate() {
        let oid = oid?;
        let commit = repo.find_commit(oid)?;
        let diff = diff_to_parent(repo, &commit)?;
        let matched_file = Cell::new(false);
        let mut ranges = Vec::new();

        let mut file_cb = |delta: DiffDelta<'_>, _progress: f32| {
            if delta_matches_path(delta, &rel_path) {
                matched_file.set(true);
            }
            true
        };
        let mut hunk_cb = |delta: DiffDelta<'_>, hunk: DiffHunk<'_>| {
            if delta_matches_path(delta, &rel_path) {
                matched_file.set(true);
                ranges.push(hunk_to_range(hunk));
            }
            true
        };

        diff.foreach(&mut file_cb, None, Some(&mut hunk_cb), None)?;

        if matched_file.get() {
            touches.push(FileTouch {
                timestamp: commit.time().seconds(),
                ranges,
            });
        }

        if max_commits.is_some_and(|limit| walked + 1 >= limit) {
            break;
        }
    }

    Ok(touches)
}

fn diff_to_parent<'repo>(
    repo: &'repo Repository,
    commit: &Commit<'repo>,
) -> Result<git2::Diff<'repo>, git2::Error> {
    let tree = commit.tree()?;
    if commit.parent_count() == 0 {
        repo.diff_tree_to_tree(None, Some(&tree), None)
    } else {
        let parent = commit.parent(0)?;
        let parent_tree = parent.tree()?;
        repo.diff_tree_to_tree(Some(&parent_tree), Some(&tree), None)
    }
}

fn delta_matches_path(delta: DiffDelta<'_>, rel_path: &Path) -> bool {
    delta.new_file().path() == Some(rel_path) || delta.old_file().path() == Some(rel_path)
}

fn hunk_to_range(hunk: DiffHunk<'_>) -> LineRange {
    let (start, lines) = if hunk.new_lines() > 0 {
        (hunk.new_start(), hunk.new_lines())
    } else {
        (hunk.old_start(), hunk.old_lines())
    };

    let end = start.saturating_add(lines.saturating_sub(1));
    LineRange {
        start,
        end: if lines == 0 { start } else { end },
    }
}

fn age_days_from_history(symbol: &Symbol, history: &[FileTouch], now_ts: i64) -> f64 {
    for touch in history {
        if touch.ranges.is_empty()
            || touch
                .ranges
                .iter()
                .any(|range| ranges_overlap(symbol.line_start, symbol.line_end, range.start, range.end))
        {
            let elapsed_seconds = now_ts.saturating_sub(touch.timestamp).max(0);
            return elapsed_seconds as f64 / 86_400.0;
        }
    }

    0.0
}

fn commit_count_from_history(history: &[FileTouch]) -> usize {
    let cutoff = Utc::now().timestamp() - (90 * 86_400);
    history
        .iter()
        .filter(|touch| touch.timestamp >= cutoff)
        .count()
}

fn ranges_overlap(left_start: u32, left_end: u32, right_start: u32, right_end: u32) -> bool {
    left_start <= right_end && right_start <= left_end
}

fn relative_path<'a>(path: &'a Path, repo: &Repository, repo_root: &'a Path) -> &'a Path {
    let workdir = repo.workdir().unwrap_or(repo_root);
    path.strip_prefix(workdir)
        .or_else(|_| path.strip_prefix(repo_root))
        .unwrap_or(path)
}

fn file_cache_key(file: &Path, repo: &Repository, repo_root: &Path, head_sha: &str) -> String {
    format!(
        "{}:{head_sha}",
        relative_path(file, repo, repo_root).to_slash_lossy()
    )
}

fn log_git_fallback(file: &Path, error: &anyhow::Error) {
    if error
        .downcast_ref::<git2::Error>()
        .is_some_and(|git_error| git_error.code() == ErrorCode::NotFound)
    {
        eprintln!(
            "[WARN]  Shallow git clone detected for {} — static-only for this file",
            file.to_slash_lossy()
        );
    } else {
        tracing::warn!(path = ?file, %error, "git history unavailable; static-only for this file");
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use chrono::{Duration, Utc};
    use tempfile::tempdir;

    use crate::parse::types::{Symbol, SymbolKind};

    use super::{commit_count_90d, deadness_age_days, get_head_sha, open_repo, score_all_git};

    fn git(args: &[&str], cwd: &Path) {
        let output = Command::new("git")
            .args(args)
            .current_dir(cwd)
            .output()
            .expect("git command should run");

        assert!(
            output.status.success(),
            "git {:?} failed: {}\n{}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn git_commit_all(cwd: &Path, message: &str, timestamp: i64) {
        git(&["add", "."], cwd);

        let date = chrono::DateTime::<Utc>::from_timestamp(timestamp, 0)
            .expect("timestamp should be valid")
            .to_rfc3339();

        let output = Command::new("git")
            .args(["commit", "-m", message])
            .current_dir(cwd)
            .env("GIT_AUTHOR_NAME", "graveyard")
            .env("GIT_AUTHOR_EMAIL", "graveyard@example.com")
            .env("GIT_COMMITTER_NAME", "graveyard")
            .env("GIT_COMMITTER_EMAIL", "graveyard@example.com")
            .env("GIT_AUTHOR_DATE", &date)
            .env("GIT_COMMITTER_DATE", &date)
            .output()
            .expect("git commit should run");

        assert!(
            output.status.success(),
            "git commit failed: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn write_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent directories should exist");
        }

        fs::write(path, content).expect("file should be written");
    }

    fn make_symbol(file: PathBuf, line_start: u32, line_end: u32, fqn: &str, name: &str) -> Symbol {
        Symbol {
            fqn: fqn.to_string(),
            name: name.to_string(),
            kind: SymbolKind::Function,
            language: "python".to_string(),
            file,
            line_start,
            line_end,
            is_exported: false,
            is_test: false,
        }
    }

    #[test]
    fn open_repo_returns_none_for_non_git_directory() {
        let temp = tempdir().expect("temp dir should be created");

        let repo = open_repo(temp.path());

        assert!(repo.is_none());
    }

    #[test]
    fn get_head_sha_returns_unknown_for_detached_head() {
        let temp = tempdir().expect("temp dir should be created");
        git(&["init"], temp.path());
        write_file(&temp.path().join("src/main.py"), "def main():\n    return 1\n");
        let now = Utc::now().timestamp();
        git_commit_all(temp.path(), "initial", now);
        git(&["checkout", "--detach", "HEAD"], temp.path());

        let repo = open_repo(temp.path()).expect("repo should open");
        let head_sha = get_head_sha(&repo);

        assert_eq!(head_sha, "UNKNOWN");
    }

    #[test]
    fn deadness_age_days_tracks_latest_touch_for_symbol_lines() {
        let temp = tempdir().expect("temp dir should be created");
        git(&["init"], temp.path());
        let file = temp.path().join("src/module.py");
        let old_ts = (Utc::now() - Duration::days(120)).timestamp();
        let new_ts = (Utc::now() - Duration::days(15)).timestamp();

        write_file(&file, "def helper():\n    return 1\n");
        git_commit_all(temp.path(), "add helper", old_ts);

        write_file(&file, "def helper():\n    return 2\n");
        git_commit_all(temp.path(), "touch helper", new_ts);

        let repo = open_repo(temp.path()).expect("repo should open");
        let symbol = make_symbol(file, 1, 2, "src/module.py::helper", "helper");

        let age_days = deadness_age_days(&symbol, &repo, temp.path());

        assert!(age_days >= 14.0, "age_days={age_days}");
        assert!(age_days <= 16.5, "age_days={age_days}");
    }

    #[test]
    fn commit_count_90d_counts_recent_file_touches() {
        let temp = tempdir().expect("temp dir should be created");
        git(&["init"], temp.path());
        let file = temp.path().join("src/module.py");

        write_file(&file, "def helper():\n    return 1\n");
        git_commit_all(
            temp.path(),
            "old touch",
            (Utc::now() - Duration::days(140)).timestamp(),
        );

        write_file(&file, "def helper():\n    return 2\n");
        git_commit_all(
            temp.path(),
            "recent touch one",
            (Utc::now() - Duration::days(30)).timestamp(),
        );

        write_file(&file, "def helper():\n    return 3\n");
        git_commit_all(
            temp.path(),
            "recent touch two",
            (Utc::now() - Duration::days(5)).timestamp(),
        );

        let repo = open_repo(temp.path()).expect("repo should open");
        let commits_90d = commit_count_90d(&file, &repo, temp.path());

        assert_eq!(commits_90d, 2);
    }

    #[test]
    fn score_all_git_returns_scores_keyed_by_symbol_fqn() {
        let temp = tempdir().expect("temp dir should be created");
        git(&["init"], temp.path());
        let file = temp.path().join("src/module.py");

        write_file(
            &file,
            "def helper():\n    return 1\n\n\ndef other():\n    return helper()\n",
        );
        git_commit_all(
            temp.path(),
            "initial",
            (Utc::now() - Duration::days(10)).timestamp(),
        );

        let repo = open_repo(temp.path()).expect("repo should open");
        let helper = make_symbol(file.clone(), 1, 2, "src/module.py::helper", "helper");
        let other = make_symbol(file, 5, 6, "src/module.py::other", "other");

        let scores = score_all_git(&[helper.clone(), other.clone()], &repo, temp.path());

        assert_eq!(scores.len(), 2);
        assert!(scores.contains_key(&helper.fqn));
        assert!(scores.contains_key(&other.fqn));
    }
}
