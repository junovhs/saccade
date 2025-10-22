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

    pub fn generate_structure(&self, files: &[std::path::PathBuf]) -> Result<String> {
        let mut dirs = BTreeSet::new();
        let mut filtered_files = BTreeSet::new();

        for path in files {
            let depth = path.components().count();

            if depth <= self.config.max_depth {
                filtered_files.insert(path.display().to_string());
            }

            // Extract parent directories up to max_depth
            let mut current = String::new();
            for (i, component) in path.components().enumerate() {
                if i >= self.config.max_depth - 1 {
                    break;
                }
                if !current.is_empty() {
                    current.push('/');
                }
                current.push_str(&component.as_os_str().to_string_lossy());
                dirs.insert(current.clone());
            }
        }

        let mut output = String::new();
        output.push_str(&format!("Directories (depth<={}):\n", self.config.max_depth));
        for dir in &dirs {
            output.push_str(dir);
            output.push('\n');
        }

        output.push('\n');
        output.push_str(&format!("Files (depth<={}, filtered common junk):\n", self.config.max_depth));
        for file in &filtered_files {
            output.push_str(file);
            output.push('\n');
        }

        Ok(output)
    }

    pub fn generate_tokens(&self, files: &[std::path::PathBuf]) -> Result<String> {
        let mut file_sizes: Vec<(u64, String)> = Vec::new();

        for path in files {
            if let Ok(metadata) = fs::metadata(path) {
                let bytes = metadata.len();
                file_sizes.push((bytes, path.display().to_string()));
            }
        }

        // Sort by size descending
        file_sizes.sort_by(|a, b| b.0.cmp(&a.0));

        // Take top 50
        file_sizes.truncate(50);

        let mut output = String::new();
        output.push_str("Heuristic Size Heat Map (bytes; ~tokens ≈ bytes/3.5). Top 50:\n");

        for (bytes, path) in file_sizes {
            let est_tokens = (bytes as f64 / 3.5) as u64;
            output.push_str(&format!(
                "{:>12} bytes  ~{:>8} tokens  {}\n",
                bytes, est_tokens, path
            ));
        }

        Ok(output)
    }

    pub fn generate_file_index(&self, files: &[std::path::PathBuf]) -> Result<String> {
        let mut sorted: Vec<String> = files.iter().map(|p| p.display().to_string()).collect();
        sorted.sort();

        Ok(sorted.join("\n") + "\n")
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
        output.push_str("# Language/Extension snapshot\n\n");
        output.push_str("| Extension | Files |\n");
        output.push_str("|----------:|------:|\n");

        // Sort by count descending
        let mut sorted: Vec<_> = ext_counts.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));

        for (ext, count) in sorted {
            output.push_str(&format!("| {} | {} |\n", ext, count));
        }

        output.push_str(&format!("\n_Total files counted: {}_\n", total));

        Ok(output)
    }
}