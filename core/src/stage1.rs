use crate::error::{Result, SaccadeError};
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Stage1Generator;

impl Stage1Generator {
    pub fn new() -> Self {
        Self
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
                        crates.push(src_dir);
                    }
                }
            }
        }

        Ok(crates)
    }

    pub fn find_frontend_dirs(&self) -> Result<Vec<PathBuf>> {
        let mut dirs = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // First: find package.json locations
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
                    let path = parent.to_path_buf();
                    if seen.insert(path.clone()) {
                        dirs.push(path);
                    }
                }
            }
        }

        // Fallback: if no package.json found, check common dirs
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
        let output = Command::new("cargo")
            .args(&["tree", "-e", "normal", "-d"])
            .output();

        match output {
            Ok(out) if out.status.success() => Ok(String::from_utf8_lossy(&out.stdout).to_string()),
            _ => Ok(String::new()),
        }
    }

    pub fn extract_rust_api(&self, crates: &[PathBuf], file_index: &[PathBuf]) -> Result<String> {
        if crates.is_empty() {
            return Ok("(no Rust crates found)\n".to_string());
        }

        let pattern = Regex::new(
            r"^\s*pub(\s+|\s*\([^)]*\)\s+)(fn|struct|enum|trait|type|const|static|use|mod|macro_rules!)"
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

    pub fn extract_ts_api(&self, frontend_dirs: &[PathBuf], file_index: &[PathBuf]) -> Result<String> {
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

    pub fn extract_python_api(&self, file_index: &[PathBuf]) -> Result<String> {
        let pattern = Regex::new(r"^\s*(def|class)\s+([A-Za-z][A-Za-z0-9_]*)")?;

        let mut output = String::new();

        for file_path in file_index {
            let file_str = file_path.to_string_lossy();

            if file_str.ends_with(".py") {
                if let Ok(content) = fs::read_to_string(file_path) {
                    for (line_num, line) in content.lines().enumerate() {
                        if let Some(caps) = pattern.captures(line) {
                            if let Some(name) = caps.get(2) {
                                // Exclude names starting with underscore
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

    pub fn extract_go_api(&self, file_index: &[PathBuf]) -> Result<String> {
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