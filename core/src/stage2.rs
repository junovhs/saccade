// In saccade/core/src/stage2.rs

use crate::error::Result;
use crate::parser;
use std::fs;
use std::path::{Path, PathBuf};

pub struct Stage2Generator;

impl Stage2Generator {
    pub fn new() -> Self {
        Self
    }

    /// Run the internal parser to generate a compressed skeleton.
    ///
    /// This now iterates through all known files, skeletonizes them using our
    /// internal Tree-sitter parser, and aggregates the results into a single XML file.
    pub fn generate(
        &self,
        files_to_process: &[PathBuf],
        output_path: &Path,
    ) -> Result<Option<String>> {
        // Ensure output directory exists
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).ok();
        }

        let mut final_output = String::new();
        final_output.push_str("<files>\n");

        let mut files_processed = 0;
        for file_path in files_to_process {
            let extension = file_path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("");

            // Attempt to read and parse the file
            if let Ok(content) = fs::read_to_string(file_path) {
                if let Some(skeleton) = parser::skeletonize_file(&content, extension) {
                    files_processed += 1;
                    final_output.push_str(&format!("<file path=\"{}\">\n", file_path.display()));
                    // Basic XML escaping for the content
                    final_output.push_str(
                        &skeleton
                            .replace('&', "&amp;")
                            .replace('<', "&lt;")
                            .replace('>', "&gt;"),
                    );
                    final_output.push_str("\n</file>\n");
                }
            }
        }

        final_output.push_str("</files>\n");

        if files_processed > 0 {
            fs::write(output_path, final_output)?;
            let msg = format!(
                "Saccade's internal parser wrote a compressed skeleton for {} files to: {}",
                files_processed,
                output_path.display()
            );
            Ok(Some(msg))
        } else {
            Ok(Some(
                "No supported files found for Stage 2 skeletonization.".to_string(),
            ))
        }
    }
}