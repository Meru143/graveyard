use crate::config::Config;
use crate::parse::types::{ScoreBreakdown, Symbol};

use super::static_score::{age_factor, churn_factor, ref_factor, scope_factor};

pub fn confidence(
    symbol: &Symbol,
    in_degree: usize,
    age_days: f64,
    commits_90d: usize,
    config: &Config,
) -> (f64, ScoreBreakdown) {
    let af = age_factor(age_days, &config.scoring);
    let rf = ref_factor(in_degree);
    let sf = scope_factor(symbol);
    let cf = churn_factor(commits_90d);

    let mut score = config.scoring.age_weight * af
        + config.scoring.ref_weight * rf
        + config.scoring.scope_weight * sf
        + config.scoring.churn_weight * cf;
    score = score.clamp(0.0, 1.0);
    score = (score * 100.0).round() / 100.0;

    (
        score,
        ScoreBreakdown {
            age_factor: af,
            ref_factor: rf,
            scope_factor: sf,
            churn_factor: cf,
        },
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::config::Config;
    use crate::parse::types::{Symbol, SymbolKind};

    use super::confidence;

    fn symbol(is_exported: bool) -> Symbol {
        Symbol {
            fqn: "src/main.rs::candidate".to_string(),
            name: "candidate".to_string(),
            kind: SymbolKind::Function,
            language: "rust".to_string(),
            file: PathBuf::from("src/main.rs"),
            line_start: 1,
            line_end: 1,
            is_exported,
            is_test: false,
        }
    }

    #[test]
    fn confidence_returns_one_when_all_factors_are_one() {
        let config = Config::default();

        let (score, breakdown) = confidence(
            &symbol(false),
            0,
            config.scoring.age_max_days as f64,
            0,
            &config,
        );

        assert_eq!(score, 1.0);
        assert_eq!(breakdown.age_factor, 1.0);
        assert_eq!(breakdown.ref_factor, 1.0);
        assert_eq!(breakdown.scope_factor, 1.0);
        assert_eq!(breakdown.churn_factor, 1.0);
    }

    #[test]
    fn confidence_uses_weights_and_rounds_to_two_decimals() {
        let config = Config::default();

        let (score, breakdown) = confidence(
            &symbol(true),
            1,
            config.scoring.age_max_days as f64,
            1,
            &config,
        );

        assert_eq!(score, 0.66);
        assert_eq!(breakdown.ref_factor, 0.5);
        assert_eq!(breakdown.scope_factor, 0.4);
        assert_eq!(breakdown.churn_factor, 0.5);
    }
}
