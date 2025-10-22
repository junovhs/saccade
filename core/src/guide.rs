use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;

pub struct GuideGenerator;

impl GuideGenerator {
    pub fn new() -> Self {
        Self
    }

    pub fn generate_static_files(&self, pack_dir: &Path) -> std::io::Result<()> {
        fs::write(
            pack_dir.join("OVERVIEW.md"),
            r#"# Project Overview (fill this out once)
- What it does:
- Primary tech (e.g., Rust backend, Tauri desktop, TS/React UI):
- Entrypoints / main binaries:
- Key modules / domains:
- Current task/question for the AI:
"#,
        )?;

        fs::write(
            pack_dir.join("REQUEST_PROTOCOL.md"),
            r#"# Ask-for-Files Protocol
You have a staged view:
- STRUCTURE.txt (shallow tree), TOKENS.txt (size heat map)
- Dependency / API surfaces (DEPS, API_SURFACE_*)
- PACK_STAGE2_COMPRESSED.xml (if present; compressed skeleton)
If you need raw source, request:

REQUEST_FILE:
  path: relative/path
  reason: >-
    What you will inspect or implement.
  range: lines 80-140        # or: symbol: Foo::bar

Minimize requests. Produce complete, compilable implementations that match project conventions.
Never hallucinate missing code—request it.
"#,
        )?;

        fs::write(
            pack_dir.join("PACK_README.md"),
            r#"# AI Pack — How to Use

Round 1 (tiny upload):
- OVERVIEW.md (fill it out)
- STRUCTURE.txt
- TOKENS.txt
- CARGO_TREE_DEDUP.txt (if Rust)
- API_SURFACE_* (language surfaces)
- REQUEST_PROTOCOL.md

Round 2 (when needed):
- PACK_STAGE2_COMPRESSED.xml (compressed skeleton)

On demand:
- Paste specific files/lines requested via REQUEST_PROTOCOL.md.
"#,
        )?;

        Ok(())
    }

    pub fn generate_chat_start(&self, pack_dir: &Path, artifacts: &[&str]) -> std::io::Result<()> {
        let mut content = String::from("# Start message for your LLM\n\n");
        content.push_str("**Files attached (from `ai-pack/`):**\n");

        for artifact in artifacts {
            content.push_str(&format!("- {}\n", artifact));
        }

        content.push_str(
            r#"
**Instructions to the AI:**
Please read the staged context. Use the Ask‑for‑Files Protocol below to request raw source files when needed — do not try to infer missing code.

```yaml
REQUEST_FILE:
  path: relative/path/to/file.ext
  reason: >-
    What you will inspect or implement.
  range: lines 80-140      # or: symbol: Foo::bar
```

**My goal:** <Describe your objective for this repo here.>
"#,
        );

        fs::write(pack_dir.join("CHAT_START.md"), content)?;
        Ok(())
    }

    pub fn print_guide(&self, pack_dir: &Path, artifacts: &[&str]) {
        let abs_pack = fs::canonicalize(pack_dir).unwrap_or_else(|_| pack_dir.to_path_buf());
        let abs_pack_str = abs_pack.display().to_string();

        let win_pack = self.get_windows_path(&abs_pack_str);

        let use_osc8 = self.supports_osc8();

        println!();
        println!("╔══════════════════════════════════════════════════════════╗");
        println!("║  Next steps — Your Saccade pack is ready!                ║");
        println!("╚══════════════════════════════════════════════════════════╝");
        println!(" 1) Open the pack folder:");
        println!("    • POSIX:   {}", abs_pack_str);

        if let Some(ref wp) = win_pack {
            println!("    • Windows: {}", wp);
            println!("    • Explorer: run → explorer \"{}\"", wp);
        }

        if use_osc8 {
            let pack_uri = self.to_file_uri(&win_pack.as_ref().unwrap_or(&abs_pack_str));
            println!("    • Click:   \x1b]8;;{}\x1b\\open ai-pack folder\x1b]8;;\x1b\\", pack_uri);
        } else {
            println!("    • Link:    file://{}", abs_pack_str.replace(" ", "%20"));
        }

        println!();
        println!(" 2) Start a chat and attach these files:");
        for artifact in artifacts {
            println!("    - {}", artifact);
        }
        println!("    (Round 2: attach PACK_STAGE2_COMPRESSED.xml if the model asks for more detail.)");

        println!();
        println!(" 3) Paste the ready-to-go message:");
        let chat_path = pack_dir.join("CHAT_START.md");
        println!("    • Saved at: {}", chat_path.display());

        if use_osc8 {
            let chat_uri = self.to_file_uri(
                &win_pack
                    .as_ref()
                    .map(|w| format!("{}\\CHAT_START.md", w))
                    .unwrap_or_else(|| format!("{}/CHAT_START.md", abs_pack_str)),
            );
            println!("    • Click:    \x1b]8;;{}\x1b\\open CHAT_START.md\x1b]8;;\x1b\\", chat_uri);
        }

        self.try_clipboard_copy(&chat_path);

        println!();
        println!("==> Done. AI pack is ready in ./{}", pack_dir.display());
    }

    fn get_windows_path(&self, posix_path: &str) -> Option<String> {
        // Try cygpath first
        if let Ok(output) = Command::new("cygpath").args(&["-w", posix_path]).output() {
            if output.status.success() {
                if let Ok(s) = String::from_utf8(output.stdout) {
                    return Some(s.trim().to_string());
                }
            }
        }

        // Fallback: detect MSYS/Git Bash and convert /c/... to C:\...
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
            // Windows path
            format!("file:///{}", path.replace('\\', "/").replace(' ', "%20"))
        } else {
            format!("file://{}", path.replace(' ', "%20"))
        }
    }

    fn try_clipboard_copy(&self, path: &Path) {
        let content = fs::read_to_string(path).unwrap_or_default();

        // Try various clipboard tools
        let commands = vec![
            ("clip", vec![]),                              // Windows
            ("pbcopy", vec![]),                            // macOS
            ("xclip", vec!["-selection", "clipboard"]),     // Linux
            ("xsel", vec!["--clipboard", "--input"]),       // Linux
        ];

        for (cmd, args) in commands {
            if let Ok(mut child) = Command::new(cmd)
                .args(&args)
                .stdin(std::process::Stdio::piped())
                .spawn()
            {
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(content.as_bytes());
                    drop(stdin);
                    if child.wait().map_or(false, |s| s.success()) {
                        eprintln!("    Copied chat message to clipboard via '{}'.", cmd);
                        return;
                    }
                }
            }
        }
    }
}