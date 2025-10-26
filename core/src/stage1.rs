// saccade/core/src/stage1.rs

use crate::detection::BuildSystemType;
use crate::error::Result;
use once_cell::sync::Lazy;
use regex::Regex;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use tree_sitter::{Parser, Query};

/// === Dependency output budgets (visible, enforceable) =====================
const DEPS_SECTION_MAX_LINES: usize = 300;
const DEPS_SECTION_MAX_BYTES: usize = 128 * 1024; // 128 KiB
const DEPS_JOINER: &str = "\n\n----------------------------------------\n";
const INCLUDE_CARGO_METADATA: bool = false; // OFF by default (too noisy)

static EMAIL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").unwrap());
static REGISTRY_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"index\.crates\.io-[^\s/\\]+[\\/]").unwrap());

const CMAKE_DEPS_QUERY: &str = r#"
(normal_command) @command
"#;

// Tree-sitter query to find `requires = "..."` assignments in Python.
const PYTHON_CONAN_DEPS_QUERY: &str = r#"
(assignment
  left: (identifier) @name
  right: (string) @value)
"#;

pub struct Stage1Generator;

impl Stage1Generator {
    pub fn new() -> Self {
        Self
    }

    // ---------------------------------------------------------------------
    // API SURFACE
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
    // DEPENDENCIES (Dynamically Configured)
    // ---------------------------------------------------------------------

    /// Build a consolidated DEPS section, dynamically configured by the Layer 2 detector.
    pub fn generate_all_deps(&self, detected_systems: &[BuildSystemType]) -> Result<String> {
        let mut sections: Vec<String> = Vec::new();

        // --- DCA in action: Only run tools for detected systems ---
        if detected_systems.contains(&BuildSystemType::Rust) {
            sections.push(self.deps_rust());
        }
        if detected_systems.contains(&BuildSystemType::Node) {
            sections.push(self.deps_node());
        }
        if detected_systems.contains(&BuildSystemType::Python) {
            sections.push(self.deps_python());
        }
        if detected_systems.contains(&BuildSystemType::Go) {
            sections.push(self.deps_go());
        }
        if detected_systems.contains(&BuildSystemType::CMake) {
            sections.push(self.deps_cmake(detected_systems)?);
        }
        if detected_systems.contains(&BuildSystemType::Conan) {
            sections.push(self.deps_conan()?);
        }
        // --- End DCA section ---

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
        if tool_exists("npm") {
            parts.push("Tool: npm".to_string());
            if let Some(s) = run_collect_any_status("npm", &["ls", "--depth", "2"]) {
                parts.push(format!("npm ls --depth 2\n{}\n", clamp_and_scrub(&s, "npm ls --depth 2")));
            } else {
                parts.push(warn_tool_failed("npm ls --depth 2"));
            }
            return parts.join("\n");
        }
        if tool_exists("pnpm") {
            parts.push("Tool: pnpm".to_string());
            if let Some(s) = run_collect_any_status("pnpm", &["list", "--depth", "2"]) {
                parts.push(format!("pnpm list --depth 2\n{}\n", clamp_and_scrub(&s, "pnpm list --depth 2")));
            } else {
                parts.push(warn_tool_failed("pnpm list --depth 2"));
            }
            return parts.join("\n");
        }
        if tool_exists("yarn") {
            parts.push("Tool: yarn".to_string());
            if let Some(s) = run_collect_any_status("yarn", &["list", "--depth=2"]) {
                parts.push(format!("yarn list --depth=2\n{}\n", clamp_and_scrub(&s, "yarn list --depth=2")));
            } else {
                parts.push(warn_tool_failed("yarn list --depth=2"));
            }
            return parts.join("\n");
        }
        parts.push(warn_tool_missing("npm|pnpm|yarn"));
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
        let mut appended = false;
        for name in &["requirements.txt", "requirements-dev.txt"] {
            if let Ok(s) = fs::read_to_string(name) {
                parts.push(format!("({} present; head)", name));
                parts.push(clamp_and_scrub(&s, name));
                appended = true;
            }
        }
        if appended { return parts.join("\n"); }
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

    /// Parse CMakeLists.txt for `find_package` dependencies.
    fn deps_cmake(&self, _detected_systems: &[BuildSystemType]) -> Result<String> {
        let mut parts: Vec<String> = vec!["C++ (CMake)".to_string()];
        let mut found_any = false;

        let cmake_files: Vec<_> = walkdir::WalkDir::new(".")
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy();
                name == "CMakeLists.txt" || name.ends_with(".cmake")
            })
            .collect();

        for entry in cmake_files {
            let path = entry.path();
            if let Ok(content) = fs::read_to_string(path) {
                if let Some(deps) = self.extract_cmake_deps(&content) {
                    parts.push(format!(
                        "Dependencies from: {}\n{}",
                        path.display(),
                        deps
                    ));
                    found_any = true;
                }
            }
        }

        if !found_any {
            parts.push("(No `find_package` dependencies found in CMake files)".to_string());
        }

        Ok(parts.join("\n"))
    }
    
    /// Helper to extract dependencies from a single CMake file's content.
    fn extract_cmake_deps(&self, content: &str) -> Option<String> {
        let mut parser = Parser::new();
        if parser.set_language(&tree_sitter_cmake::language()).is_err() {
            return None;
        }
        let tree = parser.parse(content, None)?;
        let query = Query::new(&tree_sitter_cmake::language(), CMAKE_DEPS_QUERY).ok()?;
        
        let mut cursor = tree_sitter::QueryCursor::new();
        let matches = cursor.matches(&query, tree.root_node(), content.as_bytes());
        
        let mut packages = Vec::new();
        for m in matches {
            let node = m.captures[0].node;
            
            if let Some(name_node) = node.child(0) {
                if name_node.kind() == "identifier" {
                    if let Ok(name) = name_node.utf8_text(content.as_bytes()) {
                        if name.to_lowercase() == "find_package" {
                            // CORRECTED: The package name is at child index 2.
                            if let Some(arg_node) = node.child(2) {
                                 if let Ok(arg_text) = arg_node.utf8_text(content.as_bytes()) {
                                    packages.push(format!("- {}", arg_text.trim()));
                                 }
                            }
                        }
                    }
                }
            }
        }

        if packages.is_empty() {
            None
        } else {
            Some(packages.join("\n"))
        }
    }

    /// REFACTORED: Parse conanfile.py for `requires` dependencies using Tree-sitter.
    fn deps_conan(&self) -> Result<String> {
        let mut parts: Vec<String> = vec!["C++ (Conan)".to_string()];
        let mut found_any = false;

        let conan_files: Vec<_> = walkdir::WalkDir::new(".")
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy() == "conanfile.py")
            .collect();

        for entry in conan_files {
            let path = entry.path();
            if let Ok(content) = fs::read_to_string(path) {
                if let Some(deps) = self.extract_conan_deps(&content) {
                    parts.push(format!(
                        "Dependencies from: {}\n{}",
                        path.display(),
                        deps
                    ));
                    found_any = true;
                }
            }
        }

        if !found_any {
            parts.push("(No `requires` dependencies found in Conan files)".to_string());
        }

        Ok(parts.join("\n"))
    }

    /// CORRECTED: Helper to extract `requires` from a conanfile.py's content using Tree-sitter.
    fn extract_conan_deps(&self, content: &str) -> Option<String> {
        let mut parser = Parser::new();
        if parser.set_language(&tree_sitter_python::language()).is_err() {
            return None;
        }
        let tree = parser.parse(content, None)?;
        let query = Query::new(&tree_sitter_python::language(), PYTHON_CONAN_DEPS_QUERY).ok()?;
        
        let mut cursor = tree_sitter::QueryCursor::new();
        let matches = cursor.matches(&query, tree.root_node(), content.as_bytes());

        let mut packages = Vec::new();
        for m in matches {
            // Robustly find captures by name, not index.
            let mut potential_name = "";
            let mut potential_value = "";
    
            for capture in m.captures {
                let capture_name = &query.capture_names()[capture.index as usize];
                if *capture_name == "name" {
                    if let Ok(text) = capture.node.utf8_text(content.as_bytes()) {
                        potential_name = text;
                    }
                } else if *capture_name == "value" {
                    if let Ok(text) = capture.node.utf8_text(content.as_bytes()) {
                        potential_value = text;
                    }
                }
            }
            
            if potential_name == "requires" {
                let cleaned_value = potential_value.trim_matches(|c| c == '\'' || c == '"');
                packages.push(format!("- {}", cleaned_value));
            }
        }

        if packages.is_empty() {
            None
        } else {
            Some(packages.join("\n"))
        }
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