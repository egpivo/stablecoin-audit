#!/usr/bin/env bash
# Run remaining pending transfer-audits on a remote machine with sufficient disk space.
# Block numbers are pre-resolved from the local Alchemy RPC on 2026-05-19.
#
# Requirements on remote:
#   - Rust toolchain (cargo)
#   - .env file in repo root with ALCHEMY_ETHEREUM_URL / ALCHEMY_BASE_URL / ALCHEMY_ARBITRUM_URL
#   - ~50 GB free disk (checkpoint transfer CSVs are large)
#
# After completion, copy docs/benchmarks/<run_id>/ back to the local machine and run:
#   python3 scripts/join_window_sentiment.py
#   python3 scripts/build_market_conditioned_panel.py
#   python3 scripts/build_window_event_context.py
#   python3 scripts/build_data_completeness.py

set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

run_window() {
  local run_id="$1"; shift
  echo
  echo "=== transfer-audit: $run_id ==="
  cargo run --release -- transfer-audit --asset USDC --run-id "$run_id" "$@"
  echo "=== cross-chain-summary: $run_id ==="
  cargo run --release -- cross-chain-summary --asset USDC --run-id "$run_id"
  echo "=== publish: $run_id ==="
  bash scripts/publish_benchmark.sh "$run_id"
  echo "=== cleanup large intermediates: $run_id ==="
  rm -f "out/usdc/runs/${run_id}/decoded_transfers.csv"
  rm -f "out/usdc/runs/${run_id}/checkpoint/transfers_"*.csv
  echo "cleaned up transfer CSVs for $run_id"
}

# ── Window 1: usdc_7d_20260514_20260521 ──────────────────────────────────────
# Context: CLARITY Act committee advance (day 0), Fed Chair transition (day +5)
# F&G: Fear (mean 31.3, 6/7 days available)
# Blocks resolved 2026-05-19:
#   Arbitrum: 462559764 → 464438118  (2026-05-14 → ~2026-05-19 chain tip)
#   Base:     45963727  → 46199501
#   Ethereum: 25089645  → 25128822
run_window usdc_7d_20260514_20260521 \
  --window arbitrum:462559764:464438118 \
  --window base:45963727:46199501 \
  --window ethereum:25089645:25128822

# ── Window 2: usdc_7d_20260512_20260519 ──────────────────────────────────────
# Context: CLARITY Act within window (day +2), Fed Chair transition 1d after end
# F&G: Fear (mean 36.3, 7 days)
# Blocks resolved 2026-05-19:
#   Arbitrum: 461870006 → 464280529  (2026-05-12 → 2026-05-19T00:00:00)
#   Base:     45877327  → 46179726
#   Ethereum: 25075306  → 25125536
run_window usdc_7d_20260512_20260519 \
  --window arbitrum:461870006:464280529 \
  --window base:45877327:46179726 \
  --window ethereum:25075306:25125536

# ── Window 3: usdc_7d_20241117_20241124 (re-run for mints/burns) ──────────────
# Reason: existing benchmark supply_audit.csv was reconstructed from
#         cross_chain_summary.json and lacks sum_mints_raw / sum_burns_raw.
#         Re-run with --fresh to get a complete artifact.
# F&G: Extreme Greed (mean 87.86, 7 days)
# Original blocks (confirmed from cross_chain_summary.json):
#   Arbitrum: 275231008 → 277637793
#   Base:     22506127  → 22808526
#   Ethereum: 21203704  → 21253879
run_window usdc_7d_20241117_20241124 --fresh \
  --window arbitrum:275231008:277637793 \
  --window base:22506127:22808526 \
  --window ethereum:21203704:21253879

echo
echo "=== All 3 windows complete. Rebuilding analysis files ==="
cd scripts
python3 join_window_sentiment.py
python3 build_market_conditioned_panel.py
python3 build_window_event_context.py
python3 build_data_completeness.py

echo
echo "Done. Copy docs/benchmarks/ back to your local machine, then re-run the scripts above locally."
