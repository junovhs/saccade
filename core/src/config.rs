// saccade/core/src/config.rs

use crate::error::{Result, SaccadeError};
use regex::Regex;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum GitMode {
    Auto,
    Yes,
    No,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub pack_dir: PathBuf,
    pub max_depth: usize,
    pub git_mode: GitMode,
    pub include_patterns: Vec<Regex>,
    pub exclude_patterns: Vec<Regex>,
    pub code_only: bool,
    pub dry_run: bool,
    pub verbose: bool,
}

impl Config {
    pub fn new() -> Self {
        Self {
            pack_dir: PathBuf::from("ai-pack"),
            max_depth: 3,
            git_mode: GitMode::Auto,
            include_patterns: Vec::new(),
            exclude_patterns: Vec::new(),
            code_only: false,
            dry_run: false,
            verbose: false,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.max_depth < 1 || self.max_depth > 10 {
            return Err(SaccadeError::InvalidConfig {
                field: "max_depth".to_string(),
                value: self.max_depth.to_string(),
                reason: "must be between 1 and 10".to_string(),
            });
        }

        Ok(())
    }

    pub fn parse_patterns(input: &str) -> Result<Vec<Regex>> {
        input
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| Regex::new(s.trim()).map_err(Into::into))
            .collect()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

// Pattern constants
pub const PRUNE_DIRS: &[&str] = &[
    ".git", "node_modules", "dist", "build", "target", "gen", "schemas",
    "tests", "test", "__tests__", ".venv", "venv", ".tox", ".cache",
    "coverage", "vendor", "third_party",
];

pub const BIN_EXT_PATTERN: &str = r"(?i)\.(png|jpe?g|gif|svg|ico|icns|webp|woff2?|ttf|otf|pdf|mp4|mov|mkv|avi|mp3|wav|flac|zip|gz|bz2|xz|7z|rar|jar|csv|tsv|parquet|sqlite|db|bin|exe|dll|so|dylib|pkl|onnx|torch|tgz|zst)$";

pub const SECRET_PATTERN: &str = r"(?i)(^\.?env(\..*)?$|/\.?env(\..*)?$|(^|/)(id_rsa(\.pub)?|id_ed25519(\.pub)?|.*\.(pem|p12|jks|keystore|pfx))$)";

// --- These must be public for the HeuristicFilter ---
pub const CODE_EXT_PATTERN: &str = r"(?i)\.(c|h|cc|hh|cpp|hpp|rs|go|py|js|jsx|ts|tsx|java|kt|kts|rb|php|scala|cs|swift|m|mm|lua|sh|bash|zsh|fish|ps1|sql|html|xhtml|xml|xsd|xslt|yaml|yml|toml|ini|cfg|conf|json|ndjson|md|rst|tex|s|asm|cmake|gradle|proto|graphql|gql|nix|dart|scss|less|css)$";

pub const CODE_BARE_PATTERN: &str = r"(?i)(Makefile|Dockerfile|dockerfile|CMakeLists\.txt|BUILD|WORKSPACE)$";