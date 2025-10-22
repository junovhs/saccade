use crate::error::{Result, SaccadeError};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Stage2Generator;

impl Stage2Generator {
    pub fn new() -> Self {
        Self
    }

    pub fn has_repomix(&self) -> bool {
        Command::new("repomix")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    pub fn generate(
        &self,
        rust_crates: &[PathBuf],
        frontend_dirs: &[PathBuf],
        out_path: &Path,
    ) -> Result<Option<String>> {
        if !self.has_repomix() {
            return Ok(None);
        }

        let mut include_patterns = String::new();
        for c in rust_crates {
            let mut s = c.display().to_string();
            if s.starts_with("./") {
                s = s.trim_start_matches("./").to_string();
            }
            include_patterns.push_str(&format!("{}/**,", s));
        }
        for f in frontend_dirs {
            let mut s = f.display().to_string();
            if s.starts_with("./") {
                s = s.trim_start_matches("./").to_string();
            }
            include_patterns.push_str(&format!("{}/**,", s));
        }
        include_patterns.push_str("*.toml,*.json,*.md");
        if include_patterns.ends_with(',') {
            include_patterns.pop();
        }

        let ignore = "**/target/**,**/dist/**,**/build/**,**/node_modules/**,**/gen/**,**/schemas/**,**/tests/**,**/test/**,**/__tests__/**,**/*.lock,AI_*.*,**/.*";

        let out = Command::new("repomix")
            .args([
                "--compress",
                "--remove-comments",
                "--remove-empty-lines",
                "--include",
                &include_patterns,
                "--ignore",
                ignore,
                "--style",
                "xml",
                "-o",
                &out_path.display().to_string(),
            ])
            .output()?; // io::Error -> SaccadeError (From)

        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            return Err(SaccadeError::RepomixFailed { stderr });
        }

        Ok(Some("repomix: packed successfully".into()))
    }
}
