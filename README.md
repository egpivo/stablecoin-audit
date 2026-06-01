# stablecoin-audit

[![CI](https://github.com/egpivo/stablecoin-audit/actions/workflows/ci.yml/badge.svg)](https://github.com/egpivo/stablecoin-audit/actions/workflows/ci.yml)
[![codecov](https://codecov.io/github/egpivo/stablecoin-audit/graph/badge.svg?token=mN0h7zLOtR)](https://codecov.io/github/egpivo/stablecoin-audit)

**0.1.0** — ERC-20 mint/burn vs `totalSupply` at pinned blocks, per chain (`--asset`, `configs/tokens/`). USDC on Ethereum / Base / Arbitrum is the first benchmark.

**Not claimed:** reserves, peg, liquidity, oracles, bridge backing, holder census, intent. [Scope →](docs/README.md#scope-and-interpretation)

## Architecture

Evidence is filesystem-first: CLI writes artifacts + `artifact_manifest.json`; API and UI read them only (no audit logic in the browser).

![v0 stack](docs/product/screenshots/architecture_pipeline.svg)

```text
Rust CLI → out/<asset>/runs/<run_id>/ → artifact_manifest.json → API (read-only) → /ui/
```

Full layers, modules, roadmap: [`docs/product/backend_architecture_v0.md`](docs/product/backend_architecture_v0.md). Manifest schema: [`docs/product/artifact_manifest_schema_v0.md`](docs/product/artifact_manifest_schema_v0.md).

## Quick start

```bash
cargo build   # RPC URLs in .env — see .env.example

cargo run -- transfer-audit --asset USDC --run-id smoke \
  --window ethereum:24000000:24001000

cargo run -- cross-chain-summary --asset USDC --run-id smoke
```

Add `--window base:…` / `arbitrum:…` for more chains. Outputs: `out/<asset>/runs/<run_id>/`; gates in `qa_report.json`. CLI details: [`docs/README.md#cli`](docs/README.md#cli).

## Evidence console

```bash
cargo run --features api -- serve --artifact-root out/
# http://127.0.0.1:8080/ui/
```

Public snapshot: [egpivo.github.io/stablecoin-audit/ui/](https://egpivo.github.io/stablecoin-audit/ui/) (`github_pages_demo`). [`docs/GITHUB_PAGES.md`](docs/GITHUB_PAGES.md)

## Related writing

This repo grew out of a short stablecoin evidence series. The posts are not required to run the toolkit, but they explain the motivation behind the artifact-first design.

| Post | Focus | How it connects to this repo |
|------|-------|------------------------------|
| [USDC Shows Why Stablecoin Risk Analysis Is Not One Signal](https://medium.com/coinmonks/usdc-shows-why-stablecoin-risk-analysis-is-not-one-signal-bb9c333ed169) | Per-chain USDC transfer and supply signals | Motivation for treating each asset × chain deployment as a separate audit unit |
| [Local Pegs, Dollar Rails: Auditing XSGD and EURC Liquidity](https://medium.com/towards-finance/local-pegs-dollar-rails-auditing-xsgd-and-eurc-liquidity-9ed540b84cfe) | Local-currency stablecoins and observed USDC-paired liquidity | Motivation for keeping liquidity observations separate from reserve, peg, and routing claims |
| [The Stablecoin Map: What Crypto’s Cash Rails Depend On](https://medium.com/thecapital/the-stablecoin-map-what-cryptos-cash-rails-depend-on-e50e429d0fbe) | Stablecoin footprint, transfer activity, DEX counterparts, and claim boundaries | Motivation for turning article evidence into reproducible artifacts, manifests, and an Evidence Console |

## Docs

[`docs/README.md`](docs/README.md) — benchmarks, experimental CLI, research joins, blog evidence map.

MIT — [LICENSE](LICENSE).
