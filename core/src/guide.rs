use crate::error::Result;
use std::path::Path;

const GUIDE_CONTENT: &str = r#"========================================
SACCADE PACK GUIDE (Single-File)
========================================

Saccade now generates ONE file: PACK.txt
with clearly labeled sections so an LLM
can parse deterministically.

SECTION MARKERS (exact):

=======PROJECT=======
... metadata, stats, languages
=======END-OF-PROJECT=======

=======STRUCTURE=======
... directory tree, file index, token heatmap
=======END-OF-STRUCTURE=======

=======APIS=======
... API surface across Rust / TS-JS / Python / Go
=======END-OF-APIS=======

=======DEPS=======
... multi-ecosystem dependency snapshot (if present)
=======END-OF-DEPS=======

=======GUIDE=======
... this protocol & usage guide
=======END-OF-GUIDE=======

========================================
REAL-WORLD USE (DO THIS)
========================================

Attach: PACK.txt  (and PACK_STAGE2_COMPRESSED.xml if asked)

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
  - Use STRUCTURE and APIS sections to pick targets.
  - Never hallucinate missing code—request it explicitly.

========================================
TIPS
========================================
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
        let absolute_pack_dir = dunce::canonicalize(pack_dir)?;
        eprintln!("✅ Success! Generated pack (single file)");
        eprintln!("   In: {}\n", absolute_pack_dir.display());

        eprintln!(
            "   - {} (single-text pack with markers)",
            crate::PACK_FILE_NAME
        );
        eprintln!("   - PACK_STAGE2_COMPRESSED.xml (signatures-only skeleton)\n");

        if has_deps {
            eprintln!("ℹ️  DEPS section included (summarized, bounded).");
        }
        Ok(())
    }
}