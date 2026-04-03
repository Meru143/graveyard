use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::parse::types::{Reference, Symbol};

pub enum ParseCache {
    Enabled(sled::Db),
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    symbols: Vec<Symbol>,
    references: Vec<Reference>,
}

impl ParseCache {
    pub fn open(dir: &Path, disabled: bool) -> Self {
        if disabled {
            return Self::Disabled;
        }

        match sled::open(dir) {
            Ok(db) => Self::Enabled(db),
            Err(error) => {
                tracing::warn!(path = ?dir, %error, "cache disabled");
                Self::Disabled
            }
        }
    }

    pub fn get(&self, key: &str) -> Option<(Vec<Symbol>, Vec<Reference>)> {
        let Self::Enabled(db) = self else {
            return None;
        };

        let value = db.get(key.as_bytes()).ok().flatten()?;
        let entry = serde_json::from_slice::<CacheEntry>(&value).ok()?;
        Some((entry.symbols, entry.references))
    }

    pub fn set(&self, key: &str, symbols: &[Symbol], references: &[Reference]) {
        let Self::Enabled(db) = self else {
            return;
        };

        let entry = CacheEntry {
            symbols: symbols.to_vec(),
            references: references.to_vec(),
        };

        match serde_json::to_vec(&entry) {
            Ok(serialized) => {
                if let Err(error) = db.insert(key.as_bytes(), serialized) {
                    tracing::warn!(%error, "failed to write parse cache");
                }
            }
            Err(error) => tracing::warn!(%error, "failed to serialize parse cache"),
        }
    }

    pub fn clear(&self) {
        if let Self::Enabled(db) = self {
            if let Err(error) = db.clear() {
                tracing::warn!(%error, "failed to clear parse cache");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use crate::parse::types::{Reference, Symbol, SymbolKind};

    use super::ParseCache;

    #[test]
    fn cache_round_trips_symbols_and_references() {
        let temp = tempdir().expect("temp dir should be created");
        let cache = ParseCache::open(temp.path(), false);
        let symbols = vec![Symbol {
            fqn: "src/main.py::helper".to_string(),
            name: "helper".to_string(),
            kind: SymbolKind::Function,
            language: "python".to_string(),
            file: "src/main.py".into(),
            line_start: 1,
            line_end: 2,
            is_exported: false,
            is_test: false,
        }];
        let references = vec![Reference {
            source_fqn: "src/main.py::main".to_string(),
            target_name: "helper".to_string(),
            file: "src/main.py".into(),
            line: 1,
        }];

        cache.set("cache-key", &symbols, &references);
        let loaded = cache.get("cache-key").expect("cached value should exist");

        assert_eq!(loaded.0, symbols);
        assert_eq!(loaded.1, references);
    }
}
