````markdown
# 👁️ Saccade

**Give your AI eyes — the sensory organ for your codebase.**

Saccade scans your repo and produces a layered, token‑efficient “context pack” that lets LLMs *see* the architecture without slurping every byte. It pairs perfectly with **ApplyDiff** to form an end‑to‑end loop: scan → reason → patch.

---

## Why Saccade?

LLMs are powerful but *context‑hungry*. Pasting raw repos is slow, expensive, and leaky. Saccade mimics human vision:

1. **Peripheral Vision (Stage 0)** – Fast global map
   - `STRUCTURE.txt` — shallow tree (dirs/files)
   - `TOKENS.txt` — size heatmap (top 50 by bytes)
   - `FILE_INDEX.txt` — filtered file list
2. **Feature Detection (Stage 1)** – Contracts & wiring
   - `CARGO_TREE_DEDUP.txt` (if Rust)
   - `API_SURFACE_*` — Rust pub items; TS/JS exports; Python defs/classes; Go exported funcs
3. **Focused Gaze (Stage 2)** – Compressed code skeleton (optional)
   - `PACK_STAGE2_COMPRESSED.xml` via Repomix (imports, signatures, types – no bodies)

A simple **Ask‑for‑Files Protocol** teaches the AI to request raw source on demand.

---

## Quickstart

```bash
# Clone or curl the single-file script
git clone https://github.com/junovhs/saccade.git
cd saccade
chmod +x saccade

# Run inside any project
./saccade

# The pack appears in ./ai-pack/
````

Optional: add to your PATH:

```bash
export PATH="$PATH:$(pwd)"
```

---

## CLI

```text
saccade [options]

  -o, --out <dir>          Output directory (default: ai-pack)
      --max-depth <N>      Stage-0 overview depth (default: 3)
      --git-only           Use Git tracked/unignored files (default in Git repos)
      --no-git             Force find-based enumeration
      --include "<re,...>" Only include paths matching any of the regexes (comma-separated, case-insensitive)
      --exclude "<re,...>" Exclude paths matching any of the regexes
      --code-only          Restrict Stage-0 lists to code/config/markup
  -v, --verbose            Verbose logs
      --version            Show version
      -h, --help           Help
```

**Examples**

```bash
# Minimal
saccade

# Strict and small
saccade --git-only --code-only --max-depth 2

# Focus on src/ and tools/, but skip migrations
saccade --include "^(src|tools)/" --exclude "migrations|fixtures"
```

---

## What’s in the Pack?

* `OVERVIEW.md` – Fill this once; it orients the AI.
* `STRUCTURE.txt` – Directories/files (depth‑limited).
* `TOKENS.txt` – File size heatmap (~token estimate).
* `FILE_INDEX.txt` – Filtered file list.
* `LANGUAGES.md` – Extension snapshot.
* `CARGO_TREE_DEDUP.txt` – Rust deps (if Cargo).
* `API_SURFACE_RUST.txt` – Public Rust items (`pub`).
* `API_SURFACE_TS.txt` – TS/JS/TSX/JSX exports & defs.
* `API_SURFACE_PYTHON.txt` – `def`/`class` signatures.
* `API_SURFACE_GO.txt` – Exported Go functions.
* `PACK_STAGE2_COMPRESSED.xml` – (Optional) compressed skeleton via Repomix.
* `REQUEST_PROTOCOL.md` – YAML ask‑for‑files template.
* `PACK_MANIFEST.json` – Version, timestamp, counts, artifact sizes.

---

## Safety & Performance

* **Respects `.gitignore` by default** (uses `git ls-files` if in a repo).
* **Prunes** common vendor/build/cache directories (`node_modules`, `dist`, `target`, etc.).
* **Excludes secrets** like `.env*`, `*.pem`, SSH keys, and **ignores binaries** (`png`, `pdf`, `zip`, media, dbs).
* **Zero mandatory dependencies**: `git`, `cargo`, and `repomix` are optional accelerators.

---

## Saccade + ApplyDiff

1. **👁️ Saccade** – Generate the pack.
2. **🧠 Your LLM** – Analyze, propose changes, generate a patch.
3. **🖐️ ApplyDiff** – Apply the AI’s patch safely.

---

## Contributing

PRs and issues welcome! See `CONTRIBUTING.md` for guidelines.

## License

MIT. See `LICENSE`.

