#!/usr/bin/env bash
set -euo pipefail

ROOT="/Users/bayulse/projects/ooxml2pdf"
PY="$ROOT/compat/.venv/bin/python"
COMPAT="$ROOT/compat/scripts/compat.py"
OFFICE="/Applications/Microsoft PowerPoint.app/Contents/Resources/DFonts"
DISPLAY="$HOME/Library/Group Containers/UBF8T346G9.Office/FontCache/4/CloudFonts/Aptos Display"

cd "$ROOT"

for CASE in 001-basic-shapes 002-images 003-text 005-transforms; do
  "$PY" "$COMPAT" check \
    "compat/corpus/public/$CASE.pptx" \
    --reference-dir "compat/references/local/$CASE" \
    --font "$OFFICE/Calibri.ttf" \
    --font "$OFFICE/Calibrib.ttf" \
    --font "$OFFICE/Aptos.ttf" \
    --font "$OFFICE/Aptos-Bold.ttf"
done

for CASE in 004-theme-layout 006-style-matrix; do
  "$PY" "$COMPAT" check \
    "compat/corpus/public/$CASE.pptx" \
    --reference-dir "compat/references/local/$CASE" \
    --font "$OFFICE/Aptos.ttf" \
    --font "$OFFICE/Aptos-Bold.ttf" \
    --font "$OFFICE/Aptos-Italic.ttf" \
    --font "$OFFICE/Aptos-Bold-Italic.ttf" \
    --font "$DISPLAY/30169865670.ttf" \
    --font "$DISPLAY/32677218994.ttf" \
    --font "$DISPLAY/25576969643.ttf"
done