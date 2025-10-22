use crate::config::Config;
use crate::error::Result;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

pub struct Stage0Generator {
    config: Config,
}

impl Stage0Generator {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn generate_combined_structure(&self, files: &[std::path::PathBuf]) -> Result<String> {
        let mut output = String::new();

        // Section 1: Directory Tree
        output.push_str("========================================\n");
        output.push_str("DIRECTORY TREE\n");
        output.push_str("========================================\n\n");

        // Collect only directory prefixes (not filenames)
        let mut dirs = BTreeSet::new();
        for path in files {
            let comps: Vec<_> = path.components().collect();
            // Up to (max_depth - 1) components, but stop before the final filename
            let max = self.config.max_depth.saturating_sub(1);
            let limit = comps.len().saturating_sub(1).min(max);
            if limit == 0 {
                continue;
            }
            let mut current = String::new();
            for i in 0..limit {
                if !current.is_empty() {
                    current.push('/');
                }
                current.push_str(&comps[i].as_os_str().to_string_lossy());
                dirs.insert(current.clone());
            }
        }

        output.push_str(&format!(
            "Directories (depth <= {}):\n\n",
            self.config.max_depth
        ));
        for dir in &dirs {
            output.push_str(dir);
            output.push('\n');
        }

        // Section 2: File Index
        output.push_str("\n========================================\n");
        output.push_str("FILE INDEX\n");
        output.push_str("========================================\n\n");

        let mut sorted: Vec<String> = files
            .iter()
            .map(|p| {
                let mut s = p.display().to_string();
                if s.starts_with("./") {
                    s = s.trim_start_matches("./").to_string();
                }
                s
            })
            .collect();
        sorted.sort();
        sorted.dedup();

        output.push_str(&format!("All files (n = {}):\n\n", sorted.len()));
        for file in &sorted {
            output.push_str(file);
            output.push('\n');
        }

        // Section 3: Token Heatmap
        output.push_str("\n========================================\n");
        output.push_str("TOKEN HEATMAP\n");
        output.push_str("========================================\n\n");

        let mut file_sizes: Vec<(u64, String)> = Vec::new();
        for path in files {
            if let Ok(metadata) = fs::metadata(path) {
                let bytes = metadata.len();
                file_sizes.push((bytes, path.display().to_string()));
            }
        }

        file_sizes.sort_by(|a, b| b.0.cmp(&a.0));
        file_sizes.truncate(50);

        output.push_str("Size estimates (bytes → ~tokens via /3.5). Top 50:\n\n");
        for (bytes, path) in file_sizes {
            let est_tokens = (bytes as f64 / 3.5) as u64;
            output.push_str(&format!(
                "{:>12} bytes  ~{:>8} tokens  {}\n",
                bytes, est_tokens, path
            ));
        }

        Ok(output)
    }

    pub fn generate_languages(&self, files: &[std::path::PathBuf]) -> Result<String> {
        let mut ext_counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut total = 0;

        for path in files {
            total += 1;

            let ext = if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if matches!(
                    name,
                    "Makefile" | "Dockerfile" | "dockerfile" | "CMakeLists.txt" | "BUILD" | "WORKSPACE"
                ) {
                    name.to_string()
                } else if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
                    extension.to_string()
                } else {
                    "(noext)".to_string()
                }
            } else {
                "(noext)".to_string()
            };

            *ext_counts.entry(ext).or_insert(0) += 1;
        }

        let mut output = String::new();
        output.push_str("Language/Extension Snapshot\n\n");
        output.push_str("| Extension | Files |\n");
        output.push_str("|----------:|------:|\n");

        let mut sorted: Vec<_> = ext_counts.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));

        for (ext, count) in sorted {
            output.push_str(&format!("| {} | {} |\n", ext, count));
        }

        output.push_str(&format!("\nTotal files: {}\n", total));

        Ok(output)
    }
}
