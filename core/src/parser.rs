// In saccade/core/src/parser.rs

use tree_sitter::{Parser, Query, QueryCursor};

// These queries are the "secret sauce" from Repomix, translated into Rust constants.
// They are designed to capture the high-level structural nodes of the code.
const TYPESCRIPT_QUERY: &str = r#"
(import_statement) @capture
(comment) @capture
(function_signature) @capture
(method_signature) @capture
(abstract_method_signature) @capture
(abstract_class_declaration) @capture
(module) @capture
(interface_declaration) @capture
(function_declaration) @capture
(method_definition) @capture
(class_declaration) @capture
(type_alias_declaration) @capture
(enum_declaration) @capture
(lexical_declaration (variable_declarator value: (arrow_function))) @capture
(variable_declaration (variable_declarator value: (arrow_function))) @capture
(export_statement) @capture
"#;

const RUST_QUERY: &str = r#"
(line_comment) @capture
(block_comment) @capture
(use_declaration) @capture
(extern_crate_declaration) @capture
(struct_item) @capture
(enum_item) @capture
(union_item) @capture
(type_item) @capture
(function_item) @capture
(trait_item) @capture
(mod_item) @capture
(macro_definition) @capture
(impl_item) @capture
"#;

// A basic query for Python to demonstrate extensibility.
const PYTHON_QUERY: &str = r#"
(comment) @capture
(import_statement) @capture
(import_from_statement) @capture
(function_definition) @capture
(class_definition) @capture
"#;

// The delimiter observed in the ground-truth XML file from Repomix.
const CHUNK_SEPARATOR: &str = "\n⋮----\n";

/// Skeletonizes a single file's content using Tree-sitter.
///
/// This function parses the code, runs a language-specific query to find
/// all high-level structural nodes (imports, classes, functions, etc.),
/// and joins their text content together with a special separator.
pub fn skeletonize_file(content: &str, file_extension: &str) -> Option<String> {
    let (language, query_str) = match file_extension {
        "ts" | "tsx" | "js" | "jsx" => (tree_sitter_typescript::language_tsx(), TYPESCRIPT_QUERY),
        "rs" => (tree_sitter_rust::language(), RUST_QUERY),
        "py" => (tree_sitter_python::language(), PYTHON_QUERY),
        _ => return None, // Unsupported language
    };

    let mut parser = Parser::new();
    parser
        .set_language(&language) // FIXED: Added '&'
        .expect("Error loading grammar");

    let tree = parser.parse(content, None)?;
    let query = Query::new(&language, query_str).expect("Failed to create query"); // FIXED: Added '&'
    let mut cursor = QueryCursor::new();

    let matches = cursor.matches(&query, tree.root_node(), content.as_bytes());

    let mut results = Vec::new();
    // Using a byte offset to avoid re-capturing nested nodes.
    let mut last_capture_end = 0;

    for m in matches {
        for capture in m.captures {
            let node = capture.node;
            // Ensure we only process a node once if it's part of nested matches.
            if node.start_byte() >= last_capture_end {
                if let Ok(text) = node.utf8_text(content.as_bytes()) {
                    results.push(text.trim());
                    last_capture_end = node.end_byte();
                }
            }
        }
    }

    if results.is_empty() {
        None
    } else {
        Some(results.join(CHUNK_SEPARATOR))
    }
}