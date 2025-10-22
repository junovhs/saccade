use anyhow::Result;
use clap::Parser;
use saccade_core::config::{Config, GitMode};
use saccade_core::SaccadePack;
use std::path::PathBuf;

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

    // Determine Git mode
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

    // Parse include patterns
    if let Some(patterns) = cli.include {
        config.include_patterns = Config::parse_patterns(&patterns)?;
    }

    // Parse exclude patterns
    if let Some(patterns) = cli.exclude {
        config.exclude_patterns = Config::parse_patterns(&patterns)?;
    }

    // Save pack_dir before moving config into pack
    let pack_dir = config.pack_dir.clone();
    
    let pack = SaccadePack::new(config);
    pack.generate()?;

    // Output clickable link on Windows for better UX in terminals
    #[cfg(target_os = "windows")]
    {
        if let Ok(abs_path) = std::fs::canonicalize(&pack_dir) {
            let file_url = format!("file:///{}", abs_path.display().to_string().replace("\\", "/"));
            println!("\nClick: {}", file_url);
        }
    }

    Ok(())
}