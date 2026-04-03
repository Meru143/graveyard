pub mod cache;
pub mod go;
pub mod javascript;
pub mod python;
pub mod rust_lang;
pub mod types;
pub mod typescript;

use std::fs;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use indicatif::{ParallelProgressIterator, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use sha2::{Digest, Sha256};
use tree_sitter::Node;

use crate::config::Config;
use crate::walker::Language;

use self::cache::ParseCache;
use self::go::extract_go;
use self::javascript::extract_javascript;
use self::python::extract_python;
use self::rust_lang::extract_rust;
use self::types::{build_fqn, Reference, Symbol};
use self::typescript::extract_typescript;

pub fn parse_all(
    files: &[(PathBuf, Language)],
    root: &Path,
    cache: &ParseCache,
    git_head: &str,
    _config: &Config,
) -> (Vec<Symbol>, Vec<Reference>) {
    let progress = ProgressBar::new(files.len() as u64);
    if let Ok(style) =
        ProgressStyle::with_template("[{elapsed_precise}] {bar:40} {pos}/{len} {msg}")
    {
        progress.set_style(style);
    }
    progress.set_message("parsing");

    let parsed = files
        .par_iter()
        .progress_with(progress.clone())
        .map(|(path, language)| {
            let cache_key = build_cache_key(path, git_head);

            if let Some(entry) = cache.get(&cache_key) {
                return entry;
            }

            let result = catch_unwind(AssertUnwindSafe(|| {
                let source = fs::read(path).map_err(|error| error.to_string())?;
                let parsed = dispatch_extract(path, root, &source, *language);
                cache.set(&cache_key, &parsed.0, &parsed.1);
                Ok::<_, String>(parsed)
            }));

            match result {
                Ok(Ok(parsed)) => parsed,
                Ok(Err(error)) => {
                    tracing::warn!(path = ?path, %error, "parse error");
                    (Vec::new(), Vec::new())
                }
                Err(_) => {
                    tracing::warn!(path = ?path, "parse panic");
                    (Vec::new(), Vec::new())
                }
            }
        })
        .collect::<Vec<_>>();

    progress.finish_and_clear();

    let mut symbols = Vec::new();
    let mut references = Vec::new();
    for (file_symbols, file_references) in parsed {
        symbols.extend(file_symbols);
        references.extend(file_references);
    }

    (symbols, references)
}

pub(crate) fn node_text(node: Node<'_>, source: &[u8]) -> Option<String> {
    node.utf8_text(source).ok().map(ToString::to_string)
}

pub(crate) fn line_span(node: Node<'_>) -> (u32, u32) {
    (
        u32::try_from(node.start_position().row + 1).unwrap_or(u32::MAX),
        u32::try_from(node.end_position().row + 1).unwrap_or(u32::MAX),
    )
}

pub(crate) fn has_ancestor_kind(node: Node<'_>, kind: &str) -> bool {
    let mut current = node.parent();
    while let Some(parent) = current {
        if parent.kind() == kind {
            return true;
        }
        current = parent.parent();
    }
    false
}

pub(crate) fn enclosing_symbol_parts(node: Node<'_>, source: &[u8]) -> Vec<String> {
    let mut current = node.parent();
    let mut parts = Vec::new();

    while let Some(parent) = current {
        if parent.kind() == "impl_item" {
            if let Some(type_node) = parent.child_by_field_name("type") {
                if let Some(text) = node_text(type_node, source) {
                    if parts.last() != Some(&text) {
                        parts.push(text);
                    }
                }
            }
        }

        if let Some(name_node) = parent.child_by_field_name("name") {
            if let Some(text) = node_text(name_node, source) {
                if parts.last() != Some(&text) {
                    parts.push(text);
                }
            }
        }
        current = parent.parent();
    }

    parts.reverse();
    parts
}

pub(crate) fn symbol_parts(node: Node<'_>, own_name: &str, source: &[u8]) -> Vec<String> {
    let mut parts = enclosing_symbol_parts(node, source);
    if parts.last().map(String::as_str) != Some(own_name) {
        parts.push(own_name.to_string());
    }
    parts
}

pub(crate) fn source_fqn_for_node(
    file: &Path,
    root: &Path,
    node: Node<'_>,
    source: &[u8],
) -> String {
    let parts = enclosing_symbol_parts(node, source);
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    build_fqn(file, root, &refs)
}

pub(crate) fn path_is_test(path: &Path) -> bool {
    let value = path.to_string_lossy();
    value.contains("/tests/")
        || value.contains("\\tests\\")
        || value.ends_with("_test.go")
        || value.ends_with(".spec.ts")
        || value.ends_with(".spec.js")
        || value.ends_with(".test.ts")
        || value.ends_with(".test.js")
        || path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("test_"))
}

fn dispatch_extract(
    path: &Path,
    root: &Path,
    source: &[u8],
    language: Language,
) -> (Vec<Symbol>, Vec<Reference>) {
    match language {
        Language::Python => extract_python(path, root, source),
        Language::JavaScript => extract_javascript(path, root, source),
        Language::TypeScript => extract_typescript(
            path,
            root,
            source,
            path.extension().and_then(|ext| ext.to_str()) == Some("tsx"),
        ),
        Language::Go => extract_go(path, root, source),
        Language::Rust => extract_rust(path, root, source),
    }
}

fn build_cache_key(path: &Path, git_head: &str) -> String {
    let mtime = path
        .metadata()
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_secs());

    let mut hasher = Sha256::new();
    hasher.update(format!("{}:{mtime}:{git_head}", path.display()));
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use crate::config::Config;
    use crate::walker::Language;

    use super::{cache::ParseCache, parse_all};

    #[test]
    fn parse_all_extracts_python_symbols() {
        let temp = tempdir().expect("temp dir should be created");
        let file = temp.path().join("main.py");
        fs::write(&file, "def helper():\n    return 1\n").expect("source should be written");
        let files = vec![(file.clone(), Language::Python)];
        let cache = ParseCache::open(temp.path(), false);

        let (symbols, references) =
            parse_all(&files, temp.path(), &cache, "HEAD", &Config::default());

        assert!(symbols.iter().any(|symbol| symbol.name == "helper"));
        assert!(references.is_empty());
    }
}
