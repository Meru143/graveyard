use std::collections::HashSet;
use std::path::Path;

use once_cell::sync::Lazy;
use tree_sitter::{Node, Parser, Query, QueryCursor, StreamingIterator};

use crate::parse::types::{build_fqn, Symbol, SymbolKind};

use super::javascript::{extract_script, has_export_ancestor};
use super::{line_span, node_text, path_is_test, symbol_parts};

pub static TS_LANGUAGE: Lazy<tree_sitter::Language> =
    Lazy::new(|| tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into());
pub static TSX_LANGUAGE: Lazy<tree_sitter::Language> =
    Lazy::new(|| tree_sitter_typescript::LANGUAGE_TSX.into());

const INTERFACE_QUERY: &str = r#"
    (interface_declaration
      name: (type_identifier) @name) @definition.interface
"#;

const TYPE_ALIAS_QUERY: &str = r#"
    (type_alias_declaration
      name: (type_identifier) @name) @definition.type_alias
"#;

const EXPORT_QUERY: &str = r#"
    (export_statement
      (export_clause
        (export_specifier
          name: [
            (identifier)
            (type_identifier)
          ] @name)))
"#;

pub fn extract_typescript(
    path: &Path,
    root: &Path,
    source: &[u8],
    is_tsx: bool,
) -> (Vec<Symbol>, Vec<crate::parse::types::Reference>) {
    let language = if is_tsx { &TSX_LANGUAGE } else { &TS_LANGUAGE };
    let (mut symbols, references) = extract_script(path, root, source, language, "typescript");

    let mut parser = Parser::new();
    if parser.set_language(language).is_err() {
        return (symbols, references);
    }

    let Some(tree) = parser.parse(source, None) else {
        return (symbols, references);
    };
    let root_node = tree.root_node();
    let named_exports = collect_named_exports(root_node, source, language);

    symbols.extend(collect_interface_symbols(
        path,
        root,
        root_node,
        source,
        language,
        &named_exports,
    ));
    symbols.extend(collect_type_alias_symbols(
        path,
        root,
        root_node,
        source,
        language,
        &named_exports,
    ));

    if is_declaration_file(path) {
        for symbol in &mut symbols {
            symbol.is_exported = true;
        }
    }

    (symbols, references)
}

fn collect_interface_symbols(
    path: &Path,
    root: &Path,
    root_node: Node<'_>,
    source: &[u8],
    language: &tree_sitter::Language,
    named_exports: &HashSet<String>,
) -> Vec<Symbol> {
    let query = Query::new(language, INTERFACE_QUERY).expect("typescript interface query is valid");
    let capture_names = query.capture_names();
    let mut cursor = QueryCursor::new();
    let mut symbols = Vec::new();

    let mut matches = cursor.matches(&query, root_node, source);
    while let Some(query_match) = matches.next() {
        let name_node = query_match.captures.iter().find_map(|capture| {
            (capture_names[capture.index as usize] == "name").then_some(capture.node)
        });
        let definition_node = query_match.captures.iter().find_map(|capture| {
            (capture_names[capture.index as usize] == "definition.interface")
                .then_some(capture.node)
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
            name: name.clone(),
            kind: SymbolKind::Interface,
            language: "typescript".to_string(),
            file: path.to_path_buf(),
            line_start,
            line_end,
            is_exported: has_export_ancestor(definition_node) || named_exports.contains(&name),
            is_test: path_is_test(path),
        });
    }

    symbols
}

fn collect_type_alias_symbols(
    path: &Path,
    root: &Path,
    root_node: Node<'_>,
    source: &[u8],
    language: &tree_sitter::Language,
    named_exports: &HashSet<String>,
) -> Vec<Symbol> {
    let query = Query::new(language, TYPE_ALIAS_QUERY).expect("typescript type query is valid");
    let capture_names = query.capture_names();
    let mut cursor = QueryCursor::new();
    let mut symbols = Vec::new();

    let mut matches = cursor.matches(&query, root_node, source);
    while let Some(query_match) = matches.next() {
        let name_node = query_match.captures.iter().find_map(|capture| {
            (capture_names[capture.index as usize] == "name").then_some(capture.node)
        });
        let definition_node = query_match.captures.iter().find_map(|capture| {
            (capture_names[capture.index as usize] == "definition.type_alias")
                .then_some(capture.node)
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
            name: name.clone(),
            kind: SymbolKind::TypeAlias,
            language: "typescript".to_string(),
            file: path.to_path_buf(),
            line_start,
            line_end,
            is_exported: has_export_ancestor(definition_node) || named_exports.contains(&name),
            is_test: path_is_test(path),
        });
    }

    symbols
}

fn collect_named_exports(
    root_node: Node<'_>,
    source: &[u8],
    language: &tree_sitter::Language,
) -> HashSet<String> {
    let query = Query::new(language, EXPORT_QUERY).expect("typescript export query is valid");
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

fn is_declaration_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".d.ts"))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use tree_sitter::Parser;

    use super::{extract_typescript, TSX_LANGUAGE, TS_LANGUAGE};
    use crate::parse::types::SymbolKind;

    #[test]
    fn typescript_language_parses_source() {
        let mut parser = Parser::new();
        parser
            .set_language(&TS_LANGUAGE)
            .expect("typescript language should load");

        let tree = parser
            .parse("function main(): number { return 1; }", None)
            .expect("typescript tree should parse");

        assert!(!tree.root_node().has_error());
    }

    #[test]
    fn tsx_language_parses_source() {
        let mut parser = Parser::new();
        parser
            .set_language(&TSX_LANGUAGE)
            .expect("tsx language should load");

        let tree = parser
            .parse("const view = <div>Hello</div>;", None)
            .expect("tsx tree should parse");

        assert!(!tree.root_node().has_error());
    }

    #[test]
    fn extract_typescript_symbols() {
        let source = br#"
export interface User { id: string }
export type Id = string;
const helper = (): number => 1;
"#;

        let (symbols, _) =
            extract_typescript(Path::new("src/example.ts"), Path::new("."), source, false);

        assert!(
            symbols
                .iter()
                .any(|symbol| symbol.name == "User" && symbol.kind == SymbolKind::Interface)
        );
        assert!(symbols.iter().any(|symbol| {
            symbol.name == "Id"
                && symbol.kind == SymbolKind::TypeAlias
                && symbol.is_exported
        }));
        assert!(
            symbols
                .iter()
                .any(|symbol| symbol.name == "helper" && symbol.kind == SymbolKind::Function)
        );
    }
}
