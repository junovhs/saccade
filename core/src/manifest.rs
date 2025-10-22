use crate::config::{Config, GitMode};
use crate::error::Result;
use chrono::Utc;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Serialize)]
pub struct Manifest {
    pack_version: String,
    timestamp_utc: String,
    git_commit: Option<String>,
    args: ManifestArgs,
    counts: ManifestCounts,
    tools: ManifestTools,
    artifacts: HashMap<String, u64>,
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

    pub fn generate(
        &self,
        raw_count: usize,
        filtered_count: usize,
        pack_dir: &Path,
        in_git: bool,
    ) -> Result<String> {
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

        let artifacts = self.collect_artifact_sizes(pack_dir)?;

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

        let manifest = Manifest {
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
            artifacts,
        };

        let json = serde_json::to_string_pretty(&manifest)?;
        Ok(json)
    }

    fn collect_artifact_sizes(&self, pack_dir: &Path) -> Result<HashMap<String, u64>> {
        let artifact_names = vec![
            "OVERVIEW.md",
            "STRUCTURE.txt",
            "TOKENS.txt",
            "FILE_INDEX.txt",
            "LANGUAGES.md",
            "API_SURFACE_RUST.txt",
            "API_SURFACE_TS.txt",
            "API_SURFACE_PYTHON.txt",
            "API_SURFACE_GO.txt",
            "CARGO_TREE_DEDUP.txt",
            "PACK_STAGE2_COMPRESSED.xml",
            "REQUEST_PROTOCOL.md",
            "PACK_README.md",
        ];

        let mut sizes = HashMap::new();

        for name in artifact_names {
            let path = pack_dir.join(name);
            let size = if path.exists() {
                fs::metadata(&path).map(|m| m.len()).unwrap_or(0)
            } else {
                0
            };
            sizes.insert(name.to_string(), size);
        }

        Ok(sizes)
    }
}