// In saccade/core/src/lib.rs

pub mod config;
pub mod enumerate;
pub mod error;
pub mod filter;
pub mod guide;
pub mod manifest;
pub mod parser;
pub mod stage0;
pub mod stage1;
pub mod stage2;

use config::{Config, GitMode};
use enumerate::FileEnumerator;
use error::{Result, SaccadeError};
use filter::FileFilter;
use guide::GuideGenerator;
use manifest::ManifestGenerator;
use stage0::Stage0Generator;
use stage1::Stage1Generator;
use stage2::Stage2Generator;

use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub(crate) const PACK_FILE_NAME: &str = "PACK.txt";

pub struct SaccadePack {
    config: Config,
}

impl SaccadePack {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn generate(&self) -> Result<()> {
        // --- Validation ---
        self.config.validate()?;

        // --- Enumerate files ---
        eprintln!("📂  Enumerating files…");
        let enumerator = FileEnumerator::new(self.config.clone());
        let raw_files = enumerator.enumerate()?;
        let raw_count = raw_files.len();
        eprintln!("    • Found {} files (raw)", raw_count);

        // --- Filter files ---
        eprintln!(
            "🧹  Filtering (secrets, binaries, includes/excludes, code-only={})…",
            self.config.code_only
        );
        let filter = FileFilter::new(self.config.clone())?;
        let filtered_files = filter.filter(raw_files);
        let filtered_count = filtered_files.len();
        eprintln!("    • Kept {} files after filtering", filtered_count);

        // --- Discover layout ---
        eprintln!("🧭  Discovering project layout (Rust crates, frontend dirs)…");
        let stage1 = Stage1Generator::new();
        let rust_crates = stage1.find_rust_crates()?;
        let frontend_dirs = stage1.find_frontend_dirs()?;

        // --- Dry run? ---
        if self.config.dry_run {
            return self.print_dry_run_stats(filtered_count, &rust_crates, &frontend_dirs);
        }

        // --- Ensure output directory ---
        fs::create_dir_all(&self.config.pack_dir).map_err(|e| SaccadeError::Io {
            source: e,
            path: self.config.pack_dir.clone(),
        })?;

        // --- Create sections (strings) ---
        eprintln!("📦  Generating consolidated pack (single file) …");

        // 1) GUIDE (embedded)
        let guide_gen = GuideGenerator::new();
        let guide_content = guide_gen.generate_guide()?;

        // 2) PROJECT
        let in_git = Command::new("git")
            .args(&["rev-parse", "--is-inside-work-tree"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        let manifest_gen = ManifestGenerator::new(self.config.clone());
        let project_content = manifest_gen.generate_project_info(
            /* raw_count   */ raw_count,
            /* filtered    */ filtered_count,
            &self.config.pack_dir,
            in_git,
            &filtered_files,
        )?;

        // 3) STRUCTURE
        let stage0 = Stage0Generator::new(self.config.clone());
        let structure_content = stage0.generate_combined_structure(&filtered_files)?;

        // 4) APIS
        let apis_content =
            stage1.generate_combined_apis(&rust_crates, &frontend_dirs, &filtered_files)?;

        // 5) DEPS (optional)
        let deps_txt = stage1.generate_all_deps()?;
        let has_deps = !deps_txt.trim().is_empty();

        // --- Assemble single PACK.txt with clear markers ---
        let mut combined = String::new();
        combined.push_str("=======PROJECT=======\n");
        combined.push_str(&project_content);
        combined.push_str("\n=======END-OF-PROJECT=======\n\n");

        combined.push_str("=======STRUCTURE=======\n");
        combined.push_str(&structure_content);
        combined.push_str("\n=======END-OF-STRUCTURE=======\n\n");

        combined.push_str("=======APIS=======\n");
        combined.push_str(&apis_content);
        combined.push_str("\n=======END-OF-APIS=======\n\n");

        if has_deps {
            combined.push_str("=======DEPS=======\n");
            combined.push_str(&deps_txt);
            combined.push_str("\n=======END-OF-DEPS=======\n\n");
        }

        combined.push_str("=======GUIDE=======\n");
        combined.push_str(&guide_content);
        combined.push_str("\n=======END-OF-GUIDE=======\n");

        // --- Write the single pack file ---
        let pack_path = self.config.pack_dir.join(PACK_FILE_NAME);
        fs::write(&pack_path, combined).map_err(|e| SaccadeError::Io {
            source: e,
            path: pack_path.clone(),
        })?;

        // --- Optional Stage 2 (compressed skeleton/XML) ---
        eprintln!("🔧  [Stage 2] Generating compressed skeleton with internal parser…");
        let stage2_path = self.config.pack_dir.join("PACK_STAGE2_COMPRESSED.xml");
        match Stage2Generator::new().generate(&filtered_files, &stage2_path) {
            Ok(Some(msg)) => eprintln!("    {}", msg),
            Ok(None) => eprintln!("    Internal parser returned no message."),
            Err(e) => eprintln!("    WARN: Internal parser failed: {}", e),
        }

        // --- Pretty summary ---
        let total_bytes: u64 = filtered_files
            .iter()
            .filter_map(|p| fs::metadata(p).ok().map(|m| m.len()))
            .sum();
        let est_tokens = (total_bytes as f64 / 3.5) as u64;

        eprintln!();
        eprintln!("📊 Pack Summary");
        eprintln!("────────────────────────────────");
        eprintln!("  Output File : {}", pack_path.display());
        eprintln!("  Files Kept  : {} files", filtered_count);
        eprintln!(
            "  Size (est.) : {} bytes  (~{} tokens)",
            total_bytes, est_tokens
        );
        eprintln!("  Security    : ✔ Secrets & obvious binaries filtered");
        eprintln!("  Stage-2 XML : {}", stage2_path.display());
        eprintln!("────────────────────────────────\n");

        // CLI-friendly footer
        guide_gen.print_guide(&self.config.pack_dir, has_deps)?;

        Ok(())
    }

    fn print_dry_run_stats(
        &self,
        filtered_count: usize,
        rust_crates: &[PathBuf],
        frontend_dirs: &[PathBuf],
    ) -> Result<()> {
        eprintln!("==> [Dry Run] Would generate the following artifacts:");
        eprintln!("  - {} files would be processed", filtered_count);
        eprintln!("  - Output directory: {}", self.config.pack_dir.display());

        // Check for git repo
        let in_git = Command::new("git")
            .args(&["rev-parse", "--is-inside-work-tree"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        match self.config.git_mode {
            GitMode::Yes => eprintln!("  - Using Git file enumeration (forced)"),
            GitMode::No => eprintln!("  - Using find-based file enumeration (forced)"),
            GitMode::Auto => {
                if in_git {
                    eprintln!("  - Using Git file enumeration (auto-detected)");
                } else {
                    eprintln!("  - Using find-based file enumeration (no git repo)");
                }
            }
        }

        eprintln!("  - Found {} Rust crate(s)", rust_crates.len());
        eprintln!("  - Found {} frontend dir(s)", frontend_dirs.len());
        eprintln!(
            "  - Would produce: ai-pack/{} (single file) + PACK_STAGE2_COMPRESSED.xml",
            PACK_FILE_NAME
        );

        Ok(())
    }
}
