use indicatif::ProgressBar;
use owo_colors::OwoColorize;
use path_slash::PathExt;

use crate::config::Config;
use crate::parse::types::Finding;

pub fn render_table(findings: &[Finding], config: &Config) -> String {
    let spinner = ProgressBar::hidden();
    spinner.finish_with_message(String::new());

    let color_enabled = colors_enabled(config);

    if findings.is_empty() {
        let message = format!(
            "✓ No dead code found above confidence {:.1}",
            config.min_confidence
        );
        return if color_enabled {
            format!("{}\n", message.green())
        } else {
            format!("{message}\n")
        };
    }

    let mut output = String::new();
    let header = format!(
        "{:<10}  {:<16}  {:<10}  {:<24}  {}",
        "CONFIDENCE", "TAG", "AGE", "LOCATION", "FQN"
    );
    if color_enabled {
        output.push_str(&format!("{}\n", header.bold().underline()));
    } else {
        output.push_str(&format!("{header}\n"));
    }

    for finding in findings {
        let confidence_text = format!("{:<10}", format!("{:.2}", finding.confidence));
        let confidence = if color_enabled {
            if finding.confidence >= 0.9 {
                format!("{}", confidence_text.red())
            } else if finding.confidence >= 0.7 {
                format!("{}", confidence_text.yellow())
            } else {
                format!("{}", confidence_text.white())
            }
        } else {
            confidence_text
        };
        let tag = format!("{:<16}", finding.tag.to_string());
        let age = format!("{:<10}", format_deadness_age(finding.deadness_age_days));
        let location = format!(
            "{:<24}",
            format!(
                "{}:{}",
                finding.symbol.file.to_slash_lossy(),
                finding.symbol.line_start
            )
        );
        let fqn = truncate_fqn(&finding.symbol.fqn, 60);

        output.push_str(&format!(
            "{}  {}  {}  {}  {}\n",
            confidence, tag, age, location, fqn
        ));
    }

    let min_age = config
        .min_age
        .map(|duration| format_deadness_age(duration.as_secs_f64() / 86_400.0))
        .unwrap_or_else(|| "none".to_string());
    let footer = format!(
        "Found {} dead symbol(s) — min-confidence {:.1}, min-age {}",
        findings.len(),
        config.min_confidence,
        min_age
    );
    if color_enabled {
        output.push_str(&format!("{}\n", footer.bold()));
    } else {
        output.push_str(&format!("{footer}\n"));
    }

    output
}

fn colors_enabled(config: &Config) -> bool {
    !(config.no_color
        || std::env::var_os("NO_COLOR").is_some()
        || std::env::var_os("GRAVEYARD_NO_COLOR").is_some())
}

fn truncate_fqn(fqn: &str, max_chars: usize) -> String {
    if fqn.chars().count() <= max_chars {
        return fqn.to_string();
    }

    let visible = max_chars.saturating_sub(1);
    let prefix = fqn.chars().take(visible).collect::<String>();
    format!("{prefix}…")
}

fn format_deadness_age(age_days: f64) -> String {
    if age_days < 1.0 {
        "< 1 day".to_string()
    } else if age_days <= 30.0 {
        format!("{} days", age_days.round() as u32)
    } else if age_days <= 365.0 {
        format!("{} months", (age_days / 30.0).round() as u32)
    } else {
        format!("{:.1} years", age_days / 365.0)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::config::Config;
    use crate::parse::types::{Finding, FindingTag, ScoreBreakdown, Symbol, SymbolKind};

    use super::render_table;

    fn finding() -> Finding {
        Finding {
            symbol: Symbol {
                fqn: "src/main.rs::very_long_symbol_name_that_should_be_truncated_in_the_table_renderer".to_string(),
                name: "very_long_symbol_name_that_should_be_truncated_in_the_table_renderer".to_string(),
                kind: SymbolKind::Function,
                language: "rust".to_string(),
                file: PathBuf::from("src/main.rs"),
                line_start: 42,
                line_end: 45,
                is_exported: false,
                is_test: false,
            },
            tag: FindingTag::Dead,
            confidence: 0.95,
            deadness_age_days: 400.0,
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
    fn render_table_formats_rows_and_footer() {
        let output = render_table(&[finding()], &Config::default());

        assert!(output.contains("CONFIDENCE"));
        assert!(output.contains("src/main.rs:42"));
        assert!(output.contains("1.1 years"));
        assert!(output.contains("Found 1 dead symbol(s)"));
        assert!(output.contains("…"));
    }

    #[test]
    fn render_table_reports_empty_results() {
        let output = render_table(&[], &Config::default());

        assert!(output.contains("No dead code found above confidence 0.5"));
    }
}
