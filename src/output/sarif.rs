use anyhow::Result;
use path_slash::PathExt;
use serde::Serialize;

use crate::parse::types::{Finding, FindingTag};

#[derive(Serialize)]
struct SarifRoot {
    #[serde(rename = "$schema")]
    schema: &'static str,
    version: &'static str,
    runs: Vec<SarifRun>,
}

#[derive(Serialize)]
struct SarifRun {
    tool: SarifTool,
    results: Vec<SarifResult>,
}

#[derive(Serialize)]
struct SarifTool {
    driver: SarifDriver,
}

#[derive(Serialize)]
struct SarifDriver {
    name: &'static str,
    version: &'static str,
    rules: Vec<SarifRule>,
}

#[derive(Serialize)]
struct SarifRule {
    id: &'static str,
    name: &'static str,
    #[serde(rename = "shortDescription")]
    short_description: SarifMessage,
}

#[derive(Serialize)]
struct SarifResult {
    #[serde(rename = "ruleId")]
    rule_id: &'static str,
    level: &'static str,
    message: SarifMessage,
    locations: Vec<SarifLocation>,
}

#[derive(Serialize)]
struct SarifMessage {
    text: String,
}

#[derive(Serialize)]
struct SarifLocation {
    #[serde(rename = "physicalLocation")]
    physical_location: SarifPhysicalLocation,
}

#[derive(Serialize)]
struct SarifPhysicalLocation {
    #[serde(rename = "artifactLocation")]
    artifact_location: SarifArtifactLocation,
    region: SarifRegion,
}

#[derive(Serialize)]
struct SarifArtifactLocation {
    uri: String,
}

#[derive(Serialize)]
struct SarifRegion {
    #[serde(rename = "startLine")]
    start_line: i64,
}

pub fn render_sarif(findings: &[Finding]) -> Result<String> {
    let root = SarifRoot {
        schema: "https://json.schemastore.org/sarif-2.1.0.json",
        version: "2.1.0",
        runs: vec![SarifRun {
            tool: SarifTool {
                driver: SarifDriver {
                    name: "graveyard",
                    version: env!("CARGO_PKG_VERSION"),
                    rules: all_rules(),
                },
            },
            results: findings.iter().map(result_for_finding).collect(),
        }],
    };

    serde_json::to_string_pretty(&root).map_err(Into::into)
}

fn all_rules() -> Vec<SarifRule> {
    [
        FindingTag::Dead,
        FindingTag::ExportedUnused,
        FindingTag::InDeadCycle,
        FindingTag::TestOnly,
    ]
    .iter()
    .map(rule_for_tag)
    .collect()
}

fn rule_for_tag(tag: &FindingTag) -> SarifRule {
    let (id, name, description) = rule_metadata(tag);
    SarifRule {
        id,
        name,
        short_description: SarifMessage {
            text: description.to_string(),
        },
    }
}

fn result_for_finding(finding: &Finding) -> SarifResult {
    let (rule_id, _, _) = rule_metadata(&finding.tag);
    SarifResult {
        rule_id,
        level: if finding.confidence >= 0.8 {
            "error"
        } else {
            "warning"
        },
        message: SarifMessage {
            text: format!(
                "{} appears to be dead code (confidence {:.2})",
                finding.symbol.fqn, finding.confidence
            ),
        },
        locations: vec![SarifLocation {
            physical_location: SarifPhysicalLocation {
                artifact_location: SarifArtifactLocation {
                    uri: format!("file:///{}", finding.symbol.file.to_slash_lossy()),
                },
                region: SarifRegion {
                    start_line: i64::from(finding.symbol.line_start),
                },
            },
        }],
    }
}

fn rule_metadata(tag: &FindingTag) -> (&'static str, &'static str, &'static str) {
    match tag {
        FindingTag::Dead => ("GY001", "Dead", "Symbol appears unreachable"),
        FindingTag::ExportedUnused => (
            "GY002",
            "ExportedUnused",
            "Exported symbol has no internal callers",
        ),
        FindingTag::InDeadCycle => (
            "GY003",
            "InDeadCycle",
            "Symbol is only referenced within a dead cycle",
        ),
        FindingTag::TestOnly => ("GY004", "TestOnly", "Symbol is only referenced from tests"),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::Value;

    use crate::parse::types::{Finding, FindingTag, ScoreBreakdown, Symbol, SymbolKind};

    use super::render_sarif;

    fn finding() -> Finding {
        Finding {
            symbol: Symbol {
                fqn: "src/main.rs::candidate".to_string(),
                name: "candidate".to_string(),
                kind: SymbolKind::Function,
                language: "rust".to_string(),
                file: PathBuf::from("src/main.rs"),
                line_start: 8,
                line_end: 9,
                is_exported: false,
                is_test: false,
            },
            tag: FindingTag::Dead,
            confidence: 0.95,
            deadness_age_days: 365.0,
            in_degree: 0,
            score_breakdown: ScoreBreakdown {
                age_factor: 1.0,
                ref_factor: 1.0,
                scope_factor: 1.0,
                churn_factor: 1.0,
            },
        }
    }

    #[test]
    fn render_sarif_includes_schema_and_rule_id() {
        let output = render_sarif(&[finding()]).expect("sarif should render");
        let value: Value = serde_json::from_str(&output).expect("output should be valid json");

        assert_eq!(
            value["$schema"],
            "https://json.schemastore.org/sarif-2.1.0.json"
        );
        assert_eq!(value["version"], "2.1.0");
        assert_eq!(value["runs"][0]["results"][0]["ruleId"], "GY001");
        assert_eq!(
            value["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["region"]
                ["startLine"],
            8
        );
    }
}
