# Article notes: Building a Stablecoin Audit Toolkit

Draft title: **Building a Stablecoin Audit Toolkit: CLI First, Artifacts First, Frontend Last**

Audience: engineers and product readers evaluating how to ship audit evidence without over-claiming.

---

## 1. Why a stablecoin audit tool should start with evidence boundaries

Stablecoin discourse mixes reserve adequacy, peg stability, liquidity, and on-chain accounting. A toolkit that blurs those boundaries becomes a dashboard that over-claims.

Starting with **claim boundaries** forces explicit supported vs unsupported statements before any UI. The central claim registry (`src/audit/claims.rs`) and manifest fields `supported_claims` / `unsupported_claims` are the product contract — not marketing copy.

Key message: readers should see what was measured, under what definitions, and what was never in scope.

## 2. Rust CLI as the evidence producer

The CLI runs reproducible workflows (`transfer-audit`, `cross-chain-summary`) against pinned block windows. It writes CSV/JSON/Markdown artifacts and only emits `artifact_manifest.json` after a **successful** run.

Why Rust CLI first:

- Deterministic, testable pipelines
- No hidden server state
- Artifacts are inspectable without the tool running
- RPC and decode logic stay out of the HTTP layer

Outputs land at `out/<asset>/runs/<run_id>/`.

## 3. artifact_manifest.json as the product contract

One JSON file per run:

- Lists every artifact (kind, path, format, checksum)
- Records workflow steps and source snapshots
- Embeds supported and unsupported claim boundaries
- Schema: `artifact-manifest-v0`

Discovery, API listing, and package generation all read this file only. Directories with `qa_report.json` but no manifest are intentionally invisible to the product API — incomplete runs stay incomplete.

## 4. Read-only API as a thin wrapper

`cargo run --features api -- serve --artifact-root out/` exposes GET (and package POST) endpoints over the filesystem jail. Handlers deserialize manifests; they do not re-run audits or call RPC.

Endpoints: runs, manifest, artifacts, raw bytes, package build/download/verify.

The API is a **viewer contract** for a future browser, not an orchestration plane. No `POST /api/runs` in v0.3.

## 5. Evidence browser as the final layer

The v0 browser (`ui/`, served at `/ui/`) is static HTML/JS:

- Run list → manifest overview → claim boundaries → artifact table → package actions
- Fixed demo note: no reserve/peg/redemption/geo/intent/routing claims
- No charts, scores, or wallet connection

Frontend last because the manifest already encodes the story. The browser arranges evidence; it does not invent it.

## 6. What this toolkit can prove

Within stated definitions and when QA gates PASS:

- Transfer activity reconstructible for configured windows
- Pinned totalSupply snapshots available
- Mint/burn aggregates compared to supply deltas per chain (supply invariant)
- Cross-chain per-deployment comparison on one schema (after cross-chain-summary)

Conditional claims cite specific artifacts (`supply_audit.csv`, `qa_report.json`, etc.).

## 7. What it explicitly does not prove

Catalogued as unsupported in every manifest:

- Fiat reserve backing
- Peg or price stability
- Redemption capacity
- User geography / holder identity
- Issuer intent
- Actual swap routing
- Bridge backing without bridge collateral data
- Circulating supply across chains (double-count risk)

The browser repeats this boundary in UI copy — product consistency, not legalese.

## 8. Roadmap

| Layer | Next steps |
|-------|------------|
| Run orchestration | `POST /api/runs`, status, logs, cancel (v0.4) |
| Frontend polish | Filtering, artifact preview, deep links, mobile layout |
| Audit engines | Additional workflows beyond transfer-audit; same manifest contract |
| Packaging | Global package listing; signed bundles |
| Deployment | Auth, multi-tenant artifact roots — only if product scope expands |

Principle unchanged: **CLI produces evidence, manifest defines claims, API serves bytes, browser displays boundaries.**

---

## Demo commands (for article sidebar)

```bash
# Produce evidence
cargo run -- transfer-audit --asset USDC --run-id article_demo \
  --window ethereum:24000000:24001000

# Serve API + browser
cargo run --features api -- serve --artifact-root out/
open http://127.0.0.1:8080/ui/
```

## Files to cite in the article

- Claim catalog: `src/audit/claims.rs`
- Manifest schema: `docs/product/artifact_manifest_schema_v0.md`
- API design: `docs/product/api_design_v0.md`
- Browser doc: `docs/product/evidence_browser_v0.md`
