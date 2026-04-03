use std::collections::HashSet;

use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::Dfs;

use crate::parse::types::Symbol;

pub fn find_reachable(
    graph: &DiGraph<Symbol, ()>,
    entry_names: &[String],
) -> HashSet<NodeIndex> {
    let entry_names = entry_names
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut reachable = HashSet::new();

    for entry_idx in graph
        .node_indices()
        .filter(|idx| entry_names.contains(graph[*idx].name.as_str()))
    {
        let mut dfs = Dfs::new(graph, entry_idx);
        while let Some(idx) = dfs.next(graph) {
            reachable.insert(idx);
        }
    }

    reachable
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::graph::build_graph;
    use crate::parse::types::{Reference, Symbol, SymbolKind};

    use super::find_reachable;

    fn symbol(name: &str) -> Symbol {
        Symbol {
            fqn: format!("src/main.rs::{name}"),
            name: name.to_string(),
            kind: SymbolKind::Function,
            language: "rust".to_string(),
            file: PathBuf::from("src/main.rs"),
            line_start: 1,
            line_end: 1,
            is_exported: false,
            is_test: false,
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
    fn reachable_walks_from_entry_points() {
        let main = symbol("main");
        let helper = symbol("helper");
        let deep = symbol("deep");
        let main_fqn = main.fqn.clone();
        let helper_fqn = helper.fqn.clone();
        let deep_fqn = deep.fqn.clone();

        let (graph, fqn_to_idx) = build_graph(
            vec![main, helper, deep],
            vec![
                reference(&main_fqn, "helper"),
                reference(&helper_fqn, "deep"),
            ],
        );

        let reachable = find_reachable(&graph, &[String::from("main")]);

        assert!(reachable.contains(&fqn_to_idx[&main_fqn]));
        assert!(reachable.contains(&fqn_to_idx[&helper_fqn]));
        assert!(reachable.contains(&fqn_to_idx[&deep_fqn]));
    }
}
