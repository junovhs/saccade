// In saccade/core/src/lib.rs

pub mod config;
pub mod enumerate;
pub mod error;
pub mod filter;
pub mod guide;
pub mod heuristics;
pub mod manifest;
pub mod parser;
pub mod request;
pub mod stage0;
pub mod stage1;
pub mod stage2;

use config::Config; // <--- MODIFIED: Removed unused 'GitMode'
use enumerate::FileEnumerator;
use error::{Result, SaccadeError};
use filter::FileFilter;
use guide::GuideGenerator;
use heuristics::HeuristicFilter;
use manifest::{ManifestGenerator, ProjectInfoContext};
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

struct PackContent {
    project: String,
    structure: String,
    apis: String,
    deps: String,
    guide: String,
}

impl SaccadePack {
    pub fn new(config: Config) -> Self { Self { config } }

    pub fn generate(&self) -> Result<()> {
        self.config.validate()?;
        let (raw_count, filtered_files) = self.enumerate_and_filter_files()?;
        let stage1 = Stage1Generator::new();
        let rust_crates = stage1.find_rust_crates()?;
        let frontend_dirs = stage1.find_frontend_dirs()?;

        if self.config.dry_run {
            return self.print_dry_run_stats(filtered_files.len(), &rust_crates, &frontend_dirs);
        }

        self.prepare_output_directory()?;
        let pack_content = self.generate_pack_content(raw_count, &filtered_files, &rust_crates, &frontend_dirs)?;
        self.write_pack_file(&pack_content, &filtered_files)?;

        // --- MODIFIED: Handle Stage 2 failure immediately and loudly ---
        let stage2_result = self.generate_stage2(&filtered_files);
        if let Err(e) = &stage2_result {
            eprintln!("    WARN: Internal parser failed: {}", e);
        }
        // --- End Modification ---

        self.print_summary(&filtered_files, !pack_content.deps.is_empty(), &stage2_result)?;
        Ok(())
    }

    fn enumerate_and_filter_files(&self) -> Result<(usize, Vec<PathBuf>)> {
        eprintln!("📂  Enumerating files…");
        let enumerator = FileEnumerator::new(self.config.clone());
        let raw_files = enumerator.enumerate()?;
        let raw_count = raw_files.len();
        eprintln!("    • Found {} files (raw)", raw_count);

        eprintln!("🔬  [Layer 1] Applying heuristic filters (entropy, content patterns)…");
        let heuristic_files = HeuristicFilter::new().filter(raw_files);
        eprintln!("    • Kept {} files after heuristic pre-filtering", heuristic_files.len());

        eprintln!("🧹  Filtering (secrets, binaries, includes/excludes, code-only={})…", self.config.code_only);
        let filter = FileFilter::new(self.config.clone())?;
        let filtered_files = filter.filter(heuristic_files);
        eprintln!("    • Kept {} files after final filtering", filtered_files.len());
        Ok((raw_count, filtered_files))
    }

    fn prepare_output_directory(&self) -> Result<()> {
        fs::create_dir_all(&self.config.pack_dir).map_err(|e| SaccadeError::Io {
            source: e,
            path: self.config.pack_dir.clone(),
        })
    }

    fn generate_pack_content(&self, raw_count: usize, files: &[PathBuf], rust_crates: &[PathBuf], frontend_dirs: &[PathBuf]) -> Result<PackContent> {
        eprintln!("📦  Generating consolidated pack content…");
        let info_ctx = ProjectInfoContext { raw_count, filtered_count: files.len(), pack_dir: &self.config.pack_dir, in_git: is_in_git_repo(), files };
        Ok(PackContent {
            project: ManifestGenerator::new(self.config.clone()).generate_project_info(&info_ctx)?,
            structure: Stage0Generator::new(self.config.clone()).generate_combined_structure(files)?,
            apis: Stage1Generator::new().generate_combined_apis(rust_crates, frontend_dirs, files)?,
            deps: Stage1Generator::new().generate_all_deps()?,
            guide: GuideGenerator::new().generate_guide()?,
        })
    }

    fn write_pack_file(&self, content: &PackContent, _filtered_files: &[PathBuf]) -> Result<()> {
        let mut combined = format!("=======PROJECT=======\n{}\n=======END-OF-PROJECT=======\n\n", content.project);
        combined.push_str(&format!("=======STRUCTURE=======\n{}\n=======END-OF-STRUCTURE=======\n\n", content.structure));
        combined.push_str(&format!("=======APIS=======\n{}\n=======END-OF-APIS=======\n\n", content.apis));
        if !content.deps.trim().is_empty() {
            combined.push_str(&format!("=======DEPS=======\n{}\n=======END-OF-DEPS=======\n\n", content.deps));
        }
        combined.push_str(&format!("=======GUIDE=======\n{}\n=======END-OF-GUIDE=======\n", content.guide));
        let pack_path = self.config.pack_dir.join(PACK_FILE_NAME);
        fs::write(&pack_path, combined).map_err(|e| SaccadeError::Io { source: e, path: pack_path })
    }

    fn generate_stage2(&self, filtered_files: &[PathBuf]) -> Result<Option<String>> {
        eprintln!("🔧  [Stage 2] Generating compressed skeleton with internal parser…");
        let stage2_path = self.config.pack_dir.join("PACK_STAGE2_COMPRESSED.xml");
        Stage2Generator::new().with_verbose(self.config.verbose).generate(filtered_files, &stage2_path)
    }

    fn print_summary(&self, filtered_files: &[PathBuf], has_deps: bool, stage2_result: &Result<Option<String>>) -> Result<()> {
        let total_bytes: u64 = filtered_files.iter().filter_map(|p| fs::metadata(p).ok().map(|m| m.len())).sum();
        eprintln!("\n📊 Pack Summary\n────────────────────────────────");
        eprintln!("  Output File : {}", self.config.pack_dir.join(PACK_FILE_NAME).display());
        eprintln!("  Files Kept  : {} files", filtered_files.len());
        eprintln!("  Size (est.) : {} bytes  (~{} tokens)", total_bytes, (total_bytes as f64 / 3.5) as u64);
        eprintln!("  Security    : ✔ Secrets & obvious binaries filtered");

        match stage2_result {
            Ok(_) => eprintln!("  Stage-2 XML : {}", self.config.pack_dir.join("PACK_STAGE2_COMPRESSED.xml").display()),
            Err(e) => eprintln!("  Stage-2 XML : FAILED ({})", e),
        }
        eprintln!("────────────────────────────────\n");

        if stage2_result.is_ok() {
            GuideGenerator::new().print_guide(&self.config.pack_dir, has_deps)?;
        } else {
            eprintln!("🟡 Partial Success. PACK.txt was generated, but Stage-2 skeletonization failed.");
            eprintln!("   The `WARN` message above contains the specific error.");
            eprintln!("   In: {}\n", dunce::canonicalize(&self.config.pack_dir)?.display());
        }
        Ok(())
    }

    fn print_dry_run_stats(&self, filtered_count: usize, rust_crates: &[PathBuf], frontend_dirs: &[PathBuf]) -> Result<()> {
        eprintln!("==> [Dry Run] Would generate the following artifacts:");
        eprintln!("  - {} files would be processed", filtered_count);
        eprintln!("  - Output directory: {}", self.config.pack_dir.display());
        
        // --- MODIFIED: Use the variables to prevent warnings ---
        eprintln!("  - Found {} Rust crate(s)", rust_crates.len());
        eprintln!("  - Found {} frontend dir(s)", frontend_dirs.len());
        // --- End Modification ---

        eprintln!("  - Would produce: ai-pack/{} (single file) + PACK_STAGE2_COMPRESSED.xml", PACK_FILE_NAME);
        Ok(())
    }
}

fn is_in_git_repo() -> bool {
    Command::new("git").args(["rev-parse", "--is-inside-work-tree"]).output().map(|o| o.status.success()).unwrap_or(false)
}