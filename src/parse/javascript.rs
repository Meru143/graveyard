use std::collections::HashSet;
use std::path::Path;

use once_cell::sync::Lazy;
use tree_sitter::{Node, Parser, Query, QueryCursor, StreamingIterator};

use crate::parse::types::{build_fqn, Reference, Symbol, SymbolKind};

use super::{line_span, node_text, path_is_test, source_fqn_for_node, symbol_parts};

pub static JS_LANGUAGE: Lazy<tree_sitter::Language> =
    Lazy::new(|| tree_sitter_javascript::LANGUAGE.into());

const FUNCTION_QUERY: &str = r#"
    [
      (function_declaration
        name: (identifier) @name) @definition.function
      (generator_function_declaration
        name: (identifier) @name) @definition.function
      (lexical_declaration
        (variable_declarator
          name: (identifier) @name
          value: [(arrow_function) (function_expression)]) @definition.function)
      (variable_declaration
        (variable_declarator
          name: (identifier) @name
          value: [(arrow_function) (function_expression)]) @definition.function)
      (assignment_expression
        left: [
          (identifier) @name
          (member_expression property: (property_identifier) @name)
        ]
        right: [(arrow_function) (function_expression)]) @definition.function
    ]
"#;

const CLASS_QUERY: &str = r#"
    [
      (class
        name: (_) @name)
      (class_declaration
        name: (_) @name)
    ] @definition.class
"#;

const EXPORT_QUERY: &str = r#"
    (export_statement
      (export_clause
        (export_specifier
          name: (identifier) @name)))
"#;

const CALL_QUERY: &str = r#"
    [
      (call_expression
        function: (identifier) @name) @reference.call
      (call_expression
        function: (member_expression
          property: (property_identifier) @name)
        arguments: (_) @reference.call)
    ]
"#;

const EXPORT_ALL_QUERY: &str = r#"
    (export_statement) @export
"#;

pub fn extract_javascript(
    path: &Path,
    root: &Path,
    source: &[u8],
) -> (Vec<Symbol>, Vec<Reference>) {
    extract_script(path, root, source, &JS_LANGUAGE, "javascript")
}

pub(crate) fn extract_script(
    path: &Path,
    root: &Path,
    source: &[u8],
    language: &tree_sitter::Language,
    language_name: &str,
) -> (Vec<Symbol>, Vec<Reference>) {
    let mut parser = Parser::new();
    if parser.set_language(language).is_err() {
        return (Vec::new(), Vec::new());
    }

    let Some(tree) = parser.parse(source, None) else {
        return (Vec::new(), Vec::new());
    };
    let root_node = tree.root_node();
    let named_exports = collect_named_exports(language, root_node, source);

    let mut symbols = collect_function_symbols(
        path,
        root,
        root_node,
        source,
        language,
        language_name,
        &named_exports,
    );
    symbols.extend(collect_class_symbols(
        path,
        root,
        root_node,
        source,
        language,
        language_name,
        &named_exports,
    ));

    let references = collect_references(path, root, root_node, source, language);
    (symbols, references)
}

pub(crate) fn has_export_ancestor(node: Node<'_>) -> bool {
    let mut current = node.parent();
    while let Some(parent) = current {
        if parent.kind() == "export_statement" {
            return true;
        }
        current = parent.parent();
    }

    false
}

pub(crate) fn collect_named_exports(
    language: &tree_sitter::Language,
    root_node: Node<'_>,
    source: &[u8],
) -> HashSet<String> {
    let query = Query::new(language, EXPORT_QUERY).expect("export query is valid");
    let capture_names = query.capture_names();
    let mut cursor = QueryCursor::new();
    let mut exports = HashSet::new();

    let mut matches = cursor.matches(&query, root_node, source);
    while let Some(query_match) = matches.next() {
        for capture in query_match.captures {
            if capture_names[capture.index as usize] != "name" {
                continue;
            }

            if let Some(name) = node_text(capture.node, source) {
                exports.insert(name);
            }
        }
    }

    exports
}

fn collect_function_symbols(
    path: &Path,
    root: &Path,
    root_node: Node<'_>,
    source: &[u8],
    language: &tree_sitter::Language,
    language_name: &str,
    named_exports: &HashSet<String>,
) -> Vec<Symbol> {
    let query = Query::new(language, FUNCTION_QUERY).expect("function query is valid");
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
            language: language_name.to_string(),
            file: path.to_path_buf(),
            line_start,
            line_end,
            is_exported: has_export_ancestor(function_node) || named_exports.contains(&name),
            is_test: path_is_test(path),
        });
    }

    symbols
}

fn collect_class_symbols(
    path: &Path,
    root: &Path,
    root_node: Node<'_>,
    source: &[u8],
    language: &tree_sitter::Language,
    language_name: &str,
    named_exports: &HashSet<String>,
) -> Vec<Symbol> {
    let query = Query::new(language, CLASS_QUERY).expect("class query is valid");
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
            name: name.clone(),
            kind: SymbolKind::Class,
            language: language_name.to_string(),
            file: path.to_path_buf(),
            line_start,
            line_end,
            is_exported: has_export_ancestor(class_node) || named_exports.contains(&name),
            is_test: path_is_test(path),
        });
    }

    symbols
}

fn collect_references(
    path: &Path,
    root: &Path,
    root_node: Node<'_>,
    source: &[u8],
    language: &tree_sitter::Language,
) -> Vec<Reference> {
    let query = Query::new(language, CALL_QUERY).expect("call query is valid");
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

    references.extend(collect_export_star_references(path, root, root_node, source, language));
    references
}

fn collect_export_star_references(
    path: &Path,
    root: &Path,
    root_node: Node<'_>,
    source: &[u8],
    language: &tree_sitter::Language,
) -> Vec<Reference> {
    let query = Query::new(language, EXPORT_ALL_QUERY).expect("export-all query is valid");
    let capture_names = query.capture_names();
    let mut cursor = QueryCursor::new();
    let mut references = Vec::new();

    let mut matches = cursor.matches(&query, root_node, source);
    while let Some(query_match) = matches.next() {
        let export_node = query_match.captures.iter().find_map(|capture| {
            (capture_names[capture.index as usize] == "export").then_some(capture.node)
        });
        let Some(export_node) = export_node else {
            continue;
        };

        let Some(export_text) = node_text(export_node, source) else {
            continue;
        };
        if !export_text.starts_with("export *") {
            continue;
        }

        let Some(module_path) = extract_quoted_value(&export_text) else {
            continue;
        };

        references.push(Reference {
            source_fqn: build_fqn(path, root, &[]),
            target_name: module_path,
            file: path.to_path_buf(),
            line: line_span(export_node).0,
        });
    }

    references
}

fn extract_quoted_value(text: &str) -> Option<String> {
    let mut quote = None;
    let mut value = String::new();

    for ch in text.chars() {
        match quote {
            Some(active) if ch == active => return Some(value),
            Some(_) => value.push(ch),
            None if ch == '"' || ch == '\'' => quote = Some(ch),
            None => {}
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use tree_sitter::Parser;

    use super::{extract_javascript, JS_LANGUAGE};
    use crate::parse::types::SymbolKind;

    #[test]
    fn javascript_language_parses_source() {
        let mut parser = Parser::new();
        parser
            .set_language(&JS_LANGUAGE)
            .expect("javascript language should load");

        let tree = parser
            .parse("function main() { return 1; }", None)
            .expect("javascript tree should parse");

        assert!(!tree.root_node().has_error());
    }

    #[test]
    fn extract_javascript_symbols_and_references() {
        let source = br#"
export function main() { helper(); }
const helper = () => 1;
class Box {}
"#;

        let (symbols, references) =
            extract_javascript(Path::new("src/example.js"), Path::new("."), source);

        assert!(
            symbols
                .iter()
                .any(|symbol| symbol.name == "main" && symbol.is_exported)
        );
        assert!(
            symbols
                .iter()
                .any(|symbol| symbol.name == "helper" && symbol.kind == SymbolKind::Function)
        );
        assert!(
            symbols
                .iter()
                .any(|symbol| symbol.name == "Box" && symbol.kind == SymbolKind::Class)
        );
        assert!(
            references
                .iter()
                .any(|reference| reference.target_name == "helper")
        );
    }

    #[test]
    fn extract_javascript_records_export_star_reexports() {
        let source = br#"export * from "./utils";"#;

        let (_, references) =
            extract_javascript(Path::new("src/example.js"), Path::new("."), source);

        assert!(
            references
                .iter()
                .any(|reference| reference.target_name == "./utils")
        );
    }
}
