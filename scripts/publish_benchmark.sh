#!/usr/bin/env bash
# Copy transfer-audit summary artifacts from out/ to docs/benchmarks/<run_id>/.
set -euo pipefail

RUN_ID="${1:-}"
if [[ -z "$RUN_ID" ]]; then
  echo "usage: $(basename "$0") <run_id> [asset]" >&2
  exit 1
fi

ASSET="${2:-USDC}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="$ROOT/out/${ASSET}/runs/${RUN_ID}"
DST="$ROOT/docs/benchmarks/${RUN_ID}"

FILES=(
  supply_audit.csv
  supply_audit.md
  cross_chain_summary.json
  cross_chain_summary.md
  qa_report.json
  provenance.json
  summary.md
)

[[ -d "$SRC" ]] || { echo "error: missing $SRC" >&2; exit 1; }
mkdir -p "$DST"

for f in "${FILES[@]}"; do
  if [[ -f "$SRC/$f" ]]; then
    cp "$SRC/$f" "$DST/$f"
    echo "copied $f"
  else
    echo "skip (missing): $f" >&2
  fi
done

echo "published summaries -> $DST"
