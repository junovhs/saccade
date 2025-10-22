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

This pack is built to be *smaller than your repo* yet *more reliable*
for planning, diagnosis, and patching.

========================================
OPERATIONAL PROTOCOL (FOR THE AI)
========================================

If the human provides no brief, you must drive the session.

Round 1 (default; smallest upload)
  Inputs available: GUIDE.txt, PROJECT.txt, STRUCTURE.txt, APIS.txt, DEPS.txt (if present)
  Your job:
    1) Triage: ask one question to classify intent → {bug, new feature, refactor}.
    2) Read PROJECT.txt; infer entry points and constraints.
    3) From STRUCTURE.txt + APIS.txt (+ DEPS.txt), propose a minimal plan.
    4) When you need implementations, send a REQUEST_FILE (format below).
  Do NOT ask for whole files unless strictly necessary.

Escalation rule
  • After your first failed attempt, or if you are uncertain about layout/contracts,
    ask for PACK_STAGE2_COMPRESSED.xml (signatures-only skeleton).
  • If still blocked, request the smallest useful code slice via REQUEST_FILE.

REQUEST_FILE (exact format)

REQUEST_FILE:
  path: relative/path/to/file.ext
  reason: >
    What you will inspect or implement.
  range: lines 80-140        # or: symbol: FunctionName

Guidelines
  - Minimize tokens: prefer line ranges over whole files.
  - Use STRUCTURE.txt and APIS.txt to pick targets.
  - Never hallucinate missing code—request it explicitly.

========================================
WHAT THE HUMAN CAN SAY (OPTIONAL)
========================================

Helpful (but not required) on first message:
  • Goal: what you want changed/added/fixed.
  • Symptoms: errors/logs/stack traces.
  • Constraints: perf, API contracts, deadlines.

If you say nothing, the AI will begin with triage and lead the process.

========================================
FILES IN THIS PACK
========================================

1. GUIDE.txt                 — This file (protocol + usage)
2. PROJECT.txt               — Repo intent, entry points, current task
3. STRUCTURE.txt             — Tree (depth-limited), file index, size heatmap
4. APIS.txt                  — Public surfaces (Rust/TS-JS/Python/…)
5. DEPS.txt (optional)       — Dependency snapshot (e.g., cargo tree)
6. PACK_STAGE2_COMPRESSED.xml— Signatures-only skeleton (attach only if needed)

Tips
  - Use --code-only, --include/--exclude, and --max-depth to reduce tokens.
  - Re-run Saccade after changes; packs are fast to refresh.
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
