pub mod csv;
pub mod json;
pub mod sarif;
pub mod table;

use std::fs;

use anyhow::Result;

use crate::config::{Config, OutputFormat};
use crate::parse::types::Finding;

use self::csv::render_csv;
use self::json::render_json;
use self::sarif::render_sarif;
use self::table::render_table;

pub fn write_output(findings: &[Finding], config: &Config) -> Result<()> {
    let content = match config.format {
        OutputFormat::Table => render_table(findings, config),
        OutputFormat::Json => render_json(findings, config)?,
        OutputFormat::Sarif => render_sarif(findings)?,
        OutputFormat::Csv => render_csv(findings),
    };

    if let Some(path) = &config.output {
        fs::write(path, &content)?;
        eprintln!("Output written to {}", path.display());
    } else {
        print!("{content}");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use tempfile::tempdir;

    use crate::config::{Config, OutputFormat};
    use crate::parse::types::{Finding, FindingTag, ScoreBreakdown, Symbol, SymbolKind};

    use super::write_output;

    fn finding() -> Finding {
        Finding {
            symbol: Symbol {
                fqn: "src/main.rs::candidate".to_string(),
                name: "candidate".to_string(),
                kind: SymbolKind::Function,
                language: "rust".to_string(),
                file: PathBuf::from("src/main.rs"),
                line_start: 1,
                line_end: 1,
                is_exported: false,
                is_test: false,
            },
            tag: FindingTag::Dead,
            confidence: 0.95,
            deadness_age_days: 180.0,
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
    fn write_output_dispatches_and_writes_to_file() {
        let temp = tempdir().expect("temp dir should be created");
        let path = temp.path().join("findings.json");
        let config = Config {
            format: OutputFormat::Json,
            output: Some(path.clone()),
            ..Config::default()
        };

        write_output(&[finding()], &config).expect("output should be written");

        let written = fs::read_to_string(path).expect("output file should exist");
        assert!(written.contains("\"graveyard_version\""));
        assert!(written.contains("\"symbol_fqn\""));
    }
}
