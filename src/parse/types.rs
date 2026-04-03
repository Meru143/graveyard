use std::fmt;
use std::path::{Path, PathBuf};

use path_slash::PathExt;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Struct,
    Enum,
    Variable,
    Interface,
    TypeAlias,
}

impl fmt::Display for SymbolKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Function => "function",
            Self::Method => "method",
            Self::Class => "class",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Variable => "variable",
            Self::Interface => "interface",
            Self::TypeAlias => "type_alias",
        };

        formatter.write_str(value)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum FindingTag {
    Dead,
    ExportedUnused,
    InDeadCycle,
    TestOnly,
}

impl fmt::Display for FindingTag {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Dead => "dead",
            Self::ExportedUnused => "exported_unused",
            Self::InDeadCycle => "in_dead_cycle",
            Self::TestOnly => "test_only",
        };

        formatter.write_str(value)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Symbol {
    pub fqn: String,
    pub name: String,
    pub kind: SymbolKind,
    pub language: String,
    pub file: PathBuf,
    pub line_start: u32,
    pub line_end: u32,
    pub is_exported: bool,
    pub is_test: bool,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Reference {
    pub source_fqn: String,
    pub target_name: String,
    pub file: PathBuf,
    pub line: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Finding {
    pub symbol: Symbol,
    pub tag: FindingTag,
    pub confidence: f64,
    pub deadness_age_days: f64,
    pub in_degree: usize,
    pub score_breakdown: ScoreBreakdown,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScoreBreakdown {
    pub age_factor: f64,
    pub ref_factor: f64,
    pub scope_factor: f64,
    pub churn_factor: f64,
}

pub fn build_fqn(file: &Path, root: &Path, parts: &[&str]) -> String {
    let relative = file.strip_prefix(root).unwrap_or(file);
    let mut segments = vec![relative.to_slash_lossy().to_string()];
    segments.extend(parts.iter().map(|part| (*part).to_string()));
    segments.join("::")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{build_fqn, FindingTag, ScoreBreakdown, Symbol, SymbolKind};

    #[test]
    fn build_fqn_normalizes_relative_paths() {
        let file = Path::new(r"C:\repo\src\foo.py");
        let root = Path::new(r"C:\repo");

        let fqn = build_fqn(file, root, &["MyClass", "my_method"]);

        assert_eq!(fqn, "src/foo.py::MyClass::my_method");
    }

    #[test]
    fn display_uses_expected_names() {
        assert_eq!(SymbolKind::TypeAlias.to_string(), "type_alias");
        assert_eq!(FindingTag::ExportedUnused.to_string(), "exported_unused");
    }

    #[test]
    fn finding_related_types_are_constructible() {
        let symbol = Symbol {
            fqn: "src/foo.py::func".to_string(),
            name: "func".to_string(),
            kind: SymbolKind::Function,
            language: "python".to_string(),
            file: Path::new("src/foo.py").to_path_buf(),
            line_start: 1,
            line_end: 2,
            is_exported: false,
            is_test: false,
        };

        let breakdown = ScoreBreakdown {
            age_factor: 1.0,
            ref_factor: 1.0,
            scope_factor: 1.0,
            churn_factor: 1.0,
        };

        assert_eq!(symbol.name, "func");
        assert_eq!(breakdown.churn_factor, 1.0);
    }
}
