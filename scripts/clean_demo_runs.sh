#!/usr/bin/env bash
# Remove completed product runs (artifact_manifest.json present) under the artifact root.
# Safe for local demos only — does not touch paths outside ARTIFACT_ROOT.
set -euo pipefail

ROOT="${1:-out}"
ROOT="$(cd "$ROOT" && pwd)"

echo "Cleaning product runs under: $ROOT"

removed=0
while IFS= read -r manifest; do
  run_dir="$(dirname "$manifest")"
  rel="${run_dir#"$ROOT"/}"
  echo "  remove $rel"
  rm -rf "$run_dir"
  removed=$((removed + 1))
done < <(find "$ROOT" -type f -name artifact_manifest.json 2>/dev/null | sort)

echo "Removed $removed run(s)."
