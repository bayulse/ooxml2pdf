#!/usr/bin/env bash

set -euo pipefail

ROOT="/Users/bayulse/projects/ooxml2pdf"
REFERENCE_ROOT="$ROOT/compat/references/local"
PYTHON="$ROOT/compat/.venv/bin/python"
COMPAT="$ROOT/compat/scripts/compat.py"

# Ganti dengan versi PowerPoint yang Anda gunakan.
# Contoh:
# POWERPOINT_VERSION="Microsoft PowerPoint for Mac 16.101"
POWERPOINT_VERSION="Microsoft PowerPoint for Mac Version 16.113.3 (26092714)"

DPI=144
WIDTH_PT=960
HEIGHT_PT=540

CASES=(
  "001-basic-shapes"
  "002-images"
  "003-text"
  "004-theme-layout"
  "005-transforms"
  "006-style-matrix"
)

cd "$ROOT"

echo "== Checking required tools =="

command -v pdftoppm >/dev/null 2>&1 || {
  echo "ERROR: pdftoppm not found."
  echo "Install Poppler first:"
  echo "  brew install poppler"
  exit 1
}

command -v pdfinfo >/dev/null 2>&1 || {
  echo "ERROR: pdfinfo not found."
  echo "Install Poppler first:"
  echo "  brew install poppler"
  exit 1
}

if [[ ! -x "$PYTHON" ]]; then
  echo "ERROR: compatibility Python environment not found:"
  echo "  $PYTHON"
  echo
  echo "Create it with:"
  echo "  python3 -m venv compat/.venv"
  echo "  compat/.venv/bin/python -m pip install -r compat/scripts/requirements.txt"
  exit 1
fi

echo
echo "== Building PowerPoint references =="

for CASE in "${CASES[@]}"; do
  PDF="$ROOT/compat/references/local/${CASE}.pdf"
  OUT="$REFERENCE_ROOT/$CASE"

  echo
  echo "------------------------------------------------------------"
  echo "CASE: $CASE"
  echo "PDF : $PDF"
  echo "OUT : $OUT"
  echo "------------------------------------------------------------"

  if [[ ! -f "$PDF" ]]; then
    echo "ERROR: PDF not found: $PDF"
    exit 1
  fi

  mkdir -p "$OUT"

  echo
  echo "[1/3] Inspecting PowerPoint PDF..."
  pdfinfo "$PDF" | grep -E '^(Pages|Page size|PDF version):' || true

  echo
  echo "[2/3] Rasterizing PowerPoint PDF at ${DPI} DPI..."

  # Remove only previously generated slide PNGs.
  rm -f "$OUT"/Slide-*.png

  pdftoppm \
    -r "$DPI" \
    -png \
    "$PDF" \
    "$OUT/Slide"

  echo "Generated:"
  ls -lh "$OUT"/Slide-*.png

  echo
  echo "[3/3] Creating reference manifest..."

  if [[ -f "$OUT/reference.json" ]]; then
    echo "Reference manifest already exists:"
    echo "  $OUT/reference.json"
    echo "Skipping manifest creation to avoid overwriting provenance."
  else
    "$PYTHON" "$COMPAT" reference \
      "$OUT" \
      --renderer "Microsoft PowerPoint (PDF export; Poppler rasterization)" \
      --version "$POWERPOINT_VERSION" \
      --width-pt "$WIDTH_PT" \
      --height-pt "$HEIGHT_PT" \
      --notes "macOS; PowerPoint PDF export; Poppler ${DPI} DPI; project test fonts used where applicable"
  fi
done

echo
echo "============================================================"
echo "All PowerPoint reference images are ready."
echo "Reference root:"
echo "  $REFERENCE_ROOT"
echo "============================================================"
