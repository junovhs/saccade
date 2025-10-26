use crate::config::Config;
use crate::error::Result;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;

pub struct Stage0Generator {
    config: Config,
}

impl Stage0Generator {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn generate_combined_structure(
        &self,
        files: &[std::path::PathBuf],
        detected_systems: &[crate::detection::BuildSystemType],
    ) -> Result<String> {
        let mut output = String::new();

        // --- Find project roots by locating manifest files ---
        let mut project_roots = HashMap::new();
        for path in files {
            let file_name = path.file_name().and_then(|n| n.to_str());
            if let Some(name) = file_name {
                let system_type = match name {
                    "Cargo.toml" => Some(crate::detection::BuildSystemType::Rust),
                    "package.json" => Some(crate::detection::BuildSystemType::Node),
                    "go.mod" => Some(crate::detection::BuildSystemType::Go),
                    "requirements.txt" | "pyproject.toml" | "Pipfile" => {
                        Some(crate::detection::BuildSystemType::Python)
                    }
                    "CMakeLists.txt" => Some(crate::detection::BuildSystemType::CMake),
                    _ => None,
                };

                if let (Some(st), Some(parent)) = (system_type, path.parent()) {
                    if detected_systems.contains(&st) {
                        let parent_path = parent.to_string_lossy().replace('\\', "/");
                        let key = if parent_path.is_empty() {
                            ".".to_string()
                        } else {
                            parent_path
                        };
                        project_roots.insert(key, st);
                    }
                }
            }
        }
        // --- End project root detection ---

        // Section 1: Directory Tree
        output.push_str("========================================\n");
        output.push_str("DIRECTORY TREE\n");
        output.push_str("========================================\n\n");

        // Collect only directory prefixes (not filenames)
        let mut dirs = BTreeSet::new();
        dirs.insert(".".to_string()); // Always include the root
        for path in files {
            if let Some(parent) = path.parent() {
                if parent.as_os_str().is_empty() {
                    continue; // Handled by the explicit "." insert
                }
                let comps: Vec<_> = parent.components().collect();
                let limit = comps.len().min(self.config.max_depth);

                for i in 1..=limit {
                    let dir_path: std::path::PathBuf = comps[..i].iter().collect();
                    dirs.insert(dir_path.to_string_lossy().replace('\\', "/"));
                }
            }
        }

        output.push_str(&format!(
            "Directories (depth <= {}, with detected project roots):\n\n",
            self.config.max_depth
        ));
        for dir in &dirs {
            if let Some(system_type) = project_roots.get(dir.as_str()) {
                output.push_str(&format!("{}  <-- [{} Project]\n", dir, system_type));
            } else {
                output.push_str(dir);
                output.push('\n');
            }
        }

        // Section 2: File Index
        output.push_str("\n========================================\n");
        output.push_str("FILE INDEX\n");
        output.push_str("========================================\n\n");

        let mut sorted: Vec<String> = files
            .iter()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
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
                file_sizes.push((bytes, path.to_string_lossy().replace('\\', "/")));
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