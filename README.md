# 👁️ Saccade

**Give your AI eyes — sensing for your codebase.**

Saccade is a tool that empowers AI to work on vast codebases. It scans your repo and emits a **small, reliable, token-efficient pack** so LLMs can understand structure, APIs, and hotspots **without** ingesting the whole codebase. It includes a built-in Tree-sitter skeletonizer (signatures only; bodies stripped).

---

## The Workflow

**Scenario:** Your API server throws `500` on `/users/:id` after a refactor. You want the AI to diagnose and patch it.

**1) Run Saccade at repo root**
```bash
# Build once
cargo build --workspace

# Run in the repo you want to analyze
./target/debug/saccade     # Windows: .\target\debug\saccade.exe
# Output appears in ./ai-pack/
````

**2) Attach these files to the AI (Round 1)**

* `GUIDE.txt`
* `PROJECT.txt`
* `STRUCTURE.txt`
* `APIS.txt`
* `DEPS.txt` (if present)

**3) Include (or don't, it will prompt you if not) a message like this:**

```
Goal: /users/:id returns 500 since yesterday. Want a minimal fix + test.
Evidence: stack shows Null pointer in users_service.get_user; logs attached below.
Context: Entrypoint is server/src/main.rs; users routes in server/src/routes/users.rs.
Constraints: keep API stable; Rust 1.79; clippy clean; add a failing test first.
Definition of Done: green tests, same CLI flags, no public API break.

Please use the Saccade pack to map the repo. If you need code, request the smallest file slice with REQUEST_FILE. Only ask for PACK_STAGE2_COMPRESSED.xml if layout/signatures are unclear.
```

**4) When asked, provide the exact file or slice**

```yaml
REQUEST_FILE:
  path: server/src/services/users_service.rs
  reason: Debug get_user on 500 path
  range: lines 40-120
```

**Escalate only if needed:**
If the AI is unsure about layout/contracts, or the **first attempt fails**, then attach `PACK_STAGE2_COMPRESSED.xml` (signatures-only skeleton).

---

## What goes in the pack (and why)

* **GUIDE.txt** — Built-in protocol (AI-led), escalation rules, `REQUEST_FILE` format.
* **PROJECT.txt** — Repo intent, entry points, current task.
* **STRUCTURE.txt** — Depth-limited tree, filtered index, size heatmap.
* **APIS.txt** — Public/API surfaces across languages.
* **DEPS.txt** — Dependency snapshot (e.g., `cargo tree`) when applicable.
* **PACK_STAGE2_COMPRESSED.xml** — Signatures-only skeleton (attach only if needed).

**Best practice:** keep Round-1 tiny, then send **precise code ranges** as requested. Minimal tokens → maximal reliability.

---

## Quick commands

Scan another repo:

```bash
cd /path/to/other/repo
"/path/to/saccade/target/debug/saccade.exe" --out ai-pack-other
```

Size levers:

* `--code-only` (drop non-code)
* `--include/--exclude "<regex,regex>"` (focus scope)
* `--max-depth N` (shallower tree)

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

* Prefer **line ranges**; avoid whole files.
* Use `STRUCTURE.txt` and `APIS.txt` to pick targets.
* Don’t guess — request missing code explicitly.

---

## Why “Saccade”?

A **saccade** is a rapid eye movement (3–5/sec) that re-aims your fovea at the most informative spot. Humans do: **peripheral scan → feature guidance → focused inspection**.
Saccade mirrors this for code:

* **Peripheral:** `STRUCTURE.txt` (map/size/index)
* **Features:** `APIS.txt`, `DEPS.txt` (contracts/wiring)
* **Focus:** `PACK_STAGE2_COMPRESSED.xml` (signatures) → precisely requested source slices

---

## Safety & performance

* Honors `.gitignore` when available (uses `git ls-files`).
* Prunes vendor/build/cache dirs; drops secrets & binaries.
* Bounded passes; small, single-purpose functions.

---

## Contributing

Keep functions ≤60 SLOC, loops bounded, explicit errors, tests for filters/parsers. PRs welcome.

## License

MIT