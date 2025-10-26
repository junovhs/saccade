// saccade/cli/src/main.rs

use anyhow::Result;
use clap::Parser;
use saccade_core::config::{Config, GitMode};
use saccade_core::SaccadePack;
use std::path::PathBuf;

#[cfg(target_os = "windows")]
use std::path::Path;

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
    config.pack_dir = cli.out.clone(); // Clone here for config, cli.out remains available
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

    let pack = SaccadePack::new(config);
    pack.generate()?;

    // ✅ Windows-only clickable file:// link.
    // Use `cli.out` directly, which is still in scope. This is the "minimal scope" solution.
    #[cfg(target_os = "windows")]
    {
        if let Ok(abs_path) = std::fs::canonicalize(&cli.out) {
            println!("\nClick: {}", file_uri(&abs_path));
        }
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn file_uri(path: &Path) -> String {
    use std::path::Component;
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => {
                let s = prefix.as_os_str().to_string_lossy();
                components.push(s.trim_end_matches(':').to_string());
            }
            Component::RootDir => continue,
            Component::Normal(part) => {
                components.push(percent_encode_path_segment(&part.to_string_lossy()));
            }
            _ => continue,
        }
    }
    format!("file:///{}", components.join("/"))
}

#[cfg(target_os = "windows")]
fn percent_encode_path_segment(segment: &str) -> String {
    segment
        .chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            ' ' => "%20".to_string(),
            '#' => "%23".to_string(),
            '%' => "%25".to_string(),
            '&' => "%26".to_string(),
            '?' => "%3F".to_string(),
            '[' => "%5B".to_string(),
            ']' => "%5D".to_string(),
            _ if c.is_ascii() => format!("%{:02X}", c as u8),
            _ => c.to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "windows")]
    #[test]
    fn file_uri_strips_verbatim_and_normalizes() {
        let p = std::path::PathBuf::from(r"\\?\C:\Users\Alice\ai pack");
        let got = super::file_uri(&p);
        assert_eq!(got, "file:///C:/Users/Alice/ai%20pack");
    }
}