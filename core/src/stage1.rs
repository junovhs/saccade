// saccade/core/src/stage1.rs

use crate::error::Result;
use once_cell::sync::Lazy;
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// === Dependency output budgets (visible, enforceable) =====================
const DEPS_SECTION_MAX_LINES: usize = 300;
const DEPS_SECTION_MAX_BYTES: usize = 128 * 1024; // 128 KiB
const DEPS_JOINER: &str = "\n\n----------------------------------------\n";
const INCLUDE_CARGO_METADATA: bool = false; // OFF by default (too noisy)

// Pre-compiled regex for scrubbing sensitive info.
// Using Lazy from once_cell avoids panics from unwrap() on invalid patterns.
static EMAIL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").unwrap());
static REGISTRY_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"index\.crates\.io-[^\s/\\]+[\\/]").unwrap());

pub struct Stage1Generator;

impl Stage1Generator {
    pub fn new() -> Self {
        Self
    }

    // ---------------------------------------------------------------------
    // API SURFACE (unchanged behavior)
    // ---------------------------------------------------------------------

    pub fn generate_combined_apis(
        &self,
        rust_crates: &[PathBuf],
        frontend_dirs: &[PathBuf],
        file_index: &[PathBuf],
    ) -> Result<String> {
        let mut output = String::new();

        output.push_str("========================================\n");
        output.push_str("API SURFACE: RUST\n");
        output.push_str("========================================\n\n");
        output.push_str(&self.extract_rust_api(rust_crates, file_index)?);

        output.push_str("\n========================================\n");
        output.push_str("API SURFACE: TYPESCRIPT/JAVASCRIPT\n");
        output.push_str("========================================\n\n");
        output.push_str(&self.extract_ts_api(frontend_dirs, file_index)?);

        output.push_str("\n========================================\n");
        output.push_str("API SURFACE: PYTHON\n");
        output.push_str("========================================\n\n");
        output.push_str(&self.extract_python_api(file_index)?);

        output.push_str("\n========================================\n");
        output.push_str("API SURFACE: GO\n");
        output.push_str("========================================\n\n");
        output.push_str(&self.extract_go_api(file_index)?);

        Ok(output)
    }

    pub fn find_rust_crates(&self) -> Result<Vec<PathBuf>> {
        let mut crates = Vec::new();
        for entry in walkdir::WalkDir::new(".")
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_name() == "Cargo.toml" {
                let p = entry.path().to_string_lossy();
                if p.contains("/target/") || p.contains("\\target\\") {
                    continue;
                }
                if let Some(parent) = entry.path().parent() {
                    let src_dir = parent.join("src");
                    if src_dir.exists() && src_dir.is_dir() {
                        crates.push(
                            src_dir
                                .strip_prefix(".")
                                .unwrap_or(&src_dir)
                                .to_path_buf(),
                        );
                    }
                }
            }
        }
        Ok(crates)
    }

    pub fn find_frontend_dirs(&self) -> Result<Vec<PathBuf>> {
        let mut dirs = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for entry in walkdir::WalkDir::new(".")
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                let name = e.file_name().to_string_lossy();
                !matches!(name.as_ref(), "node_modules" | "dist" | "build" | ".git")
            })
            .filter_map(|e| e.ok())
        {
            if entry.file_name() == "package.json" {
                if let Some(parent) = entry.path().parent() {
                    let normalized = parent.strip_prefix(".").unwrap_or(parent).to_path_buf();
                    if seen.insert(normalized.clone()) {
                        dirs.push(normalized);
                    }
                }
            }
        }
        if dirs.is_empty() {
            for name in &["app", "frontend", "web", "client", "ui", "src"] {
                let path = PathBuf::from(name);
                if path.exists() && path.is_dir() {
                    dirs.push(path);
                    break;
                }
            }
        }
        Ok(dirs)
    }

    // ---------------------------------------------------------------------
    // DEPENDENCIES (compact, multi-ecosystem, redacted, bounded)
    // ---------------------------------------------------------------------

    pub fn generate_all_deps(&self) -> Result<String> {
        let mut sections: Vec<String> = Vec::new();
        if Path::new("Cargo.toml").exists() {
            sections.push(self.deps_rust());
        }
        if Path::new("package.json").exists() {
            sections.push(self.deps_node());
        }
        if ["pyproject.toml", "requirements.txt", "Pipfile", "poetry.lock"]
            .iter()
            .any(|f| Path::new(f).exists())
        {
            sections.push(self.deps_python());
        }
        if Path::new("go.mod").exists() {
            sections.push(self.deps_go());
        }
        if sections.is_empty() {
            return Ok(String::new());
        }
        let mut out = String::new();
        out.push_str("========================================\n");
        out.push_str("DEPENDENCIES (multi-ecosystem, summarized)\n");
        out.push_str("========================================\n\n");
        out.push_str(&sections.join(DEPS_JOINER));
        out.push('\n');
        Ok(out)
    }

    fn deps_rust(&self) -> String {
        let mut parts: Vec<String> = vec!["RUST (cargo)".to_string(), "Tools: cargo tree".to_string()];
        if let Some(s) = run_and_capture("cargo", &["tree", "-d"]) {
            parts.push(format!("cargo tree -d (duplicates)\n{}\n", clamp_and_scrub(&s, "cargo tree -d")));
        } else {
            parts.push(warn_tool_missing("cargo tree -d"));
        }
        if let Some(s) = run_and_capture("cargo", &["tree", "-e", "normal,build", "--depth", "2"]) {
            parts.push(format!("cargo tree -e normal,build --depth 2\n{}\n", clamp_and_scrub(&s, "cargo tree -e normal,build --depth 2")));
        }
        if INCLUDE_CARGO_METADATA {
            if let Some(s) = run_and_capture("cargo", &["metadata", "--format-version", "1"]) {
                parts.push(format!("cargo metadata --format-version 1 (truncated)\n{}\n", clamp_and_scrub(&s, "cargo metadata")));
            }
        }
        parts.join("\n")
    }

    fn deps_node(&self) -> String {
        let mut parts: Vec<String> = vec!["NODE (npm/pnpm/yarn)".to_string()];
        let tool = if tool_exists("npm") { Some("npm") } else if tool_exists("pnpm") { Some("pnpm") } else if tool_exists("yarn") { Some("yarn") } else { None };
        if let Some(tool_name) = tool {
            parts.push(format!("Tool: {}", tool_name));
            let (cmd, args) = match tool_name {
                "pnpm" => ("pnpm", vec!["list", "--depth", "2"]),
                "yarn" => ("yarn", vec!["list", "--depth=2"]),
                _ => ("npm", vec!["ls", "--depth", "2"]),
            };
            if let Some(s) = run_collect_any_status(cmd, &args) {
                parts.push(format!("{} {}\n{}\n", cmd, args.join(" "), clamp_and_scrub(&s, &format!("{} {}", cmd, args.join(" ")))));
            } else {
                parts.push(warn_tool_failed(&format!("{} {}", cmd, args.join(" "))));
            }
        } else {
            parts.push(warn_tool_missing("npm|pnpm|yarn"));
        }
        parts.join("\n")
    }

    fn deps_python(&self) -> String {
        let mut parts: Vec<String> = vec!["PYTHON (pip/poetry)".to_string()];
        if tool_exists("pipdeptree") {
            if let Some(s) = run_collect_any_status("pipdeptree", &["--json-tree", "-w", "silence"]) {
                parts.push(format!("pipdeptree --json-tree -w silence (truncated)\n{}\n", clamp_and_scrub(&s, "pipdeptree --json-tree")));
                return parts.join("\n");
            }
        }
        if let Ok(s) = fs::read_to_string("poetry.lock") {
            parts.push("(poetry.lock present; head)".to_string());
            parts.push(clamp_and_scrub(&s, "poetry.lock"));
            return parts.join("\n");
        }
        for name in &["requirements.txt", "requirements-dev.txt"] {
            if let Ok(s) = fs::read_to_string(name) {
                parts.push(format!("({} present; head)", name));
                parts.push(clamp_and_scrub(&s, name));
                return parts.join("\n");
            }
        }
        if tool_exists("pip") {
            if let Some(s) = run_collect_any_status("pip", &["list"]) {
                parts.push(format!("pip list\n{}\n", clamp_and_scrub(&s, "pip list")));
                return parts.join("\n");
            }
        }
        parts.push(warn_tool_missing("pipdeptree|poetry.lock|requirements*.txt|pip"));
        parts.join("\n")
    }

    fn deps_go(&self) -> String {
        let mut parts: Vec<String> = vec!["GO (modules)".to_string()];
        if tool_exists("go") {
            if let Some(s) = run_collect_any_status("go", &["version"]) {
                parts.push(scrub(s.trim()));
            }
            if let Some(s) = run_collect_any_status("go", &["mod", "graph"]) {
                parts.push(format!("go mod graph\n{}\n", clamp_and_scrub(&s, "go mod graph")));
            } else {
                parts.push(warn_tool_failed("go mod graph"));
            }
        } else {
            parts.push(warn_tool_missing("go"));
        }
        parts.join("\n")
    }

    // ---------------------------------------------------------------------
    // API extraction helpers
    // ---------------------------------------------------------------------

    fn extract_rust_api(&self, crates: &[PathBuf], file_index: &[PathBuf]) -> Result<String> {
        if crates.is_empty() { return Ok("(no Rust crates found)\n".to_string()); }
        let pattern = Regex::new(r"pub(\s+|\s*\([^)]*\)\s+)(fn|struct|enum|trait|type|const|static|use|mod|macro_rules!)")?;
        let mut output = String::new();
        for crate_dir in crates {
            let crate_str = crate_dir.to_string_lossy().replace('\\', "/");
            for file_path in file_index {
                let file_str = file_path.to_string_lossy().replace('\\', "/");
                if file_str.starts_with(&*crate_str) && file_str.ends_with(".rs") {
                    if let Ok(content) = fs::read_to_string(file_path) {
                        for (line_num, line) in content.lines().enumerate() {
                            if pattern.is_match(line) {
                                output.push_str(&format!("{}:{}:{}\n", file_str, line_num + 1, line));
                            }
                        }
                    }
                }
            }
        }
        if output.is_empty() { output = "(no public Rust items found)\n".to_string(); }
        Ok(output)
    }

    fn extract_ts_api(&self, frontend_dirs: &[PathBuf], file_index: &[PathBuf]) -> Result<String> {
        if frontend_dirs.is_empty() { return Ok("(no frontend dirs found)\n".to_string()); }
        let pattern = Regex::new(r"^(\s*export\s+(default\s+)?(function|class|interface|type|enum|const|let|var|async|function\*)|\s*(function|class)\s+[A-Z])")?;
        let mut output = String::new();
        for frontend_dir in frontend_dirs {
            let dir_str = frontend_dir.to_string_lossy().replace('\\', "/");
            for file_path in file_index {
                let file_str = file_path.to_string_lossy().replace('\\', "/");
                if file_str.starts_with(&*dir_str) && (file_str.ends_with(".js") || file_str.ends_with(".jsx") || file_str.ends_with(".ts") || file_str.ends_with(".tsx") || file_str.ends_with(".mjs") || file_str.ends_with(".cjs")) && !file_str.ends_with(".d.ts") {
                    if let Ok(content) = fs::read_to_string(file_path) {
                        for (line_num, line) in content.lines().enumerate() {
                            if pattern.is_match(line) {
                                output.push_str(&format!("{}:{}:{}\n", file_str, line_num + 1, line));
                            }
                        }
                    }
                }
            }
        }
        if output.is_empty() { output = "(no TS/JS items found)\n".to_string(); }
        Ok(output)
    }

    fn extract_python_api(&self, file_index: &[PathBuf]) -> Result<String> {
        let pattern = Regex::new(r"^\s*(def|class)\s+([A-Za-z][A-Za-z0-9_]*)")?;
        let mut output = String::new();
        for file_path in file_index {
            if file_path.extension().map_or(false, |e| e == "py") {
                if let Ok(content) = fs::read_to_string(file_path) {
                    for (line_num, line) in content.lines().enumerate() {
                        if let Some(caps) = pattern.captures(line) {
                            if let Some(name) = caps.get(2) {
                                if !name.as_str().starts_with('_') {
                                    output.push_str(&format!("{}:{}:{}\n", file_path.display(), line_num + 1, line));
                                }
                            }
                        }
                    }
                }
            }
        }
        if output.is_empty() { output = "(no Python items found)\n".to_string(); }
        Ok(output)
    }

    fn extract_go_api(&self, file_index: &[PathBuf]) -> Result<String> {
        let pattern = Regex::new(r"^\s*func\s+([A-Z][A-Za-z0-9_]*)\s*\(")?;
        let mut output = String::new();
        for file_path in file_index {
            if file_path.extension().map_or(false, |e| e == "go") {
                if let Ok(content) = fs::read_to_string(file_path) {
                    for (line_num, line) in content.lines().enumerate() {
                        if pattern.is_match(line) {
                            output.push_str(&format!("{}:{}:{}\n", file_path.display(), line_num + 1, line));
                        }
                    }
                }
            }
        }
        if output.is_empty() { output = "(no Go items found)\n".to_string(); }
        Ok(output)
    }
}

// -------------------------------------------------------------------------
// Helpers
// -------------------------------------------------------------------------

fn tool_exists(cmd: &str) -> bool {
    Command::new(cmd).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

fn run_collect_any_status(cmd: &str, args: &[&str]) -> Option<String> {
    Command::new(cmd).args(args).output().ok().and_then(collect_string)
}

fn run_and_capture(cmd: &str, args: &[&str]) -> Option<String> {
    Command::new(cmd).args(args).output().ok().filter(|o| o.status.success()).and_then(collect_string)
}

fn collect_string(out: Output) -> Option<String> {
    String::from_utf8(out.stdout).ok().map(|s| s.replace("\r\n", "\n"))
}

fn warn_tool_missing(name: &str) -> String { format!("(tool not found or not installed: {})", name) }
fn warn_tool_failed(name: &str) -> String { format!("(tool failed or produced no output: {})", name) }

fn clamp_and_scrub(s: &str, label: &str) -> String {
    scrub(&clamp_block(s, label))
}

fn clamp_block(s: &str, label: &str) -> String {
    let text = if s.len() > DEPS_SECTION_MAX_BYTES {
        format!("{}\n… [truncated to {} bytes for {}]", &s[..DEPS_SECTION_MAX_BYTES], DEPS_SECTION_MAX_BYTES, label)
    } else { s.to_string() };
    let mut lines: Vec<&str> = text.lines().collect();
    if lines.len() > DEPS_SECTION_MAX_LINES {
        lines.truncate(DEPS_SECTION_MAX_LINES);
        format!("{}\n… [truncated to {} lines for {}]", lines.join("\n"), DEPS_SECTION_MAX_LINES, label)
    } else { text }
}

fn scrub(s: &str) -> String {
    let out = EMAIL_RE.replace_all(s, "<email>").to_string();
    REGISTRY_RE.replace_all(&out, "index.crates.io/…/").to_string()
}