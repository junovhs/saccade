// core/src/parser.rs

use std::collections::{HashMap, HashSet};
use std::str;
use tree_sitter::{Node, Parser, Query, QueryCursor};

const CHUNK_SEPARATOR: &str = "\n---⋯\n";

/// ─────────────────────────────────────────────────────────────────────
/// LANGUAGE-SPECIFIC QUERIES (separate per language to avoid drift)
/// ─────────────────────────────────────────────────────────────────────

// JavaScript / JSX / MJS / CJS
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

(arrow_function
  body: (_) @body) @def

; NOTE: We intentionally skip the object-literal `(pair value: (function ...))`
; rule here to reduce grammar fragility across versions.
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

(arrow_function
  body: (_) @body) @def
"#;

// Rust — use only field names for body to avoid node-type drift
const RUST_QUERY: &str = r#"
(line_comment) @capture
(block_comment) @capture
(use_declaration) @capture
(extern_crate_declaration) @capture
(struct_item) @capture
(enum_item) @capture
(union_item) @capture
(type_item) @capture
(mod_item) @capture
(macro_definition) @capture

(function_item
  body: (_) @body) @def

(trait_item
  body: (_) @body) @def

(impl_item
  body: (_) @body) @def
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
"#;

/// Skeletonizes a single file's content using Tree-sitter.
/// Returns a token-light "skeleton": defs with bodies stripped + salient captures.
pub fn skeletonize_file(content: &str, file_extension: &str) -> Option<String> {
    enum Lang<'a> {
        Js(&'a str),
        Ts(&'a str, bool), // Add bool flag for TSX vs TS
        Rs(&'a str),
        Py(&'a str),
    }

    let lang = match file_extension {
        // JavaScript-family (explicitly include mjs/cjs)
        "js" | "jsx" | "mjs" | "cjs" => {
            Lang::Js(JAVASCRIPT_QUERY)
        }
        // TypeScript-family: use correct grammar per extension
        "ts" => Lang::Ts(TYPESCRIPT_QUERY, false),  // Use TS grammar
        "tsx" => Lang::Ts(TYPESCRIPT_QUERY, true),  // Use TSX grammar
        "rs" => Lang::Rs(RUST_QUERY),
        "py" => Lang::Py(PYTHON_QUERY),
        _ => return None,
    };

    let mut parser = Parser::new();

    // Select language + query string
    let (language, query_str) = match lang {
        Lang::Js(q) => (tree_sitter_javascript::language(), q),
        Lang::Ts(q, is_tsx) => {
            let language = if is_tsx {
                tree_sitter_typescript::language_tsx()
            } else {
                tree_sitter_typescript::language_typescript()
            };
            (language, q)
        },
        Lang::Rs(q) => (tree_sitter_rust::language(), q),
        Lang::Py(q) => (tree_sitter_python::language(), q),
    };

    if let Err(e) = parser.set_language(&language) {
        eprintln!("WARN: set_language failed for .{}: {}", file_extension, e);
        return None;
    }

    let tree = match parser.parse(content, None) {
        Some(t) => t,
        None => return None,
    };

    let query = match Query::new(&language, query_str) {
        Ok(q) => q,
        Err(e) => {
            // Avoid noisy panics; print a compact one-liner and skip.
            eprintln!(
                "WARN: query compile failed for .{} at row {} col {}: {}",
                file_extension, e.row, e.column, e.message
            );
            return None;
        }
    };

    let mut cursor = QueryCursor::new();
    let matches = cursor.matches(&query, tree.root_node(), content.as_bytes());

    let mut results = Vec::new();
    let mut seen_ids: HashSet<usize> = HashSet::new();

    for m in matches {
        // Map capture name → node
        let mut caps: HashMap<&str, Node> = HashMap::new();
        for c in m.captures {
            let name = &query.capture_names()[c.index as usize];
            caps.insert(name, c.node);
        }

        // Prefer def/body pairs → slice signature text only
        if let (Some(def), Some(body)) = (caps.get("def"), caps.get("body")) {
            let def_id = def.id();
            if !seen_ids.contains(&def_id) {
                let start = def.start_byte();
                let end = body.start_byte();
                if end >= start {
                    if let Some(sig) = safe_slice(content, start, end) {
                        let sig_trim = sig.trim();
                        if !sig_trim.is_empty() {
                            results.push(sig_trim.to_string());
                        }
                    }
                }
                seen_ids.insert(def_id);
            }
            continue;
        }

        // Otherwise, simple capture (imports/comments/etc.)
        if let Some(cap) = caps.get("capture") {
            let id = cap.id();
            if !seen_ids.contains(&id) {
                if let Ok(text) = cap.utf8_text(content.as_bytes()) {
                    let t = text.trim();
                    if !t.is_empty() {
                        results.push(t.to_string());
                    }
                }
                seen_ids.insert(id);
            }
            continue;
        }

        // Def with no body (e.g., TS overloads)
        if let Some(def) = caps.get("def") {
            let id = def.id();
            if !seen_ids.contains(&id) {
                if let Ok(text) = def.utf8_text(content.as_bytes()) {
                    let t = text.trim();
                    if !t.is_empty() {
                        results.push(t.to_string());
                    }
                }
                seen_ids.insert(id);
            }
        }
    }

    if results.is_empty() {
        None
    } else {
        Some(results.join(CHUNK_SEPARATOR))
    }
}

/// Return a &str slice by byte offsets, guarding UTF-8 boundaries.
fn safe_slice<'a>(s: &'a str, start: usize, end: usize) -> Option<&'a str> {
    if start > end || end > s.len() {
        return None;
    }
    // Walk to valid char boundaries (cheap scans; files are small-ish here)
    let mut a = start;
    while a > 0 && !s.is_char_boundary(a) {
        a -= 1;
    }
    let mut b = end;
    while b < s.len() && !s.is_char_boundary(b) {
        b += 1;
    }
    s.get(a..b)
}