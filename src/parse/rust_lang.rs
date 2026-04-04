use std::path::Path;

use once_cell::sync::Lazy;
use tree_sitter::{Node, Parser, Query, QueryCursor, StreamingIterator};

use crate::parse::types::{build_fqn, Reference, Symbol, SymbolKind};

use super::{
    has_ancestor_kind, line_span, node_text, path_is_test, source_fqn_for_node, symbol_parts,
};

pub static RUST_LANGUAGE: Lazy<tree_sitter::Language> =
    Lazy::new(|| tree_sitter_rust::LANGUAGE.into());

const FUNCTION_QUERY: &str = r#"
    (function_item
      name: (identifier) @name) @definition.function
"#;

const STRUCT_QUERY: &str = r#"
    (struct_item
      name: (type_identifier) @name) @definition.struct
"#;

const ENUM_QUERY: &str = r#"
    (enum_item
      name: (type_identifier) @name) @definition.enum
"#;

const CALL_QUERY: &str = r#"
    [
      (call_expression
        function: (identifier) @name) @reference.call
      (call_expression
        function: (field_expression field: (field_identifier) @name)) @reference.call
      (call_expression
        function: (scoped_identifier name: (identifier) @name)) @reference.call
    ]
"#;

pub fn extract_rust(path: &Path, root: &Path, source: &[u8]) -> (Vec<Symbol>, Vec<Reference>) {
    let mut parser = Parser::new();
    if parser.set_language(&RUST_LANGUAGE).is_err() {
        return (Vec::new(), Vec::new());
    }

    let Some(tree) = parser.parse(source, None) else {
        return (Vec::new(), Vec::new());
    };
    let root_node = tree.root_node();

    let mut symbols = Vec::new();
    symbols.extend(collect_rust_functions(path, root, root_node, source));
    symbols.extend(collect_rust_structs(path, root, root_node, source));
    symbols.extend(collect_rust_enums(path, root, root_node, source));

    let references = collect_rust_references(path, root, root_node, source);
    (symbols, references)
}

fn collect_rust_functions(
    path: &Path,
    root: &Path,
    root_node: Node<'_>,
    source: &[u8],
) -> Vec<Symbol> {
    let query = Query::new(&RUST_LANGUAGE, FUNCTION_QUERY).expect("rust function query is valid");
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

        let kind = if has_impl_ancestor(function_node) {
            SymbolKind::Method
        } else {
            SymbolKind::Function
        };
        let parts = if matches!(kind, SymbolKind::Method) {
            if let Some(impl_type) = impl_type_name(function_node, source) {
                vec![impl_type, name.clone()]
            } else {
                symbol_parts(function_node, &name, source)
            }
        } else {
            symbol_parts(function_node, &name, source)
        };
        let part_refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
        let (line_start, line_end) = line_span(function_node);

        symbols.push(Symbol {
            fqn: build_fqn(path, root, &part_refs),
            name,
            kind,
            language: "rust".to_string(),
            file: path.to_path_buf(),
            line_start,
            line_end,
            is_exported: visibility_text(function_node, source).as_deref() == Some("pub"),
            is_test: path_is_test(path) || has_test_attribute(function_node, source),
        });
    }

    symbols
}

fn collect_rust_structs(
    path: &Path,
    root: &Path,
    root_node: Node<'_>,
    source: &[u8],
) -> Vec<Symbol> {
    collect_rust_type_symbols(
        path,
        root,
        root_node,
        source,
        STRUCT_QUERY,
        "definition.struct",
        SymbolKind::Struct,
    )
}

fn collect_rust_enums(path: &Path, root: &Path, root_node: Node<'_>, source: &[u8]) -> Vec<Symbol> {
    collect_rust_type_symbols(
        path,
        root,
        root_node,
        source,
        ENUM_QUERY,
        "definition.enum",
        SymbolKind::Enum,
    )
}

fn collect_rust_type_symbols(
    path: &Path,
    root: &Path,
    root_node: Node<'_>,
    source: &[u8],
    query_text: &str,
    definition_capture: &str,
    kind: SymbolKind,
) -> Vec<Symbol> {
    let query = Query::new(&RUST_LANGUAGE, query_text).expect("rust type query is valid");
    let capture_names = query.capture_names();
    let mut cursor = QueryCursor::new();
    let mut symbols = Vec::new();

    let mut matches = cursor.matches(&query, root_node, source);
    while let Some(query_match) = matches.next() {
        let name_node = query_match.captures.iter().find_map(|capture| {
            (capture_names[capture.index as usize] == "name").then_some(capture.node)
        });
        let definition_node = query_match.captures.iter().find_map(|capture| {
            (capture_names[capture.index as usize] == definition_capture).then_some(capture.node)
        });

        let (Some(name_node), Some(definition_node)) = (name_node, definition_node) else {
            continue;
        };
        let Some(name) = node_text(name_node, source) else {
            continue;
        };

        let parts = symbol_parts(definition_node, &name, source);
        let part_refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
        let (line_start, line_end) = line_span(definition_node);

        symbols.push(Symbol {
            fqn: build_fqn(path, root, &part_refs),
            name,
            kind: kind.clone(),
            language: "rust".to_string(),
            file: path.to_path_buf(),
            line_start,
            line_end,
            is_exported: visibility_text(definition_node, source).as_deref() == Some("pub"),
            is_test: path_is_test(path) || has_test_attribute(definition_node, source),
        });
    }

    symbols
}

fn collect_rust_references(
    path: &Path,
    root: &Path,
    root_node: Node<'_>,
    source: &[u8],
) -> Vec<Reference> {
    let query = Query::new(&RUST_LANGUAGE, CALL_QUERY).expect("rust call query is valid");
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

fn visibility_text(node: Node<'_>, source: &[u8]) -> Option<String> {
    let mut cursor = node.walk();
    let result = node.children(&mut cursor).find_map(|child| {
        (child.kind() == "visibility_modifier")
            .then(|| node_text(child, source))
            .flatten()
    });
    result
}

fn has_impl_ancestor(node: Node<'_>) -> bool {
    has_ancestor_kind(node, "impl_item")
}

fn impl_type_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    let mut current = node.parent();
    while let Some(parent) = current {
        if parent.kind() == "impl_item" {
            return parent
                .child_by_field_name("type")
                .and_then(|type_node| node_text(type_node, source));
        }
        current = parent.parent();
    }

    None
}

fn has_test_attribute(node: Node<'_>, source: &[u8]) -> bool {
    let mut current = Some(node);

    while let Some(item) = current {
        let mut previous = item.prev_sibling();
        while let Some(attribute) = previous {
            if attribute.kind() != "attribute_item" {
                break;
            }

            if let Some(text) = node_text(attribute, source) {
                if text.contains("cfg(test)")
                    || text.contains("#[test]")
                    || text.contains("::test]")
                {
                    return true;
                }
            }

            previous = attribute.prev_sibling();
        }

        current = item.parent();
    }

    false
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use tree_sitter::Parser;

    use super::{extract_rust, RUST_LANGUAGE};
    use crate::parse::types::SymbolKind;

    #[test]
    fn rust_language_parses_source() {
        let mut parser = Parser::new();
        parser
            .set_language(&RUST_LANGUAGE)
            .expect("rust language should load");

        let tree = parser
            .parse("fn main() {}", None)
            .expect("rust tree should parse");

        assert!(!tree.root_node().has_error());
    }

    #[test]
    fn extract_rust_symbols_and_references() {
        let source = br#"
pub fn public() { helper(); }
fn helper() {}
pub(crate) fn crate_only() {}
pub struct Item {}
pub enum Kind { A }

#[test]
fn test_public() { public(); }
"#;

        let (symbols, references) =
            extract_rust(Path::new("src/example.rs"), Path::new("."), source);

        assert!(symbols
            .iter()
            .any(|symbol| symbol.name == "public" && symbol.is_exported));
        assert!(symbols
            .iter()
            .any(|symbol| symbol.name == "crate_only" && !symbol.is_exported));
        assert!(symbols
            .iter()
            .any(|symbol| symbol.name == "Item" && symbol.kind == SymbolKind::Struct));
        assert!(symbols
            .iter()
            .any(|symbol| symbol.name == "Kind" && symbol.kind == SymbolKind::Enum));
        assert!(symbols
            .iter()
            .any(|symbol| symbol.name == "test_public" && symbol.is_test));
        assert!(references
            .iter()
            .any(|reference| reference.target_name == "helper"));
    }
}
