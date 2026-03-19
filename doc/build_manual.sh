#!/bin/bash
set -euo pipefail

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
readonly MANUAL_DIR="$SCRIPT_DIR/manual/en"
readonly OUTPUT_DIR="$PROJECT_ROOT/target/doc"
readonly PDF_OUTPUT="$OUTPUT_DIR/pgopr-en.pdf"
readonly HTML_OUTPUT="$OUTPUT_DIR/pgopr-en.html"

if ! command -v pandoc >/dev/null 2>&1; then
    echo "Error: pandoc is required but was not found in PATH." >&2
    exit 1
fi

shopt -s nullglob
manual_sources=("$MANUAL_DIR"/??-*.md)
shopt -u nullglob

if [[ ${#manual_sources[@]} -eq 0 ]]; then
    echo "Error: no manual sources found in $MANUAL_DIR matching ??-*.md" >&2
    exit 1
fi

mkdir -p "$OUTPUT_DIR"

echo "Generating PDF manual: $PDF_OUTPUT"
pandoc \
  -o "$PDF_OUTPUT" \
  --from markdown \
  --template eisvogel \
  --listings \
  -N \
  --toc \
  "${manual_sources[@]}"

echo "Generating HTML manual: $HTML_OUTPUT"
pandoc \
  -o "$HTML_OUTPUT" \
  -s \
  -f markdown-smart \
  -N \
  --toc \
  -t html5 \
  "${manual_sources[@]}"

echo "Manual generated:"
echo "  $PDF_OUTPUT"
echo "  $HTML_OUTPUT"
