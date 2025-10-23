// Enhanced Stage-2 Generator with Concurrent Processing
// File: core/src/stage2.rs
//
// This replaces the existing Stage2Generator with:
// 1. Parallel file processing using Rayon
// 2. File size limits to avoid parsing huge files
// 3. Proper XML escaping for attributes and content
// 4. Deterministic output (sorted by path)
// 5. Progress reporting for large codebases

use crate::error::Result;
use crate::parser;
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

// Configuration constants
const MAX_FILE_SIZE_FOR_PARSING: u64 = 5 * 1024 * 1024; // 5 MB
const PROGRESS_REPORT_INTERVAL: usize = 100; // Report every N files

pub struct Stage2Generator {
    verbose: bool,
}

impl Stage2Generator {
    pub fn new() -> Self {
        Self { verbose: false }
    }

    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Generate compressed skeleton using parallel processing
    pub fn generate(
        &self,
        files_to_process: &[PathBuf],
        output_path: &Path,
    ) -> Result<Option<String>> {
        // Ensure output directory exists
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).ok();
        }

        let total_files = files_to_process.len();
        if total_files == 0 {
            return Ok(Some("No files to process for Stage 2.".to_string()));
        }

        if self.verbose {
            eprintln!("    Stage-2: Processing {} files in parallel...", total_files);
        }

        // Thread-safe counters and result storage
        let processed_count = AtomicUsize::new(0);
        let skipped_large = AtomicUsize::new(0);
        let skipped_unsupported = AtomicUsize::new(0);
        let results = Mutex::new(Vec::new());

        // Process files in parallel
        files_to_process
            .par_iter()
            .for_each(|file_path| {
                // Check file size first (avoid reading huge files)
                if let Ok(metadata) = fs::metadata(file_path) {
                    if metadata.len() > MAX_FILE_SIZE_FOR_PARSING {
                        skipped_large.fetch_add(1, Ordering::Relaxed);
                        return;
                    }
                }

                let extension = match file_path.extension().and_then(|s| s.to_str()) {
                    Some(ext) => ext,
                    None => {
                        skipped_unsupported.fetch_add(1, Ordering::Relaxed);
                        return;
                    }
                };

                // Read and parse file
                match fs::read_to_string(file_path) {
                    Ok(content) => {
                        if let Some(skeleton) = parser::skeletonize_file(&content, extension) {
                            let count = processed_count.fetch_add(1, Ordering::Relaxed) + 1;
                            
                            // Store result
                            let mut results_guard = results.lock().unwrap();
                            results_guard.push((file_path.clone(), skeleton));
                            drop(results_guard);

                            // Progress reporting
                            if self.verbose && count % PROGRESS_REPORT_INTERVAL == 0 {
                                eprintln!("    Stage-2: Processed {} / {} files", count, total_files);
                            }
                        } else {
                            skipped_unsupported.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    Err(_) => {
                        skipped_unsupported.fetch_add(1, Ordering::Relaxed);
                    }
                }
            });

        // Get final counts
        let final_count = processed_count.load(Ordering::Relaxed);
        let large_count = skipped_large.load(Ordering::Relaxed);
        let unsupported_count = skipped_unsupported.load(Ordering::Relaxed);

        if self.verbose {
            eprintln!("    Stage-2: Successfully parsed {} files", final_count);
            if large_count > 0 {
                eprintln!("    Stage-2: Skipped {} files (>5MB)", large_count);
            }
            if unsupported_count > 0 {
                eprintln!("    Stage-2: Skipped {} files (unsupported/errors)", unsupported_count);
            }
        }

        if final_count == 0 {
            return Ok(Some("No supported files found for Stage 2 skeletonization.".to_string()));
        }

        // Build final XML output
        let mut sorted_results = results.into_inner().unwrap();
        // Sort for deterministic output
        sorted_results.sort_by(|a, b| a.0.cmp(&b.0));

        let mut final_output = String::new();
        final_output.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        final_output.push_str("<files>\n");

        for (file_path, skeleton) in sorted_results {
            final_output.push_str("  <file path=\"");
            final_output.push_str(&escape_xml_attr(&file_path.display().to_string()));
            final_output.push_str("\">\n");
            
            // Indent the skeleton content
            for line in skeleton.lines() {
                final_output.push_str("    ");
                final_output.push_str(&escape_xml_content(line));
                final_output.push('\n');
            }
            
            final_output.push_str("  </file>\n");
        }

        final_output.push_str("</files>\n");

        // Write to disk
        fs::write(output_path, final_output)?;

        let msg = format!(
            "Stage-2: Wrote compressed skeleton for {} files to: {}",
            final_count,
            output_path.display()
        );
        Ok(Some(msg))
    }
}

/// Escape XML attribute values (more strict than content)
fn escape_xml_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Escape XML content (less strict than attributes)
fn escape_xml_content(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_xml_escaping() {
        assert_eq!(escape_xml_attr("a&b"), "a&amp;b");
        assert_eq!(escape_xml_attr("<tag>"), "&lt;tag&gt;");
        assert_eq!(escape_xml_attr("\"quoted\""), "&quot;quoted&quot;");
    }

    #[test]
    fn test_concurrent_processing() {
        let tmp = TempDir::new().unwrap();
        
        // Create test files
        let mut files = Vec::new();
        for i in 0..20 {
            let path = tmp.path().join(format!("test{}.rs", i));
            let mut file = fs::File::create(&path).unwrap();
            writeln!(file, "pub fn func{}() {{}}", i).unwrap();
            files.push(path);
        }

        // Generate skeleton
        let output = tmp.path().join("output.xml");
        let gen = Stage2Generator::new();
        let result = gen.generate(&files, &output).unwrap();

        assert!(result.is_some());
        assert!(output.exists());

        // Verify XML structure
        let content = fs::read_to_string(&output).unwrap();
        assert!(content.starts_with("<?xml"));
        assert!(content.contains("<files>"));
        assert!(content.contains("</files>"));
        assert_eq!(content.matches("<file path=").count(), 20);
    }

    #[test]
    fn test_file_size_limit() {
        let tmp = TempDir::new().unwrap();
        
        // Create a large file
        let large_file = tmp.path().join("large.rs");
        let mut file = fs::File::create(&large_file).unwrap();
        let large_content = "x".repeat(6 * 1024 * 1024); // 6 MB
        write!(file, "{}", large_content).unwrap();

        // Create a normal file
        let normal_file = tmp.path().join("normal.rs");
        fs::write(&normal_file, "pub fn small() {}").unwrap();

        let files = vec![large_file, normal_file];
        let output = tmp.path().join("output.xml");
        
        let gen = Stage2Generator::new().with_verbose(true);
        let result = gen.generate(&files, &output).unwrap();

        assert!(result.is_some());
        
        // Should only have processed 1 file (the small one)
        let content = fs::read_to_string(&output).unwrap();
        assert_eq!(content.matches("<file path=").count(), 1);
    }

    #[test]
    fn test_deterministic_output() {
        let tmp = TempDir::new().unwrap();
        
        // Create files in random order
        let files = vec!["z.rs", "a.rs", "m.rs"];
        let mut paths = Vec::new();
        for name in files {
            let path = tmp.path().join(name);
            fs::write(&path, "pub fn test() {}").unwrap();
            paths.push(path);
        }

        let output = tmp.path().join("output.xml");
        let gen = Stage2Generator::new();
        gen.generate(&paths, &output).unwrap();

        let content = fs::read_to_string(&output).unwrap();
        
        // Check that files appear in sorted order
        let a_pos = content.find("a.rs").unwrap();
        let m_pos = content.find("m.rs").unwrap();
        let z_pos = content.find("z.rs").unwrap();
        
        assert!(a_pos < m_pos);
        assert!(m_pos < z_pos);
    }
}