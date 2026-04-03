use std::path::Path;

use once_cell::sync::Lazy;
use tree_sitter::{Node, Parser, Query, QueryCursor, StreamingIterator};

use crate::parse::types::{build_fqn, Reference, Symbol, SymbolKind};

use super::{line_span, node_text, path_is_test, source_fqn_for_node, symbol_parts};

pub static GO_LANGUAGE: Lazy<tree_sitter::Language> =
    Lazy::new(|| tree_sitter_go::LANGUAGE.into());

const FUNCTION_QUERY: &str = r#"
    (function_declaration
      name: (identifier) @name) @definition.function
"#;

const METHOD_QUERY: &str = r#"
    (method_declaration
      name: (field_identifier) @name) @definition.method
"#;

const TYPE_QUERY: &str = r#"
    (type_spec
      name: (type_identifier) @name
      type: [
        (struct_type)
        (interface_type)
      ] @type) @definition.type
"#;

const CALL_QUERY: &str = r#"
    (call_expression
      function: [
        (identifier) @name
        (parenthesized_expression (identifier) @name)
        (selector_expression field: (field_identifier) @name)
        (parenthesized_expression (selector_expression field: (field_identifier) @name))
      ]) @reference.call
"#;

pub fn extract_go(path: &Path, root: &Path, source: &[u8]) -> (Vec<Symbol>, Vec<Reference>) {
    let mut parser = Parser::new();
    if parser.set_language(&GO_LANGUAGE).is_err() {
        return (Vec::new(), Vec::new());
    }

    let Some(tree) = parser.parse(source, None) else {
        return (Vec::new(), Vec::new());
    };
    let root_node = tree.root_node();

    let mut symbols = Vec::new();
    symbols.extend(collect_go_functions(path, root, root_node, source));
    symbols.extend(collect_go_methods(path, root, root_node, source));
    symbols.extend(collect_go_types(path, root, root_node, source));

    let references = collect_go_references(path, root, root_node, source);
    (symbols, references)
}

fn collect_go_functions(
    path: &Path,
    root: &Path,
    root_node: Node<'_>,
    source: &[u8],
) -> Vec<Symbol> {
    let query = Query::new(&GO_LANGUAGE, FUNCTION_QUERY).expect("go function query is valid");
    let capture_names = query.capture_names();
    let mut cursor = QueryCursor::new();
    let mut symbols = Vec::new();

    let mut matches = cursor.matches(&query, root_node, source);
    while let Some(query_match) = matches.next() {
        let name_node = query_match.captures.iter().find_map(|capture| {
            (capture_names[capture.index as usize] == "name").then_some(capture.node)
        });
        let function_node = query_match.captures.iter().find_map(|capture| {
            (capture_names[capture.index as usize] == "definition.function").then_some(capture.node)
        });

        let (Some(name_node), Some(function_node)) = (name_node, function_node) else {
            continue;
        };
        let Some(name) = node_text(name_node, source) else {
            continue;
        };

        let parts = symbol_parts(function_node, &name, source);
        let part_refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
        let (line_start, line_end) = line_span(function_node);

        symbols.push(Symbol {
            fqn: build_fqn(path, root, &part_refs),
            name: name.clone(),
            kind: SymbolKind::Function,
            language: "go".to_string(),
            file: path.to_path_buf(),
            line_start,
            line_end,
            is_exported: is_go_exported(&name),
            is_test: path_is_test(path),
        });
    }

    symbols
}

fn collect_go_methods(
    path: &Path,
    root: &Path,
    root_node: Node<'_>,
    source: &[u8],
) -> Vec<Symbol> {
    let query = Query::new(&GO_LANGUAGE, METHOD_QUERY).expect("go method query is valid");
    let capture_names = query.capture_names();
    let mut cursor = QueryCursor::new();
    let mut symbols = Vec::new();

    let mut matches = cursor.matches(&query, root_node, source);
    while let Some(query_match) = matches.next() {
        let name_node = query_match.captures.iter().find_map(|capture| {
            (capture_names[capture.index as usize] == "name").then_some(capture.node)
        });
        let method_node = query_match.captures.iter().find_map(|capture| {
            (capture_names[capture.index as usize] == "definition.method").then_some(capture.node)
        });

        let (Some(name_node), Some(method_node)) = (name_node, method_node) else {
            continue;
        };
        let Some(name) = node_text(name_node, source) else {
            continue;
        };

        let receiver = method_node
            .child_by_field_name("receiver")
            .and_then(|node| receiver_type_name(node, source));
        let parts = if let Some(receiver) = receiver.as_deref() {
            vec![receiver.to_string(), name.clone()]
        } else {
            symbol_parts(method_node, &name, source)
        };
        let part_refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
        let (line_start, line_end) = line_span(method_node);

        symbols.push(Symbol {
            fqn: build_fqn(path, root, &part_refs),
            name: name.clone(),
            kind: SymbolKind::Method,
            language: "go".to_string(),
            file: path.to_path_buf(),
            line_start,
            line_end,
            is_exported: is_go_exported(&name),
            is_test: path_is_test(path),
        });
    }

    symbols
}

fn collect_go_types(
    path: &Path,
    root: &Path,
    root_node: Node<'_>,
    source: &[u8],
) -> Vec<Symbol> {
    let query = Query::new(&GO_LANGUAGE, TYPE_QUERY).expect("go type query is valid");
    let capture_names = query.capture_names();
    let mut cursor = QueryCursor::new();
    let mut symbols = Vec::new();

    let mut matches = cursor.matches(&query, root_node, source);
    while let Some(query_match) = matches.next() {
        let name_node = query_match.captures.iter().find_map(|capture| {
            (capture_names[capture.index as usize] == "name").then_some(capture.node)
        });
        let type_node = query_match.captures.iter().find_map(|capture| {
            (capture_names[capture.index as usize] == "type").then_some(capture.node)
        });
        let definition_node = query_match.captures.iter().find_map(|capture| {
            (capture_names[capture.index as usize] == "definition.type").then_some(capture.node)
        });

        let (Some(name_node), Some(type_node), Some(definition_node)) =
            (name_node, type_node, definition_node)
        else {
            continue;
        };
        let Some(name) = node_text(name_node, source) else {
            continue;
        };

        let parts = symbol_parts(definition_node, &name, source);
        let part_refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
        let (line_start, line_end) = line_span(definition_node);
        let kind = match type_node.kind() {
            "interface_type" => SymbolKind::Interface,
            _ => SymbolKind::Struct,
        };

        symbols.push(Symbol {
            fqn: build_fqn(path, root, &part_refs),
            name: name.clone(),
            kind,
            language: "go".to_string(),
            file: path.to_path_buf(),
            line_start,
            line_end,
            is_exported: is_go_exported(&name),
            is_test: path_is_test(path),
        });
    }

    symbols
}

fn collect_go_references(
    path: &Path,
    root: &Path,
    root_node: Node<'_>,
    source: &[u8],
) -> Vec<Reference> {
    let query = Query::new(&GO_LANGUAGE, CALL_QUERY).expect("go call query is valid");
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

fn receiver_type_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    let text = node_text(node, source)?;
    text.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .rfind(|part| !part.is_empty())
        .map(ToString::to_string)
}

fn is_go_exported(name: &str) -> bool {
    name.chars().next().is_some_and(|ch| ch.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use tree_sitter::Parser;

    use super::{extract_go, GO_LANGUAGE};
    use crate::parse::types::SymbolKind;

    #[test]
    fn go_language_parses_source() {
        let mut parser = Parser::new();
        parser
            .set_language(&GO_LANGUAGE)
            .expect("go language should load");

        let tree = parser
            .parse("package main\nfunc main() {}\n", None)
            .expect("go tree should parse");

        assert!(!tree.root_node().has_error());
    }

    #[test]
    fn extract_go_symbols_and_references() {
        let source = br#"
package main

func Public() { private() }
func private() {}
type Service struct {}
"#;

        let (symbols, references) = extract_go(Path::new("src/example.go"), Path::new("."), source);

        assert!(
            symbols
                .iter()
                .any(|symbol| symbol.name == "Public" && symbol.is_exported)
        );
        assert!(
            symbols
                .iter()
                .any(|symbol| symbol.name == "private" && !symbol.is_exported)
        );
        assert!(
            symbols
                .iter()
                .any(|symbol| symbol.name == "Service" && symbol.kind == SymbolKind::Struct)
        );
        assert!(
            references
                .iter()
                .any(|reference| reference.target_name == "private")
        );
    }
}
