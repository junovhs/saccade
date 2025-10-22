use crate::error::Result;
use regex::Regex;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub struct Stage1Generator;

impl Stage1Generator {
    pub fn new() -> Self {
        Self
    }

    pub fn generate_combined_apis(
        &self,
        rust_crates: &[PathBuf],
        frontend_dirs: &[PathBuf],
        file_index: &[PathBuf],
    ) -> Result<String> {
        let mut output = String::new();

        // Rust API
        output.push_str("========================================\n");
        output.push_str("API SURFACE: RUST\n");
        output.push_str("========================================\n\n");
        
        let rust_api = self.extract_rust_api(rust_crates, file_index)?;
        output.push_str(&rust_api);

        // TypeScript/JavaScript API
        output.push_str("\n========================================\n");
        output.push_str("API SURFACE: TYPESCRIPT/JAVASCRIPT\n");
        output.push_str("========================================\n\n");
        
        let ts_api = self.extract_ts_api(frontend_dirs, file_index)?;
        output.push_str(&ts_api);

        // Python API
        output.push_str("\n========================================\n");
        output.push_str("API SURFACE: PYTHON\n");
        output.push_str("========================================\n\n");
        
        let py_api = self.extract_python_api(file_index)?;
        output.push_str(&py_api);

        // Go API
        output.push_str("\n========================================\n");
        output.push_str("API SURFACE: GO\n");
        output.push_str("========================================\n\n");
        
        let go_api = self.extract_go_api(file_index)?;
        output.push_str(&go_api);

        Ok(output)
    }

    pub fn find_rust_crates(&self) -> Result<Vec<PathBuf>> {
        let mut crates = Vec::new();

        for entry in walkdir::WalkDir::new(".")
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_name() == "Cargo.toml" && !entry.path().to_string_lossy().contains("/target/") {
                if let Some(parent) = entry.path().parent() {
                    let src_dir = parent.join("src");
                    if src_dir.exists() && src_dir.is_dir() {
                        let normalized = if let Ok(stripped) = src_dir.strip_prefix(".") {
                            stripped.to_path_buf()
                        } else {
                            src_dir
                        };
                        crates.push(normalized);
                    }
                }
            }
        }

        Ok(crates)
    }

    pub fn find_frontend_dirs(&self) -> Result<Vec<PathBuf>> {
        let mut dirs = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for entry in walkdir::WalkDir::new(".")
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                let name = e.file_name().to_string_lossy();
                !matches!(name.as_ref(), "node_modules" | "dist" | "build" | ".git")
            })
            .filter_map(|e| e.ok())
        {
            if entry.file_name() == "package.json" {
                if let Some(parent) = entry.path().parent() {
                    let normalized = if let Ok(stripped) = parent.strip_prefix(".") {
                        stripped.to_path_buf()
                    } else {
                        parent.to_path_buf()
                    };
                    if seen.insert(normalized.clone()) {
                        dirs.push(normalized);
                    }
                }
            }
        }

        if dirs.is_empty() {
            for name in &["app", "frontend", "web", "client", "ui", "src"] {
                let path = PathBuf::from(name);
                if path.exists() && path.is_dir() {
                    dirs.push(path);
                    break;
                }
            }
        }

        Ok(dirs)
    }

    pub fn generate_cargo_tree(&self) -> Result<String> {
        if let Ok(cwd) = std::env::current_dir() {
            let cwd_str = cwd.to_string_lossy();
            if cwd_str.contains("/tmp/") || cwd_str.contains("\\Temp\\") {
                return Ok(String::new());
            }
        }
        
        let output = Command::new("cargo")
            .args(&["tree", "-e", "normal", "-d"])
            .output();

        match output {
            Ok(out) if out.status.success() => {
                let mut result = String::from("========================================\n");
                result.push_str("RUST DEPENDENCIES (cargo tree -d)\n");
                result.push_str("========================================\n\n");
                result.push_str(&String::from_utf8_lossy(&out.stdout));
                Ok(result)
            }
            _ => Ok(String::new()),
        }
    }

    fn extract_rust_api(&self, crates: &[PathBuf], file_index: &[PathBuf]) -> Result<String> {
        if crates.is_empty() {
            return Ok("(no Rust crates found)\n".to_string());
        }

        let pattern = Regex::new(
            r"pub(\s+|\s*\([^)]*\)\s+)(fn|struct|enum|trait|type|const|static|use|mod|macro_rules!)"
        )?;

        let mut output = String::new();

        for crate_dir in crates {
            let crate_str = crate_dir.to_string_lossy();

            for file_path in file_index {
                let file_str = file_path.to_string_lossy();

                if file_str.starts_with(&*crate_str) && file_str.ends_with(".rs") {
                    if let Ok(content) = fs::read_to_string(file_path) {
                        for (line_num, line) in content.lines().enumerate() {
                            if pattern.is_match(line) {
                                output.push_str(&format!("{}:{}:{}\n", file_str, line_num + 1, line));
                            }
                        }
                    }
                }
            }
        }

        if output.is_empty() {
            output = "(no public Rust items found)\n".to_string();
        }

        Ok(output)
    }

    fn extract_ts_api(&self, frontend_dirs: &[PathBuf], file_index: &[PathBuf]) -> Result<String> {
        if frontend_dirs.is_empty() {
            return Ok("(no frontend dirs found)\n".to_string());
        }

        let pattern = Regex::new(
            r"^(\s*export\s+(default\s+)?(function|class|interface|type|enum|const|let|var|async|function\*)|\s*(function|class)\s+[A-Z])"
        )?;

        let mut output = String::new();

        for frontend_dir in frontend_dirs {
            let dir_str = frontend_dir.to_string_lossy();

            for file_path in file_index {
                let file_str = file_path.to_string_lossy();

                if file_str.starts_with(&*dir_str)
                    && (file_str.ends_with(".js")
                        || file_str.ends_with(".jsx")
                        || file_str.ends_with(".ts")
                        || file_str.ends_with(".tsx")
                        || file_str.ends_with(".mjs")
                        || file_str.ends_with(".cjs"))
                    && !file_str.ends_with(".d.ts")
                {
                    if let Ok(content) = fs::read_to_string(file_path) {
                        for (line_num, line) in content.lines().enumerate() {
                            if pattern.is_match(line) {
                                output.push_str(&format!("{}:{}:{}\n", file_str, line_num + 1, line));
                            }
                        }
                    }
                }
            }
        }

        if output.is_empty() {
            output = "(no TS/JS items found)\n".to_string();
        }

        Ok(output)
    }

    fn extract_python_api(&self, file_index: &[PathBuf]) -> Result<String> {
        let pattern = Regex::new(r"^\s*(def|class)\s+([A-Za-z][A-Za-z0-9_]*)")?;

        let mut output = String::new();

        for file_path in file_index {
            let file_str = file_path.to_string_lossy();

            if file_str.ends_with(".py") {
                if let Ok(content) = fs::read_to_string(file_path) {
                    for (line_num, line) in content.lines().enumerate() {
                        if let Some(caps) = pattern.captures(line) {
                            if let Some(name) = caps.get(2) {
                                if !name.as_str().starts_with('_') {
                                    output.push_str(&format!("{}:{}:{}\n", file_str, line_num + 1, line));
                                }
                            }
                        }
                    }
                }
            }
        }

        if output.is_empty() {
            output = "(no Python items found)\n".to_string();
        }

        Ok(output)
    }

    fn extract_go_api(&self, file_index: &[PathBuf]) -> Result<String> {
        let pattern = Regex::new(r"^\s*func\s+([A-Z][A-Za-z0-9_]*)\s*\(")?;

        let mut output = String::new();

        for file_path in file_index {
            let file_str = file_path.to_string_lossy();

            if file_str.ends_with(".go") {
                if let Ok(content) = fs::read_to_string(file_path) {
                    for (line_num, line) in content.lines().enumerate() {
                        if pattern.is_match(line) {
                            output.push_str(&format!("{}:{}:{}\n", file_str, line_num + 1, line));
                        }
                    }
                }
            }
        }

        if output.is_empty() {
            output = "(no Go items found)\n".to_string();
        }

        Ok(output)
    }
}