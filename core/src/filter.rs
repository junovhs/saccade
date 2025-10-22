use crate::config::{Config, BIN_EXT_PATTERN, CODE_BARE_PATTERN, CODE_EXT_PATTERN, SECRET_PATTERN};
use crate::error::Result;
use regex::Regex;
use std::path::Path;

pub struct FileFilter {
    config: Config,
    bin_ext_re: Regex,
    secret_re: Regex,
    code_ext_re: Option<Regex>,
    code_bare_re: Option<Regex>,
}

impl FileFilter {
    pub fn new(config: Config) -> Result<Self> {
        let bin_ext_re = Regex::new(BIN_EXT_PATTERN)?;
        let secret_re = Regex::new(SECRET_PATTERN)?;

        let (code_ext_re, code_bare_re) = if config.code_only {
            (
                Some(Regex::new(CODE_EXT_PATTERN)?),
                Some(Regex::new(CODE_BARE_PATTERN)?),
            )
        } else {
            (None, None)
        };

        Ok(Self {
            config,
            bin_ext_re,
            secret_re,
            code_ext_re,
            code_bare_re,
        })
    }

    pub fn filter(&self, files: Vec<std::path::PathBuf>) -> Vec<std::path::PathBuf> {
        files.into_iter().filter(|p| self.should_keep(p)).collect()
    }

    fn should_keep(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        // Secrets check
        if self.secret_re.is_match(&path_str) {
            return false;
        }

        // Binary extensions check
        if self.bin_ext_re.is_match(&path_str) {
            return false;
        }

        // Exclude patterns
        for pattern in &self.config.exclude_patterns {
            if pattern.is_match(&path_str) {
                return false;
            }
        }

        // Include patterns (if any)
        if !self.config.include_patterns.is_empty() {
            let mut matched = false;
            for pattern in &self.config.include_patterns {
                if pattern.is_match(&path_str) {
                    matched = true;
                    break;
                }
            }
            if !matched {
                return false;
            }
        }

        // Code-only mode: keep if (code extension) OR (known bare build file).
        if let (Some(ext_re), Some(bare_re)) = (&self.code_ext_re, &self.code_bare_re) {
            if !(ext_re.is_match(&path_str) || bare_re.is_match(&path_str)) {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn cfg_code_only() -> Config {
        let mut c = Config::new();
        c.code_only = true;
        c
    }

    #[test]
    fn code_only_keeps_rust_and_bare_build_files() {
        let f = FileFilter::new(cfg_code_only()).unwrap();

        // ext match (.rs) should be kept
        assert!(f.should_keep(Path::new("src/lib.rs")));

        // bare build file (Makefile) should be kept even w/o extension
        assert!(f.should_keep(Path::new("Makefile")));
    }

    #[test]
    fn code_only_rejects_non_code_misc() {
        let f = FileFilter::new(cfg_code_only()).unwrap();

        // Something obviously not code or a bare build file
        assert!(!f.should_keep(Path::new("notes.randomdata")));
    }

    #[test]
    fn filter_applies_to_list() {
        let f = FileFilter::new(cfg_code_only()).unwrap();
        let files = vec![
            PathBuf::from("src/main.rs"),
            PathBuf::from("Makefile"),
            PathBuf::from("assets/logo.png"),
        ];
        let kept = f.filter(files);
        // Expect 2 kept: rs + Makefile; png dropped by binary pattern
        assert_eq!(kept.len(), 2);
    }
}
