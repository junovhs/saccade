// saccade/core/src/detection.rs

use crate::error::Result;
use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::path::Path;
use tree_sitter::{Parser, Query};

/// Represents the detected, high-confidence build systems in a repository.
/// This acts as the "Environmental Signal/Cue" for the Dynamic Configuration Architecture.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum BuildSystemType {
    Rust,
    Node,
    Python,
    Go,
    CMake,
    Conan,
}

impl fmt::Display for BuildSystemType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// The Layer 2 detector, analogous to "Alternative Splicing Factors".
/// It analyzes file content to confirm build system identity.
pub struct Detector;

// CORRECTED: This query is compatible with tree-sitter-cmake v0.5.0
// It simply finds all identifiers, which is a fundamental and stable node type.
const CMAKE_AST_QUERY: &str = r#"
(identifier) @cmd
"#;

// Keywords that, when found as commands, confirm a file is a CMake manifest.
const CMAKE_CONFIRMATION_KEYWORDS: &[&str] = &[
    "add_executable",
    "target_link_libraries",
    "project",
    "cmake_minimum_required",
    "find_package",
];

impl Detector {
    pub fn new() -> Self {
        Self
    }

    /// The main detection entry point. It orchestrates the identification of all
    /// supported build systems within the provided file list.
    pub fn detect_build_systems(&self, files: &[std::path::PathBuf]) -> Result<Vec<BuildSystemType>> {
        let mut detected = HashSet::new();

        for file in files {
            if self.is_cargo(file) {
                detected.insert(BuildSystemType::Rust);
            }
            if self.is_npm(file) {
                detected.insert(BuildSystemType::Node);
            }
            if self.is_python(file) {
                detected.insert(BuildSystemType::Python);
            }
            if self.is_go(file) {
                detected.insert(BuildSystemType::Go);
            }
            if self.is_cmake_validated(file)? {
                detected.insert(BuildSystemType::CMake);
            }
            if self.is_conan(file) {
                detected.insert(BuildSystemType::Conan);
            }
        }

        Ok(detected.into_iter().collect())
    }

    // --- Simple, filename-based detectors for unambiguous ecosystems ---
    fn is_cargo(&self, path: &Path) -> bool {
        path.ends_with("Cargo.toml")
    }
    fn is_npm(&self, path: &Path) -> bool {
        path.ends_with("package.json")
    }
    fn is_python(&self, path: &Path) -> bool {
        matches!(
            path.file_name().and_then(|n| n.to_str()),
            Some("requirements.txt" | "pyproject.toml" | "Pipfile")
        )
    }
    fn is_go(&self, path: &Path) -> bool {
        path.ends_with("go.mod")
    }

    /// High-confidence structural validation for CMake files using Tree-sitter.
    fn is_cmake_validated(&self, path: &Path) -> Result<bool> {
        // A fast-path to avoid reading every file. Only check likely candidates.
        let path_str = path.to_string_lossy();
        if !path_str.contains("CMakeLists.txt") && !path_str.ends_with(".cmake") {
            return Ok(false);
        }

        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Ok(false),
        };

        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_cmake::language()).map_err(|e| crate::error::SaccadeError::Other(e.to_string()))?;
        let tree = match parser.parse(&content, None) {
            Some(t) => t,
            None => return Ok(false),
        };

        let query = Query::new(&tree_sitter_cmake::language(), CMAKE_AST_QUERY).map_err(|e| crate::error::SaccadeError::Other(e.to_string()))?;
        let mut cursor = tree_sitter::QueryCursor::new();
        let matches = cursor.matches(&query, tree.root_node(), content.as_bytes());

        for m in matches {
            for capture in m.captures {
                if let Ok(cmd) = capture.node.utf8_text(content.as_bytes()) {
                    if CMAKE_CONFIRMATION_KEYWORDS.contains(&cmd) {
                        return Ok(true);
                    }
                }
            }
        }

        Ok(false)
    }

    /// Simple, filename-based detector for Conan.
    fn is_conan(&self, path: &Path) -> bool {
        matches!(
            path.file_name().and_then(|n| n.to_str()),
            Some("conanfile.txt" | "conanfile.py")
        )
    }
}