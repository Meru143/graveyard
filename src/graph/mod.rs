pub mod reachability;

use std::collections::{HashMap, HashSet};

use petgraph::algo::kosaraju_scc;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Direction;

use crate::parse::types::{Reference, Symbol};

pub fn build_graph(
    symbols: Vec<Symbol>,
    references: Vec<Reference>,
) -> (DiGraph<Symbol, ()>, HashMap<String, NodeIndex>) {
    let mut graph = DiGraph::new();
    let mut fqn_to_idx = HashMap::new();
    let mut name_to_indices: HashMap<String, Vec<NodeIndex>> = HashMap::new();

    for symbol in symbols {
        let idx = graph.add_node(symbol.clone());
        fqn_to_idx.insert(symbol.fqn.clone(), idx);
        name_to_indices
            .entry(symbol.name.clone())
            .or_default()
            .push(idx);
    }

    for reference in references {
        let Some(&source_idx) = fqn_to_idx.get(&reference.source_fqn) else {
            continue;
        };
        let Some(target_indices) = name_to_indices.get(&reference.target_name) else {
            continue;
        };

        for &target_idx in target_indices {
            if graph.find_edge(source_idx, target_idx).is_none() {
                graph.add_edge(source_idx, target_idx, ());
            }
        }
    }

    (graph, fqn_to_idx)
}

pub fn find_dead_candidates(
    graph: &DiGraph<Symbol, ()>,
    reachable: &HashSet<NodeIndex>,
) -> Vec<NodeIndex> {
    graph
        .node_indices()
        .filter(|idx| !reachable.contains(idx))
        .filter(|idx| !graph[*idx].is_test)
        .filter(|idx| {
            graph.edges_directed(*idx, Direction::Incoming)
                .all(|edge| graph[edge.source()].is_test)
        })
        .collect()
}

pub fn find_dead_cycles(
    graph: &DiGraph<Symbol, ()>,
    reachable: &HashSet<NodeIndex>,
) -> HashSet<NodeIndex> {
    let mut dead_cycles = HashSet::new();

    for scc in kosaraju_scc(graph) {
        if scc.len() <= 1 || scc.iter().any(|idx| reachable.contains(idx)) {
            continue;
        }

        let scc_set = scc.iter().copied().collect::<HashSet<_>>();
        let has_external_caller = scc.iter().any(|idx| {
            graph
                .edges_directed(*idx, Direction::Incoming)
                .any(|edge| !scc_set.contains(&edge.source()))
        });

        if !has_external_caller {
            dead_cycles.extend(scc_set);
        }
    }

    dead_cycles
}

pub fn find_test_only(
    graph: &DiGraph<Symbol, ()>,
    dead_candidates: &[NodeIndex],
) -> HashSet<NodeIndex> {
    dead_candidates
        .iter()
        .copied()
        .filter(|idx| {
            let callers = graph
                .edges_directed(*idx, Direction::Incoming)
                .map(|edge| edge.source())
                .collect::<Vec<_>>();

            !callers.is_empty() && callers.iter().all(|caller| graph[*caller].is_test)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::PathBuf;

    use crate::graph::reachability::find_reachable;
    use crate::parse::types::{Reference, Symbol, SymbolKind};

    use super::{build_graph, find_dead_candidates, find_dead_cycles, find_test_only};

    fn symbol(name: &str, is_test: bool) -> Symbol {
        Symbol {
            fqn: format!("src/main.rs::{name}"),
            name: name.to_string(),
            kind: SymbolKind::Function,
            language: "rust".to_string(),
            file: PathBuf::from("src/main.rs"),
            line_start: 1,
            line_end: 1,
            is_exported: false,
            is_test,
        }
    }

    fn reference(source_fqn: &str, target_name: &str) -> Reference {
        Reference {
            source_fqn: source_fqn.to_string(),
            target_name: target_name.to_string(),
            file: PathBuf::from("src/main.rs"),
            line: 1,
        }
    }

    #[test]
    fn build_graph_connects_references_by_name() {
        let main = symbol("main", false);
        let helper = symbol("helper", false);
        let main_fqn = main.fqn.clone();
        let helper_fqn = helper.fqn.clone();

        let (graph, fqn_to_idx) = build_graph(
            vec![main, helper],
            vec![reference(&main_fqn, "helper")],
        );

        let main_idx = fqn_to_idx[&main_fqn];
        let helper_idx = fqn_to_idx[&helper_fqn];

        assert_eq!(graph.node_count(), 2);
        assert!(graph.find_edge(main_idx, helper_idx).is_some());
    }

    #[test]
    fn dead_candidates_filter_reachable_and_track_test_only_callers() {
        let main = symbol("main", false);
        let helper = symbol("helper", false);
        let orphan = symbol("orphan", false);
        let test_helper = symbol("test_helper", true);
        let only_test = symbol("only_test", false);

        let main_fqn = main.fqn.clone();
        let helper_fqn = helper.fqn.clone();
        let test_helper_fqn = test_helper.fqn.clone();
        let orphan_fqn = orphan.fqn.clone();
        let only_test_fqn = only_test.fqn.clone();

        let (graph, fqn_to_idx) = build_graph(
            vec![main, helper, orphan, test_helper, only_test],
            vec![
                reference(&main_fqn, "helper"),
                reference(&test_helper_fqn, "only_test"),
            ],
        );

        let reachable = find_reachable(&graph, &[String::from("main")]);
        let dead = find_dead_candidates(&graph, &reachable);
        let dead_set = dead.iter().copied().collect::<HashSet<_>>();

        assert!(dead_set.contains(&fqn_to_idx[&orphan_fqn]));
        assert!(dead_set.contains(&fqn_to_idx[&only_test_fqn]));
        assert!(!dead_set.contains(&fqn_to_idx[&main_fqn]));
        assert!(!dead_set.contains(&fqn_to_idx[&helper_fqn]));

        let test_only = find_test_only(&graph, &dead);

        assert!(test_only.contains(&fqn_to_idx[&only_test_fqn]));
        assert!(!test_only.contains(&fqn_to_idx[&orphan_fqn]));
    }

    #[test]
    fn dead_cycles_detect_mutual_recursion_without_external_callers() {
        let alpha = symbol("alpha", false);
        let beta = symbol("beta", false);
        let alpha_fqn = alpha.fqn.clone();
        let beta_fqn = beta.fqn.clone();

        let (graph, fqn_to_idx) = build_graph(
            vec![alpha, beta],
            vec![
                reference(&alpha_fqn, "beta"),
                reference(&beta_fqn, "alpha"),
            ],
        );

        let cycles = find_dead_cycles(&graph, &HashSet::new());

        assert!(cycles.contains(&fqn_to_idx[&alpha_fqn]));
        assert!(cycles.contains(&fqn_to_idx[&beta_fqn]));
    }
}
