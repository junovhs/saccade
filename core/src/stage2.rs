// saccade/core/src/stage2.rs

use crate::error::{Result, SaccadeError};
use crate::parser;
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::panic;

// Configuration constants
const MAX_FILE_SIZE_FOR_PARSING: u64 = 5 * 1024 * 1024; // 5 MB
const PROGRESS_REPORT_INTERVAL: usize = 100; // Report every N files

pub struct Stage2Generator {
    verbose: bool,
}

type ParseResult = (PathBuf, String);

impl Stage2Generator {
    pub fn new() -> Self { Self { verbose: false } }

    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Generate compressed skeleton, now with a panic boundary.
    pub fn generate(&self, files_to_process: &[PathBuf], output_path: &Path) -> Result<Option<String>> {
        if let Some(parent) = output_path.parent() { fs::create_dir_all(parent).ok(); }
        if files_to_process.is_empty() { return Ok(Some("No files to process for Stage 2.".to_string())); }
        if self.verbose { eprintln!("    Stage-2: Processing {} files in parallel...", files_to_process.len()); }

        // --- Panic Boundary ---
        // This catches panics from any worker thread and converts them into a Result::Err.
        // This is the "Build to Survive" mandate in action.
        let processing_result = panic::catch_unwind(|| {
            self.process_files_concurrently(files_to_process)
        });

        let (results, stats) = match processing_result {
            Ok(Ok(res)) => res, // Success: No panic, and the function returned Ok.
            Ok(Err(e)) => return Err(e), // No panic, but the function returned a recoverable error.
            Err(_) => return Err(SaccadeError::MutexPoisoned), // A panic was caught.
        };
        // --- End Panic Boundary ---

        let processed_count = stats.processed.load(Ordering::Relaxed);
        if self.verbose {
            eprintln!("    Stage-2: Successfully parsed {} files", processed_count);
            let skipped_large_count = stats.skipped_large.load(Ordering::Relaxed);
            if skipped_large_count > 0 { eprintln!("    Stage-2: Skipped {} files (>5MB)", skipped_large_count); }
            let skipped_unsupported_count = stats.skipped_unsupported.load(Ordering::Relaxed);
            if skipped_unsupported_count > 0 { eprintln!("    Stage-2: Skipped {} files (unsupported/read-errors)", skipped_unsupported_count); }
        }
        if results.is_empty() { return Ok(Some("No supported files found for Stage 2 skeletonization.".to_string())); }

        let final_output = self.build_xml_output(results);
        fs::write(output_path, final_output).map_err(|e| SaccadeError::Io {
            source: e,
            path: output_path.to_path_buf(),
        })?;

        let msg = format!("Stage-2: Wrote compressed skeleton for {} files to: {}", processed_count, output_path.display());
        Ok(Some(msg))
    }

    /// Processes files in parallel. This function is now panic-safe when called via `generate`.
    fn process_files_concurrently(&self, files_to_process: &[PathBuf]) -> Result<(Vec<ParseResult>, Stage2Stats)> {
        let stats = Stage2Stats::default();
        let results = Mutex::new(Vec::new());
        let total_files = files_to_process.len();

        files_to_process.par_iter().for_each(|file_path| {
            //panic!("Simulating panic"); Keep this line for the test!
            if let Ok(metadata) = fs::metadata(file_path) {
                if metadata.len() > MAX_FILE_SIZE_FOR_PARSING {
                    stats.skipped_large.fetch_add(1, Ordering::Relaxed);
                    return;
                }
            }
            let Some(extension) = file_path.extension().and_then(|s| s.to_str()) else {
                stats.skipped_unsupported.fetch_add(1, Ordering::Relaxed);
                return;
            };
            if let Ok(content) = fs::read_to_string(file_path) {
                if let Some(skeleton) = parser::skeletonize_file(&content, extension) {
                    let count = stats.processed.fetch_add(1, Ordering::Relaxed) + 1;
                    if let Ok(mut guard) = results.lock() { guard.push((file_path.clone(), skeleton)); }
                    if self.verbose && count % PROGRESS_REPORT_INTERVAL == 0 {
                        eprintln!("    Stage-2: Processed {} / {} files", count, total_files);
                    }
                } else { stats.skipped_unsupported.fetch_add(1, Ordering::Relaxed); }
            } else { stats.skipped_unsupported.fetch_add(1, Ordering::Relaxed); }
        });

        let final_results = results.into_inner().map_err(|_| SaccadeError::MutexPoisoned)?;
        Ok((final_results, stats))
    }
    
    fn build_xml_output(&self, mut results: Vec<ParseResult>) -> String {
        results.sort_by(|a, b| a.0.cmp(&b.0));
        let mut final_output = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<files>\n");
        for (file_path, skeleton) in results {
            final_output.push_str(&format!("  <file path=\"{}\">\n", escape_xml_attr(&file_path.to_string_lossy())));
            for line in skeleton.lines() {
                final_output.push_str(&format!("    {}\n", escape_xml_content(line)));
            }
            final_output.push_str("  </file>\n");
        }
        final_output.push_str("</files>\n");
        final_output
    }
}

#[derive(Default)]
struct Stage2Stats {
    processed: AtomicUsize,
    skipped_large: AtomicUsize,
    skipped_unsupported: AtomicUsize,
}

fn escape_xml_attr(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

fn escape_xml_content(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}