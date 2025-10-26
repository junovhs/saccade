// saccade/core/src/heuristics.rs

use crate::config::{CODE_BARE_PATTERN, CODE_EXT_PATTERN};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

// --- Configuration Constants for Heuristics ---
const MIN_TEXT_ENTROPY: f64 = 3.5;
const MAX_TEXT_ENTROPY: f64 = 5.5;

const BUILD_SYSTEM_PAMPS: &[&str] = &[
    "find_package", "add_executable", "target_link_libraries", "cmake_minimum_required",
    "project(", "add-apt-repository", "conanfile.py", "dependency", "require",
    "include", "import", "version", "dependencies",
];

// Pre-compiled regexes for known code files
static CODE_EXT_RE: Lazy<Regex> = Lazy::new(|| Regex::new(CODE_EXT_PATTERN).unwrap());
static CODE_BARE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(CODE_BARE_PATTERN).unwrap());

pub struct HeuristicFilter;

impl HeuristicFilter {
    pub fn new() -> Self { Self }

    pub fn filter(&self, files: Vec<std::path::PathBuf>) -> Vec<std::path::PathBuf> {
        files.into_iter().filter(|path| self.should_keep(path)).collect()
    }

    /// Determines whether a file should be kept based on layered heuristic rules.
    fn should_keep(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        // Rule 1: Always keep known source code/config/markup files.
        // This prevents small test files or simple scripts from being discarded by entropy.
        if CODE_EXT_RE.is_match(&path_str) || CODE_BARE_RE.is_match(&path_str) {
            return true;
        }

        // Rule 2: For unknown file types, apply entropy analysis to reject binaries.
        if let Ok(entropy) = calculate_entropy(path) {
            if entropy < MIN_TEXT_ENTROPY || entropy > MAX_TEXT_ENTROPY {
                return false;
            }
        } else {
            return false; // Could not read file, reject.
        }

        // Rule 3: If an unknown file passes entropy, check for PAMPs.
        // This is how we discover non-standard manifests like `custom_build.cfg`.
        if let Ok(content) = fs::read_to_string(path) {
            let lower_content = content.to_lowercase();
            for pamp in BUILD_SYSTEM_PAMPS {
                if lower_content.contains(pamp) {
                    return true;
                }
            }
        }
        
        // Default: If it's an unknown file type that passed entropy but has no PAMPs,
        // we assume it's likely just a text file (e.g., notes.txt) and keep it for now.
        // The next filter stage will handle it.
        true
    }
}

/// Calculates the Shannon entropy of a file's content.
fn calculate_entropy(path: &Path) -> std::io::Result<f64> {
    let bytes = fs::read(path)?;
    if bytes.is_empty() { return Ok(0.0); }

    let mut freq_map = HashMap::new();
    for &byte in &bytes {
        *freq_map.entry(byte).or_insert(0) += 1;
    }

    let len = bytes.len() as f64;
    let entropy = freq_map.values().fold(0.0, |acc, &count| {
        let probability = count as f64 / len;
        acc - probability * probability.log2()
    });

    Ok(entropy)
}