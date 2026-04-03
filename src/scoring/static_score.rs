use crate::config::ScoringConfig;
use crate::parse::types::{Symbol, SymbolKind};

pub fn age_factor(age_days: f64, config: &ScoringConfig) -> f64 {
    if age_days < config.age_min_days as f64 {
        return 0.0;
    }

    if age_days >= config.age_max_days as f64 {
        return 1.0;
    }

    let midpoint = config.age_max_days as f64 / 2.0;
    let value = 1.0 / (1.0 + (-0.01 * (age_days - midpoint)).exp());
    value.clamp(0.0, 1.0)
}

pub fn ref_factor(in_degree: usize) -> f64 {
    match in_degree {
        0 => 1.0,
        1 => 0.5,
        _ => 0.0,
    }
}

pub fn scope_factor(symbol: &Symbol) -> f64 {
    if symbol.is_exported {
        0.4
    } else if symbol.kind == SymbolKind::Method {
        0.6
    } else {
        1.0
    }
}

pub fn churn_factor(commits_90d: usize) -> f64 {
    match commits_90d {
        0 => 1.0,
        1..=2 => 0.5,
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::config::ScoringConfig;
    use crate::parse::types::{Symbol, SymbolKind};

    use super::{age_factor, churn_factor, ref_factor, scope_factor};

    fn symbol(kind: SymbolKind, is_exported: bool) -> Symbol {
        Symbol {
            fqn: format!("src/main.rs::{kind}"),
            name: "candidate".to_string(),
            kind,
            language: "rust".to_string(),
            file: PathBuf::from("src/main.rs"),
            line_start: 1,
            line_end: 1,
            is_exported,
            is_test: false,
        }
    }

    #[test]
    fn age_factor_respects_config_bounds() {
        let config = ScoringConfig::default();

        assert_eq!(age_factor(6.0, &config), 0.0);
        assert_eq!(age_factor(730.0, &config), 1.0);

        let midpoint = age_factor(365.0, &config);
        assert!(midpoint > 0.0 && midpoint < 1.0, "midpoint={midpoint}");
    }

    #[test]
    fn ref_factor_uses_bucketed_in_degree_scores() {
        assert_eq!(ref_factor(0), 1.0);
        assert_eq!(ref_factor(1), 0.5);
        assert_eq!(ref_factor(2), 0.0);
    }

    #[test]
    fn scope_factor_penalizes_exported_symbols_and_methods() {
        assert_eq!(scope_factor(&symbol(SymbolKind::Function, true)), 0.4);
        assert_eq!(scope_factor(&symbol(SymbolKind::Method, false)), 0.6);
        assert_eq!(scope_factor(&symbol(SymbolKind::Function, false)), 1.0);
    }

    #[test]
    fn churn_factor_uses_recent_commit_buckets() {
        assert_eq!(churn_factor(0), 1.0);
        assert_eq!(churn_factor(2), 0.5);
        assert_eq!(churn_factor(3), 0.0);
    }
}
