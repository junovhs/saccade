use crate::config::{Config, PRUNE_DIRS};
use crate::error::{Result, SaccadeError};
use std::path::PathBuf;
use std::process::Command;
use walkdir::WalkDir;

pub struct FileEnumerator {
    config: Config,
}

impl FileEnumerator {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn enumerate(&self) -> Result<Vec<PathBuf>> {
        use crate::config::GitMode;

        match self.config.git_mode {
            GitMode::Yes => {
                // Force Git mode
                if !self.in_git_repo()? {
                    return Err(SaccadeError::NotInGitRepo);
                }
                self.git_ls_files()
            }
            GitMode::No => {
                // Force find mode
                self.walk_all_files()
            }
            GitMode::Auto => {
                // Prefer Git when available and inside a repo; otherwise fallback to WalkDir
                if self.in_git_repo()? {
                    if let Ok(files) = self.git_ls_files() {
                        return Ok(files);
                    }
                }
                self.walk_all_files()
            }
        }
    }

    fn in_git_repo(&self) -> Result<bool> {
        let out = Command::new("git")
            .arg("rev-parse")
            .arg("--is-inside-work-tree")
            .output(); // io::Error -> SaccadeError via From

        match out {
            Ok(o) if o.status.success() => Ok(true),
            _ => Ok(false),
        }
    }

    fn git_ls_files(&self) -> Result<Vec<PathBuf>> {
        let out = Command::new("git")
            .arg("ls-files")
            .arg("-z")
            .arg("--exclude-standard")
            .output()?; // io::Error -> SaccadeError

        if !out.status.success() {
            return Err(SaccadeError::Other(format!(
                "git ls-files failed: exit {}",
                out.status
            )));
        }

        let mut paths = Vec::new();
        for chunk in out.stdout.split(|b| *b == 0) {
            if chunk.is_empty() {
                continue;
            }
            let s = String::from_utf8_lossy(chunk);
            paths.push(PathBuf::from(s.as_ref()));
        }
        Ok(paths)
    }

    fn walk_all_files(&self) -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();
        let mut errors = Vec::new();

        let walker = WalkDir::new(".").follow_links(false).into_iter();

        for item in walker.filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !PRUNE_DIRS.iter().any(|p| name == *p)
        }) {
            let entry = match item {
                Ok(e) => e,
                Err(e) => {
                    // Collect error but continue walking (graceful degradation)
                    errors.push(format!("walkdir: {}", e));
                    continue;
                }
            };

            if entry.file_type().is_file() {
                // Store path relative to CWD
                let p = entry.path().strip_prefix(".").unwrap_or(entry.path());
                paths.push(p.to_path_buf());
            }
        }

        // Report errors if verbose mode is enabled
        if !errors.is_empty() && self.config.verbose {
            eprintln!("WARN: Encountered {} errors during file walk:", errors.len());
            for (i, err) in errors.iter().take(5).enumerate() {
                eprintln!("  {}. {}", i + 1, err);
            }
            if errors.len() > 5 {
                eprintln!("  ... and {} more", errors.len() - 5);
            }
        }

        Ok(paths)
    }
}