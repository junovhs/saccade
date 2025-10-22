pub mod config;
pub mod enumerate;
pub mod error;
pub mod filter;
pub mod guide;
pub mod manifest;
pub mod stage0;
pub mod stage1;
pub mod stage2;

use config::Config;
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

        // Generate static helper files
        eprintln!("==> Writing helper files...");
        let guide = GuideGenerator::new();
        guide
            .generate_static_files(&self.config.pack_dir)
            .map_err(|e| SaccadeError::Io {
                source: e,
                path: self.config.pack_dir.clone(),
            })?;

        // Stage 0
        eprintln!("==> [Stage 0] Generating STRUCTURE.txt and TOKENS.txt...");
        let stage0 = Stage0Generator::new(self.config.clone());

        let structure = stage0.generate_structure(&filtered_files)?;
        let structure_path = self.config.pack_dir.join("STRUCTURE.txt");
        fs::write(&structure_path, structure).map_err(|e| SaccadeError::Io {
            source: e,
            path: structure_path.clone(),
        })?;

        let tokens = stage0.generate_tokens(&filtered_files)?;
        let tokens_path = self.config.pack_dir.join("TOKENS.txt");
        fs::write(&tokens_path, tokens).map_err(|e| SaccadeError::Io {
            source: e,
            path: tokens_path.clone(),
        })?;

        let file_index = stage0.generate_file_index(&filtered_files)?;
        let file_index_path = self.config.pack_dir.join("FILE_INDEX.txt");
        fs::write(&file_index_path, &file_index).map_err(|e| SaccadeError::Io {
            source: e,
            path: file_index_path.clone(),
        })?;

        let languages = stage0.generate_languages(&filtered_files)?;
        let languages_path = self.config.pack_dir.join("LANGUAGES.md");
        fs::write(&languages_path, languages).map_err(|e| SaccadeError::Io {
            source: e,
            path: languages_path.clone(),
        })?;

        // Stage 1
        eprintln!("==> [Stage 1] Generating dependency and API surfaces...");
        eprintln!("    Processing {} files for API surfaces...", filtered_count);

        let cargo_tree = stage1.generate_cargo_tree()?;
        if !cargo_tree.is_empty() {
            let ct_path = self.config.pack_dir.join("CARGO_TREE_DEDUP.txt");
            fs::write(&ct_path, cargo_tree).map_err(|e| SaccadeError::Io {
                source: e,
                path: ct_path.clone(),
            })?;
        }

        let rust_api = stage1.extract_rust_api(&rust_crates, &filtered_files)?;
        let rust_api_path = self.config.pack_dir.join("API_SURFACE_RUST.txt");
        fs::write(&rust_api_path, rust_api).map_err(|e| SaccadeError::Io {
            source: e,
            path: rust_api_path.clone(),
        })?;

        let ts_api = stage1.extract_ts_api(&frontend_dirs, &filtered_files)?;
        let ts_api_path = self.config.pack_dir.join("API_SURFACE_TS.txt");
        fs::write(&ts_api_path, ts_api).map_err(|e| SaccadeError::Io {
            source: e,
            path: ts_api_path.clone(),
        })?;

        let py_api = stage1.extract_python_api(&filtered_files)?;
        let py_api_path = self.config.pack_dir.join("API_SURFACE_PYTHON.txt");
        fs::write(&py_api_path, py_api).map_err(|e| SaccadeError::Io {
            source: e,
            path: py_api_path.clone(),
        })?;

        let go_api = stage1.extract_go_api(&filtered_files)?;
        let go_api_path = self.config.pack_dir.join("API_SURFACE_GO.txt");
        fs::write(&go_api_path, go_api).map_err(|e| SaccadeError::Io {
            source: e,
            path: go_api_path.clone(),
        })?;

        // Stage 2
        eprintln!("==> [Stage 2] Generating compressed skeleton (if repomix is available)...");
        let stage2 = Stage2Generator::new();
        let stage2_path = self.config.pack_dir.join("PACK_STAGE2_COMPRESSED.xml");
        match stage2.generate(&rust_crates, &frontend_dirs, &stage2_path) {
            Ok(Some(msg)) => eprintln!("    {}", msg),
            Ok(None) => eprintln!("    repomix not found; skipping compressed skeleton"),
            Err(e) => eprintln!("    WARN: repomix failed: {}", e),
        }

        // Manifest
        eprintln!("==> Writing PACK_MANIFEST.json...");
        let manifest_gen = ManifestGenerator::new(self.config.clone());
        // Cheap check for "in_git" (non-fatal)
        let in_git = FileEnumerator::new(self.config.clone()).enumerate().is_ok();
        let manifest_json =
            manifest_gen.generate(raw_count, filtered_count, &self.config.pack_dir, in_git)?;
        let manifest_path = self.config.pack_dir.join("PACK_MANIFEST.json");
        fs::write(&manifest_path, manifest_json).map_err(|e| SaccadeError::Io {
            source: e,
            path: manifest_path.clone(),
        })?;

        // Determine which artifacts to upload
        let mut upload_artifacts = vec![
            "OVERVIEW.md",
            "STRUCTURE.txt",
            "TOKENS.txt",
            "REQUEST_PROTOCOL.md",
        ];

        for artifact in &[
            "API_SURFACE_RUST.txt",
            "API_SURFACE_TS.txt",
            "API_SURFACE_PYTHON.txt",
            "API_SURFACE_GO.txt",
            "CARGO_TREE_DEDUP.txt",
        ] {
            let path = self.config.pack_dir.join(artifact);
            if path.exists() && fs::metadata(&path).map(|m| m.len() > 0).unwrap_or(false) {
                upload_artifacts.push(artifact);
            }
        }

        // Generate CHAT_START.md
        GuideGenerator::new()
            .generate_chat_start(&self.config.pack_dir, &upload_artifacts)
            .map_err(|e| SaccadeError::Io {
                source: e,
                path: self.config.pack_dir.join("CHAT_START.md"),
            })?;

        // Print post-pack guide
        GuideGenerator::new().print_guide(&self.config.pack_dir, &upload_artifacts);

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

        let in_git = FileEnumerator::new(self.config.clone()).enumerate().is_ok();
        if in_git {
            eprintln!("  - Using Git file enumeration");
        } else {
            eprintln!("  - Using find-based file enumeration");
        }

        eprintln!("  - Found {} Rust crate(s)", rust_crates.len());
        eprintln!("  - Found {} frontend dir(s)", frontend_dirs.len());

        let stage2 = Stage2Generator::new();
        if stage2.has_repomix() {
            eprintln!("  - Repomix available for Stage 2 compression");
        } else {
            eprintln!("  - Repomix NOT available (Stage 2 skipped)");
        }

        Ok(())
    }
}
