#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

echo "Building mdw..."
cargo build 2>&1

BINARY="./target/debug/mdw"
OUTDIR="test-screen"
mkdir -p "$OUTDIR"

PASS=0
FAIL=0
ERRORS=""

for file in examples/**/*.md examples/**/*.mermaid examples/**/*.d2 examples/**/*.json; do
    [ -f "$file" ] || continue
    name="$(echo "$file" | sed 's|/|_|g; s| |_|g')"
    out="$OUTDIR/${name}.txt"

    if "$BINARY" --screenshot "$out" "$file" 2>/dev/null; then
        echo "  PASS  $file"
        PASS=$((PASS + 1))
    else
        echo "  FAIL  $file"
        FAIL=$((FAIL + 1))
        ERRORS="$ERRORS\n  $file"
    fi
done

echo ""
echo "Results: $PASS passed, $FAIL failed"

if [ "$FAIL" -gt 0 ]; then
    echo -e "Failed files:$ERRORS"
    exit 1
fi
