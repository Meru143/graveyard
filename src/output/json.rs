use anyhow::Result;
use chrono::Utc;
use path_slash::PathExt;
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::parse::types::Finding;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonScoreBreakdown {
    pub age_factor: f64,
    pub ref_factor: f64,
    pub scope_factor: f64,
    pub churn_factor: f64,
}

impl From<&crate::parse::types::ScoreBreakdown> for JsonScoreBreakdown {
    fn from(value: &crate::parse::types::ScoreBreakdown) -> Self {
        Self {
            age_factor: value.age_factor,
            ref_factor: value.ref_factor,
            scope_factor: value.scope_factor,
            churn_factor: value.churn_factor,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonFinding {
    pub symbol_fqn: String,
    pub symbol_name: String,
    pub file: String,
    pub line_start: u32,
    pub line_end: u32,
    pub language: String,
    pub kind: String,
    pub tag: String,
    pub confidence: f64,
    pub deadness_age_days: f64,
    pub in_degree: usize,
    pub score_breakdown: JsonScoreBreakdown,
}

impl From<&Finding> for JsonFinding {
    fn from(value: &Finding) -> Self {
        Self {
            symbol_fqn: value.symbol.fqn.clone(),
            symbol_name: value.symbol.name.clone(),
            file: value.symbol.file.to_slash_lossy().to_string(),
            line_start: value.symbol.line_start,
            line_end: value.symbol.line_end,
            language: value.symbol.language.clone(),
            kind: value.symbol.kind.to_string(),
            tag: value.tag.to_string(),
            confidence: value.confidence,
            deadness_age_days: value.deadness_age_days,
            in_degree: value.in_degree,
            score_breakdown: JsonScoreBreakdown::from(&value.score_breakdown),
        }
    }
}

#[derive(Debug, Serialize)]
struct JsonEnvelope {
    graveyard_version: &'static str,
    scanned_at: String,
    total_findings: usize,
    min_confidence: f64,
    findings: Vec<JsonFinding>,
}

pub fn render_json(findings: &[Finding], config: &Config) -> Result<String> {
    let envelope = JsonEnvelope {
        graveyard_version: env!("CARGO_PKG_VERSION"),
        scanned_at: Utc::now().to_rfc3339(),
        total_findings: findings.len(),
        min_confidence: config.min_confidence,
        findings: findings.iter().map(JsonFinding::from).collect(),
    };

    serde_json::to_string_pretty(&envelope).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::Value;

    use crate::config::Config;
    use crate::parse::types::{Finding, FindingTag, ScoreBreakdown, Symbol, SymbolKind};

    use super::render_json;

    fn finding() -> Finding {
        Finding {
            symbol: Symbol {
                fqn: "src/main.rs::candidate".to_string(),
                name: "candidate".to_string(),
                kind: SymbolKind::Function,
                language: "rust".to_string(),
                file: PathBuf::from("src/main.rs"),
                line_start: 10,
                line_end: 12,
                is_exported: false,
                is_test: false,
            },
            tag: FindingTag::Dead,
            confidence: 0.91,
            deadness_age_days: 120.0,
            in_degree: 0,
            score_breakdown: ScoreBreakdown {
                age_factor: 0.8,
                ref_factor: 1.0,
                scope_factor: 1.0,
                churn_factor: 1.0,
            },
        }
    }

    #[test]
    fn render_json_wraps_findings_in_expected_envelope() {
        let output = render_json(&[finding()], &Config::default()).expect("json should render");
        let value: Value = serde_json::from_str(&output).expect("output should be valid json");

        assert_eq!(value["graveyard_version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(value["total_findings"], 1);
        assert_eq!(value["min_confidence"], 0.5);
        assert_eq!(value["findings"][0]["symbol_fqn"], "src/main.rs::candidate");
    }
}
