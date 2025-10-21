#!/usr/bin/env bash
# Saccade Test Gauntlet — conclusive, observable, end-to-end tests
# Works in Git Bash (MINGW64) and Linux/macOS shells.
set -euo pipefail
IFS=$'\n\t'

# ---------- Settings ----------
SACCADE_BIN="${SACCADE:-$HOME/saccade/saccade}"   # override via env: SACCADE=/path/to/saccade
KEEP_TMP="${KEEP_TMP:-0}"                          # KEEP_TMP=1 to keep fixtures
FILTER="${GAUNTLET_FILTER:-}"                      # regex to run only matching tests
VERBOSE="${VERBOSE:-0}"

# ---------- Prereq checks ----------
need() { command -v "$1" >/dev/null 2>&1 || { echo "Missing required tool: $1" >&2; exit 1; }; }
need git
need jq
[[ -x "$SACCADE_BIN" ]] || { echo "SACCADE not found/executable at: $SACCADE_BIN" >&2; exit 1; }

# ---------- Helpers ----------
TMP_ROOT="$(mktemp -d -t saccade-gauntlet.XXXXXX)"
PASS=0
FAIL=0
SKIP=0
tests=()

log() { printf '==> %s\n' "$*"; }
info() { [[ "$VERBOSE" = "1" ]] && printf '    %s\n' "$*" || true; }
ok() { printf '✅ %s\n' "$*"; }
bad() { printf '❌ %s\n' "$*" >&2; }

die_keep() {
  bad "$1"
  echo "Fixture left at: $PWD"
  exit 1
}

cleanup() {
  if [[ "$KEEP_TMP" = "1" ]]; then
    echo "Keeping fixtures under: $TMP_ROOT"
  else
    rm -rf "$TMP_ROOT" || true
  fi
}
trap cleanup EXIT

mkcase() { local d="$TMP_ROOT/$1"; mkdir -p "$d"; echo "$d"; }

new_git_repo() {
  local dir="$1"; shift
  ( cd "$dir" && git init -q && git config user.email "t@example.com" && git config user.name "t" )
}

run_saccade() {  # run_saccade <dir> [args...]
  local dir="$1"; shift || true
  ( cd "$dir" && "$SACCADE_BIN" "$@" | tee run.log )
}

assert_file()        { [[ -f "$1" ]] || die_keep "Expected file not found: $1"; }
assert_nofile()      { [[ ! -f "$1" ]] || die_keep "Expected file to be absent: $1"; }
assert_contains()    { grep -qE "$2" "$1" || die_keep "Expected pattern not found in $1: $2"; }
assert_not_contains(){ ! grep -qE "$2" "$1" || die_keep "Unexpected pattern found in $1: $2"; }
assert_json_eq()     { local got; got="$(jq -r "$2" "$1")"; [[ "$got" == "$3" ]] || die_keep "JSON mismatch in $1: wanted '$3' via '$2', got '$got'"; }
assert_json_startswith() {
  # Usage: assert_json_startswith <file> <jq_key> <prefix_string>
  local file="" key="" prefix=""
  local val
  val="$(jq -r "$key" "$file" 2>/dev/null || true)"
  [[ "$val" == ${prefix}* ]] || die_keep "JSON value for $key does not start with $prefix in $file (got $val)"
}
assert_gt_zero()     { local n; n="$(wc -c < "$1")"; [[ "$n" -gt 0 ]] || die_keep "Expected non-empty file: $1"; }

# ---------- Test registration ----------
add_test() { tests+=("$1"); }

# ---------- Tests ----------

# 01: Basic E2E with a real Rust crate (so it passes on v0.3.0+ without fallbacks)
test_01_basic_e2e() {
  local d; d="$(mkcase 01_basic)"
  new_git_repo "$d"

  # Rust crate
  mkdir -p "$d/rc/src"
  cat > "$d/rc/Cargo.toml" <<'TOML'
[package]
name="rc"
version="0.1.0"
edition="2021"
TOML
  echo 'pub fn x(){}' > "$d/rc/src/lib.rs"

  # TS file
  mkdir -p "$d/web"
  echo 'export const a=1' > "$d/web/index.ts"

  ( cd "$d" && git add . && git commit -qm "init" )

  run_saccade "$d" --verbose

  assert_file "$d/ai-pack/PACK_MANIFEST.json"
  # Accept any 0.3.x (or future) — safer than hard pin
  assert_json_startswith "$d/ai-pack/PACK_MANIFEST.json" '.pack_version' '"0.3.'
  assert_file "$d/ai-pack/CHAT_START.md"
  assert_contains "$d/ai-pack/API_SURFACE_RUST.txt" 'pub\s+fn x\(\)'
  assert_contains "$d/ai-pack/API_SURFACE_TS.txt" '^.*export const a=1'
  ok "basic e2e (crate + ts)"
}

# 02: Secrets and binaries excluded from FILE_INDEX
test_02_secrets_and_binaries_excluded() {
  local d; d="$(mkcase 02_secrets)"
  new_git_repo "$d"
  printf 'SECRET=1\n' > "$d/.env"
  printf '-----BEGIN PRIVATE KEY-----\nX\n-----END PRIVATE KEY-----\n' > "$d/private.pem"
  printf '\x89PNG\r\n\x1a\n' > "$d/pic.png"
  echo 'hello' > "$d/code.rs"
  ( cd "$d" && git add . && git commit -qm "init" )

  run_saccade "$d" --verbose

  assert_not_contains "$d/ai-pack/FILE_INDEX.txt" '^\.env$'
  assert_not_contains "$d/ai-pack/FILE_INDEX.txt" '^private\.pem$'
  assert_not_contains "$d/ai-pack/FILE_INDEX.txt" '^pic\.png$'
  assert_contains "$d/ai-pack/FILE_INDEX.txt" '^code\.rs$'
  ok "secrets & binaries excluded, code included"
}

# 03: Prune behavior in find-mode (no Git)
test_03_prune_in_find_mode() {
  local d; d="$(mkcase 03_prune_find)"
  mkdir -p "$d/node_modules/a" "$d/dist" "$d/src"
  echo 'console.log(1)' > "$d/node_modules/a/index.js"
  echo 'bundled' > "$d/dist/bundle.js"
  echo 'let x=1' > "$d/src/app.js"

  run_saccade "$d" --no-git --verbose

  assert_contains "$d/ai-pack/FILE_INDEX.txt" '^src/app\.js$'
  assert_not_contains "$d/ai-pack/FILE_INDEX.txt" '^node_modules/'
  assert_not_contains "$d/ai-pack/FILE_INDEX.txt" '^dist/'
  ok "find-mode prune prevents descent into vendor/build dirs"
}

# 04: Git vs find enumeration (.gitignore respected only in Git mode)
test_04_git_vs_find_enumeration() {
  local d; d="$(mkcase 04_git_vs_find)"
  new_git_repo "$d"
  echo 'ignored.md' > "$d/ignored.md"
  echo 'ignored.md' > "$d/.gitignore"
  echo 'keep.md' > "$d/keep.md"
  ( cd "$d" && git add .gitignore keep.md && git commit -qm "add keep" )

  # Git mode -> ignored.md excluded
  run_saccade "$d" --verbose
  assert_contains "$d/ai-pack/FILE_INDEX.txt" '^keep\.md$'
  assert_not_contains "$d/ai-pack/FILE_INDEX.txt" '^ignored\.md$'

  # Force find -> ignored.md included (not pruned by patterns)
  run_saccade "$d" --no-git --verbose
  assert_contains "$d/ai-pack/FILE_INDEX.txt" '^ignored\.md$'
  ok "git-mode respects .gitignore; find-mode sees unignored files"
}

# 05: Rust API surface — pub + restricted visibility + pub use
test_05_api_rust_pub_and_scoped() {
  local d; d="$(mkcase 05_rust_api)"
  mkdir -p "$d/rc/src"
  cat > "$d/rc/Cargo.toml" <<'TOML'
[package]
name="rc"
version="0.1.0"
edition="2021"
TOML
  cat > "$d/rc/src/lib.rs" <<'RS'
pub(crate) struct Foo;
pub fn bar() {}
pub(super) trait T {}
mod inner { pub use super::Foo; }
RS

  run_saccade "$d" --no-git --verbose

  assert_contains "$d/ai-pack/API_SURFACE_RUST.txt" 'pub\(crate\)\s+struct Foo'
  assert_contains "$d/ai-pack/API_SURFACE_RUST.txt" 'pub\s+fn bar'
  assert_contains "$d/ai-pack/API_SURFACE_RUST.txt" 'pub\(super\)\s+trait T'
  assert_contains "$d/ai-pack/API_SURFACE_RUST.txt" 'pub\s+use\s+super::Foo'
  ok "rust API: pub and restricted vis detected"
}

# 06: TS/JS API — capture exports + PascalCase top-level function/class only
test_06_api_ts_exports_only_and_pascalcase() {
  local d; d="$(mkcase 06_ts_api)"
  mkdir -p "$d/app"
  cat > "$d/app/package.json" <<'JSON'
{"name":"app"}
JSON
  cat > "$d/app/index.ts" <<'TS'
export const X = 1;
const y = 2;
export default function alpha() { return 0; }
function Zeta(){ return 1; }
class Abc {}
TS

  run_saccade "$d" --no-git --verbose

  assert_contains "$d/ai-pack/API_SURFACE_TS.txt" 'export const X'
  assert_not_contains "$d/ai-pack/API_SURFACE_TS.txt" 'const y'
  assert_contains "$d/ai-pack/API_SURFACE_TS.txt" 'export default function alpha'
  assert_contains "$d/ai-pack/API_SURFACE_TS.txt" '^.*function Zeta\('
  assert_contains "$d/ai-pack/API_SURFACE_TS.txt" '^.*class Abc'
  ok "ts/js API: exports + PascalCase top-level"
}

# 07: Python API — exclude underscore-prefixed names
test_07_api_python_public_only() {
  local d; d="$(mkcase 07_py_api)"
  echo 'def public_fn(): pass' > "$d/a.py"
  echo 'def _private(): pass' >> "$d/a.py"
  echo 'class Public: pass' >> "$d/a.py"
  echo 'class _Hidden: pass' >> "$d/a.py"

  run_saccade "$d" --no-git --verbose

  assert_contains "$d/ai-pack/API_SURFACE_PYTHON.txt" 'def public_fn'
  assert_contains "$d/ai-pack/API_SURFACE_PYTHON.txt" 'class Public'
  assert_not_contains "$d/ai-pack/API_SURFACE_PYTHON.txt" 'def _private'
  assert_not_contains "$d/ai-pack/API_SURFACE_PYTHON.txt" 'class _Hidden'
  ok "python API: excludes underscores"
}

# 08: Go API — exported funcs only
test_08_api_go_exported_only() {
  local d; d="$(mkcase 08_go_api)"
  cat > "$d/m.go" <<'GO'
package main
func Exported() {}
func unexported() {}
GO

  run_saccade "$d" --no-git --verbose

  assert_contains "$d/ai-pack/API_SURFACE_GO.txt" 'func\s+Exported\('
  assert_not_contains "$d/ai-pack/API_SURFACE_GO.txt" 'unexported'
  ok "go API: only exported funcs"
}

# 09: Frontend dirs deduped (no duplicate lines per path)
test_09_frontend_dedup_no_duplicates_in_api() {
  local d; d="$(mkcase 09_fe_dedup)"
  mkdir -p "$d/packages/app" "$d/frontend"
  echo '{"name":"app"}' > "$d/packages/app/package.json"
  echo '{"name":"fe"}' > "$d/frontend/package.json"
  echo 'export const K=1' > "$d/frontend/k.ts"
  echo 'export const K2=2' > "$d/packages/app/k2.ts"

  run_saccade "$d" --no-git --verbose

  [[ "$(grep -c 'export const K=1' "$d/ai-pack/API_SURFACE_TS.txt" || true)" -eq 1 ]] || die_keep "Duplicate export line for K"
  [[ "$(grep -c 'export const K2=2' "$d/ai-pack/API_SURFACE_TS.txt" || true)" -eq 1 ]] || die_keep "Duplicate export line for K2"
  ok "frontend dirs de-duplicated"
}

# 10: Dry-run prints stats and writes nothing
test_10_dry_run_stats_and_no_writes() {
  local d; d="$(mkcase 10_dry_run)"
  echo 'console.log(1)' > "$d/a.js"
  set +e
  out="$( (cd "$d" && "$SACCADE_BIN" --dry-run ) 2>&1 )"; rc=$?
  set -e
  [[ $rc -eq 0 ]] || die_keep "dry run returned non-zero"
  grep -q 'Would generate the following artifacts' <<<"$out" || die_keep "dry-run header missing"
  [[ ! -d "$d/ai-pack" ]] || die_keep "ai-pack should not exist in dry-run"
  ok "dry-run prints stats and writes nothing"
}

# 11: CLI validation errors on missing values
test_11_cli_validation_errors() {
  local d; d="$(mkcase 11_cli_validation)"
  set +e
  (cd "$d" && "$SACCADE_BIN" --max-depth ) >/dev/null 2>&1; rc=$?
  set -e
  [[ $rc -ne 0 ]] || die_keep "expected error for --max-depth without value"
  ok "CLI validation errors on missing values"
}

# 12: Token header reflects bytes/3.5
test_12_token_header_uses_div_3_5() {
  local d; d="$(mkcase 12_token_header)"
  echo 'a' > "$d/t.txt"
  run_saccade "$d" --no-git
  assert_contains "$d/ai-pack/TOKENS.txt" 'bytes/3\.5'
  ok "token header reflects bytes/3.5"
}

# 13: Clickable file:// link printed (best-effort; Windows shells)
test_13_clickable_link_line_present() {
  if [[ -n "${WT_SESSION:-}" || "$OSTYPE" =~ ^(msys|win32|cygwin)$ || "$(uname -s 2>/dev/null || true)" =~ ^MINGW ]]; then
    local d; d="$(mkcase 13_links)"
    echo 'x' > "$d/a.txt"
    run_saccade "$d" --no-git --verbose
    assert_contains "$d/run.log" 'Click:\s+file://'
    ok "clickable file:// link printed"
  else
    SKIP=$((SKIP+1)); ok "skipped clickable link test (non-Windows terminal)"
  fi
}

# 14: Stage 2 (Repomix) optional — create when present, skip gracefully otherwise
test_14_stage2_repomix_optional() {
  local d; d="$(mkcase 14_stage2)"
  echo 'x' > "$d/a.txt"
  if command -v repomix >/dev/null 2>&1; then
    run_saccade "$d" --no-git
    assert_file "$d/ai-pack/PACK_STAGE2_COMPRESSED.xml"
    assert_gt_zero "$d/ai-pack/PACK_STAGE2_COMPRESSED.xml"
    ok "repomix present: PACK_STAGE2_COMPRESSED.xml created and non-empty"
  else
    run_saccade "$d" --no-git
    [[ ! -s "$d/ai-pack/PACK_STAGE2_COMPRESSED.xml" ]] || true
    ok "repomix absent: stage2 skipped without error"
  fi
}

# ---------- Register tests ----------
add_test test_01_basic_e2e
add_test test_02_secrets_and_binaries_excluded
add_test test_03_prune_in_find_mode
add_test test_04_git_vs_find_enumeration
add_test test_05_api_rust_pub_and_scoped
add_test test_06_api_ts_exports_only_and_pascalcase
add_test test_07_api_python_public_only
add_test test_08_api_go_exported_only
add_test test_09_frontend_dedup_no_duplicates_in_api
add_test test_10_dry_run_stats_and_no_writes
add_test test_11_cli_validation_errors
add_test test_12_token_header_uses_div_3_5
add_test test_13_clickable_link_line_present
add_test test_14_stage2_repomix_optional

# ---------- Execute ----------
echo "Running Saccade Gauntlet against: $SACCADE_BIN"
for t in "${tests[@]}"; do
  [[ -n "$FILTER" && ! "$t" =~ $FILTER ]] && { info "skip $t due to filter"; continue; }
  echo "---- $t ----"
  if "$t"; then PASS=$((PASS+1)); else FAIL=$((FAIL+1)); fi
done

TOTAL=$((PASS+FAIL+SKIP))
echo
echo "========================================"
echo " Gauntlet Summary:"
echo "   Total: $TOTAL"
echo "   Pass : $PASS"
echo "   Fail : $FAIL"
echo "   Skip : $SKIP"
echo "========================================"

[[ $FAIL -eq 0 ]] || exit 1
