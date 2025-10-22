
# 👁️ Saccade

**Give your AI eyes — the sensory organ for your codebase.**

Saccade scans a repo and produces a **small, reliable, token-efficient pack** so LLMs can understand structure, APIs, and hotspots **without** slurping the whole tree. It includes a built-in Tree-sitter skeletonizer (signatures only; bodies stripped).

---

## What to attach & when (progressive by default)

**Round 1 (default, smallest):**  
Attach `GUIDE.txt`, `PROJECT.txt`, `STRUCTURE.txt`, `APIS.txt`, `DEPS.txt` (if present).

**Escalate only if needed:**  
If the model is unsure about layout/contracts or the first attempt fails, attach `PACK_STAGE2_COMPRESSED.xml` (signatures-only skeleton).

**Rounds 3+:**  
Provide code *only* via the `REQUEST_FILE` block (file + line range). Goal: **minimal tokens, maximal reliability**.

> You don’t have to instruct the model — the **protocol is embedded in `GUIDE.txt`**.  
> If you paste nothing, the AI should still triage and lead the session.

---

## Quickstart

```bash
# Build once
cargo build --workspace

# Run inside any repository
./target/debug/saccade        # Windows: .\target\debug\saccade.exe
# Artifacts appear in ./ai-pack/
````

Scan another repo:

```bash
cd /path/to/other/repo
"/path/to/saccade/target/debug/saccade.exe" --out ai-pack-other
```

Key size levers: `--code-only`, `--include/--exclude` (regex), `--max-depth`.

---

## Files in the pack

* **GUIDE.txt** — Operational protocol (AI-led), escalation rules, `REQUEST_FILE` format.
* **PROJECT.txt** — Intent, entry points, current task.
* **STRUCTURE.txt** — Depth-limited tree, filtered index, size heatmap.
* **APIS.txt** — Public/API surfaces across languages.
* **DEPS.txt** — Dependency snapshot (e.g., `cargo tree`) when applicable.
* **PACK_STAGE2_COMPRESSED.xml** — Signatures-only skeleton (attach only if needed).

---

## REQUEST_FILE protocol

```yaml
REQUEST_FILE:
  path: relative/path/to/file.ext
  reason: >
    What you will inspect or implement.
  range: lines 80-140        # or: symbol: FunctionName
```

Rules:

* Prefer line ranges; avoid whole files.
* Use `STRUCTURE.txt` and `APIS.txt` to pick targets.
* Never hallucinate—request missing code explicitly.

---

## Language support (Stage 2)

* **TS/JS**: exported functions/classes; bodies removed.
* **Rust**: `pub` functions/traits signatures only.
* **Python**: public `def`/`class` (names not starting with `_`), bodies removed.

---

## Safety & performance

* Honors `.gitignore` (via `git ls-files` when available).
* Prunes vendor/build/cache directories.
* Drops secrets (`.env*`, keys, certs) and binaries/media.
* Bounded passes; no recursion in hot paths.

---

## Contributing

Keep functions small, loops bounded, add tests for filters/parsers. PRs welcome.

## License

MIT

```

