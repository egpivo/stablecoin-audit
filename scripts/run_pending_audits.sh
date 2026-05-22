#!/usr/bin/env bash
# Run all pending transfer-audits in sequence, then rebuild the analysis panel.
# Safe to re-run: existing checkpoints are reused; completed windows skip fast.
# Usage: bash scripts/run_pending_audits.sh [--skip-nov2024]
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

SKIP_NOV2024=false
for arg in "$@"; do
  [[ "$arg" == "--skip-nov2024" ]] && SKIP_NOV2024=true
done

run_window() {
  local run_id="$1"; shift
  echo
  echo "=== transfer-audit: $run_id ==="
  cargo run --release -- transfer-audit --asset USDC --run-id "$run_id" "$@"
  echo "=== cross-chain-summary: $run_id ==="
  cargo run --release -- cross-chain-summary --asset USDC --run-id "$run_id"
  echo "=== publish: $run_id ==="
  bash scripts/publish_benchmark.sh "$run_id"
}

# --- New May 2026 windows ---
run_window usdc_7d_20260507_20260514 \
  --window arbitrum:460146890:462559767 \
  --window base:45661327:45963726 \
  --window ethereum:25039433:25089644

run_window usdc_7d_20260514_20260521 \
  --window arbitrum:462559764:464438118 \
  --window base:45963727:46199501 \
  --window ethereum:25089645:25128822

run_window usdc_7d_20260512_20260519 \
  --window arbitrum:461870006:464280529 \
  --window base:45877327:46179726 \
  --window ethereum:25075306:25125536

# --- Nov 2024 re-run (re-fetches mints/burns; --fresh clears any stale checkpoint) ---
if [[ "$SKIP_NOV2024" == "false" ]]; then
  echo
  echo "=== transfer-audit re-run: usdc_7d_20241117_20241124 (--fresh) ==="
  run_window usdc_7d_20241117_20241124 --fresh \
    --window arbitrum:275231008:277637793 \
    --window base:22506127:22808526 \
    --window ethereum:21203704:21253879
fi

# --- Rebuild analysis files ---
echo
echo "=== rebuilding analysis files ==="
cd scripts
python3 join_window_sentiment.py
python3 build_market_conditioned_panel.py
python3 build_window_event_context.py
python3 build_data_completeness.py

echo
echo "All done."
