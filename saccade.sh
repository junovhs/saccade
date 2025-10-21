#!/usr/bin/env bash
set -euo pipefail

PACK_DIR="ai-pack"
mkdir -p "$PACK_DIR"

echo "==> Starting AI Pack generation..."

# --- Helper Function ---
have() { command -v "$1" >/dev/null 2>&1; }

# --- Centralized Prune Rules ---
# This is the key fix: tells `find` to not even enter these directories.
PRUNE_RULES="-name .git -o -name node_modules -o -name dist -o -name build -o -name target -o -name gen -o -name schemas -o -name tests -o -name test -o -name __tests__"

# --- Static Helper Files ---
echo "    -> Writing helper files (OVERVIEW.md, etc.)..."
cat > "$PACK_DIR/OVERVIEW.md" <<'MD'
# Project Overview (edit me)
- What it does:
- Primary tech (Rust backend, Tauri desktop, TS/React UI, etc.):
- Entrypoints / main binaries:
- Key modules / domains:
- Current task/question for the AI:
MD
cat > "$PACK_DIR/REQUEST_PROTOCOL.md" <<'MD'
# Ask-for-Files Protocol
You have a staged view:
- STRUCTURE.txt (shallow tree), TOKENS.txt (size heat map), dependency tree, API surfaces
- PACK_STAGE2_COMPRESSED.xml (if present; compressed skeleton)
If you need raw source, request:
REQUEST_FILE:
path: <relative/path>
why: <what you will inspect or implement>
range: <optional lines or symbol>
Minimize requests. Produce complete, compilable implementations that match project conventions.
Never hallucinate missing code—request it.
MD
cat > "$PACK_DIR/PACK_README.md" <<'MD'
# AI Pack — How to Use
Round 1 (tiny upload):
- OVERVIEW.md (fill it out)
- STRUCTURE.txt
- CARGO_TREE_DEDUP.txt
- API_SURFACE_RUST.txt (and API_SURFACE_TS.txt if present)
- REQUEST_PROTOCOL.md
Round 2 (when needed):
- PACK_STAGE2_COMPRESSED.xml (if it contains a compressed skeleton)
On demand:
- Paste specific files/lines the model requests using REQUEST_PROTOCOL.md.
MD

# --- Dynamic Discovery ---
RUST_CRATES=()
FRONTEND_DIRS=()
echo "==> [Discovery] Finding Rust crates and Frontend dirs..."
while IFS= read -r d; do
  CRATE_DIR=$(dirname "$d")
  if [ -d "${CRATE_DIR}/src" ]; then RUST_CRATES+=("${CRATE_DIR}/src"); fi
done < <(find . -name "Cargo.toml" -not -path "*/target/*")
for dir in app frontend src ui; do
  if [ -d "$dir" ]; then FRONTEND_DIRS+=("$dir"); break; fi
done

# --- Stage 0 Artifacts (Fast Version) ---
echo "==> [Stage 0] Generating STRUCTURE.txt and TOKENS.txt..."
{
  echo "Directories (depth<=3):"
  find . -mindepth 1 -maxdepth 3 -type d \( $PRUNE_RULES \) -prune -o -type d -print | sed 's|^\./||' | sort
  echo
  echo "Files (depth<=3, filtered common junk):"
  find . -mindepth 1 -maxdepth 3 -type f \( $PRUNE_RULES \) -prune -o -type f -print \
    | grep -Evi '\.(png|jpg|svg|ico|icns|woff|ttf|lock|log|zip|jar)$' \
    | sed 's|^\./||' | sort
} > "$PACK_DIR/STRUCTURE.txt"

{
  echo "Heuristic Size Heat Map (chars ≈ 4*tokens). Top 50 by chars:"
  find . -type d \( $PRUNE_RULES \) -prune -o -type f -print0 \
    | xargs -0 -I{} sh -c 'C=$(wc -m < "{}" 2>/dev/null || echo 0); printf "%9s chars  ~%7d tokens  %s\n" "$C" $((C/4)) "{}"' \
    | grep -Evi '\.(png|jpg|svg|ico|icns|woff|ttf|lock|log|zip|jar)$' \
    | sort -k1,1nr | head -50
} > "$PACK_DIR/TOKENS.txt"

find . -type d \( $PRUNE_RULES \) -prune -o -type f -print | sed 's|^\./||' | sort > "$PACK_DIR/FILE_INDEX.txt"

# --- Stage 1 Artifacts ---
echo "==> [Stage 1] Generating dependency and API surfaces..."
if have cargo && [ -f "Cargo.toml" ]; then
  cargo tree -e normal -d > "$PACK_DIR/CARGO_TREE_DEDUP.txt" || true
fi

API_RUST_OUT="$PACK_DIR/API_SURFACE_RUST.txt"
if [ ${#RUST_CRATES[@]} -gt 0 ]; then
  find "${RUST_CRATES[@]}" -type f -name "*.rs" -print0 | xargs -0 grep -hnE '^\s*pub\s' > "$API_RUST_OUT" || echo "(no public Rust items found)" > "$API_RUST_OUT"
else
  echo "(no Rust crates found)" > "$API_RUST_OUT"
fi

API_TS_OUT="$PACK_DIR/API_SURFACE_TS.txt"
if [ ${#FRONTEND_DIRS[@]} -gt 0 ]; then
  find "${FRONTEND_DIRS[@]}" -type f \( -name "*.js" -o -name "*.ts" \) -print0 | xargs -0 grep -hnE '^\s*(export|function|class|const|let|var)\s' > "$API_TS_OUT" || echo "(no TS/JS items found)" > "$API_TS_OUT"
else
  echo "(no frontend dirs found)" > "$API_TS_OUT"
fi

# --- Stage 2 Artifacts (Repomix Optional) ---
echo "==> [Stage 2] Generating compressed skeleton..."
if have repomix; then
  INCLUDE_PATTERNS=""
  for crate in "${RUST_CRATES[@]}"; do INCLUDE_PATTERNS+="${crate}/**,"; done
  for dir in "${FRONTEND_DIRS[@]}"; do INCLUDE_PATTERNS+="${dir}/**,"; done
  INCLUDE_PATTERNS+="*.toml,*.json,*.md"
  
  repomix --compress --remove-comments --remove-empty-lines \
    --include "${INCLUDE_PATTERNS}" \
    --ignore "**/target/**,**/dist/**,**/build/**,**/node_modules/**,**/gen/**,**/schemas/**,**/tests/**,**/test/**,**/__tests__/**,**/*.lock,AI_*.*,**/.*" \
    --style xml -o "$PACK_DIR/PACK_STAGE2_COMPRESSED.xml" || true
else
  echo "(Repomix not found, skipping compressed skeleton)"
fi

echo
echo "✅ Done. AI pack is ready in ./$PACK_DIR"
ls -lh "$PACK_DIR"
