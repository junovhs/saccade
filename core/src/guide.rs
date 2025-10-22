use crate::error::Result;
use std::path::Path;

const GUIDE_CONTENT: &str = r#"========================================
SACCADE PACK GUIDE
========================================

This is a Saccade context pack—a token-efficient representation
of a codebase designed for LLMs. It mimics human vision:

Stage 0 (Peripheral): Structure overview (what exists, where, how big)
Stage 1 (Features):   API surfaces (contracts, exports, signatures)
Stage 2 (Focus):      On-demand source via REQUEST_FILE protocol

========================================
FILES IN THIS PACK
========================================

1. GUIDE.txt (this file)     - How to use the pack
2. PROJECT.txt               - Overview, metadata, languages
3. STRUCTURE.txt             - Directory tree, file list, token heatmap
4. APIS.txt                  - API surfaces (Rust/TS/Python/Go)
5. DEPS.txt (if exists)      - Dependencies (cargo tree, etc.)

========================================
REQUEST_FILE PROTOCOL
========================================

When you need to see actual source code, use this format:


REQUEST_FILE:
  path: relative/path/to/file.ext
  reason: >
    What you will inspect or implement.
  range: lines 80-140        # optional: specific lines
                             # or: symbol: FunctionName

Guidelines:
- Request only what you need (minimize requests)
- Consult STRUCTURE.txt and APIS.txt first
- Use specific line ranges when possible
- Never hallucinate missing code—request it
"#;

pub struct GuideGenerator;

impl GuideGenerator {
    pub fn new() -> Self {
        Self
    }

    pub fn generate_guide(&self) -> Result<String> {
        Ok(GUIDE_CONTENT.to_string())
    }

    pub fn print_guide(&self, pack_dir: &Path, has_deps: bool) -> Result<()> {
        let pack_name = if has_deps { "5 files" } else { "4 files" };
        eprintln!("\n✅ Success! Generated pack ({})", pack_name);

        let absolute_pack_dir = dunce::canonicalize(pack_dir)?;

        // The test specifically checks for this line on Windows.
        if cfg!(target_os = "windows") {
            eprintln!("   Click: file:///{}\n", absolute_pack_dir.display());
        } else {
            eprintln!("   In: {}\n", absolute_pack_dir.display());
        }

        eprintln!("   - GUIDE.txt (how to use the pack)");
        eprintln!("   - PROJECT.txt (overview, metadata)");
        Ok(())
    }
}