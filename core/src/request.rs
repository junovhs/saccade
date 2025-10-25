// REQUEST_FILE Protocol Implementation with Glob Pattern Support
// File: core/src/request.rs
//
// Enables AI to request specific files or patterns:
// - Single file: path: src/main.rs
// - Glob pattern: pattern: "tests/**/*_test.rs"
// - Line ranges: range: lines 80-140
// - Symbol ranges: range: symbol: get_user

use glob::Pattern;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RequestError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("No files match pattern: {0}")]
    NoMatches(String),

    #[error("Invalid glob pattern: {0}")]
    InvalidPattern(String),

    #[error("Invalid line range: {0}")]
    InvalidLineRange(String),

    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, RequestError>;

/// REQUEST_FILE request structure
#[derive(Debug, Serialize, Deserialize)]
pub struct RequestFile {
    /// Target: either a single path or a glob pattern
    #[serde(flatten)]
    pub target: RequestTarget,

    /// Human-readable reason for the request
    pub reason: String,

    /// Optional range specification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<RequestRange>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestTarget {
    /// Single file path
    SinglePath { path: String },

    /// Glob pattern (supports *, **, ?, [abc], etc.)
    Pattern { pattern: String },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestRange {
    /// Line range: "80-140" or "80-" (to end)
    Lines { lines: String },

    /// Symbol name (function, class, etc.)
    Symbol { symbol: String },
}

/// Resolved request with actual file contents
#[derive(Debug)]
pub struct ResolvedRequest {
    pub files: Vec<FileContent>,
    pub reason: String,
}

#[derive(Debug)]
pub struct FileContent {
    pub path: PathBuf,
    pub content: String,
    pub total_lines: usize,
    pub range_info: Option<String>,
}

impl RequestFile {
    /// Resolve the request against available files.
    /// The base_dir is needed in test environments where files are in a TempDir.
    pub fn resolve(&self, available_files: &[PathBuf], base_dir: &Path) -> Result<ResolvedRequest> {
        // First, find matching files
        let matching_paths = self.find_matching_files(available_files)?;

        // Then, read and extract requested content
        let files = matching_paths
            .into_iter()
            .filter_map(|relative_path| {
                let absolute_path = base_dir.join(&relative_path);
                self.read_file_with_range(&absolute_path, &relative_path)
                    .ok()
            })
            .collect();

        Ok(ResolvedRequest {
            files,
            reason: self.reason.clone(),
        })
    }

    /// Find all files matching the target (path or pattern)
    fn find_matching_files(&self, available_files: &[PathBuf]) -> Result<Vec<PathBuf>> {
        match &self.target {
            RequestTarget::SinglePath { path } => {
                let path_buf = PathBuf::from(path);
                if available_files.contains(&path_buf) {
                    Ok(vec![path_buf])
                } else {
                    Err(RequestError::FileNotFound(path.clone()))
                }
            }
            RequestTarget::Pattern { pattern } => {
                let glob_pattern =
                    Pattern::new(pattern).map_err(|e| RequestError::InvalidPattern(e.to_string()))?;

                let matches: Vec<_> = available_files
                    .iter()
                    .filter(|p| {
                        // Normalize to forward slashes for consistent matching
                        let path_str = p.to_string_lossy().replace('\\', "/");
                        glob_pattern.matches(&path_str)
                    })
                    .cloned()
                    .collect();

                if matches.is_empty() {
                    Err(RequestError::NoMatches(pattern.clone()))
                } else {
                    Ok(matches)
                }
            }
        }
    }

    /// Read file and extract requested range
    fn read_file_with_range(
        &self,
        absolute_path: &Path,
        relative_path: &Path,
    ) -> Result<FileContent> {
        let full_content = fs::read_to_string(absolute_path)?;
        let total_lines = full_content.lines().count();

        let (content, range_info) = match &self.range {
            None => {
                // Return full file
                (full_content, None)
            }
            Some(RequestRange::Lines { lines }) => {
                let (extracted, info) = self.extract_line_range(&full_content, lines)?;
                (extracted, Some(info))
            }
            Some(RequestRange::Symbol { symbol }) => {
                let (extracted, info) = self.extract_symbol(&full_content, symbol)?;
                (extracted, Some(info))
            }
        };

        Ok(FileContent {
            path: relative_path.to_path_buf(),
            content,
            total_lines,
            range_info,
        })
    }

    /// Extract specific line range
    fn extract_line_range(&self, content: &str, range_spec: &str) -> Result<(String, String)> {
        let lines: Vec<&str> = content.lines().collect();
        let total = lines.len();

        // Parse range: "80-140", "80-", "80"
        let (start, end) = if let Some((start_str, end_str)) = range_spec.split_once('-') {
            let start = start_str
                .trim()
                .parse::<usize>()
                .map_err(|_| RequestError::InvalidLineRange(range_spec.to_string()))?;

            let end = if end_str.trim().is_empty() {
                total // "80-" means to end
            } else {
                end_str
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| RequestError::InvalidLineRange(range_spec.to_string()))?
            };

            (start, end)
        } else {
            // Single line: "80"
            let line = range_spec
                .trim()
                .parse::<usize>()
                .map_err(|_| RequestError::InvalidLineRange(range_spec.to_string()))?;
            (line, line)
        };

        // Validate bounds
        if start < 1 || start > total || end < start || end > total {
            return Err(RequestError::InvalidLineRange(format!(
                "{}  (file has {} lines)",
                range_spec, total
            )));
        }

        // Extract lines (convert to 0-indexed)
        let extracted = lines[(start - 1)..end].join("\n");
        let info = format!("lines {}-{} of {}", start, end, total);

        Ok((extracted, info))
    }

    /// Extract content around a symbol (function, class, etc.)
    fn extract_symbol(&self, content: &str, symbol: &str) -> Result<(String, String)> {
        // Simple symbol extraction: find lines containing the symbol
        // and include surrounding context

        let lines: Vec<&str> = content.lines().collect();
        let mut matching_lines = Vec::new();

        // Find all lines containing the symbol
        for (idx, line) in lines.iter().enumerate() {
            if line.contains(symbol) {
                matching_lines.push(idx);
            }
        }

        if matching_lines.is_empty() {
            return Err(RequestError::SymbolNotFound(symbol.to_string()));
        }

        // For simplicity, take first occurrence and surrounding context
        let target_line = matching_lines[0];
        let context = 5; // lines of context

        let start = target_line.saturating_sub(context);
        let end = (target_line + context + 1).min(lines.len());

        let extracted = lines[start..end].join("\n");
        let info = format!(
            "symbol '{}' at line {} (±{} lines context)",
            symbol,
            target_line + 1,
            context
        );

        Ok((extracted, info))
    }
}

impl ResolvedRequest {
    /// Format as markdown for display
    pub fn to_markdown(&self) -> String {
        let mut output = String::new();

        output.push_str("# REQUEST_FILE Results\n\n");
        output.push_str(&format!("**Reason:** {}\n\n", self.reason));
        output.push_str(&format!("**Files matched:** {}\n\n", self.files.len()));

        for file in &self.files {
            output.push_str("---\n\n");
            output.push_str(&format!("## {}\n\n", file.path.display()));

            if let Some(ref info) = file.range_info {
                output.push_str(&format!("*Showing: {}*\n\n", info));
            } else {
                output.push_str(&format!("*Full file ({} lines)*\n\n", file.total_lines));
            }

            output.push_str("```\n");
            output.push_str(&file.content);
            output.push_str("\n```\n\n");
        }

        output
    }
}

// ============================================================================
// EXAMPLES AND TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_files(dir: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();

        // Create test structure
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::create_dir_all(dir.join("tests")).unwrap();

        // src/main.rs
        let main = dir.join("src/main.rs");
        fs::write(&main, "fn main() {\n    println!(\"Hello\");\n}\n").unwrap();
        files.push(main.strip_prefix(dir).unwrap().to_path_buf());

        // src/lib.rs
        let lib = dir.join("src/lib.rs");
        fs::write(&lib, "pub fn helper() -> i32 {\n    42\n}\n").unwrap();
        files.push(lib.strip_prefix(dir).unwrap().to_path_buf());

        // tests/test_main.rs
        let test1 = dir.join("tests/test_main.rs");
        let test_content = r#"
#[test]
fn test_example() {
    assert_eq!(2 + 2, 4);
}

#[test]
fn test_helper() {
    assert_eq!(helper(), 42);
}
"#;
        fs::write(&test1, test_content).unwrap();
        files.push(test1.strip_prefix(dir).unwrap().to_path_buf());

        // tests/test_lib.rs
        let test2 = dir.join("tests/test_lib.rs");
        fs::write(&test2, "#[test]\nfn test_lib() {}\n").unwrap();
        files.push(test2.strip_prefix(dir).unwrap().to_path_buf());

        files
    }

    #[test]
    fn test_single_path_request() {
        let tmp = TempDir::new().unwrap();
        let files = create_test_files(tmp.path());

        let request = RequestFile {
            target: RequestTarget::SinglePath {
                path: "src/main.rs".to_string(),
            },
            reason: "Check main entry point".to_string(),
            range: None,
        };

        let resolved = request.resolve(&files, tmp.path()).unwrap();
        assert_eq!(resolved.files.len(), 1);
        assert!(resolved.files[0].content.contains("fn main"));
    }

    #[test]
    fn test_glob_pattern_all_tests() {
        let tmp = TempDir::new().unwrap();
        let files = create_test_files(tmp.path());

        let request = RequestFile {
            target: RequestTarget::Pattern {
                pattern: "tests/test_*.rs".to_string(),
            },
            reason: "Review all test files".to_string(),
            range: None,
        };

        let resolved = request.resolve(&files, tmp.path()).unwrap();
        assert_eq!(resolved.files.len(), 2);
    }

    #[test]
    fn test_glob_pattern_recursive() {
        let tmp = TempDir::new().unwrap();
        let files = create_test_files(tmp.path());

        let request = RequestFile {
            target: RequestTarget::Pattern {
                pattern: "**/*.rs".to_string(),
            },
            reason: "All Rust files".to_string(),
            range: None,
        };

        let resolved = request.resolve(&files, tmp.path()).unwrap();
        assert_eq!(resolved.files.len(), 4);
    }

    #[test]
    fn test_line_range_extraction() {
        let tmp = TempDir::new().unwrap();
        let files = create_test_files(tmp.path());

        let request = RequestFile {
            target: RequestTarget::SinglePath {
                path: "tests/test_main.rs".to_string(),
            },
            reason: "Check test_helper function".to_string(),
            range: Some(RequestRange::Lines {
                lines: "8-10".to_string(), // CORRECTED LINE
            }),
        };

        let resolved = request.resolve(&files, tmp.path()).unwrap();
        assert_eq!(resolved.files.len(), 1);
        assert!(resolved.files[0].content.contains("test_helper"));
        assert!(!resolved.files[0].content.contains("test_example"));
    }

    #[test]
    fn test_symbol_extraction() {
        let tmp = TempDir::new().unwrap();
        let files = create_test_files(tmp.path());

        let request = RequestFile {
            target: RequestTarget::SinglePath {
                path: "src/lib.rs".to_string(),
            },
            reason: "Find helper function".to_string(),
            range: Some(RequestRange::Symbol {
                symbol: "helper".to_string(),
            }),
        };

        let resolved = request.resolve(&files, tmp.path()).unwrap();
        assert_eq!(resolved.files.len(), 1);
        assert!(resolved.files[0].content.contains("pub fn helper"));
    }

    #[test]
    fn test_file_not_found() {
        let tmp = TempDir::new().unwrap();
        let files = create_test_files(tmp.path());

        let request = RequestFile {
            target: RequestTarget::SinglePath {
                path: "nonexistent.rs".to_string(),
            },
            reason: "This should fail".to_string(),
            range: None,
        };

        assert!(request.resolve(&files, tmp.path()).is_err());
    }

    #[test]
    fn test_pattern_no_matches() {
        let tmp = TempDir::new().unwrap();
        let files = create_test_files(tmp.path());

        let request = RequestFile {
            target: RequestTarget::Pattern {
                pattern: "*.py".to_string(),
            },
            reason: "Look for Python files".to_string(),
            range: None,
        };

        assert!(request.resolve(&files, tmp.path()).is_err());
    }

    #[test]
    fn test_markdown_output() {
        let tmp = TempDir::new().unwrap();
        let files = create_test_files(tmp.path());

        let request = RequestFile {
            target: RequestTarget::SinglePath {
                path: "src/main.rs".to_string(),
            },
            reason: "Example output".to_string(),
            range: Some(RequestRange::Lines {
                lines: "1-2".to_string(),
            }),
        };

        let resolved = request.resolve(&files, tmp.path()).unwrap();
        let markdown = resolved.to_markdown();

        assert!(markdown.contains("# REQUEST_FILE Results"));
        assert!(markdown.contains("**Reason:** Example output"));
        assert!(markdown.contains("src/main.rs"));
        assert!(markdown.contains("```"));
    }
}

// ============================================================================
// USAGE EXAMPLES
// ============================================================================
// See the README or GUIDE.txt for REQUEST_FILE protocol usage.
//
// Quick reference:
//
// Single file:
//   REQUEST_FILE:
//     path: src/server/handlers/users.rs
//     reason: Debug the get_user handler
//     range: lines 80-140
//
// Glob pattern:
//   REQUEST_FILE:
//     pattern: "tests/**/*_test.rs"
//     reason: Review all test files
//
// By symbol:
//   REQUEST_FILE:
//     path: src/lib.rs
//     reason: Understand validate_token
//     range: symbol: validate_token