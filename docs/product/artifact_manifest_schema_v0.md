# Artifact manifest schema (v0)

Schema id: `artifact-manifest-v0`

File name: `artifact_manifest.json` (sibling to `provenance.json` inside a run directory or package directory).

## Purpose

One machine-readable index per toolkit run (or evidence package) so that:

- CLI can print a summary and exit code context
- v0.3 API can return `GET /api/runs/{run_id}/manifest`
- v0.5 frontend can navigate claims → artifacts without hard-coding per-command file lists
- Articles can link claims to checksum-backed files

## Top-level: `ArtifactManifest`

| Field | Type | Required | Meaning |
|-------|------|----------|---------|
| `schema` | string | yes | Always `"artifact-manifest-v0"` |
| `toolkit_version` | string | yes | Crate version (e.g. `0.1.0`) |
| `generated_at` | string (RFC 3339 UTC) | yes | When manifest was written |
| `command` | string | yes | CLI subcommand that produced the run (e.g. `transfer-audit`) |
| `run_id` | string \| null | no | Run directory name under `out/<asset>/runs/` |
| `package_id` | string \| null | no | Stablecoin-map or bundled evidence id |
| `asset` | string \| null | no | Uppercase asset symbol when applicable |
| `inputs` | array of `InputRef` | yes | CLI flags and resolved window inputs |
| `artifacts` | array of `ArtifactRef` | yes | Files relative to manifest directory |
| `source_snapshots` | array of `SourceSnapshot` | yes | RPC/config/external data lineage |
| `supported_claims` | array of `ClaimBoundary` | yes | Claims the toolkit supports with listed evidence |
| `unsupported_claims` | array of `ClaimBoundary` | yes | Explicit non-claims (boundary documentation) |
| `warnings` | array of string | yes | Non-fatal issues (empty array if none) |

## `InputRef`

| Field | Type | Meaning |
|-------|------|---------|
| `name` | string | Input label (e.g. `asset`, `window`, `run_id`) |
| `value` | string | Serialized value as run configuration |

## `ArtifactRef`

| Field | Type | Meaning |
|-------|------|---------|
| `kind` | `ArtifactKind` | Semantic category (see enum below) |
| `path` | string | Relative path from manifest directory (POSIX `/`) |
| `format` | `ArtifactFormat` | `csv`, `json`, `markdown`, `other` |
| `row_count` | u64 \| null | Optional row count for tabular files |
| `checksum_sha256` | string \| null | Hex digest of file bytes when computed |
| `description` | string | Human-readable one-liner |

### `ArtifactKind` (v0)

| Variant | Typical files |
|---------|----------------|
| `provenance` | `provenance.json` |
| `qa_report` | `qa_report.json` |
| `supply_audit` | `supply_audit.csv`, `supply_audit.md` |
| `transfer_log` | `decoded_transfers.csv` |
| `summary` | `summary.md` |
| `cross_chain_summary` | `cross_chain_summary.json`, `.md` |
| `checkpoint` | `checkpoint/manifest.json` (resume only) |
| `metadata` | `metadata.json` |
| `map_package` | stablecoin-map CSV outputs |
| `other` | Extension artifacts |

### `ArtifactFormat`

`csv` | `json` | `markdown` | `other`

## `SourceSnapshot`

| Field | Type | Meaning |
|-------|------|---------|
| `source_name` | string | e.g. `alchemy-ethereum`, `configs/tokens` |
| `source_url` | string \| null | RPC or HTTP endpoint if safe to record |
| `retrieved_at` | string \| null | When data was fetched |
| `window_start` | string \| null | Window start (RFC 3339), often from block header |
| `window_end` | string \| null | Window end (RFC 3339) |

Populate from existing `provenance.json` chain rows where possible.

## `ClaimBoundary`

| Field | Type | Meaning |
|-------|------|---------|
| `claim` | string | Short claim id or sentence |
| `status` | `ClaimStatus` | `supported`, `unsupported`, `conditional` |
| `evidence_artifacts` | string[] | Paths (relative) backing the claim |
| `caveat` | string | Limits, definitions, or failure interpretation |

### `ClaimStatus`

- **supported** — Toolkit attests this within stated definitions when gates PASS.
- **unsupported** — Out of scope; listed for reader clarity.
- **conditional** — Supported only if referenced artifacts exist and gates PASS.

## Example manifest JSON

```json
{
  "schema": "artifact-manifest-v0",
  "toolkit_version": "0.1.0",
  "generated_at": "2026-05-15T08:03:31.695921+00:00",
  "command": "transfer-audit",
  "run_id": "usdc_7d_20260501_20260508",
  "package_id": null,
  "asset": "USDC",
  "inputs": [
    { "name": "asset", "value": "USDC" },
    { "name": "per_chain_spans", "value": "true" }
  ],
  "artifacts": [
    {
      "kind": "provenance",
      "path": "provenance.json",
      "format": "json",
      "row_count": null,
      "checksum_sha256": null,
      "description": "Per-chain block windows and contract addresses"
    },
    {
      "kind": "qa_report",
      "path": "qa_report.json",
      "format": "json",
      "row_count": null,
      "checksum_sha256": null,
      "description": "Per-chain QA gates (PASS/FAIL)"
    },
    {
      "kind": "supply_audit",
      "path": "supply_audit.csv",
      "format": "csv",
      "row_count": 3,
      "checksum_sha256": "abc123…",
      "description": "Mint/burn aggregate vs totalSupply delta per chain"
    }
  ],
  "source_snapshots": [
    {
      "source_name": "rpc:ethereum",
      "source_url": null,
      "retrieved_at": "2026-05-15T08:00:00+00:00",
      "window_start": "2026-05-01T00:00:11Z",
      "window_end": "2026-05-07T23:59:59Z"
    }
  ],
  "supported_claims": [
    {
      "claim": "supply_invariant_per_chain",
      "status": "conditional",
      "evidence_artifacts": ["supply_audit.csv", "qa_report.json"],
      "caveat": "Holds under toolkit mint/burn definitions at pinned blocks; FAIL is not proof of fraud."
    }
  ],
  "unsupported_claims": [
    {
      "claim": "reserve_backing",
      "status": "unsupported",
      "evidence_artifacts": [],
      "caveat": "No bank or attestation data in this toolkit."
    },
    {
      "claim": "peg_or_price_stability",
      "status": "unsupported",
      "evidence_artifacts": [],
      "caveat": "No DEX or oracle price series in transfer-audit alone."
    }
  ],
  "warnings": []
}
```

## Claim boundary examples (transfer-audit)

| Claim | Status | Evidence | Caveat |
|-------|--------|----------|--------|
| ERC-20 metadata calls succeeded | conditional | `qa_report.json` | Live `totalSupply` probe is not window-pinned |
| No duplicate Transfer logs in window | conditional | `qa_report.json` | RPC completeness assumed |
| Mint/burn sum matches Δ totalSupply | conditional | `supply_audit.csv`, `qa_report.json` | Bridged inventory not consolidated across chains |
| Cross-chain circulating supply | unsupported | — | Summing per-chain `totalSupply` double-counts bridges |
| Holder census / intent | unsupported | — | Transfers are not labeled by actor type |

## JSON serialization contract (v0)

Optional fields use **explicit JSON `null`** when absent (`run_id`, `package_id`, `asset`, `row_count`, `checksum_sha256`, source snapshot timestamps). Do not omit keys — API and frontend consumers may rely on stable key presence.

## Path safety notes

- Manifest `path` values must be **relative file paths**: `/` only, no NUL, no `..` or `.` segments, no trailing `/`, at least one path segment (`validate_relative_artifact_path`).
- **`write_artifact_manifest`** (v0.2): before write, validates every `artifacts[].path` and every `claim.evidence_artifacts` entry; evidence paths must appear in `artifacts`; existing targets must be **files** under a canonical manifest root (blocks `.`, directories, symlink escape).
- **`validate_manifest_paths(..., require_existing_files: false)`** allows dry-run validation when artifacts are not on disk yet.
- v0.3 API reuses `resolve_artifact_under_root` (canonicalizes `--artifact-root` internally; same NUL/dot rules → HTTP 400 `invalid_path`).
- `run_id` and `asset` directory names use the same charset rules as `report::validate_run_id` (alphanumeric, `-`, `_`).

## Distinction from checkpoint manifest

`checkpoint/manifest.json` (`CheckpointManifest`, schema `transfer-audit-checkpoint-v1`) is for **resume** only. Do not conflate with `artifact_manifest.json`. A completed run may delete checkpoint data; the product manifest describes published outputs.

## Adoption plan

1. v0.2: types + writer + unit test; optional manual manifest in benchmarks.
2. Follow-up PR: `transfer-audit` writes `artifact_manifest.json` at end of `write_run_artifacts`.
3. `cross-chain-summary` and `stablecoin-map-package` emit manifests with appropriate `command` and claim sets.
