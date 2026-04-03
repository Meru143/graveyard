use std::fmt::Write;

use path_slash::PathExt;

use crate::parse::types::Finding;

pub fn render_csv(findings: &[Finding]) -> String {
    let mut output = String::new();
    output.push_str(
        "fqn,name,file,line_start,line_end,language,kind,tag,confidence,deadness_age_days,in_degree\n",
    );

    for finding in findings {
        let row = [
            escape_csv(&finding.symbol.fqn),
            escape_csv(&finding.symbol.name),
            escape_csv(&finding.symbol.file.to_slash_lossy()),
            finding.symbol.line_start.to_string(),
            finding.symbol.line_end.to_string(),
            escape_csv(&finding.symbol.language),
            escape_csv(&finding.symbol.kind.to_string()),
            escape_csv(&finding.tag.to_string()),
            format!("{:.2}", finding.confidence),
            format!("{:.2}", finding.deadness_age_days),
            finding.in_degree.to_string(),
        ];
        writeln!(output, "{}", row.join(",")).expect("writing to a string should not fail");
    }

    output
}

fn escape_csv(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::parse::types::{Finding, FindingTag, ScoreBreakdown, Symbol, SymbolKind};

    use super::render_csv;

    fn finding() -> Finding {
        Finding {
            symbol: Symbol {
                fqn: "src/main.rs::candidate,with,comma".to_string(),
                name: "candidate,with,comma".to_string(),
                kind: SymbolKind::Function,
                language: "rust".to_string(),
                file: PathBuf::from("src/main.rs"),
                line_start: 1,
                line_end: 2,
                is_exported: false,
                is_test: false,
            },
            tag: FindingTag::Dead,
            confidence: 0.75,
            deadness_age_days: 90.0,
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
    fn render_csv_writes_header_and_escapes_fields() {
        let output = render_csv(&[finding()]);

        assert!(output.starts_with("fqn,name,file,line_start,line_end,language,kind,tag,confidence,deadness_age_days,in_degree"));
        assert!(output.contains("\"src/main.rs::candidate,with,comma\""));
        assert!(output.contains("\"candidate,with,comma\""));
    }
}
