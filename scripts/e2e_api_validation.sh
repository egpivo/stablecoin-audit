#!/usr/bin/env bash
# End-to-end validation: transfer-audit → artifact_manifest.json → read-only API discovery.
set -euo pipefail
cd "$(dirname "$0")/.."

echo "== cargo test --features api (includes e2e integration test) =="
cargo test --features api

echo ""
echo "== e2e integration test only =="
cargo test --features api rpc::transfer_audit::tests::e2e_transfer_audit_success_discovered_by_api -- --exact

echo ""
echo "OK: transfer-audit success path, API listing, manifest endpoint, partial-run exclusion verified in Rust tests."
