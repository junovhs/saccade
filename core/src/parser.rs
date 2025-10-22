use std::collections::{HashMap, HashSet};
use std::str;

use tree_sitter::{Node, Parser, Query, QueryCursor};

const CHUNK_SEPARATOR: &str = "\n---⋯\n";

/// ─────────────────────────────────────────────────────────────────────────
/// LANGUAGE-SPECIFIC QUERIES
/// Keep node-type names aligned with the grammar we load.
/// ─────────────────────────────────────────────────────────────────────────

// JavaScript / JSX
const JAVASCRIPT_QUERY: &str = r#"
(import_statement) @capture
(export_statement) @capture
(comment) @capture

(function_declaration
  body: (statement_block) @body) @def

(method_definition
  body: (statement_block) @body) @def

(class_declaration
  body: (class_body) @body) @def

(lexical_declaration
  (variable_declarator
    value: (arrow_function
      body: (_) @body
    )
  )
) @def

(pair
  key: (_)
  value: (function
    body: (statement_block) @body
  )
) @def

(arrow_function
  body: (_) @body) @def
"#;

// TypeScript / TSX
const TYPESCRIPT_QUERY: &str = r#"
(import_statement) @capture
(export_statement) @capture
(comment) @capture
(interface_declaration) @capture
(type_alias_declaration) @capture
(enum_declaration) @capture

(function_declaration
  body: (statement_block) @body) @def

(method_definition
  body: (statement_block) @body) @def

(class_declaration
  body: (class_body) @body) @def

(lexical_declaration
  (variable_declarator
    value: (arrow_function
      body: (_) @body
    )
  )
) @def

(pair
  key: (_)
  value: (function_expression
    body: (statement_block) @body
  )
) @def

(arrow_function
  body: (_) @body) @def

(function_signature) @def
(method_signature) @def
"#;

// Rust
const RUST_QUERY: &str = r#"
(line_comment) @capture
(block_comment) @capture
(attribute_item) @capture
(use_declaration) @capture
(extern_crate_declaration) @capture
(struct_item) @capture
(enum_item) @capture
(union_item) @capture
(type_item) @capture
(mod_item) @capture
(macro_definition) @capture

(function_item
  body: (block_expression) @body) @def

(trait_item
  body: (declaration_list) @body) @def

(impl_item
  body: (declaration_list) @body) @def
"#;

// Python
const PYTHON_QUERY: &str = r#"
(comment) @capture
(import_statement) @capture
(import_from_statement) @capture

(function_definition
  body: (block) @body) @def

(class_definition
  body: (block) @body) @def

(decorated_definition
  (function_definition
    body: (block) @body) @def)

(decorated_definition
  (class_definition
    body: (block) @body) @def)
"#;

/// Skeletonizes a single file's content using Tree-sitter.
pub fn skeletonize_file(content: &str, file_extension: &str) -> Option<String> {
    let (language, query_src) = match file_extension {
        "ts" | "tsx" => (tree_sitter_typescript::language_tsx(), TYPESCRIPT_QUERY),
        "js" | "jsx" | "mjs" | "cjs" => (tree_sitter_javascript::language(), JAVASCRIPT_QUERY),
        "rs" => (tree_sitter_rust::language(), RUST_QUERY),
        "py" => (tree_sitter_python::language(), PYTHON_QUERY),
        _ => return None,
    };

    let mut parser = Parser::new();
    if let Err(e) = parser.set_language(&language) {
        eprintln!("WARN: failed to load language for .{}: {}", file_extension, e);
        return None;
    }

    let tree = match parser.parse(content, None) {
        Some(t) => t,
        None => {
            eprintln!("WARN: parse returned None for .{}", file_extension);
            return None;
        }
    };

    let query = match Query::new(&language, query_src) {
        Ok(q) => q,
        Err(e) => {
            eprintln!(
                "WARN: query failed for .{}: {:?} (row {}, col {})",
                file_extension, e, e.row, e.column
            );
            return None;
        }
    };

    let mut cursor = QueryCursor::new();
    let matches = cursor.matches(&query, tree.root_node(), content.as_bytes());

    let mut results: Vec<(usize, String)> = Vec::new();
    let mut processed: HashSet<usize> = HashSet::new();

    for m in matches {
        let mut by_name: HashMap<&str, Node> = HashMap::new();
        for cap in m.captures {
            let name = &query.capture_names()[cap.index as usize];
            by_name.insert(name, cap.node);
        }

        // def/body pair → slice signature
        if let (Some(def), Some(body)) = (by_name.get("def"), by_name.get("body")) {
            let def_id = def.id();
            if processed.insert(def_id) {
                let start = def.start_byte();
                let end = body.start_byte();
                if end >= start && end <= content.len() {
                    if let Some(sig) = safe_slice(content, start, end) {
                        let text = sig.trim().to_string();
                        if !text.is_empty() {
                            results.push((start, text));
                        }
                    }
                }
            }
            continue;
        }

        // simple captures
        if let Some(node) = by_name.get("capture") {
            let id = node.id();
            if processed.insert(id) {
                if let Ok(text) = node.utf8_text(content.as_bytes()) {
                    let t = text.trim();
                    if !t.is_empty() {
                        results.push((node.start_byte(), t.to_string()));
                    }
                }
            }
        }

        // def without body
        if let Some(def) = by_name.get("def") {
            let id = def.id();
            if processed.insert(id) {
                if let Ok(text) = def.utf8_text(content.as_bytes()) {
                    let t = text.trim();
                    if !t.is_empty() {
                        results.push((def.start_byte(), t.to_string()));
                    }
                }
            }
        }
    }

    if results.is_empty() {
        return None;
    }

    results.sort_by_key(|(pos, _)| *pos);
    let chunks: Vec<String> = results.into_iter().map(|(_, s)| s).collect();
    Some(chunks.join(CHUNK_SEPARATOR))
}

/// Return a &str slice by byte offsets, guarding UTF-8 boundaries.
fn safe_slice<'a>(s: &'a str, start: usize, end: usize) -> Option<&'a str> {
    if start > end || end > s.len() {
        return None;
    }
    if !s.is_char_boundary(start) || !s.is_char_boundary(end) {
        let bytes = &s.as_bytes()[start..end];
        return str::from_utf8(bytes).ok();
    }
    Some(&s[start..end])
}
