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
use std::process::Command;

pub struct SaccadePack {
    config: Config,
}

impl SaccadePack {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn generate(&self) -> Result<()> {
        self.config.validate()?;

        // Enumerate files
        eprintln!("==> Enumerating files...");
        let enumerator = FileEnumerator::new(self.config.clone());
        let raw_files = enumerator.enumerate()?;
        let raw_count = raw_files.len();
        eprintln!("    Found {} files (raw)", raw_count);

        // Filter files
        eprintln!(
            "==> Filtering file list (secrets, binaries, includes/excludes, code-only={})...",
            self.config.code_only
        );
        let filter = FileFilter::new(self.config.clone())?;
        let filtered_files = filter.filter(raw_files);
        let filtered_count = filtered_files.len();
        eprintln!("    Kept {} files after filtering", filtered_count);

        // Discovery
        eprintln!("==> Discovering project layout (Rust crates, front-end dirs)...");
        let stage1 = Stage1Generator::new();
        let rust_crates = stage1.find_rust_crates()?;
        let frontend_dirs = stage1.find_frontend_dirs()?;

        // Dry run check
        if self.config.dry_run {
            return self.print_dry_run_stats(filtered_count, &rust_crates, &frontend_dirs);
        }

        // Create pack directory
        fs::create_dir_all(&self.config.pack_dir).map_err(|e| SaccadeError::Io {
            source: e,
            path: self.config.pack_dir.clone(),
        })?;

        eprintln!("==> Generating consolidated pack (5 files max)...");

        // File 1: GUIDE.txt
        let guide_gen = GuideGenerator::new();
        let guide_content = guide_gen.generate_guide()?;
        fs::write(self.config.pack_dir.join("GUIDE.txt"), guide_content).map_err(|e| {
            SaccadeError::Io {
                source: e,
                path: self.config.pack_dir.join("GUIDE.txt"),
            }
        })?;

        // File 2: PROJECT.txt
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
        fs::write(self.config.pack_dir.join("PROJECT.txt"), project_content).map_err(|e| {
            SaccadeError::Io {
                source: e,
                path: self.config.pack_dir.join("PROJECT.txt"),
            }
        })?;

        // File 3: STRUCTURE.txt
        let stage0 = Stage0Generator::new(self.config.clone());
        let structure_content = stage0.generate_combined_structure(&filtered_files)?;
        fs::write(self.config.pack_dir.join("STRUCTURE.txt"), structure_content).map_err(|e| {
            SaccadeError::Io {
                source: e,
                path: self.config.pack_dir.join("STRUCTURE.txt"),
            }
        })?;

        // File 4: APIS.txt
        let apis_content =
            stage1.generate_combined_apis(&rust_crates, &frontend_dirs, &filtered_files)?;
        fs::write(self.config.pack_dir.join("APIS.txt"), apis_content).map_err(|e| {
            SaccadeError::Io {
                source: e,
                path: self.config.pack_dir.join("APIS.txt"),
            }
        })?;

        // File 5: DEPS.txt (optional - now multi-ecosystem)
        let deps_txt = stage1.generate_all_deps()?;
        let has_deps = !deps_txt.trim().is_empty();
        if has_deps {
            fs::write(self.config.pack_dir.join("DEPS.txt"), deps_txt).map_err(|e| {
                SaccadeError::Io {
                    source: e,
                    path: self.config.pack_dir.join("DEPS.txt"),
                }
            })?;
        }

        // Optional Stage 2 (compressed skeleton)
        eprintln!("==> [Stage 2] Generating compressed skeleton with internal parser...");
        let stage2_path = self.config.pack_dir.join("PACK_STAGE2_COMPRESSED.xml");
        // Pass the file list to the generate function.
        // We no longer need the intermediate 'stage2' variable.
        match Stage2Generator::new().generate(&filtered_files, &stage2_path) {
            Ok(Some(msg)) => eprintln!("    {}", msg),
            Ok(None) => eprintln!("    Internal parser returned no message."),
            Err(e) => eprintln!("    WARN: Internal parser failed: {}", e),
        }

        // Print guide (summary lines)
        guide_gen.print_guide(&self.config.pack_dir, has_deps)?;

        Ok(())
    }

    fn print_dry_run_stats(
        &self,
        filtered_count: usize,
        rust_crates: &[std::path::PathBuf],
        frontend_dirs: &[std::path::PathBuf],
    ) -> Result<()> {
        eprintln!("==> [Dry Run] Would generate the following artifacts:");
        eprintln!("  - {} files would be processed", filtered_count);
        eprintln!("  - Output directory: {}", self.config.pack_dir.display());

        // Direct check for git repo instead of relying on enumerate() success
        let in_git = Command::new("git")
            .args(&["rev-parse", "--is-inside-work-tree"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        // Show enumeration mode based on config and git availability
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

        // Our internal parser is always "available"
        eprintln!("  - Saccade's internal parser is available for Stage 2 compression");

        Ok(())
    }
}