pub mod formula;
pub mod git_history;
pub mod static_score;

use std::collections::{HashMap, HashSet};

use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::Direction;

use crate::config::Config;
use crate::parse::types::{Finding, FindingTag};

use self::formula::confidence;
use self::git_history::GitScore;

pub fn assemble_findings(
    graph: &DiGraph<crate::parse::types::Symbol, ()>,
    dead_candidates: &[NodeIndex],
    dead_cycles: &HashSet<NodeIndex>,
    test_only: &HashSet<NodeIndex>,
    git_scores: &HashMap<String, GitScore>,
    config: &Config,
) -> Vec<Finding> {
    let min_age_days = config
        .min_age
        .map(|duration| duration.as_secs_f64() / 86_400.0);
    let mut findings = Vec::new();

    for idx in dead_candidates {
        let symbol = graph[*idx].clone();
        let tag = if dead_cycles.contains(idx) {
            FindingTag::InDeadCycle
        } else if symbol.is_exported {
            FindingTag::ExportedUnused
        } else if test_only.contains(idx) {
            FindingTag::TestOnly
        } else {
            FindingTag::Dead
        };

        if config.ignore_exports && tag == FindingTag::ExportedUnused {
            continue;
        }

        let in_degree = graph.edges_directed(*idx, Direction::Incoming).count();
        let git_score = git_scores.get(&symbol.fqn).copied().unwrap_or(GitScore {
            age_days: 0.0,
            commits_90d: 0,
        });
        let (score, score_breakdown) = confidence(
            &symbol,
            in_degree,
            git_score.age_days,
            git_score.commits_90d,
            config,
        );

        if min_age_days.is_some_and(|min_age| git_score.age_days < min_age) {
            continue;
        }

        if score < config.min_confidence {
            continue;
        }

        findings.push(Finding {
            symbol,
            tag,
            confidence: score,
            deadness_age_days: git_score.age_days,
            in_degree,
            score_breakdown,
        });
    }

    findings.sort_by(|left, right| right.confidence.total_cmp(&left.confidence));

    if config.top > 0 {
        findings.truncate(config.top);
    }

    findings
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};
    use std::path::PathBuf;
    use std::time::Duration;

    use petgraph::graph::{DiGraph, NodeIndex};

    use crate::config::Config;
    use crate::parse::types::{FindingTag, Symbol, SymbolKind};

    use super::assemble_findings;
    use super::git_history::GitScore;

    fn symbol(name: &str, is_exported: bool, is_test: bool) -> Symbol {
        Symbol {
            fqn: format!("src/main.rs::{name}"),
            name: name.to_string(),
            kind: SymbolKind::Function,
            language: "rust".to_string(),
            file: PathBuf::from("src/main.rs"),
            line_start: 1,
            line_end: 1,
            is_exported,
            is_test,
        }
    }

    #[test]
    fn assemble_findings_tags_and_sorts_candidates() {
        let mut graph = DiGraph::new();
        let cycle_idx = graph.add_node(symbol("cycle_dead", false, false));
        let exported_idx = graph.add_node(symbol("exported_dead", true, false));
        let test_caller_idx = graph.add_node(symbol("test_helper", false, true));
        let test_only_idx = graph.add_node(symbol("test_only_dead", false, false));
        graph.add_edge(test_caller_idx, test_only_idx, ());

        let dead_candidates = vec![cycle_idx, exported_idx, test_only_idx];
        let dead_cycles = HashSet::from([cycle_idx]);
        let test_only = HashSet::from([test_only_idx]);
        let git_scores = HashMap::from([
            (
                "src/main.rs::cycle_dead".to_string(),
                GitScore {
                    age_days: 730.0,
                    commits_90d: 0,
                },
            ),
            (
                "src/main.rs::exported_dead".to_string(),
                GitScore {
                    age_days: 730.0,
                    commits_90d: 0,
                },
            ),
            (
                "src/main.rs::test_only_dead".to_string(),
                GitScore {
                    age_days: 730.0,
                    commits_90d: 0,
                },
            ),
        ]);

        let findings = assemble_findings(
            &graph,
            &dead_candidates,
            &dead_cycles,
            &test_only,
            &git_scores,
            &Config::default(),
        );

        assert_eq!(findings.len(), 3);
        assert_eq!(findings[0].tag, FindingTag::InDeadCycle);
        assert_eq!(findings[1].tag, FindingTag::ExportedUnused);
        assert_eq!(findings[2].tag, FindingTag::TestOnly);
        assert_eq!(findings[0].confidence, 1.0);
        assert!(findings[1].confidence > findings[2].confidence);
    }

    #[test]
    fn assemble_findings_applies_filters_and_top_limit() {
        let mut graph = DiGraph::new();
        let strong_idx = graph.add_node(symbol("strong_dead", false, false));
        let exported_idx = graph.add_node(symbol("exported_dead", true, false));
        let young_idx = graph.add_node(symbol("young_dead", false, false));
        let weak_idx = graph.add_node(symbol("weak_dead", true, false));
        let dead_candidates = vec![strong_idx, exported_idx, young_idx, weak_idx];
        let git_scores = HashMap::from([
            (
                "src/main.rs::strong_dead".to_string(),
                GitScore {
                    age_days: 730.0,
                    commits_90d: 0,
                },
            ),
            (
                "src/main.rs::exported_dead".to_string(),
                GitScore {
                    age_days: 730.0,
                    commits_90d: 0,
                },
            ),
            (
                "src/main.rs::young_dead".to_string(),
                GitScore {
                    age_days: 10.0,
                    commits_90d: 0,
                },
            ),
            (
                "src/main.rs::weak_dead".to_string(),
                GitScore {
                    age_days: 0.0,
                    commits_90d: 3,
                },
            ),
        ]);

        let config = Config {
            ignore_exports: true,
            min_age: Some(Duration::from_secs(30 * 24 * 60 * 60)),
            min_confidence: 0.8,
            top: 1,
            ..Config::default()
        };

        let findings = assemble_findings(
            &graph,
            &dead_candidates,
            &HashSet::<NodeIndex>::new(),
            &HashSet::<NodeIndex>::new(),
            &git_scores,
            &config,
        );

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].symbol.fqn, "src/main.rs::strong_dead");
        assert_eq!(findings[0].tag, FindingTag::Dead);
    }
}
