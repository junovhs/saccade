### `README.md`


# Saccade v0.3.0 — Legacy Bash Prototype

> _This document serves as a technical record of the original Bash-based
> implementation of **Saccade**, the staged codebase summarizer for LLM context packing.
> It is preserved for historical and architectural reference._

---

## Overview

This Bash script (`saccade.sh`) was the first functional implementation of **Saccade** —
a deterministic, zero-dependency pipeline that transforms an arbitrary source repository
into a **multi-stage, token-efficient "AI pack"** ready for large language model ingestion.

It achieved this using only POSIX shell tools (`find`, `grep`, `awk`, `sed`, `git`, `cargo`, etc.),
and optional helpers (`repomix`, `python3`, `clip`), while remaining portable across macOS, Linux,
and Git Bash on Windows.

The script enforced a consistent *philosophy of clarity and reproducibility*:
> “The AI should see only what matters — structure, interfaces, and concise metadata —
> not noise, not bulk.”

---

## Stage Model

Saccade’s pipeline was divided into discrete **stages**, each producing an artifact
designed for predictable downstream token usage and human auditability.

| Stage | Artifact | Purpose |
|-------|-----------|----------|
| 0 | `STRUCTURE.txt`, `TOKENS.txt`, `FILE_INDEX.txt` | Directory and file summaries; token heat map |
| 1 | `API_SURFACE_*.txt`, `CARGO_TREE_DEDUP.txt` | Public API and dependency surfaces |
| 2 | `PACK_STAGE2_COMPRESSED.xml` | Compact code skeleton (via `repomix`) |
| — | `PACK_MANIFEST.json`, `LANGUAGES.md`, etc. | Machine-readable metadata for every run |

### Stage 0 — Enumeration and Filtering

Enumerated all files either through:
- **Git** (`git ls-files --exclude-standard`) for tracked and unignored files, or
- **find** for fallback enumeration when no Git repo existed.

Then filtered aggressively:
- Removed binaries, media, credentials, `.env` files, caches, and test data.
- Applied inclusion/exclusion regexes.
- Optional `--code-only` mode that restricted to known code/config/markup extensions
  or bare build files (e.g., `Makefile`, `Dockerfile`).

Produced:
- `STRUCTURE.txt`: tree up to `--max-depth`.
- `TOKENS.txt`: estimated token counts (`bytes / 3.5` heuristic).
- `FILE_INDEX.txt`: sorted flat list for later reuse.

### Stage 1 — Surface Extraction

Scanned filtered files using language-specific heuristics:

| Language | Method |
|-----------|---------|
| **Rust** | `grep -E '^\s*pub'` for public items (`fn`, `struct`, `trait`, etc.) |
| **TS/JS** | `grep -E 'export'` and top-level PascalCase functions/classes |
| **Python** | `awk` to capture `def`/`class` with names not starting `_` |
| **Go** | `grep -E '^func [A-Z]'` for exported functions |

For Rust projects, also captured:
```bash
cargo tree -e normal -d > CARGO_TREE_DEDUP.txt
```

to summarize dependency graphs.

### Stage 2 — Compression (Optional)

If the `repomix` CLI was present, Stage 2 would:

* Generate `PACK_STAGE2_COMPRESSED.xml` with
  `--compress --remove-comments --remove-empty-lines`,
  providing a syntax-aware skeleton of the codebase.
* Fallback gracefully if `repomix` was missing.

Stage 2 was conceptually the **ancestor** of the modern `Tree-sitter` skeletonizer now used in the Rust rewrite.

---

## Key Design Philosophies

### 1. **Determinism Over Heuristics**

Every stage was designed to yield deterministic, repeatable output:
same repo → same pack.
No randomness, timestamps excluded from structural data.

This enabled reproducibility across machines and easy diffing of packs over time.

### 2. **Progressive Disclosure**

The pack embodied a **progressive disclosure model**:

* Stage 0 shows shape.
* Stage 1 shows public interfaces.
* Stage 2 shows structure.
* Raw source only when requested.

This mapped directly onto the AI interaction model defined in `REQUEST_PROTOCOL.md`.

### 3. **Token Efficiency as a First-Class Metric**

Token economy was treated as a measurable engineering constraint:

* Tokens ≈ bytes / 3.5 heuristic for English code.
* Heatmaps and file size ranks enabled humans (and models) to reason about budget tradeoffs.
* Stage 2 compression targeted ~10× reduction without semantic loss.

### 4. **Zero Dependencies, Maximum Predictability**

By using only standard UNIX tools, the prototype could run anywhere, even in constrained environments.
It served as an existence proof that Saccade’s concept didn’t require large dependencies.

However, this portability came at a cost:

* Regex-based language detection was brittle.
* `grep` and `awk` lacked real AST context.
* Cross-platform path handling was messy.

### 5. **Human-Centric UX**

Colorized logging (`==>`, `INFO`, `WARN`), OSC8 hyperlinks, and clipboard automation (`clip`, `pbcopy`, `xclip`)
made the pack feel like a polished CLI product, not a research script.

The **“AI pack is ready”** summary banner and chat-template generation were deliberate affordances:
they bridged human preparation and AI consumption seamlessly.

---

## Evolution and Limitations

| Area              | Limitation in Bash Prototype               | Resolution in Rust Rewrite                       |
| ----------------- | ------------------------------------------ | ------------------------------------------------ |
| **Parsing**       | Grep/awk approximations; false positives.  | Tree-sitter AST queries with body-stripping.     |
| **Portability**   | Fragile path escaping on Windows.          | Cross-platform file handling via Rust `PathBuf`. |
| **Performance**   | O(N) spawn cost for `grep`/`awk` per file. | In-memory parsing, parallelizable in future.     |
| **Safety**        | Exit-on-error; partial cleanup on fail.    | Structured error handling, bounded loops.        |
| **Extensibility** | Adding a language = new regex.             | Plug in new Tree-sitter grammar and query.       |

The prototype demonstrated the viability of **context staging** but was eventually replaced by the
Rust implementation for correctness, performance, and maintainability.

---

## Legacy Value

This script remains instructive for:

* **Design provenance** — it codifies the initial heuristics behind Saccade’s stage model.
* **Zero-dep fallback** — still usable in environments where the Rust binary cannot run.
* **Debugging and validation** — its outputs serve as a ground-truth reference
  for comparing structural coverage between regex vs. AST-based approaches.

If you wish to run it today:

```bash
chmod +x saccade.sh
./saccade.sh --out legacy-pack --code-only
```

All artifacts will appear in `legacy-pack/`.

---

## License & Preservation

This historical version is covered by the same license as the main Saccade project.

It is preserved on the branch:

```
legacy/bash-prototype
```

It will not receive further feature updates but may be retained indefinitely as a record of
Saccade’s origin and as a fallback reference implementation.

---

> “Every robust system begins as a well-reasoned script.”
>
> — *Design note from the first Saccade commit, 2024*

