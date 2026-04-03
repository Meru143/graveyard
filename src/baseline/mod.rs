use std::path::Path;

use anyhow::Result;
use chrono::Utc;
use serde::Serialize;

use crate::output::json::JsonFinding;
use crate::parse::types::Finding;

#[derive(Debug, Serialize)]
struct BaselineEnvelope {
    graveyard_version: &'static str,
    baseline_created_at: String,
    total_findings: usize,
    findings: Vec<JsonFinding>,
}

pub fn save_baseline(findings: &[Finding], output_path: &Path) -> Result<()> {
    let envelope = BaselineEnvelope {
        graveyard_version: env!("CARGO_PKG_VERSION"),
        baseline_created_at: Utc::now().to_rfc3339(),
        total_findings: findings.len(),
        findings: findings.iter().map(JsonFinding::from).collect(),
    };
    let content = serde_json::to_string_pretty(&envelope)?;
    std::fs::write(output_path, content)?;
    eprintln!(
        "Baseline saved: {} ({} findings)",
        output_path.display(),
        findings.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::Value;
    use tempfile::TempDir;

    use crate::parse::types::{Finding, FindingTag, ScoreBreakdown, Symbol, SymbolKind};
    use super::save_baseline;

    fn sample_finding() -> Finding {
        Finding {
            symbol: Symbol {
                fqn: "src/main.rs::old_fn".to_string(),
                name: "old_fn".to_string(),
                kind: SymbolKind::Function,
                language: "rust".to_string(),
                file: PathBuf::from("src/main.rs"),
                line_start: 10,
                line_end: 12,
                is_exported: false,
                is_test: false,
            },
            tag: FindingTag::Dead,
            confidence: 0.82,
            deadness_age_days: 300.0,
            in_degree: 0,
            score_breakdown: ScoreBreakdown {
                age_factor: 1.0,
                ref_factor: 1.0,
                scope_factor: 1.0,
                churn_factor: 0.5,
            },
        }
    }

    #[test]
    fn save_baseline_writes_json_envelope_with_timestamp_and_findings() {
        let temp_dir = TempDir::new().expect("temp dir should exist");
        let output_path = temp_dir.path().join("baseline.json");

        save_baseline(&[sample_finding()], &output_path).expect("baseline save should succeed");

        let content = std::fs::read_to_string(&output_path).expect("baseline file should exist");
        let value: Value = serde_json::from_str(&content).expect("baseline output should be json");

        assert_eq!(value["graveyard_version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(value["total_findings"], 1);
        assert!(value["baseline_created_at"].as_str().is_some());
        assert_eq!(value["findings"][0]["symbol_fqn"], "src/main.rs::old_fn");
        assert_eq!(value["findings"][0]["confidence"], 0.82);
    }
}
