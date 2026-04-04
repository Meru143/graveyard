pub mod manifest;

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;
use path_slash::PathExt;

use crate::config::Config;

#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
pub enum Language {
    Python,
    JavaScript,
    TypeScript,
    Go,
    Rust,
}

impl Language {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Python => "python",
            Self::JavaScript => "javascript",
            Self::TypeScript => "typescript",
            Self::Go => "go",
            Self::Rust => "rust",
        }
    }

    pub fn from_path(path: &Path) -> Option<Self> {
        let file_name = path.file_name()?.to_string_lossy();
        let extension = path.extension()?.to_string_lossy();

        match extension.as_ref() {
            "py" => Some(Self::Python),
            "js" | "mjs" | "cjs" => {
                if file_name.ends_with(".min.js") {
                    None
                } else {
                    Some(Self::JavaScript)
                }
            }
            "ts" | "tsx" => Some(Self::TypeScript),
            "go" => Some(Self::Go),
            "rs" => Some(Self::Rust),
            _ => None,
        }
    }
}

pub fn walk(root: &Path, config: &Config) -> Vec<(PathBuf, Language)> {
    let enabled = manifest::enabled_languages(&config.languages);
    let mut builder = WalkBuilder::new(root);
    builder.hidden(false);
    builder.require_git(false);

    let mut files = Vec::new();

    for entry in builder.build() {
        let Ok(entry) = entry else {
            continue;
        };

        let path = entry.into_path();
        if !path.is_file() {
            continue;
        }

        if is_hidden_path(root, &path)
            || has_default_excluded_component(&path)
            || is_excluded_by_config(root, &path, &config.exclude)
            || is_binary_file(&path)
        {
            continue;
        }

        let Some(language) = Language::from_path(&path) else {
            continue;
        };

        if !enabled.contains(&language) {
            continue;
        }

        files.push((path, language));
    }

    files.sort_by(|left, right| left.0.cmp(&right.0));
    files
}

fn is_hidden_path(root: &Path, path: &Path) -> bool {
    path.strip_prefix(root)
        .ok()
        .into_iter()
        .flat_map(Path::components)
        .filter_map(|component| component.as_os_str().to_str())
        .any(|component| component.starts_with('.') && component != ".")
}

fn is_excluded_by_config(root: &Path, path: &Path, patterns: &[String]) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    let normalized = relative.to_slash_lossy();

    patterns
        .iter()
        .any(|pattern| wildcard_match(pattern, normalized.as_ref()))
}

fn has_default_excluded_component(path: &Path) -> bool {
    const EXCLUDED: [&str; 7] = [
        "node_modules",
        "__pycache__",
        "target",
        "dist",
        "build",
        "vendor",
        ".git",
    ];

    path.components()
        .filter_map(|component| component.as_os_str().to_str())
        .any(|component| EXCLUDED.contains(&component))
}

fn is_binary_file(path: &Path) -> bool {
    let mut buffer = [0_u8; 512];
    let Ok(mut file) = File::open(path) else {
        return false;
    };

    let Ok(bytes_read) = file.read(&mut buffer) else {
        return false;
    };

    buffer[..bytes_read].contains(&0)
}

fn wildcard_match(pattern: &str, candidate: &str) -> bool {
    let pattern = pattern.replace("**", "*");
    let pattern = pattern.as_bytes();
    let candidate = candidate.as_bytes();

    let (mut pattern_index, mut candidate_index) = (0_usize, 0_usize);
    let mut last_star = None;
    let mut last_match = 0_usize;

    while candidate_index < candidate.len() {
        if pattern_index < pattern.len()
            && (pattern[pattern_index] == candidate[candidate_index]
                || pattern[pattern_index] == b'*')
        {
            if pattern[pattern_index] == b'*' {
                last_star = Some(pattern_index);
                last_match = candidate_index;
                pattern_index += 1;
            } else {
                pattern_index += 1;
                candidate_index += 1;
            }
        } else if let Some(star_index) = last_star {
            pattern_index = star_index + 1;
            last_match += 1;
            candidate_index = last_match;
        } else {
            return false;
        }
    }

    while pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
        pattern_index += 1;
    }

    pattern_index == pattern.len()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use crate::config::Config;

    use super::{walk, Language};

    #[test]
    fn walk_respects_gitignore_and_excludes() {
        let temp = tempdir().expect("temp dir should be created");
        fs::write(temp.path().join(".gitignore"), "ignored.py\n")
            .expect("gitignore should be written");
        fs::write(
            temp.path().join("ignored.py"),
            "def ignored():\n    return 1\n",
        )
        .expect("source should be written");
        fs::write(temp.path().join("keep.py"), "def keep():\n    return 1\n")
            .expect("source should be written");
        fs::create_dir_all(temp.path().join("tests")).expect("tests dir should be created");
        fs::write(
            temp.path().join("tests").join("skip.py"),
            "def skip():\n    return 1\n",
        )
        .expect("source should be written");

        let config = Config {
            exclude: vec!["tests/**".to_string()],
            ..Config::default()
        };

        let files = walk(temp.path(), &config);

        assert_eq!(files.len(), 1);
        assert_eq!(
            files[0].0.file_name().and_then(|name| name.to_str()),
            Some("keep.py")
        );
        assert_eq!(files[0].1, Language::Python);
    }

    #[test]
    fn walk_skips_binary_files_default_dirs_and_minified_js() {
        let temp = tempdir().expect("temp dir should be created");
        fs::create_dir_all(temp.path().join("target")).expect("target dir should be created");
        fs::write(
            temp.path().join("target").join("generated.rs"),
            "fn generated() {}\n",
        )
        .expect("source should be written");
        fs::write(temp.path().join("bundle.min.js"), "function minified(){}")
            .expect("source should be written");
        fs::write(temp.path().join("data.py"), [0_u8, 159, 146, 150])
            .expect("binary file should be written");
        fs::write(
            temp.path().join("main.ts"),
            "export const main = () => 1;\n",
        )
        .expect("source should be written");

        let files = walk(temp.path(), &Config::default());

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].1, Language::TypeScript);
    }
}
