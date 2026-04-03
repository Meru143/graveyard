use std::path::Path;

use once_cell::sync::Lazy;
use tree_sitter::{Parser, Query, QueryCursor, StreamingIterator};

use crate::parse::types::{build_fqn, Reference, Symbol, SymbolKind};

use super::{
    has_ancestor_kind, line_span, node_text, path_is_test, source_fqn_for_node, symbol_parts,
};

pub static PY_LANGUAGE: Lazy<tree_sitter::Language> =
    Lazy::new(|| tree_sitter_python::LANGUAGE.into());

const FUNCTION_QUERY: &str = r#"
    (function_definition
      name: (identifier) @name) @definition.function
"#;

const CLASS_QUERY: &str = r#"
    (class_definition
      name: (identifier) @name) @definition.class
"#;

const ALL_QUERY: &str = r#"
    (assignment
      left: (identifier) @name
      right: (_) @value) @assignment
"#;

const CALL_QUERY: &str = r#"
    (call
      function: [
        (identifier) @name
        (attribute attribute: (identifier) @name)
      ]) @reference.call
"#;

pub fn extract_python(path: &Path, root: &Path, source: &[u8]) -> (Vec<Symbol>, Vec<Reference>) {
    let mut parser = Parser::new();
    if parser.set_language(&PY_LANGUAGE).is_err() {
        return (Vec::new(), Vec::new());
    }

    let Some(tree) = parser.parse(source, None) else {
        return (Vec::new(), Vec::new());
    };
    let root_node = tree.root_node();

    let mut symbols = Vec::new();
    symbols.extend(collect_python_classes(path, root, root_node, source));
    symbols.extend(collect_python_functions(path, root, root_node, source));
    apply_python_all_exports(root_node, source, &mut symbols);

    let references = collect_python_references(path, root, root_node, source);
    (symbols, references)
}

fn collect_python_functions(
    path: &Path,
    root: &Path,
    root_node: tree_sitter::Node<'_>,
    source: &[u8],
) -> Vec<Symbol> {
    let query = Query::new(&PY_LANGUAGE, FUNCTION_QUERY).expect("python function query is valid");
    let capture_names = query.capture_names();
    let mut cursor = QueryCursor::new();
    let mut symbols = Vec::new();

    let mut matches = cursor.matches(&query, root_node, source);
    while let Some(query_match) = matches.next() {
        let name_node = query_match.captures.iter().find_map(|capture| {
            (capture_names[capture.index as usize] == "name").then_some(capture.node)
        });
        let definition_node = query_match.captures.iter().find_map(|capture| {
            (capture_names[capture.index as usize] == "definition.function").then_some(capture.node)
        });

        let (Some(name_node), Some(definition_node)) = (name_node, definition_node) else {
            continue;
        };
        let Some(name) = node_text(name_node, source) else {
            continue;
        };

        let kind = if has_ancestor_kind(definition_node, "class_definition") {
            SymbolKind::Method
        } else {
            SymbolKind::Function
        };
        let parts = symbol_parts(definition_node, &name, source);
        let part_refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
        let (line_start, line_end) = line_span(definition_node);

        symbols.push(Symbol {
            fqn: build_fqn(path, root, &part_refs),
            name,
            kind,
            language: "python".to_string(),
            file: path.to_path_buf(),
            line_start,
            line_end,
            is_exported: false,
            is_test: path_is_test(path) || has_fixture_decorator(definition_node, source),
        });
    }

    symbols
}

fn collect_python_classes(
    path: &Path,
    root: &Path,
    root_node: tree_sitter::Node<'_>,
    source: &[u8],
) -> Vec<Symbol> {
    let query = Query::new(&PY_LANGUAGE, CLASS_QUERY).expect("python class query is valid");
    let capture_names = query.capture_names();
    let mut cursor = QueryCursor::new();
    let mut symbols = Vec::new();

    let mut matches = cursor.matches(&query, root_node, source);
    while let Some(query_match) = matches.next() {
        let name_node = query_match.captures.iter().find_map(|capture| {
            (capture_names[capture.index as usize] == "name").then_some(capture.node)
        });
        let class_node = query_match.captures.iter().find_map(|capture| {
            (capture_names[capture.index as usize] == "definition.class").then_some(capture.node)
        });

        let (Some(name_node), Some(class_node)) = (name_node, class_node) else {
            continue;
        };
        let Some(name) = node_text(name_node, source) else {
            continue;
        };

        let parts = symbol_parts(class_node, &name, source);
        let part_refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
        let (line_start, line_end) = line_span(class_node);

        symbols.push(Symbol {
            fqn: build_fqn(path, root, &part_refs),
            name,
            kind: SymbolKind::Class,
            language: "python".to_string(),
            file: path.to_path_buf(),
            line_start,
            line_end,
            is_exported: false,
            is_test: path_is_test(path),
        });
    }

    symbols
}

fn apply_python_all_exports(
    root_node: tree_sitter::Node<'_>,
    source: &[u8],
    symbols: &mut [Symbol],
) {
    let query = Query::new(&PY_LANGUAGE, ALL_QUERY).expect("__all__ query is valid");
    let capture_names = query.capture_names();
    let mut cursor = QueryCursor::new();

    let mut matches = cursor.matches(&query, root_node, source);
    while let Some(query_match) = matches.next() {
        let name_node = query_match.captures.iter().find_map(|capture| {
            (capture_names[capture.index as usize] == "name").then_some(capture.node)
        });
        let value_node = query_match.captures.iter().find_map(|capture| {
            (capture_names[capture.index as usize] == "value").then_some(capture.node)
        });

        let (Some(name_node), Some(value_node)) = (name_node, value_node) else {
            continue;
        };
        if node_text(name_node, source).as_deref() != Some("__all__") {
            continue;
        }

        let Some(value_text) = node_text(value_node, source) else {
            continue;
        };

        for export_name in extract_string_names(&value_text) {
            if let Some(symbol) = symbols.iter_mut().find(|symbol| symbol.name == export_name) {
                symbol.is_exported = true;
            }
        }
    }
}

fn collect_python_references(
    path: &Path,
    root: &Path,
    root_node: tree_sitter::Node<'_>,
    source: &[u8],
) -> Vec<Reference> {
    let query = Query::new(&PY_LANGUAGE, CALL_QUERY).expect("python call query is valid");
    let capture_names = query.capture_names();
    let mut cursor = QueryCursor::new();
    let mut references = Vec::new();

    let mut matches = cursor.matches(&query, root_node, source);
    while let Some(query_match) = matches.next() {
        let name_node = query_match.captures.iter().find_map(|capture| {
            (capture_names[capture.index as usize] == "name").then_some(capture.node)
        });
        let call_node = query_match.captures.iter().find_map(|capture| {
            (capture_names[capture.index as usize] == "reference.call").then_some(capture.node)
        });

        let (Some(name_node), Some(call_node)) = (name_node, call_node) else {
            continue;
        };
        let Some(target_name) = node_text(name_node, source) else {
            continue;
        };

        references.push(Reference {
            source_fqn: source_fqn_for_node(path, root, call_node, source),
            target_name,
            file: path.to_path_buf(),
            line: line_span(call_node).0,
        });
    }

    references
}

fn has_fixture_decorator(node: tree_sitter::Node<'_>, source: &[u8]) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    if parent.kind() != "decorated_definition" {
        return false;
    }

    let Some(text) = node_text(parent, source) else {
        return false;
    };

    text.contains("pytest.fixture") || text.contains("@fixture")
}

fn extract_string_names(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut current = String::new();
    let mut quote = None;

    for ch in text.chars() {
        match quote {
            Some(active) if ch == active => {
                if !current.is_empty() {
                    names.push(current.clone());
                }
                current.clear();
                quote = None;
            }
            Some(_) => current.push(ch),
            None if ch == '"' || ch == '\'' => quote = Some(ch),
            None => {}
        }
    }

    names
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use tree_sitter::Parser;

    use super::{extract_python, PY_LANGUAGE};
    use crate::parse::types::SymbolKind;

    #[test]
    fn python_language_parses_source() {
        let mut parser = Parser::new();
        parser
            .set_language(&PY_LANGUAGE)
            .expect("python language should load");

        let tree = parser
            .parse("def main():\n    return 1\n", None)
            .expect("python tree should parse");

        assert!(!tree.root_node().has_error());
    }

    #[test]
    fn extract_python_symbols_and_references() {
        let source = br#"
class Greeter:
    def hello(self):
        helper()

def helper():
    return 1

__all__ = ["helper"]
"#;

        let (symbols, references) =
            extract_python(Path::new("src/example.py"), Path::new("."), source);

        assert!(
            symbols
                .iter()
                .any(|symbol| symbol.name == "Greeter" && symbol.kind == SymbolKind::Class)
        );
        assert!(
            symbols
                .iter()
                .any(|symbol| symbol.name == "hello" && symbol.kind == SymbolKind::Method)
        );
        assert!(
            symbols
                .iter()
                .any(|symbol| symbol.name == "helper" && symbol.is_exported)
        );
        assert!(
            references
                .iter()
                .any(|reference| reference.target_name == "helper")
        );
    }
}
