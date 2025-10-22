use crate::config::{Config, GitMode};
use crate::error::Result;
use crate::stage0::Stage0Generator;
use chrono::Utc;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Serialize)]
pub struct Manifest {
    pack_version: String,
    timestamp_utc: String,
    git_commit: Option<String>,
    args: ManifestArgs,
    counts: ManifestCounts,
    tools: ManifestTools,
}

#[derive(Serialize)]
struct ManifestArgs {
    out: String,
    max_depth: usize,
    git_mode: String,
    include: String,
    exclude: String,
    code_only: bool,
    verbose: bool,
}

#[derive(Serialize)]
struct ManifestCounts {
    files_raw: usize,
    files_filtered: usize,
}

#[derive(Serialize)]
struct ManifestTools {
    git: bool,
    cargo: bool,
    repomix: bool,
}

pub struct ManifestGenerator {
    config: Config,
}

impl ManifestGenerator {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn generate_project_info(
        &self,
        raw_count: usize,
        filtered_count: usize,
        _pack_dir: &Path,
        in_git: bool,
        files: &[PathBuf],
    ) -> Result<String> {
        let mut output = String::new();

        // Section 1: README (if exists)
        output.push_str("========================================\n");
        output.push_str("PROJECT OVERVIEW\n");
        output.push_str("========================================\n\n");

        let readme_found = self.try_include_readme(&mut output)?;
        
        if !readme_found {
            output.push_str("(No README.md found in project root)\n\n");
            output.push_str("User: Describe your project and current task here.\n\n");
        }

        // Section 2: Manifest
        output.push_str("========================================\n");
        output.push_str("PACK METADATA\n");
        output.push_str("========================================\n\n");

        let manifest = self.build_manifest(raw_count, filtered_count, in_git)?;
        let manifest_json = serde_json::to_string_pretty(&manifest)?;
        output.push_str(&manifest_json);
        output.push_str("\n\n");

        // Section 3: Language Stats
        output.push_str("========================================\n");
        output.push_str("LANGUAGE STATISTICS\n");
        output.push_str("========================================\n\n");

        let stage0 = Stage0Generator::new(self.config.clone());
        let languages = stage0.generate_languages(files)?;
        output.push_str(&languages);

        Ok(output)
    }

    fn try_include_readme(&self, output: &mut String) -> Result<bool> {
        for name in &["README.md", "README.txt", "README", "readme.md"] {
            let readme_path = Path::new(name);
            if readme_path.exists() {
                if let Ok(content) = fs::read_to_string(readme_path) {
                    output.push_str(&content);
                    output.push_str("\n\n");
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    fn build_manifest(
        &self,
        raw_count: usize,
        filtered_count: usize,
        in_git: bool,
    ) -> Result<Manifest> {
        let timestamp_utc = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

        let git_commit = if in_git {
            Command::new("git")
                .args(&["rev-parse", "--short", "HEAD"])
                .output()
                .ok()
                .and_then(|o| {
                    if o.status.success() {
                        String::from_utf8(o.stdout).ok().map(|s| s.trim().to_string())
                    } else {
                        None
                    }
                })
        } else {
            None
        };

        let git_mode_str = match self.config.git_mode {
            GitMode::Auto => "auto",
            GitMode::Yes => "yes",
            GitMode::No => "no",
        }
        .to_string();

        let include_str = self
            .config
            .include_patterns
            .iter()
            .map(|r| r.as_str())
            .collect::<Vec<_>>()
            .join(",");

        let exclude_str = self
            .config
            .exclude_patterns
            .iter()
            .map(|r| r.as_str())
            .collect::<Vec<_>>()
            .join(",");

        let has_cargo = Command::new("cargo")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        let has_repomix = Command::new("repomix")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        Ok(Manifest {
            pack_version: env!("CARGO_PKG_VERSION").to_string(),
            timestamp_utc,
            git_commit,
            args: ManifestArgs {
                out: self.config.pack_dir.display().to_string(),
                max_depth: self.config.max_depth,
                git_mode: git_mode_str,
                include: include_str,
                exclude: exclude_str,
                code_only: self.config.code_only,
                verbose: self.config.verbose,
            },
            counts: ManifestCounts {
                files_raw: raw_count,
                files_filtered: filtered_count,
            },
            tools: ManifestTools {
                git: in_git,
                cargo: has_cargo,
                repomix: has_repomix,
            },
        })
    }
}