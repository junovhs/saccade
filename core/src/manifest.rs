// saccade/core/src/manifest.rs

use crate::config::Config;
use crate::detection::BuildSystemType;
use crate::error::Result;
use crate::stage0::Stage0Generator;
use chrono::{DateTime, Local};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct ManifestGenerator {
    config: Config,
}

/// Context for generating the PROJECT section.
pub struct ProjectInfoContext<'a> {
    pub raw_count: usize,
    pub filtered_count: usize,
    pub pack_dir: &'a Path,
    pub in_git: bool,
    pub files: &'a [PathBuf],
    pub detected_systems: &'a [BuildSystemType], // <-- ADDED: DCA Signal
}

impl ManifestGenerator {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn generate_project_info(&self, ctx: &ProjectInfoContext) -> Result<String> {
        let now: DateTime<Local> = Local::now();
        let git_commit = if ctx.in_git { self.get_git_commit() } else { None };
        let stage0 = Stage0Generator::new(self.config.clone());
        let lang_snapshot = stage0.generate_languages(ctx.files)?;
        let whats_in_pack = self.get_pack_contents_manifest();

        let mut out = String::new();
        out.push_str("========================================\nPROJECT OVERVIEW\n========================================\n\n");
        out.push_str(&format!("Generated: {}\nOutput dir: {}\n\n", now.format("%Y-%m-%d %H:%M:%S %Z"), ctx.pack_dir.display()));
        out.push_str("STATS\n------\n");
        out.push_str(&format!("- files.raw: {}\n- files.kept: {}\n- code_only: {}\n", ctx.raw_count, ctx.filtered_count, self.config.code_only));
        out.push_str(&format!("- max_depth: {}\n\n", self.config.max_depth));

        out.push_str("TOOLS & BUILD SYSTEMS\n----------------------\n");
        out.push_str(&format!("- tools.git: {}\n", ctx.in_git));
        if let Some(commit) = git_commit { out.push_str(&format!("- git.commit: {}\n", commit)); }
        let detected_str: Vec<String> = ctx.detected_systems.iter().map(|s| s.to_string()).collect();
        out.push_str(&format!("- detected_build_systems: [{}]\n\n", detected_str.join(", ")));

        out.push_str("WHAT'S IN THE PACK\n-------------------\n");
        out.push_str(whats_in_pack);
        out.push_str("\n\n");
        out.push_str("LANGUAGE STATISTICS\n========================================\n\n");
        out.push_str(&lang_snapshot);
        Ok(out)
    }

    fn get_git_commit(&self) -> Option<String> {
        Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    }

    fn get_pack_contents_manifest(&self) -> &'static str {
        r#"1. GUIDE.txt               - How to use the pack
2. PROJECT.txt             - Overview, metadata
3. STRUCTURE.txt           - Directory tree, file index, token heatmap
4. APIS.txt                - API surfaces (Rust/TS/Python/Go)
5. DEPS.txt (optional)     - Dependencies (from `cargo tree`)
6. PACK_STAGE2_COMPRESSED.xml (optional) - Compressed skeleton if repomix present"#
    }
}