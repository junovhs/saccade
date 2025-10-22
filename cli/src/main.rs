use anyhow::Result;
use clap::Parser;
use saccade_core::config::{Config, GitMode};
use saccade_core::SaccadePack;
use std::path::{Path, PathBuf};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(name = "saccade")]
#[command(version = VERSION)]
#[command(about = "Generate staged, token-efficient context packs for LLMs", long_about = None)]
struct Cli {
    /// Output directory for the AI pack
    #[arg(short, long, default_value = "ai-pack")]
    out: PathBuf,

    /// Stage-0 overview depth (1..10)
    #[arg(long, default_value = "3")]
    max_depth: usize,

    /// Prefer Git tracked/unignored files (default in Git repos)
    #[arg(long)]
    git_only: bool,

    /// Do not use Git; use find-based enumeration
    #[arg(long)]
    no_git: bool,

    /// Only include paths matching at least one regex (comma-separated)
    #[arg(long, value_name = "PATTERNS")]
    include: Option<String>,

    /// Exclude paths matching any regex (comma-separated)
    #[arg(long, value_name = "PATTERNS")]
    exclude: Option<String>,

    /// Keep only code/config/markup files in Stage-0 lists
    #[arg(long)]
    code_only: bool,

    /// Show stats and what would be generated, then exit
    #[arg(long)]
    dry_run: bool,

    /// Verbose logging
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let mut config = Config::new();

    config.pack_dir = cli.out;
    config.max_depth = cli.max_depth;
    config.code_only = cli.code_only;
    config.dry_run = cli.dry_run;
    config.verbose = cli.verbose;

    if cli.git_only && cli.no_git {
        eprintln!("ERROR: Cannot specify both --git-only and --no-git");
        std::process::exit(1);
    }

    config.git_mode = if cli.git_only {
        GitMode::Yes
    } else if cli.no_git {
        GitMode::No
    } else {
        GitMode::Auto
    };

    if let Some(patterns) = cli.include {
        config.include_patterns = Config::parse_patterns(&patterns)?;
    }
    if let Some(patterns) = cli.exclude {
        config.exclude_patterns = Config::parse_patterns(&patterns)?;
    }

    let pack_dir = config.pack_dir.clone();

    let pack = SaccadePack::new(config);
    pack.generate()?;

    // Output clickable link on Windows for better UX in terminals
    #[cfg(target_os = "windows")]
    {
        if let Ok(abs_path) = std::fs::canonicalize(&pack_dir) {
            println!("\nClick: {}", file_uri(&abs_path));
        }
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn file_uri(path: &Path) -> String {
    // Strip verbatim prefixes like \\?\ and normalize to forward slashes,
    // then percent-encode spaces minimally.
    let mut s = path.display().to_string();
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        s = rest.to_string();
    } else if let Some(rest) = s.strip_prefix(r"\\.\") {
        s = rest.to_string();
    }
    s = s.replace('\\', "/").replace(' ', "%20");
    format!("file:///{}", s)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Only compiles/runs on Windows — validates the URI normalization.
    #[cfg(target_os = "windows")]
    #[test]
    fn file_uri_strips_verbatim_and_normalizes() {
        let p = PathBuf::from(r"\\?\C:\Users\Alice\ai pack");
        let got = file_uri(&p);
        assert_eq!(got, "file:///C:/Users/Alice/ai%20pack");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn file_uri_handles_regular_paths() {
        let p = PathBuf::from(r"C:\tmp\ai-pack");
        let got = file_uri(&p);
        assert_eq!(got, "file:///C:/tmp/ai-pack");
    }
}
