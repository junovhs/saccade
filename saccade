#!/usr/bin/env bash
# Saccade v0.3.0 — API surface accuracy, UX polish, Windows paths, dry-run stats
# Zero-dep Bash script to generate a staged, token-efficient context pack for LLMs.
# Optional tools: git, cargo, repomix, python3 (JSON escaping), clip/pbcopy/xclip/xsel (clipboard)
set -euo pipefail
IFS=$'\n\t'
export LC_ALL=C LANG=C

VERSION="0.3.0"

# ---------- Logging & utils ----------
log()  { printf '==> %s\n' "$*" >&2; }
info() { printf '    %s\n' "$*" >&2; }
warn() { printf 'WARN: %s\n' "$*" >&2; }
die()  { printf 'ERROR: %s\n' "$*" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

usage() {
  cat <<USAGE
Saccade v${VERSION}

Usage: saccade [options]

Options:
  -o, --out <dir>          Output directory for the AI pack (default: ai-pack)
      --max-depth <N>      Stage-0 overview depth (1..10, default: 3)
      --git-only           Prefer Git tracked/unignored files (default in Git repos)
      --no-git             Do not use Git; use find-based enumeration
      --include "<re,...>" Only include paths matching at least one regex (comma-separated)
      --exclude "<re,...>" Exclude paths matching any regex (comma-separated)
      --code-only          Keep only code/config/markup files in Stage-0 lists
      --dry-run            Show stats and what would be generated, then exit
  -v, --verbose            Verbose logging
      --version            Print version and exit
  -h, --help               Show this help
USAGE
}

# ---------- Defaults ----------
PACK_DIR="ai-pack"
MAX_DEPTH=3
USE_GIT="auto"         # auto | yes | no
INCLUDE_PATTERNS=""
EXCLUDE_PATTERNS=""
CODE_ONLY="no"
DRY_RUN="no"
VERBOSE="no"

# ---------- Parse args (strict value checks where required) ----------
while [[ $# -gt 0 ]]; do
  case "${1}" in
    -o|--out)
      [[ -n "${2:-}" ]] || die "--out requires a directory path"; PACK_DIR="$2"; shift 2 ;;
    --max-depth)
      [[ -n "${2:-}" ]] || die "--max-depth requires a number"; MAX_DEPTH="$2"; shift 2 ;;
    --git-only|--use-git)
      USE_GIT="yes"; shift ;;
    --no-git)
      USE_GIT="no"; shift ;;
    --include)
      [[ -n "${2:-}" ]] || die "--include requires a comma-separated regex list"; INCLUDE_PATTERNS="$2"; shift 2 ;;
    --exclude)
      [[ -n "${2:-}" ]] || die "--exclude requires a comma-separated regex list"; EXCLUDE_PATTERNS="$2"; shift 2 ;;
    --code-only)
      CODE_ONLY="yes"; shift ;;
    --dry-run)
      DRY_RUN="yes"; shift ;;
    -v|--verbose)
      VERBOSE="yes"; shift ;;
    --version)
      echo "saccade v${VERSION}"; exit 0 ;;
    -h|--help)
      usage; exit 0 ;;
    *) die "Unknown argument: $1 (see --help)";;
  esac
done

[[ -n "${PACK_DIR}" ]] || die "--out requires a directory path"
[[ "${MAX_DEPTH}" =~ ^[0-9]+$ ]] || die "--max-depth must be a number"
(( MAX_DEPTH>=1 && MAX_DEPTH<=10 )) || die "--max-depth must be between 1 and 10"

# ---------- Traps ----------
cleanup() { [[ -d "${TMP_DIR:-}" ]] && rm -rf "$TMP_DIR" || true; }
trap cleanup EXIT INT TERM

# ---------- Colors & links ----------
if [[ -t 1 ]]; then
  BOLD=$'\033[1m'; DIM=$'\033[2m'; RED=$'\033[31m'; GREEN=$'\033[32m'
  YELLOW=$'\033[33m'; BLUE=$'\033[34m'; MAGENTA=$'\033[35m'; CYAN=$'\033[36m'; RESET=$'\033[0m'
else
  BOLD=; DIM=; RED=; GREEN=; YELLOW=; BLUE=; MAGENTA=; CYAN=; RESET=
fi

USE_OSC8="no"
# Enable OSC-8 hyperlinks on capable terminals
if [[ -n "${WT_SESSION:-}" || "${TERM_PROGRAM:-}" =~ (WezTerm|iTerm|vscode) || "${TERM:-}" =~ (xterm-kitty|foot) ]]; then
  USE_OSC8="yes"
fi
link() {
  # link <url> <label>
  local url="$1" label="$2"
  if [[ "$USE_OSC8" == "yes" && -t 1 ]]; then
    printf '\033]8;;%s\033\\%s\033]8;;\033\\' "$url" "$label"
  else
    printf '%s' "$url"
  fi
}

# ---------- Config / Patterns ----------
# Build prune expression array (portable; no nameref)
# Creates: ( -name .git -o -name node_modules -o ... ); trailing -o removed below
PRUNE_DIRS=( .git node_modules dist build target gen schemas tests test __tests__ .venv venv .tox .cache coverage vendor third_party )
# AWK filter: remove binary extensions and secret files (case-insensitive in practice)
BIN_EXT_RE='(\.(png|jpe?g|gif|svg|ico|icns|webp|woff2?|ttf|otf|pdf|mp4|mov|mkv|avi|mp3|wav|flac|zip|gz|bz2|xz|7z|rar|jar|csv|tsv|parquet|sqlite|db|bin|exe|dll|so|dylib|pkl|onnx|torch|tgz|zst))$'
SECRET_RE='(^\.?env(\..*)?$|/\.?env(\..*)?$|(^|/)(id_rsa(\.pub)?|id_ed25519(\.pub)?|.*\.(pem|p12|jks|keystore|pfx))$'
CODE_EXT_RE='(\.(c|h|cc|hh|cpp|hpp|rs|go|py|js|jsx|ts|tsx|java|kt|kts|rb|php|scala|cs|swift|m|mm|lua|sh|bash|zsh|fish|ps1|sql|html|xhtml|xml|xsd|xslt|yaml|yml|toml|ini|cfg|conf|json|ndjson|md|rst|tex|s|asm|cmake|gradle|proto|graphql|gql|nix|dart|scss|less|css))$'
CODE_BARE_RE='(Makefile|Dockerfile|dockerfile|CMakeLists\.txt|BUILD|WORKSPACE)$'

PRUNE_EXPR=()
for d in "${PRUNE_DIRS[@]}"; do PRUNE_EXPR+=( -name "$d" -o ); done
[[ ${#PRUNE_EXPR[@]} -gt 0 ]] && unset 'PRUNE_EXPR[${#PRUNE_EXPR[@]}-1]'

# ---------- Prefer Git when available (respects .gitignore); fallback to find ----------
IN_GIT="no"
if [[ "${USE_GIT}" == "no" ]]; then
  IN_GIT="no"
elif have git && git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  IN_GIT="yes"
fi
if [[ "${USE_GIT}" == "yes" && "${IN_GIT}" == "no" ]]; then
  die "--git-only requested, but this directory is not a Git repository"
fi
[[ "${VERBOSE}" == "yes" ]] && info "Git repo detected: ${IN_GIT}"

# ---------- Workspace ----------
[[ "${DRY_RUN}" == "yes" ]] || mkdir -p "${PACK_DIR}"
TMP_DIR="$(mktemp -d -t saccade.XXXXXX)"
FILES_RAW_0="${TMP_DIR}/files.raw.0"            # NUL-delimited
FILES_FILTERED="${TMP_DIR}/files.filtered"      # newline (display)
FILES_FILTERED_0="${TMP_DIR}/files.filtered.0"  # NUL-delimited

# ---------- Enumerate files (raw) ----------
log "Enumerating files..."
if [[ "${IN_GIT}" == "yes" ]]; then
  git ls-files -z --exclude-standard > "${FILES_RAW_0}"
  info "Using Git file list (tracked & unignored)"
else
  find . -type l -prune -o \( "${PRUNE_EXPR[@]}" \) -prune -o -type f -print0 > "${FILES_RAW_0}"
  info "Using find-based file list"
fi
RAW_COUNT=$(tr -cd '\0' < "${FILES_RAW_0}" | wc -c | awk '{print $1}')
info "Found ${RAW_COUNT} files (raw)"

# ---------- Filter: secrets, binaries, include/exclude, code-only ----------
log "Filtering file list (secrets, binaries, includes/excludes, code-only=${CODE_ONLY})..."
{
  while IFS= read -r -d '' p; do
    path="${p#./}"
    [[ "$path" =~ $SECRET_RE ]] && continue
    [[ "$path" =~ $BIN_EXT_RE ]] && continue
    if [[ -n "${EXCLUDE_PATTERNS}" ]]; then
      IFS=',' read -r -a _exc <<< "${EXCLUDE_PATTERNS}"
      skip="no"; for re in "${_exc[@]}"; do [[ -z "$re" ]] && continue; [[ "$path" =~ $re ]] && { skip="yes"; break; }; done
      [[ "$skip" == "yes" ]] && continue
    fi
    if [[ -n "${INCLUDE_PATTERNS}" ]]; then
      IFS=',' read -r -a _inc <<< "${INCLUDE_PATTERNS}"
      hit="no"; for re in "${_inc[@]}"; do [[ -z "$re" ]] && continue; [[ "$path" =~ $re ]] && { hit="yes"; break; }; done
      [[ "$hit" == "no" ]] && continue
    fi
    if [[ "${CODE_ONLY}" == "yes" ]]; then
      [[ "$path" =~ $CODE_EXT_RE || "$path" =~ $CODE_BARE_RE ]] || continue
    fi
    printf '%s\0' "$path"
  done
} < "${FILES_RAW_0}" > "${FILES_FILTERED_0}"

tr '\0' '\n' < "${FILES_FILTERED_0}" | sed $'s/\t/\\t/g; s/\r/\\r/g' > "${FILES_FILTERED}"
FILTERED_COUNT=$(tr -cd '\0' < "${FILES_FILTERED_0}" | wc -c | awk '{print $1}')
info "Kept ${FILTERED_COUNT} files after filtering"

# ---------- Discovery (before dry-run so we can report stats) ----------
log "Discovering project layout (Rust crates, front-end dirs)..."
RUST_CRATES=()
while IFS= read -r -d '' cargo_toml; do
  base="$(dirname "$cargo_toml")"
  [[ -d "${base}/src" ]] && RUST_CRATES+=("${base}/src")
done < <(find . -name "Cargo.toml" -not -path "*/target/*" -print0)

FRONTEND_DIRS=()
# First: package.json roots
while IFS= read -r -d '' pkg; do
  FRONTEND_DIRS+=("$(dirname "$pkg")")
done < <(find . \( "${PRUNE_EXPR[@]}" \) -prune -o -type f -name "package.json" -print0)
# Fallback: only if none found; first matching common dir wins
if [[ ${#FRONTEND_DIRS[@]} -eq 0 ]]; then
  for d in app frontend web client ui src; do
    [[ -d "$d" ]] && { FRONTEND_DIRS+=("$d"); break; }
  done
fi
# Deduplicate while preserving order
if [[ ${#FRONTEND_DIRS[@]} -gt 0 ]]; then
  readarray -t FRONTEND_DIRS < <(printf '%s\n' "${FRONTEND_DIRS[@]}" | awk '!seen[$0]++')
fi

# ---------- Dry run summary ----------
if [[ "${DRY_RUN}" == "yes" ]]; then
  log "[Dry Run] Would generate the following artifacts:"
  echo "  - ${FILTERED_COUNT} files would be processed"
  echo "  - Output directory: ${PACK_DIR}"
  [[ "${IN_GIT}" == "yes" ]] && echo "  - Using Git file enumeration" || echo "  - Using find-based file enumeration"
  echo "  - Found ${#RUST_CRATES[@]} Rust crate(s)"
  echo "  - Found ${#FRONTEND_DIRS[@]} frontend dir(s)"
  have repomix && echo "  - Repomix available for Stage 2 compression" || echo "  - Repomix NOT available (Stage 2 skipped)"
  exit 0
fi

# ---------- Helper files (static) ----------
log "Writing helper files..."
cat > "${PACK_DIR}/OVERVIEW.md" <<'MD'
# Project Overview (fill this out once)
- What it does:
- Primary tech (e.g., Rust backend, Tauri desktop, TS/React UI):
- Entrypoints / main binaries:
- Key modules / domains:
- Current task/question for the AI:
MD

cat > "${PACK_DIR}/REQUEST_PROTOCOL.md" <<'MD'
# Ask-for-Files Protocol
You have a staged view:
- STRUCTURE.txt (shallow tree), TOKENS.txt (size heat map)
- Dependency / API surfaces (DEPS, API_SURFACE_*)
- PACK_STAGE2_COMPRESSED.xml (if present; compressed skeleton)
If you need raw source, request:

REQUEST_FILE:
  path: relative/path
  reason: >-
    What you will inspect or implement.
  range: lines 80-140        # or: symbol: Foo::bar

Minimize requests. Produce complete, compilable implementations that match project conventions.
Never hallucinate missing code—request it.
MD

cat > "${PACK_DIR}/PACK_README.md" <<'MD'
# AI Pack — How to Use

Round 1 (tiny upload):
- OVERVIEW.md (fill it out)
- STRUCTURE.txt
- TOKENS.txt
- CARGO_TREE_DEDUP.txt (if Rust)
- API_SURFACE_* (language surfaces)
- REQUEST_PROTOCOL.md

Round 2 (when needed):
- PACK_STAGE2_COMPRESSED.xml (compressed skeleton)

On demand:
- Paste specific files/lines requested via REQUEST_PROTOCOL.md.
MD

# ---------- Stage 0 ----------
log "[Stage 0] Generating STRUCTURE.txt and TOKENS.txt..."
info "Building directory and file snapshots up to depth ${MAX_DEPTH}..."
awk -v DEPTH="${MAX_DEPTH}" '
  BEGIN { FS="/" }
  {
    path=$0
    n = split(path, parts, "/")
    if (n <= DEPTH) files[path]=1
    dir=""
    for (i=1; i<=n-1 && i<=DEPTH; i++) {
      if (dir=="") dir=parts[i]; else dir=dir "/" parts[i]
      dirs[dir]=1
    }
  }
  END {
    print "Directories (depth<=" DEPTH "):"
    for (d in dirs) print d | "sort -f"
    close("sort -f")
    print ""
    print "Files (depth<=" DEPTH ", filtered common junk):"
    for (f in files) print f | "sort -f"
    close("sort -f")
  }
' "${FILES_FILTERED}" > "${PACK_DIR}/STRUCTURE.txt" || true

{
  echo "Heuristic Size Heat Map (bytes; ~tokens ≈ bytes/3.5). Top 50:"
  while IFS= read -r -d '' f; do
    [[ -f "$f" ]] || continue
    bytes=$(wc -c < "$f" 2>/dev/null || echo 0)
    est_tokens=$(awk -v b="$bytes" 'BEGIN{printf "%.0f", b/3.5}')
    printf "%12s bytes  ~%8d tokens  %s\n" "$bytes" "$est_tokens" "$f"
  done < "${FILES_FILTERED_0}" \
  | sort -k1,1nr | head -50
} > "${PACK_DIR}/TOKENS.txt" || true

sort -f "${FILES_FILTERED}" > "${PACK_DIR}/FILE_INDEX.txt" || true

# ---------- Stage 1 ----------
log "[Stage 1] Generating dependency and API surfaces..."
if have cargo && [[ -f "Cargo.toml" ]]; then
  cargo tree -e normal -d > "${PACK_DIR}/CARGO_TREE_DEDUP.txt" || true
fi

info "Processing ${FILTERED_COUNT} files for API surfaces..."

# Rust: match pub + restricted visibility forms, regardless of indentation
# e.g., pub fn, pub(crate) struct, pub(super) trait, pub use, etc.
API_RUST_OUT="${PACK_DIR}/API_SURFACE_RUST.txt"
if [[ ${#RUST_CRATES[@]} -gt 0 ]]; then
  : > "${API_RUST_OUT}"
  for crate in "${RUST_CRATES[@]}"; do
    crate_rel="${crate#./}"
    grep -F "^${crate_rel}/" "${PACK_DIR}/FILE_INDEX.txt" 2>/dev/null || true \
      | grep -E '\.rs$' || true \
      | while IFS= read -r rf; do
          grep -IhnE '^\s*pub(\s+|\s*\([^)]*\)\s+)(fn|struct|enum|trait|type|const|static|use|mod|macro_rules!)' "$rf" || true
        done >> "${API_RUST_OUT}"
  done
  [[ -s "${API_RUST_OUT}" ]] || echo "(no public Rust items found)" > "${API_RUST_OUT}"
else
  echo "(no Rust crates found)" > "${API_RUST_OUT}"
fi

# TS/JS: only exports OR PascalCase top-level function/class (public by convention)
API_TS_OUT="${PACK_DIR}/API_SURFACE_TS.txt"
if [[ ${#FRONTEND_DIRS[@]} -gt 0 ]]; then
  : > "${API_TS_OUT}"
  for fd in "${FRONTEND_DIRS[@]}"; do
    fd_rel="${fd#./}"
    grep -F "^${fd_rel}/" "${PACK_DIR}/FILE_INDEX.txt" 2>/dev/null || true \
      | grep -E '\.(m?js|c?jsx|ts|tsx)$' | grep -Ev '\.d\.ts$' || true \
      | while IFS= read -r tf; do
          grep -IhnE '^(\s*export\s+(default\s+)?(function|class|interface|type|enum|const|let|var|async|function\*)|\s*(function|class)\s+[A-Z])' "$tf" || true
        done >> "${API_TS_OUT}"
  done
  [[ -s "${API_TS_OUT}" ]] || echo "(no TS/JS items found)" > "${API_TS_OUT}"
else
  echo "(no frontend dirs found)" > "${API_TS_OUT}"
fi

# Python: include def/class whose names do NOT start with underscore
# Use awk to extract the identifier and check first char != '_'
API_PY_OUT="${PACK_DIR}/API_SURFACE_PYTHON.txt"
> "${API_PY_OUT}"
grep -E '\.py$' "${PACK_DIR}/FILE_INDEX.txt" 2>/dev/null || true \
  | while IFS= read -r pf; do
      awk -v f="$pf" '
        match($0,/^\s*(def|class)\s+([A-Za-z][A-Za-z0-9_]*)/,m){
          if (substr(m[2],1,1)!="_") printf("%s:%d:%s\n", f, NR, $0)
        }
      ' "$pf" || true
    done >> "${API_PY_OUT}"
[[ -s "${API_PY_OUT}" ]] || echo "(no Python items found)" > "${API_PY_OUT}"

# Go: exported funcs (capitalized)
API_GO_OUT="${PACK_DIR}/API_SURFACE_GO.txt"
grep -E '\.go$' "${PACK_DIR}/FILE_INDEX.txt" 2>/dev/null || true \
  | while IFS= read -r gf; do
      grep -IhnE '^\s*func\s+[A-Z][A-Za-z0-9_]*\s*\(' "$gf" || true
    done > "${API_GO_OUT}" || echo "(no Go items found)" > "${API_GO_OUT}"
[[ -s "${API_GO_OUT}" ]] || echo "(no Go items found)" > "${API_GO_OUT}"

# ---------- Stage 2 (Repomix optional) ----------
log "[Stage 2] Generating compressed skeleton (if repomix is available)..."
if have repomix; then
  REPOMIX_INCLUDE_PATTERNS=""
  if [[ ${#RUST_CRATES[@]} -gt 0 ]]; then
    for crate in "${RUST_CRATES[@]}"; do
      crate_rel="${crate#./}"; REPOMIX_INCLUDE_PATTERNS+="${crate_rel}/**,"
    done
  fi
  if [[ ${#FRONTEND_DIRS[@]} -gt 0 ]]; then
    for fd in "${FRONTEND_DIRS[@]}"; do
      fd_rel="${fd#./}"; REPOMIX_INCLUDE_PATTERNS+="${fd_rel}/**,"
    done
  fi
  REPOMIX_INCLUDE_PATTERNS+="*.toml,*.json,*.md"
  REPOMIX_INCLUDE_PATTERNS="${REPOMIX_INCLUDE_PATTERNS%,}"

  REPOMIX_ERR="${TMP_DIR}/repomix.err"
  if ! repomix --compress --remove-comments --remove-empty-lines \
      --include "${REPOMIX_INCLUDE_PATTERNS}" \
      --ignore "**/target/**,**/dist/**,**/build/**,**/node_modules/**,**/gen/**,**/schemas/**,**/tests/**,**/test/**,**/__tests__/**,**/*.lock,AI_*.*,**/.*" \
      --style xml -o "${PACK_DIR}/PACK_STAGE2_COMPRESSED.xml" 2>"${REPOMIX_ERR}"; then
    warn "repomix failed: $(cat "${REPOMIX_ERR}" 2>/dev/null || echo 'no details')"
  fi
else
  info "repomix not found; skipping compressed skeleton"
fi

# ---------- LANGUAGES.md (portable) ----------
log "Producing LANGUAGES.md..."
EXT_COUNTS="${TMP_DIR}/ext.counts"
awk -F. '
  function basefile(f){
    if (match(f, /(^|\/)(Makefile|Dockerfile|dockerfile|CMakeLists\.txt|BUILD|WORKSPACE)$/)) { sub(/.*\//,"",f); return f }
    return ""
  }
  {
    f=$0
    b=basefile(f)
    if (b!="") { ext=b }
    else if (match(f, /\.([^.\/]+)$/)) { ext=substr(f,RSTART+1,RLENGTH-1) }
    else { ext="(noext)" }
    cnt[ext]++; total++
  }
  END {
    for (e in cnt) printf "%s\t%d\n", e, cnt[e]
    printf "_TOTAL_\t%d\n", total
  }
' "${PACK_DIR}/FILE_INDEX.txt" > "${EXT_COUNTS}" || true
{
  echo "# Language/Extension snapshot"
  echo
  echo "| Extension | Files |"
  echo "|----------:|------:|"
  grep -v '^_TOTAL_' "${EXT_COUNTS}" | sort -t$'\t' -k2,2nr | awk -F'\t' '{printf("| %s | %d |\n",$1,$2)}'
  echo
  TOTAL=$(grep '^_TOTAL_' "${EXT_COUNTS}" | awk -F'\t' '{print $2}')
  echo "_Total files counted: ${TOTAL}_"
} > "${PACK_DIR}/LANGUAGES.md" || true

# ---------- Manifest ----------
log "Writing PACK_MANIFEST.json..."
TIMESTAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
GIT_COMMIT="null"
if [[ "${IN_GIT}" == "yes" ]]; then
  GIT_COMMIT="$(git rev-parse --short HEAD 2>/dev/null || true)"
  [[ -z "${GIT_COMMIT}" ]] && GIT_COMMIT="null"
fi

json_escape() {
  if command -v python3 >/dev/null 2>&1; then
    python3 -c 'import json,sys; print(json.dumps(sys.argv[1]))' "$1"
  else
    printf '%s' "$1" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g' -e '1s/^/"/' -e '$s/$/"/'
  fi
}
size_of() { [[ -f "$1" ]] && wc -c < "$1" | awk '{print $1}' || echo 0; }

MANIFEST="${PACK_DIR}/PACK_MANIFEST.json"
{
  echo "{"
  echo "  \"pack_version\": $(json_escape "$VERSION"),"
  echo "  \"timestamp_utc\": $(json_escape "$TIMESTAMP"),"
  if [[ "${GIT_COMMIT}" != "null" ]]; then
    echo "  \"git_commit\": $(json_escape "$GIT_COMMIT"),"
  else
    echo "  \"git_commit\": null,"
  fi
  echo "  \"args\": {"
  echo "    \"out\": $(json_escape "$PACK_DIR"),"
  echo "    \"max_depth\": ${MAX_DEPTH},"
  echo "    \"git_mode\": $(json_escape "$USE_GIT"),"
  echo "    \"include\": $(json_escape "$INCLUDE_PATTERNS"),"
  echo "    \"exclude\": $(json_escape "$EXCLUDE_PATTERNS"),"
  echo "    \"code_only\": $(json_escape "$CODE_ONLY"),"
  echo "    \"verbose\": $(json_escape "$VERBOSE")"
  echo "  },"
  echo "  \"counts\": {"
  echo "    \"files_raw\": ${RAW_COUNT},"
  echo "    \"files_filtered\": ${FILTERED_COUNT}"
  echo "  },"
  echo "  \"tools\": {"
  echo "    \"git\": $( [[ "${IN_GIT}" == "yes" ]] && echo true || echo false ),"
  echo "    \"cargo\": $( have cargo && echo true || echo false ),"
  echo "    \"repomix\": $( have repomix && echo true || echo false )"
  echo "  },"
  echo "  \"artifacts\": {"
  echo "    \"OVERVIEW.md\": $(size_of "${PACK_DIR}/OVERVIEW.md"),"
  echo "    \"STRUCTURE.txt\": $(size_of "${PACK_DIR}/STRUCTURE.txt"),"
  echo "    \"TOKENS.txt\": $(size_of "${PACK_DIR}/TOKENS.txt"),"
  echo "    \"FILE_INDEX.txt\": $(size_of "${PACK_DIR}/FILE_INDEX.txt"),"
  echo "    \"LANGUAGES.md\": $(size_of "${PACK_DIR}/LANGUAGES.md"),"
  echo "    \"API_SURFACE_RUST.txt\": $(size_of "${PACK_DIR}/API_SURFACE_RUST.txt"),"
  echo "    \"API_SURFACE_TS.txt\": $(size_of "${PACK_DIR}/API_SURFACE_TS.txt"),"
  echo "    \"API_SURFACE_PYTHON.txt\": $(size_of "${PACK_DIR}/API_SURFACE_PYTHON.txt"),"
  echo "    \"API_SURFACE_GO.txt\": $(size_of "${PACK_DIR}/API_SURFACE_GO.txt"),"
  echo "    \"CARGO_TREE_DEDUP.txt\": $(size_of "${PACK_DIR}/CARGO_TREE_DEDUP.txt"),"
  echo "    \"PACK_STAGE2_COMPRESSED.xml\": $(size_of "${PACK_DIR}/PACK_STAGE2_COMPRESSED.xml"),"
  echo "    \"REQUEST_PROTOCOL.md\": $(size_of "${PACK_DIR}/REQUEST_PROTOCOL.md"),"
  echo "    \"PACK_README.md\": $(size_of "${PACK_DIR}/PACK_README.md")"
  echo "  }"
  echo "}"
} > "${MANIFEST}"

# ---------- Post-pack guide ----------
abs_pack="$(cd "${PACK_DIR}" && pwd)"
# Windows path (Git Bash/MSYS) — prefer cygpath; otherwise synthesize C:\ form from /c/...
win_pack=""
if have cygpath; then
  win_pack="$(cygpath -w "$abs_pack" 2>/dev/null || true)"
elif [[ "$OSTYPE" =~ ^(msys|win32|cygwin)$ ]] || [[ "$(uname -s 2>/dev/null || true)" =~ ^MINGW ]]; then
  # Convert /c/Users/Name/... -> C:\Users\Name\...
  if [[ "$abs_pack" =~ ^/([a-zA-Z])/(.*)$ ]]; then
    drive="${BASH_REMATCH[1]}"; rest="${BASH_REMATCH[2]}"
    win_pack="$(printf '%s:%s' "${drive^^}" "/${rest}")"
    win_pack="${win_pack//\//\\}"
  else
    win_pack="${abs_pack//\//\\}"
  fi
fi

UPLOADS=( "OVERVIEW.md" "STRUCTURE.txt" "TOKENS.txt" "REQUEST_PROTOCOL.md" )
for f in "API_SURFACE_RUST.txt" "API_SURFACE_TS.txt" "API_SURFACE_PYTHON.txt" "API_SURFACE_GO.txt" "CARGO_TREE_DEDUP.txt"; do
  [[ -s "${PACK_DIR}/${f}" ]] && UPLOADS+=("$f")
done

CHAT="${PACK_DIR}/CHAT_START.md"
{
  echo "# Start message for your LLM"
  echo
  echo "**Files attached (from \`ai-pack/\`):**"
  for f in "${UPLOADS[@]}"; do echo "- ${f}"; done
  echo
  cat <<'TXT'
**Instructions to the AI:**
Please read the staged context. Use the Ask‑for‑Files Protocol below to request raw source files when needed — do not try to infer missing code.

```yaml
REQUEST_FILE:
  path: relative/path/to/file.ext
  reason: >-
    What you will inspect or implement.
  range: lines 80-140      # or: symbol: Foo::bar
````

**My goal:** <Describe your objective for this repo here.>
TXT
} > "${CHAT}"

copy_clipboard() {

# copy_clipboard <file>

local f="$1"
if have clip; then
< "$f" clip && info "Copied chat message to clipboard via 'clip'."
elif have pbcopy; then
< "$f" pbcopy && info "Copied chat message to clipboard via 'pbcopy'."
elif have xclip; then
< "$f" xclip -selection clipboard && info "Copied chat message to clipboard via 'xclip'."
elif have xsel; then
< "$f" xsel --clipboard --input && info "Copied chat message to clipboard via 'xsel'."
fi
}

to_file_uri() {
local p="$1" uri
if [[ -n "$win_pack" ]]; then
uri="file:///${p//\//}"; uri="${uri// /%20}"
else
uri="file://${p// /%20}"
fi
printf '%s' "$uri"
}

echo
echo "${BOLD}╔══════════════════════════════════════════════════════════╗${RESET}"
echo "${BOLD}║  Next steps — Your Saccade pack is ready!                ║${RESET}"
echo "${BOLD}╚══════════════════════════════════════════════════════════╝${RESET}"
echo " ${GREEN}1)${RESET} Open the pack folder:"
echo "    • POSIX:   ${CYAN}${abs_pack}${RESET}"
if [[ -n "$win_pack" ]]; then
echo "    • Windows: ${CYAN}${win_pack}${RESET}"
echo "    • Explorer: run → ${CYAN}explorer "${win_pack}"${RESET}"
fi
pack_uri="$(to_file_uri "${win_pack:-$abs_pack}")"
chat_uri="$(to_file_uri "${win_pack:+${win_pack}\CHAT_START.md:-${abs_pack}/CHAT_START.md}")"
echo "    • Click:   $(link "$pack_uri" "${BLUE}open ai-pack folder${RESET}")"
echo
echo " ${GREEN}2)${RESET} Start a chat and ${BOLD}attach these files${RESET}:"
for f in "${UPLOADS[@]}"; do echo "    - ${f}"; done
echo "    (Round 2: attach ${MAGENTA}PACK_STAGE2_COMPRESSED.xml${RESET} if the model asks for more detail.)"
echo
echo " ${GREEN}3)${RESET} Paste the ready-to-go message:"
echo "    • Saved at: ${CYAN}${CHAT}${RESET}"
echo "    • Click:    $(link "$chat_uri" "${BLUE}open CHAT_START.md${RESET}")"
copy_clipboard "${CHAT}" || true
echo
echo "${GREEN}==> Done. AI pack is ready in ./$(printf %s "$PACK_DIR")${RESET}"
ls -lh "${PACK_DIR}" || true
