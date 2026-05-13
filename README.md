# stablecoin-audit

Reproducible, windowed CLI audits for stablecoin supply and (experimental) transfer/control surfaces.

## Build

```bash
cargo build
cargo build --features experimental
```

## Commands (sketch)

- **Metadata (default):** `cargo run -- metadata --asset USDC --chains ethereum,base,arbitrum --from-block <n> [--to-block <n>]`
- **Transfer audit (experimental):** `cargo run --features experimental -- transfer-audit --asset USDC --chains ethereum,base,arbitrum --from-block <n> --to-block <n|latest>`
- **Control audit / benchmark (experimental, Milestone 5):** `cargo run --features experimental -- control-audit ...` then `cargo run --features experimental -- control-report --asset USDC`
- **Fetch (experimental, transfers + control):** `cargo run --features experimental -- fetch ...` → includes `risk_flags.md`
- **Cross-chain summary (experimental, Milestone 4):** after `transfer-audit` with ≥2 chains, `cargo run --features experimental -- cross-chain-summary --asset USDC` → `out/usdc/cross_chain_summary.{json,md}`

See `docs/ARCHITECTURE.md` and `docs/DATA_MODEL.md` for layers and file shapes.