#!/usr/bin/env bash
set -euo pipefail

# Builds the documentation into target/doc:
#
#   manual      the user manual from doc/manual/en, as PDF and HTML
#
# The PDF is drawn by pgopr itself (`build-manual`), with the same printpdf
# engine that writes the reports the project produces. Pandoc is used for the
# HTML manual alone.
#
# With no argument the manual is built.

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
readonly OUTPUT_DIR="$PROJECT_ROOT/target/doc"

readonly MANUAL_DIR="$SCRIPT_DIR/manual/en"
readonly MANUAL_PDF="$OUTPUT_DIR/pgopr-en.pdf"
readonly MANUAL_HTML="$OUTPUT_DIR/pgopr-en.html"
readonly RESOURCE_PATH="$SCRIPT_DIR:$MANUAL_DIR:$SCRIPT_DIR/manual"

usage() {
    echo "Usage: ${BASH_SOURCE[0]##*/} [manual]" >&2
    exit 1
}

case "${1:-manual}" in
    manual) build_manual=yes ;;
    *)      usage ;;
esac
[[ $# -le 1 ]] || usage

# The PDF is drawn by pgopr, so cargo is the only PDF-path requirement. Pandoc
# is still needed for the HTML manual.
if [[ "$build_manual" == yes ]] && ! command -v cargo >/dev/null 2>&1; then
    echo "Error: cargo is required: pgopr draws its own PDFs." >&2
    exit 1
fi

if [[ "$build_manual" == yes ]] && ! command -v pandoc >/dev/null 2>&1; then
    echo "Error: pandoc is required for the HTML manual but was not found in PATH." >&2
    exit 1
fi

# The sources of a document, in order: ??-*.md, one chapter per file.
sources_in() {
    local dir="$1" found
    shopt -s nullglob
    found=("$dir"/??-*.md)
    shopt -u nullglob
    if [[ ${#found[@]} -eq 0 ]]; then
        echo "Error: no sources found in $dir matching ??-*.md" >&2
        exit 1
    fi
    printf '%s\n' "${found[@]}"
}

manual() {
    # Drawn by pgopr itself, like orangu: one printpdf engine for every PDF
    # the project produces. The HTML manual is still pandoc's, since a PDF
    # engine cannot make one.
    echo "Generating PDF manual: $MANUAL_PDF"
    cargo run --quiet -- build-manual "$MANUAL_DIR" "$MANUAL_PDF"

    local sources
    mapfile -t sources < <(sources_in "$MANUAL_DIR")

    echo "Generating HTML manual: $MANUAL_HTML"
    (
      cd "$SCRIPT_DIR"
      pandoc \
        -o "$MANUAL_HTML" \
        -s \
        --embed-resources \
        -f markdown-smart \
        --resource-path="$RESOURCE_PATH" \
        -N \
        --toc \
        -t html5 \
        "${sources[@]}"
    )
}

mkdir -p "$OUTPUT_DIR"

if [[ "$build_manual" == yes ]]; then
    manual
fi

echo "Documentation generated:"
if [[ "$build_manual" == yes ]]; then
    echo "  $MANUAL_PDF"
    echo "  $MANUAL_HTML"
fi
