use std::path::Path;

use std::collections::HashSet;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::output::json::JsonFinding;
use crate::parse::types::Finding;

#[derive(Debug, Serialize)]
struct BaselineEnvelope {
    graveyard_version: &'static str,
    baseline_created_at: String,
    total_findings: usize,
    findings: Vec<JsonFinding>,
}

#[derive(Debug, Deserialize)]
struct BaselineLoadEnvelope {
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

pub fn load_baseline(path: &Path) -> Result<HashSet<String>> {
    let content = std::fs::read_to_string(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            anyhow!("Baseline file not found: {:?}", path)
        } else {
            anyhow!(error)
        }
    })?;
    let envelope: BaselineLoadEnvelope = serde_json::from_str(&content)
        .with_context(|| format!("Baseline file is malformed: {:?}", path))?;

    Ok(envelope
        .findings
        .into_iter()
        .map(|finding| finding.symbol_fqn)
        .collect())
}

pub fn diff_findings(current: Vec<Finding>, baseline_fqns: HashSet<String>) -> Vec<Finding> {
    current
        .into_iter()
        .filter(|finding| !baseline_fqns.contains(&finding.symbol.fqn))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::Value;
    use tempfile::TempDir;

    use super::{diff_findings, load_baseline, save_baseline};
    use crate::parse::types::{Finding, FindingTag, ScoreBreakdown, Symbol, SymbolKind};

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

    #[test]
    fn load_baseline_returns_symbol_fqns_from_saved_file() {
        let temp_dir = TempDir::new().expect("temp dir should exist");
        let output_path = temp_dir.path().join("baseline.json");

        save_baseline(&[sample_finding()], &output_path).expect("baseline save should succeed");
        let baseline = load_baseline(&output_path).expect("baseline load should succeed");

        assert_eq!(baseline.len(), 1);
        assert!(baseline.contains("src/main.rs::old_fn"));
    }

    #[test]
    fn load_baseline_reports_missing_files_clearly() {
        let temp_dir = TempDir::new().expect("temp dir should exist");
        let missing_path = temp_dir.path().join("missing.json");

        let error = load_baseline(&missing_path).expect_err("missing baseline should fail");

        assert_eq!(
            error.to_string(),
            format!("Baseline file not found: {:?}", missing_path)
        );
    }

    #[test]
    fn diff_findings_returns_only_new_fqns() {
        let mut new_finding = sample_finding();
        new_finding.symbol.fqn = "src/main.rs::brand_new".to_string();
        new_finding.symbol.name = "brand_new".to_string();

        let current = vec![sample_finding(), new_finding.clone()];
        let baseline = std::collections::HashSet::from(["src/main.rs::old_fn".to_string()]);

        let diff = diff_findings(current, baseline);

        assert_eq!(diff, vec![new_finding]);
    }
}
