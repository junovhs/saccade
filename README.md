# 👁️ Saccade

**Give your AI eyes on your codebase.**

Saccade scans a repo and produces a **small, reliable, token-efficient context pack** so LLMs can understand structure, APIs, and hotspots **without** slurping the whole tree. It now includes a **built-in Tree-sitter skeletonizer** (no external compressor required).

---

## How it works (biologically inspired)

1) **Peripheral Vision (Stage 0)** — fast global map  
   `STRUCTURE.txt` → shallow directory tree (depth-limited) **+** file index **+** size heatmap (top offenders).  
   Goal: prune obvious noise and show where code lives.

2) **Feature Detection (Stage 1)** — contracts & wiring  
   `APIS.txt` → public surfaces across languages (Rust `pub` items; TS/JS exports; Python `def`/`class` that aren’t private).  
   `DEPS.txt` → dependency snapshots (e.g., `cargo tree` summaries).  
   Goal: expose what modules promise to the outside.

3) **Focused Gaze (Stage 2)** — compressed code skeleton  
   `PACK_STAGE2_COMPRESSED.xml` → **internal Tree-sitter** extracts **signatures only** (functions/methods/classes); bodies stripped.  
   Goal: preserve intent and shape, not implementation tokens.

`GUIDE.txt` explains how to use the pack; `PROJECT.txt` captures repo intent, entry points, and your current task.

---

## Quickstart

```bash
# Build once
cargo build --workspace

# Run inside any repository
./target/debug/saccade

# Artifacts go to ./ai-pack/
# On Windows PowerShell:
# .\target\debug\saccade.exe
````

Scan another repo:

```bash
cd /path/to/other/repo
"/path/to/saccade/target/debug/saccade.exe" --out ai-pack-other
```

---

## CLI

```text
saccade [options]

  -o, --out <dir>          Output directory (default: ai-pack)
      --max-depth <N>      Structure tree depth (default: 3)
      --git-only           Use Git tracked/unignored files
      --no-git             Force non-Git enumeration
      --include "<re,...>" Keep paths matching any regex (comma-separated)
      --exclude "<re,...>" Drop paths matching any regex
      --code-only          Keep only code/config/markup + bare build files
  -v, --verbose            Verbose logs
      --version            Show version
      -h, --help           Help
```

**Examples**

```bash
# Minimal
saccade

# Smaller, more focused
saccade --git-only --code-only --max-depth 2

# Targeted scan (regex): only src/ and tools/, skip tests and fixtures
saccade --include "^(src|tools)/" --exclude "(^|/)__?tests__?(/|$)|fixtures"
```

---

## What’s in the pack?

* `GUIDE.txt` — How to use the pack & Ask-for-Files protocol
* `PROJECT.txt` — Intent, entry points, current task
* `STRUCTURE.txt` — Directory tree (depth-limited), file index, size heatmap
* `APIS.txt` — Public/API surfaces (Rust/TS-JS/Python)
* `DEPS.txt` — Dependency overview (e.g., Cargo tree, duplicates)
* `PACK_STAGE2_COMPRESSED.xml` — **Signatures-only** skeleton (Tree-sitter)

> Design goal: the **sum** of these files must be *smaller* than dumping a repo but **more reliable** for planning and patching.

---

## Token-budget tips

* Use `--code-only` to drop prose/assets.
* Narrow with `--include/--exclude` (regex).
* Reduce `--max-depth` for shallower trees.
* Large monorepos: scan subtrees separately.

---

## Language support (Stage 2)

* **TypeScript / JavaScript** (`.ts/.tsx/.js/.jsx/.mjs/.cjs`): exported functions/classes; bodies removed.
* **Rust** (`.rs`): `pub fn` and `pub trait` signatures (no bodies).
* **Python** (`.py`): public `def`/`class` (names not starting with `_`), bodies removed.

APIs in `APIS.txt` may include additional ecosystems (language-specific heuristics).

---

## Safety & performance

* Honors `.gitignore` via `git ls-files` when available.
* Prunes vendor/build/cache directories by default.
* Drops secrets (`.env*`, keys, certs) and binaries/media.
* Bounded passes; no recursion in hot paths.

---

## Contributing

Issues and PRs welcome. Please keep functions small, loops bounded, and add tests for new filters/parsers.

## License

MIT

