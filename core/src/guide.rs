use crate::error::Result;
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

pub struct GuideGenerator;

impl GuideGenerator {
    pub fn new() -> Self {
        Self
    }

    pub fn generate_guide(&self) -> Result<String> {
        let mut content = String::new();

        content.push_str("========================================\n");
        content.push_str("SACCADE PACK GUIDE\n");
        content.push_str("========================================\n\n");

        content.push_str("This is a Saccade context pack—a token-efficient representation\n");
        content.push_str("of a codebase designed for LLMs. It mimics human vision:\n\n");

        content.push_str("Stage 0 (Peripheral): Structure overview (what exists, where, how big)\n");
        content.push_str("Stage 1 (Features):   API surfaces (contracts, exports, signatures)\n");
        content.push_str("Stage 2 (Focus):      On-demand source via REQUEST_FILE protocol\n\n");

        content.push_str("========================================\n");
        content.push_str("FILES IN THIS PACK\n");
        content.push_str("========================================\n\n");

        content.push_str("1. GUIDE.txt (this file)     - How to use the pack\n");
        content.push_str("2. PROJECT.txt               - Overview, metadata, languages\n");
        content.push_str("3. STRUCTURE.txt             - Directory tree, file list, token heatmap\n");
        content.push_str("4. APIS.txt                  - API surfaces (Rust/TS/Python/Go)\n");
        content.push_str("5. DEPS.txt (if exists)      - Dependencies (cargo tree, etc.)\n\n");

        content.push_str("========================================\n");
        content.push_str("REQUEST_FILE PROTOCOL\n");
        content.push_str("========================================\n\n");

        content.push_str("When you need to see actual source code, use this format:\n\n");

        content.push_str("```yaml\n");
        content.push_str("REQUEST_FILE:\n");
        content.push_str("  path: relative/path/to/file.ext\n");
        content.push_str("  reason: >\n");
        content.push_str("    What you will inspect or implement.\n");
        content.push_str("  range: lines 80-140        # optional: specific lines\n");
        content.push_str("                             # or: symbol: FunctionName\n");
        content.push_str("```\n\n");

        content.push_str("Guidelines:\n");
        content.push_str("- Request only what you need (minimize requests)\n");
        content.push_str("- Consult STRUCTURE.txt and APIS.txt first\n");
        content.push_str("- Use specific line ranges when possible\n");
        content.push_str("- Never hallucinate missing code—request it\n\n");

        content.push_str("========================================\n");
        content.push_str("WORKFLOW EXAMPLE\n");
        content.push_str("========================================\n\n");

        content.push_str("User: \"test_02 is failing: 'Expected pattern not found: ^code\\.rs$'\"\n\n");

        content.push_str("AI:\n");
        content.push_str("1. Check STRUCTURE.txt → see gauntlet/ and core/ exist\n");
        content.push_str("2. Check APIS.txt → see filter.rs has should_keep() method\n");
        content.push_str("3. REQUEST_FILE gauntlet/src/main.rs (lines around test_02)\n");
        content.push_str("4. Hypothesis: filtering logic is wrong\n");
        content.push_str("5. REQUEST_FILE core/src/filter.rs (should_keep method)\n");
        content.push_str("6. Diagnose: && vs || bug in code_only mode\n");
        content.push_str("7. Propose fix with diff\n\n");

        content.push_str("========================================\n");
        content.push_str("TIPS FOR BEST RESULTS\n");
        content.push_str("========================================\n\n");

        content.push_str("- Start with PROJECT.txt to understand what the codebase does\n");
        content.push_str("- Use STRUCTURE.txt to locate files by name/path\n");
        content.push_str("- Use APIS.txt to understand public contracts\n");
        content.push_str("- Request files only after forming a hypothesis\n");
        content.push_str("- When fixing bugs, request test files first to understand expectations\n");
        content.push_str("- Produce complete, compilable changes (no placeholders)\n\n");

        Ok(content)
    }

    pub fn print_guide(&self, pack_dir: &Path, has_deps: bool) {
        let abs_pack = fs::canonicalize(pack_dir).unwrap_or_else(|_| pack_dir.to_path_buf());
        let abs_pack_str = abs_pack.display().to_string();
        let win_pack = self.get_windows_path(&abs_pack_str);
        let use_osc8 = self.supports_osc8();

        println!();
        println!("╔══════════════════════════════════════════════════════════╗");
        println!("║  Saccade Pack Ready! (5 files max)                       ║");
        println!("╚══════════════════════════════════════════════════════════╝");
        println!(" Open pack folder:");
        println!("    • POSIX:   {}", abs_pack_str);

        if let Some(ref wp) = win_pack {
            println!("    • Windows: {}", wp);
        }

        if use_osc8 {
            let pack_uri = self.to_file_uri(&win_pack.as_ref().unwrap_or(&abs_pack_str));
            println!("    • Click:   \x1b]8;;{}\x1b\\open folder\x1b]8;;\x1b\\", pack_uri);
        } else {
            println!("    • Link:    file://{}", abs_pack_str.replace(" ", "%20"));
        }

        println!();
        println!(" Upload these files to your AI:");
        println!("    1. GUIDE.txt");
        println!("    2. PROJECT.txt");
        println!("    3. STRUCTURE.txt");
        println!("    4. APIS.txt");
        if has_deps {
            println!("    5. DEPS.txt");
        }

        println!();
        println!(" Then describe your task:");
        println!("    \"Here's my codebase. I need help with [problem].\"");
        println!("    \"Error: [paste error message]\"");
        println!("    \"Feature request: [describe]\"");

        println!();
        println!("==> Done. Pack ready in ./{}", pack_dir.display());
    }

    fn get_windows_path(&self, posix_path: &str) -> Option<String> {
        if let Ok(output) = Command::new("cygpath").args(&["-w", posix_path]).output() {
            if output.status.success() {
                if let Ok(s) = String::from_utf8(output.stdout) {
                    return Some(s.trim().to_string());
                }
            }
        }

        if cfg!(windows)
            || env::var("MSYSTEM").is_ok()
            || env::var("OSTYPE").map_or(false, |s| s.starts_with("msys"))
        {
            if let Some(rest) = posix_path.strip_prefix("/") {
                let parts: Vec<&str> = rest.splitn(2, '/').collect();
                if parts.len() == 2 && parts[0].len() == 1 {
                    let drive = parts[0].to_uppercase();
                    let path = parts[1].replace('/', "\\");
                    return Some(format!("{}:\\{}", drive, path));
                }
            }
            return Some(posix_path.replace('/', "\\"));
        }

        None
    }

    fn supports_osc8(&self) -> bool {
        env::var("WT_SESSION").is_ok()
            || env::var("TERM_PROGRAM")
                .map_or(false, |t| t.contains("WezTerm") || t.contains("iTerm") || t.contains("vscode"))
            || env::var("TERM")
                .map_or(false, |t| t.contains("kitty") || t.contains("foot"))
    }

    fn to_file_uri(&self, path: &str) -> String {
        if path.contains('\\') {
            format!("file:///{}", path.replace('\\', "/").replace(' ', "%20"))
        } else {
            format!("file://{}", path.replace(' ', "%20"))
        }
    }
}