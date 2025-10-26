// saccade/core/src/heuristics.rs

use std::collections::HashMap;
use std::fs;
use std::path::Path;

// --- Configuration Constants for Heuristics ---
const MIN_TEXT_ENTROPY: f64 = 3.5;
const MAX_TEXT_ENTROPY: f64 = 5.5;

/// Pathogen-Associated Molecular Patterns (PAMPs): Conserved keywords in build systems.
const BUILD_SYSTEM_PAMPS: &[&str] = &[
    "find_package", "add_executable", "target_link_libraries", "cmake_minimum_required",
    "project(", "add-apt-repository", "conanfile.py", "dependency", "require",
    "include", "import", "version", "dependencies",
];

pub struct HeuristicFilter;

impl HeuristicFilter {
    pub fn new() -> Self {
        Self
    }

    /// Filters files based on heuristic rules (entropy, content).
    pub fn filter(&self, files: Vec<std::path::PathBuf>) -> Vec<std::path::PathBuf> {
        files.into_iter().filter(|path| self.should_keep(path)).collect()
    }

    fn should_keep(&self, path: &Path) -> bool {
        if let Ok(entropy) = calculate_entropy(path) {
            if entropy < MIN_TEXT_ENTROPY || entropy > MAX_TEXT_ENTROPY {
                return false;
            }
        } else {
            return false;
        }

        if let Ok(content) = fs::read_to_string(path) {
            let lower_content = content.to_lowercase();
            for pamp in BUILD_SYSTEM_PAMPS {
                if lower_content.contains(pamp) {
                    return true;
                }
            }
        }
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