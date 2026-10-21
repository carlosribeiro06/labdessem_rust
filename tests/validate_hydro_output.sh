#!/usr/bin/env bash
set -euo pipefail

CASE_DIR="examples/24Barras"
OUTPUT_FILE="resultado_hidreletricas.csv"
REFERENCE_DIR="tests/reference"

# Ensure reference directory exists
mkdir -p "$REFERENCE_DIR"

# If reference doesn't exist, capture it
if [ ! -f "$REFERENCE_DIR/$OUTPUT_FILE" ]; then
    echo "Reference file not found. Run this script before migration to capture reference."
    exit 1
fi

# Run current implementation
cargo run -p labdessem-cli --release -- run "$CASE_DIR"

# Compare
if diff -q "$CASE_DIR/RESULTADOS/$OUTPUT_FILE" "$REFERENCE_DIR/$OUTPUT_FILE" >/dev/null 2>&1; then
    echo "✓ Hydro output is bit-identical"
    exit 0
else
    echo "✗ Hydro output differs from reference"
    diff "$CASE_DIR/RESULTADOS/$OUTPUT_FILE" "$REFERENCE_DIR/$OUTPUT_FILE" | head -50
    exit 1
fi
