use crate::error::Result;
use std::path::Path;

const GUIDE_CONTENT: &str = r#"========================================
SACCADE PACK GUIDE
========================================

Saccade creates a small, reliable, token-efficient context for LLMs.
It mimics human vision:

• Stage 0 (Peripheral)  → STRUCTURE.txt (tree, file index, size heatmap)
• Stage 1 (Features)    → APIS.txt (public surfaces) + DEPS.txt (deps)
• Stage 2 (Focus)       → PACK_STAGE2_COMPRESSED.xml (signatures only)

This pack is designed to be *smaller than your repo* yet *more reliable*
for planning, diagnosis, and patching.

========================================
REAL-WORLD USE (DO THIS)
========================================

Round 1 — Attach these files:
  • GUIDE.txt, PROJECT.txt, STRUCTURE.txt, APIS.txt, DEPS.txt (if present)

Paste a short brief (copy & edit):
  Goal: <what you want changed/added/fixed>
  Evidence: <errors/logs/stack traces; short and relevant>
  Context: <entry points, recent changes, repro>
  Constraints: <perf, API compatibility, deadlines>
  Definition of Done: <tests green, clippy clean, no API break, etc.>

If you provide nothing, the AI must still begin by triaging intent (bug/feature/refactor),
proposing a minimal plan, and requesting code slices via REQUEST_FILE when needed.

Escalation rule:
  • After the first failed attempt OR when layout/contracts are unclear,
    ask for PACK_STAGE2_COMPRESSED.xml (signatures-only skeleton).
  • If still blocked, request the smallest useful code slice via REQUEST_FILE.

========================================
REQUEST_FILE — EXACT FORMAT
========================================

REQUEST_FILE:
  path: relative/path/to/file.ext
  reason: >
    What you will inspect or implement.
  range: lines 80-140        # or: symbol: FunctionName

Guidelines:
  - Minimize tokens: prefer line ranges over whole files.
  - Use STRUCTURE.txt and APIS.txt to pick targets.
  - Never hallucinate missing code—request it explicitly.

========================================
FILES IN THIS PACK
========================================

1. GUIDE.txt                 — Protocol + usage (you are here)
2. PROJECT.txt               — Repo intent, entry points, current task
3. STRUCTURE.txt             — Tree (depth-limited), file index, size heatmap
4. APIS.txt                  — Public surfaces (Rust/TS-JS/Python/…)
5. DEPS.txt (optional)       — Dependency snapshot (e.g., cargo tree)
6. PACK_STAGE2_COMPRESSED.xml— Signatures-only skeleton (attach only if needed)

Tips:
  - Use --code-only, --include/--exclude, and --max-depth to reduce tokens.
  - Re-run Saccade after changes; packs are quick to refresh.
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
        // Single source of truth for the clickable link is the CLI.
        // Here we print a neutral location line across platforms.
        eprintln!("   In: {}\n", absolute_pack_dir.display());

        eprintln!("   - GUIDE.txt (how to use the pack)");
        eprintln!("   - PROJECT.txt (overview, metadata)");
        Ok(())
    }
}