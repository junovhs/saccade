use crate::error::Result;
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// === Dependency output budgets (visible, enforceable) =====================
/// Keep DEPS readable by default; users can run full commands locally.
/// If we ever want an "audit" mode, we can flip these or add a flag.
const DEPS_SECTION_MAX_LINES: usize = 300;
const DEPS_SECTION_MAX_BYTES: usize = 128 * 1024; // 128 KiB
const DEPS_JOINER: &str = "\n\n----------------------------------------\n";
const INCLUDE_CARGO_METADATA: bool = false; // OFF by default (too noisy)

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

        // Rust API
        output.push_str("========================================\n");
        output.push_str("API SURFACE: RUST\n");
        output.push_str("========================================\n\n");

        let rust_api = self.extract_rust_api(rust_crates, file_index)?;
        output.push_str(&rust_api);

        // TypeScript/JavaScript API
        output.push_str("\n========================================\n");
        output.push_str("API SURFACE: TYPESCRIPT/JAVASCRIPT\n");
        output.push_str("========================================\n\n");

        let ts_api = self.extract_ts_api(frontend_dirs, file_index)?;
        output.push_str(&ts_api);

        // Python API
        output.push_str("\n========================================\n");
        output.push_str("API SURFACE: PYTHON\n");
        output.push_str("========================================\n\n");

        let py_api = self.extract_python_api(file_index)?;
        output.push_str(&py_api);

        // Go API
        output.push_str("\n========================================\n");
        output.push_str("API SURFACE: GO\n");
        output.push_str("========================================\n\n");

        let go_api = self.extract_go_api(file_index)?;
        output.push_str(&go_api);

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
                // Skip target dirs on all platforms
                if p.contains("/target/") || p.contains("\\target\\") {
                    continue;
                }
                if let Some(parent) = entry.path().parent() {
                    let src_dir = parent.join("src");
                    if src_dir.exists() && src_dir.is_dir() {
                        let normalized = if let Ok(stripped) = src_dir.strip_prefix(".") {
                            stripped.to_path_buf()
                        } else {
                            src_dir
                        };
                        crates.push(normalized);
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
                    let normalized = if let Ok(stripped) = parent.strip_prefix(".") {
                        stripped.to_path_buf()
                    } else {
                        parent.to_path_buf()
                    };
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

    /// Build a consolidated, bounded DEPS.txt across ecosystems.
    /// This function never fails hard; each section degrades gracefully.
    pub fn generate_all_deps(&self) -> Result<String> {
        let mut sections: Vec<String> = Vec::new();

        // RUST
        if Path::new("Cargo.toml").exists() {
            sections.push(self.deps_rust());
        }

        // NODE (npm/pnpm/yarn)
        if Path::new("package.json").exists() {
            sections.push(self.deps_node());
        }

        // PYTHON (pipdeptree / lockfiles)
        if Path::new("pyproject.toml").exists()
            || Path::new("requirements.txt").exists()
            || Path::new("requirements-dev.txt").exists()
            || Path::new("Pipfile").exists()
            || Path::new("poetry.lock").exists()
        {
            sections.push(self.deps_python());
        }

        // GO
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
        let mut parts: Vec<String> = Vec::new();
        parts.push("RUST (cargo)".to_string());
        parts.push("Tools: cargo tree".to_string());

        // duplicates
        if let Some(s) = run_and_capture("cargo", &["tree", "-d"]) {
            parts.push(format!(
                "cargo tree -d (duplicates)\n{}\n",
                clamp_and_scrub(&s, "cargo tree -d")
            ));
        } else {
            parts.push(warn_tool_missing("cargo tree -d"));
        }

        // build-ish view (normal + build, shallow)
        if let Some(s) = run_and_capture("cargo", &["tree", "-e", "normal,build", "--depth", "2"]) {
            parts.push(format!(
                "cargo tree -e normal,build --depth 2\n{}\n",
                clamp_and_scrub(&s, "cargo tree -e normal,build --depth 2")
            ));
        }

        // metadata (HUGE) — disabled by default
        if INCLUDE_CARGO_METADATA {
            if let Some(s) = run_and_capture("cargo", &["metadata", "--format-version", "1"]) {
                parts.push(format!(
                    "cargo metadata --format-version 1 (truncated)\n{}\n",
                    clamp_and_scrub(&s, "cargo metadata")
                ));
            }
        }

        parts.join("\n")
    }

    fn deps_node(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        parts.push("NODE (npm/pnpm/yarn)".to_string());

        // prefer npm (shallow to avoid explosions)
        if tool_exists("npm") {
            parts.push("Tool: npm".to_string());
            if let Some(s) = run_collect_any_status("npm", &["ls", "--depth", "2"]) {
                parts.push(format!(
                    "npm ls --depth 2\n{}\n",
                    clamp_and_scrub(&s, "npm ls --depth 2")
                ));
            } else {
                parts.push(warn_tool_failed("npm ls --depth 2"));
            }
            return parts.join("\n");
        }

        // pnpm
        if tool_exists("pnpm") {
            parts.push("Tool: pnpm".to_string());
            if let Some(s) = run_collect_any_status("pnpm", &["list", "--depth", "2"]) {
                parts.push(format!(
                    "pnpm list --depth 2\n{}\n",
                    clamp_and_scrub(&s, "pnpm list --depth 2")
                ));
            } else {
                parts.push(warn_tool_failed("pnpm list --depth 2"));
            }
            return parts.join("\n");
        }

        // yarn (v1)
        if tool_exists("yarn") {
            parts.push("Tool: yarn".to_string());
            if let Some(s) = run_collect_any_status("yarn", &["list", "--depth=2"]) {
                parts.push(format!(
                    "yarn list --depth=2\n{}\n",
                    clamp_and_scrub(&s, "yarn list --depth=2")
                ));
            } else {
                parts.push(warn_tool_failed("yarn list --depth=2"));
            }
            return parts.join("\n");
        }

        parts.push(warn_tool_missing("npm|pnpm|yarn"));
        parts.join("\n")
    }

    fn deps_python(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        parts.push("PYTHON (pip/poetry)".to_string());

        // Prefer pipdeptree if present (still clamp hard).
        if tool_exists("pipdeptree") {
            if let Some(s) = run_collect_any_status("pipdeptree", &["--json-tree", "-w", "silence"]) {
                parts.push(format!(
                    "pipdeptree --json-tree -w silence (truncated)\n{}\n",
                    clamp_and_scrub(&s, "pipdeptree --json-tree")
                ));
                return parts.join("\n");
            }
        }

        // Poetry lock as fallback (static)
        if Path::new("poetry.lock").exists() {
            match fs::read_to_string("poetry.lock") {
                Ok(s) => {
                    parts.push("(poetry.lock present; head)".to_string());
                    parts.push(clamp_and_scrub(&s, "poetry.lock"));
                    return parts.join("\n");
                }
                Err(e) => parts.push(format!("WARN: failed to read poetry.lock: {}", e)),
            }
        }

        // requirements*.txt fallback
        let mut appended = false;
        for name in &["requirements.txt", "requirements-dev.txt"] {
            if Path::new(name).exists() {
                match fs::read_to_string(name) {
                    Ok(s) => {
                        parts.push(format!("({} present; head)", name));
                        parts.push(clamp_and_scrub(&s, name));
                        appended = true;
                    }
                    Err(e) => parts.push(format!("WARN: failed to read {}: {}", name, e)),
                }
            }
        }
        if appended {
            return parts.join("\n");
        }

        // Last ditch: try `pip list` (env-dependent)
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
        let mut parts: Vec<String> = Vec::new();
        parts.push("GO (modules)".to_string());

        if tool_exists("go") {
            if let Some(s) = run_collect_any_status("go", &["version"]) {
                parts.push(scrub(&s.trim()));
            }
            if let Some(s) = run_collect_any_status("go", &["mod", "graph"]) {
                parts.push(format!(
                    "go mod graph\n{}\n",
                    clamp_and_scrub(&s, "go mod graph")
                ));
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
        if crates.is_empty() {
            return Ok("(no Rust crates found)\n".to_string());
        }

        let pattern = Regex::new(
            r"pub(\s+|\s*\([^)]*\)\s+)(fn|struct|enum|trait|type|const|static|use|mod|macro_rules!)",
        )?;

        let mut output = String::new();

        for crate_dir in crates {
            // Normalize paths to handle cross-platform differences (e.g., git ls-files vs. walkdir on Windows)
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

        if output.is_empty() {
            output = "(no public Rust items found)\n".to_string();
        }

        Ok(output)
    }

    fn extract_ts_api(&self, frontend_dirs: &[PathBuf], file_index: &[PathBuf]) -> Result<String> {
        if frontend_dirs.is_empty() {
            return Ok("(no frontend dirs found)\n".to_string());
        }

        let pattern = Regex::new(
            r"^(\s*export\s+(default\s+)?(function|class|interface|type|enum|const|let|var|async|function\*)|\s*(function|class)\s+[A-Z])",
        )?;

        let mut output = String::new();

        for frontend_dir in frontend_dirs {
            let dir_str = frontend_dir.to_string_lossy().replace('\\', "/");

            for file_path in file_index {
                let file_str = file_path.to_string_lossy().replace('\\', "/");

                if file_str.starts_with(&*dir_str)
                    && (file_str.ends_with(".js")
                        || file_str.ends_with(".jsx")
                        || file_str.ends_with(".ts")
                        || file_str.ends_with(".tsx")
                        || file_str.ends_with(".mjs")
                        || file_str.ends_with(".cjs"))
                    && !file_str.ends_with(".d.ts")
                {
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

        if output.is_empty() {
            output = "(no TS/JS items found)\n".to_string();
        }

        Ok(output)
    }

    fn extract_python_api(&self, file_index: &[PathBuf]) -> Result<String> {
        let pattern = Regex::new(r"^\s*(def|class)\s+([A-Za-z][A-Za-z0-9_]*)")?;

        let mut output = String::new();

        for file_path in file_index {
            let file_str = file_path.to_string_lossy();

            if file_str.ends_with(".py") {
                if let Ok(content) = fs::read_to_string(file_path) {
                    for (line_num, line) in content.lines().enumerate() {
                        if let Some(caps) = pattern.captures(line) {
                            if let Some(name) = caps.get(2) {
                                if !name.as_str().starts_with('_') {
                                    output.push_str(&format!("{}:{}:{}\n", file_str, line_num + 1, line));
                                }
                            }
                        }
                    }
                }
            }
        }

        if output.is_empty() {
            output = "(no Python items found)\n".to_string();
        }

        Ok(output)
    }

    fn extract_go_api(&self, file_index: &[PathBuf]) -> Result<String> {
        let pattern = Regex::new(r"^\s*func\s+([A-Z][A-Za-z0-9_]*)\s*\(")?;

        let mut output = String::new();

        for file_path in file_index {
            let file_str = file_path.to_string_lossy();

            if file_str.ends_with(".go") {
                if let Ok(content) = fs::read_to_string(file_path) {
                    for (line_num, line) in content.lines().enumerate() {
                        if pattern.is_match(line) {
                            output.push_str(&format!("{}:{}:{}\n", file_str, line_num + 1, line));
                        }
                    }
                }
            }
        }

        if output.is_empty() {
            output = "(no Go items found)\n".to_string();
        }

        Ok(output)
    }
}

// -------------------------------------------------------------------------
// Helpers
// -------------------------------------------------------------------------

fn tool_exists(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Accepts non-zero exit codes (some tools use them to signal issues but still print useful trees).
fn run_collect_any_status(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    let s = collect_string(out).unwrap_or_default();
    if s.trim().is_empty() {
        return None;
    }
    Some(s)
}

/// Requires zero exit status.
fn run_and_capture(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    collect_string(out)
}

fn collect_string(out: Output) -> Option<String> {
    let mut s = String::from_utf8(out.stdout).ok()?;
    // Normalize newlines; some tools on Windows output \r\n.
    s = s.replace("\r\n", "\n");
    Some(s)
}

fn warn_tool_missing(name: &str) -> String {
    format!("(tool not found or not installed: {})", name)
}

fn warn_tool_failed(name: &str) -> String {
    format!("(tool failed or produced no output: {})", name)
}

fn clamp_and_scrub(s: &str, label: &str) -> String {
    let clamped = clamp_block(s, label);
    scrub(&clamped)
}

fn clamp_block(s: &str, label: &str) -> String {
    // byte clamp first (cheap guard)
    let text = if s.len() > DEPS_SECTION_MAX_BYTES {
        let mut t = s[..DEPS_SECTION_MAX_BYTES].to_string();
        t.push_str(&format!(
            "\n… [truncated to {} bytes for {}]",
            DEPS_SECTION_MAX_BYTES, label
        ));
        t
    } else {
        s.to_string()
    };

    // then line clamp for readability
    let mut lines: Vec<&str> = text.lines().collect();
    if lines.len() > DEPS_SECTION_MAX_LINES {
        lines.truncate(DEPS_SECTION_MAX_LINES);
        let mut joined = lines.join("\n");
        joined.push_str(&format!(
            "\n… [truncated to {} lines for {}]",
            DEPS_SECTION_MAX_LINES, label
        ));
        return joined;
    }

    text
}

/// Redact obvious email addresses and noisy registry urls.
fn scrub(s: &str) -> String {
    // Simple email redaction
    let email_re = Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").unwrap();
    let mut out = email_re.replace_all(s, "<email>").to_string();

    // Collapse overly long registry paths (keep basename)
    let reg_re = Regex::new(r"index\.crates\.io-[^\s/\\]+[\\/]").unwrap();
    out = reg_re
        .replace_all(&out, "index.crates.io/…/")
        .to_string();

    out
}
