# 👁️ Saccade

**Give your AI eyes — sensing for your codebase.**

Saccade is a tool that empowers AI to work on vast codebases. It scans your repo and emits a **small, reliable, token-efficient pack** so LLMs can understand structure, APIs, and hotspots **without** ingesting the whole codebase. It includes a built-in Tree-sitter skeletonizer (signatures only; bodies stripped) with **parallel processing for 4x faster performance**.

---

## ✨ What's New in v0.3.1

- 🚀 **4x faster Stage-2 processing** with Rayon parallelization (1000 files in ~8s vs ~30s)
- 🪟 **Windows support hardened** - proper path handling, secrets filtering, and clickable file:// links
- 🔒 **Security improvements** - secrets never leak on any platform
- 🎯 **Better TypeScript/TSX support** - correct grammar selection per file extension
- 💪 **Graceful error handling** - WalkDir errors don't crash the entire run
- ✅ **14/14 integration tests passing** - comprehensive test coverage

---

## Quick Start

```bash
# Clone and build (release mode recommended)
git clone https://github.com/yourusername/saccade
cd saccade
cargo build --release

# Run on your project
cd /path/to/your/project
/path/to/saccade/target/release/saccade --verbose

# Output appears in ./ai-pack/
```

---

## The Workflow

**Scenario:** Your API server throws `500` on `/users/:id` after a refactor. You want the AI to diagnose and patch it.

### 1) Run Saccade at repo root

```bash
# One-time build
cargo build --release

# Run in the repo you want to analyze
./target/release/saccade --verbose
# Windows: .\target\release\saccade.exe --verbose

# Output appears in ./ai-pack/
```

### 2) Attach these files to the AI (Round 1)

- `GUIDE.txt` - How to use the pack
- `PROJECT.txt` - Overview, metadata, languages
- `STRUCTURE.txt` - Directory tree, file index, size heatmap
- `APIS.txt` - Public surfaces (Rust/TS/Python/Go)
- `DEPS.txt` - Dependencies (when present)

### 3) Provide context (or don't - AI will prompt if needed)

```
Goal: /users/:id returns 500 since yesterday. Want a minimal fix + test.
Evidence: stack shows Null pointer in users_service.get_user; logs attached below.
Context: Entrypoint is server/src/main.rs; users routes in server/src/routes/users.rs.
Constraints: keep API stable; Rust 1.79; clippy clean; add a failing test first.
Definition of Done: green tests, same CLI flags, no public API break.

Please use the Saccade pack to map the repo. If you need code, request the smallest 
file slice with REQUEST_FILE. Only ask for PACK_STAGE2_COMPRESSED.xml if layout/signatures 
are unclear.
```

### 4) Provide exact files when requested

```yaml
REQUEST_FILE:
  path: server/src/services/users_service.rs
  reason: Debug get_user on 500 path
  range: lines 40-120
```

**Escalate only if needed:**  
If the AI is unsure about layout/contracts, or the **first attempt fails**, then attach `PACK_STAGE2_COMPRESSED.xml` (signatures-only skeleton).

---

## What Goes in the Pack (and Why)

| File | Purpose | When to Use |
|------|---------|-------------|
| **GUIDE.txt** | Protocol, escalation rules, REQUEST_FILE format | Round 1 (always) |
| **PROJECT.txt** | Repo intent, entry points, metadata | Round 1 (always) |
| **STRUCTURE.txt** | Depth-limited tree, file index, size heatmap | Round 1 (always) |
| **APIS.txt** | Public/API surfaces across languages | Round 1 (always) |
| **DEPS.txt** | Dependency snapshot (cargo tree, npm list, etc.) | Round 1 (if present) |
| **PACK_STAGE2_COMPRESSED.xml** | Signatures-only skeleton | Round 2+ (escalation) |

**Best practice:** Keep Round-1 tiny (~50KB), then send **precise code ranges** as requested. Minimal tokens → maximal reliability.

---

## Performance

Saccade processes files in parallel for maximum speed:

| Files | Time (v0.3.1) | Memory |
|-------|---------------|--------|
| 1,000 | ~8 seconds | <300MB |
| 5,000 | ~30 seconds | <500MB |
| 10,000+ | Use `--code-only` | Varies |

Files larger than 5MB are automatically skipped in Stage-2 compression.

**Benchmark your repo:**
```bash
time ./target/release/saccade --verbose
```

---

## Command Line Options

```bash
saccade [OPTIONS]

OPTIONS:
  -o, --out <DIR>              Output directory [default: ai-pack]
      --max-depth <N>          Stage-0 tree depth (1-10) [default: 3]
      --code-only              Keep only code/config/markup files
      --include <PATTERNS>     Include paths matching regex (comma-separated)
      --exclude <PATTERNS>     Exclude paths matching regex (comma-separated)
      --git-only               Force Git file enumeration
      --no-git                 Force find-based enumeration (skip .gitignore)
      --dry-run                Show stats without generating files
  -v, --verbose                Verbose logging
  -h, --help                   Print help
  -V, --version                Print version
```

### Examples

```bash
# Scan another repo
cd /path/to/other/repo
/path/to/saccade/target/release/saccade --out ai-pack-other

# Focus on source code only
saccade --code-only --max-depth 2

# Include only Rust files
saccade --include ".*\.rs$" --code-only

# Exclude test directories
saccade --exclude "tests/,test/"

# Preview what would be generated
saccade --dry-run --verbose
```

---

## REQUEST_FILE Protocol

```yaml
REQUEST_FILE:
  path: relative/path/to/file.ext
  reason: >
    What you will inspect or implement.
  range: lines 80-140        # or: symbol: FunctionName
```

**Rules:**
- Prefer **line ranges** over whole files
- Use `STRUCTURE.txt` and `APIS.txt` to pick targets
- Don't guess — request missing code explicitly

**Examples:**

```yaml
# Single file with line range
REQUEST_FILE:
  path: src/server/handlers/users.rs
  reason: Debug the get_user handler returning 500
  range: lines 80-140

# Function by symbol name
REQUEST_FILE:
  path: src/lib.rs
  reason: Understand validate_token implementation
  range: symbol: validate_token

# Entire file (use sparingly)
REQUEST_FILE:
  path: config/settings.toml
  reason: Check database configuration
```

---

## Why "Saccade"?

A **saccade** is a rapid eye movement (3–5/sec) that re-aims your fovea at the most informative spot. Humans naturally: **peripheral scan → feature guidance → focused inspection**.

Saccade mirrors this for code:

- **Peripheral:** `STRUCTURE.txt` (map/size/index) - "where is everything?"
- **Features:** `APIS.txt`, `DEPS.txt` (contracts/wiring) - "what's available?"
- **Focus:** `PACK_STAGE2_COMPRESSED.xml` (signatures) → precise source slices - "how does it work?"

This mimics human vision: you don't stare at everything equally — you scan quickly, focus precisely, and minimize wasted attention.

---

## Safety & Cross-Platform Support

### Security
- Automatically excludes secrets (`.env`, private keys, `.pem`, etc.)
- Filters binary files (images, videos, archives, executables)
- Respects `.gitignore` when in Git repos
- Redacts emails and sensitive URLs in dependency output

### Cross-Platform
- **Linux** ✅ Fully tested
- **macOS** ✅ Fully tested  
- **Windows** ✅ Fully tested (v0.3.1+)
  - Proper backslash path handling
  - Clickable `file://` URIs with percent-encoding
  - Secrets filtering works correctly

### Performance
- Parallel file processing with Rayon
- Bounded memory usage (<500MB for large repos)
- File size limits (skip files >5MB)
- Progress reporting in verbose mode

---

## Building from Source

```bash
# Prerequisites: Rust 1.70+
git clone https://github.com/yourusername/saccade
cd saccade

# Build debug version (fast compilation)
cargo build --workspace

# Build release version (optimized, 4x faster)
cargo build --release

# Run tests
cargo test --workspace

# Run full integration test suite
cd gauntlet
cargo run
```

### Development

```bash
# Watch mode for development
cargo watch -x "build --workspace"

# Run with verbose output
./target/debug/saccade --verbose

# Benchmark performance
hyperfine './target/release/saccade'
```

---

## Testing

Saccade has comprehensive test coverage:

- **7 unit tests** in core modules (filters, Stage-2, etc.)
- **14 integration tests** via gauntlet test harness
- Tests cover: secrets filtering, path handling, API extraction, dry-run, and more

```bash
# Run all tests
cargo test --workspace

# Run integration tests
cd gauntlet
cargo run

# Expected output:
# ========================================
#  Gauntlet Summary:
#    Total: 14
#    Pass : 14
#    Fail : 0
#    Skip : 0
# ========================================
```

---

## Architecture

```
saccade/
├── cli/               # Command-line interface
├── core/              # Core library
│   ├── config.rs      # Configuration and constants
│   ├── enumerate.rs   # File discovery (git/walkdir)
│   ├── filter.rs      # Security filtering (secrets, binaries)
│   ├── stage0.rs      # Structure, index, heatmap
│   ├── stage1.rs      # API extraction + dependencies
│   ├── stage2.rs      # Tree-sitter skeletonization
│   └── parser.rs      # Language-specific parsing
└── gauntlet/          # Integration test suite
```

**Key principles:**
- Functions ≤60 SLOC
- Bounded loops and passes
- Explicit error handling
- Zero dependencies on external APIs

---

## Contributing

We welcome contributions! Please:

1. Keep functions small (≤60 SLOC)
2. Add tests for new features
3. Ensure `cargo test --workspace` passes
4. Run `cd gauntlet && cargo run` for integration tests
5. Update documentation for user-facing changes

**Areas where help is welcome:**
- Additional language support (Java, C++, etc.)
- REQUEST_FILE glob pattern matching
- Incremental/cached pack generation
- Better token estimation (tiktoken integration)

---

## Roadmap

- [ ] REQUEST_FILE with glob patterns (`tests/**/*_test.rs`)
- [ ] Incremental updates (cache unchanged files)
- [ ] Optional `--prune-tests` flag (keep tests by default)
- [ ] Accurate token counting with tiktoken-rs
- [ ] Streaming writes for massive monorepos (>50k files)
- [ ] More language grammars (Java, C++, C#, etc.)

---

## FAQ

**Q: How big are the packs?**  
A: Typically 50-200KB for Round-1 (GUIDE + PROJECT + STRUCTURE + APIS + DEPS). Stage-2 XML adds ~500KB-2MB depending on codebase size.

**Q: Does it work on Windows?**  
A: Yes! v0.3.1+ has full Windows support with proper path handling and clickable file:// links.

**Q: Can I use it on private codebases?**  
A: Yes! Saccade runs entirely locally. Nothing leaves your machine.

**Q: What languages are supported?**  
A: Currently: **Rust**, **TypeScript/JavaScript**, **Python**, **Go**. Tree-sitter makes adding more languages easy.

**Q: How does it compare to uploading my entire codebase?**  
A: Saccade packs are 100-1000x smaller than full repos, contain no secrets, and guide AI to request only what's needed.

**Q: Can I run it in CI/CD?**  
A: Yes! Use `--dry-run` to validate, or generate packs for automated analysis.

---

## License

MIT License - see [LICENSE](LICENSE) for details.

---

## Acknowledgments

- Built with [Tree-sitter](https://tree-sitter.github.io/) for robust parsing
- Inspired by human visual attention patterns (saccadic eye movements)
- Thanks to the Rust community for excellent tooling

---

**Built with ❤️ by Spencer**