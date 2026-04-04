use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::config::Config;

use super::Language;

pub fn detect_languages(root: &Path, config: &Config) -> HashSet<Language> {
    let enabled = enabled_languages(&config.languages);
    let mut languages = HashSet::new();

    if root.join("pyproject.toml").is_file() || root.join("setup.py").is_file() {
        languages.insert(Language::Python);
    }

    if root.join("package.json").is_file() {
        languages.insert(Language::JavaScript);
        languages.insert(Language::TypeScript);
    }

    if root.join("go.mod").is_file() {
        languages.insert(Language::Go);
    }

    if root.join("Cargo.toml").is_file() {
        languages.insert(Language::Rust);
    }

    if languages.is_empty() {
        let Ok(entries) = fs::read_dir(root) else {
            return HashSet::new();
        };

        for entry in entries.flatten() {
            if let Some(language) = Language::from_path(&entry.path()) {
                languages.insert(language);
            }
        }
    }

    languages.retain(|language| enabled.contains(language));
    languages
}

pub fn enabled_languages(values: &[String]) -> HashSet<Language> {
    values
        .iter()
        .filter_map(|value| match value.as_str() {
            "python" => Some(Language::Python),
            "javascript" => Some(Language::JavaScript),
            "typescript" => Some(Language::TypeScript),
            "go" => Some(Language::Go),
            "rust" => Some(Language::Rust),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use crate::config::Config;

    use super::{detect_languages, Language};

    #[test]
    fn detects_languages_from_manifests() {
        let temp = tempdir().expect("temp dir should be created");
        fs::write(temp.path().join("pyproject.toml"), "").expect("manifest should be written");
        fs::write(temp.path().join("package.json"), "{}").expect("manifest should be written");
        fs::write(temp.path().join("go.mod"), "module example.com/test")
            .expect("manifest should be written");
        fs::write(
            temp.path().join("Cargo.toml"),
            "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\n",
        )
        .expect("manifest should be written");

        let languages = detect_languages(temp.path(), &Config::default());

        assert!(languages.contains(&Language::Python));
        assert!(languages.contains(&Language::JavaScript));
        assert!(languages.contains(&Language::TypeScript));
        assert!(languages.contains(&Language::Go));
        assert!(languages.contains(&Language::Rust));
    }

    #[test]
    fn falls_back_to_extension_detection_without_manifests() {
        let temp = tempdir().expect("temp dir should be created");
        fs::write(temp.path().join("orphan.py"), "def f():\n    return 1\n")
            .expect("source should be written");
        fs::write(temp.path().join("orphan.rs"), "fn main() {}\n")
            .expect("source should be written");

        let languages = detect_languages(temp.path(), &Config::default());

        assert!(languages.contains(&Language::Python));
        assert!(languages.contains(&Language::Rust));
    }
}
