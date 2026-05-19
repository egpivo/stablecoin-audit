# Geo-stablecoin feasibility assessment

**Purpose:** Determine whether EURC (euro) and XSGD (Singapore dollar) can be audited under the existing per-asset EVM transfer-audit framework before extending the geo-policy-conditioned panel.

**Scope of this document:** Token selection, config requirements, on-chain compatibility, estimated volume, and an auditability verdict. No reserve adequacy claim, safety ranking, payment-volume conclusion, or causality statement.

---

## Framework requirements (from existing transfer-audit schema)

For a token to be audited under the current framework it must satisfy:

| Requirement | How checked |
|-------------|-------------|
| ERC-20 `Transfer` event log at a known contract address | `metadata` subcommand + log probe |
| `totalSupply()` view call available at window boundaries | `metadata` subcommand + `resolve-window` |
| Zero-address mint/burn convention (`from=0x0` for mint, `to=0x0` for burn) | Log probe on known mint/burn tx |
| Sufficient 7-day transfer volume for meaningful invariant check | `eth_getLogs` probe on 100-block sample |
| Chain deployed on Ethereum, Base, or Arbitrum (existing RPC config) | Contract existence check |
| Stable config fields: `chain_id`, `contract_address`, `decimals`, `deployment_block` | YAML config creation |

---

## EURC — Circle Euro Coin

### Profile

| Field | Value |
|-------|-------|
| Issuer | Circle Internet Financial |
| Fiat peg | Euro (EUR) |
| Primary jurisdiction | EU (MiCA — regulated as Electronic Money Token) |
| Standard | ERC-20 with `MintBurn`, `Blacklist`, `Pause`, `Upgradeable` (same pattern as USDC) |
| Token symbol | EURC |
| Decimals | 6 |

### EVM deployments

| Chain | Contract address | Deployment approx. | Notes |
|-------|-----------------|-------------------|-------|
| Ethereum | `0x1aBaEA1f7C830bD89Acc67eC4af516284b1bC33c` | 2023-06 | **Verify before use** |
| Base | `0x60a3e35cc302bfa44cb288bc5a4f316fdb1adb42` | 2023-09 | **Verify before use** |
| Arbitrum | not confirmed | — | Check Circle docs |

> Contract addresses must be verified against the official Circle EURC documentation or on-chain bytecode before use. Do not run an audit against an unverified address.

### Compatibility assessment

| Criterion | Expected | Confidence | Source |
|-----------|----------|-----------|--------|
| ERC-20 Transfer logs | ✓ | High | Circle uses identical contract pattern to USDC |
| `totalSupply()` available | ✓ | High | ERC-20 standard |
| Zero-address mint/burn | ✓ | High | Circle MintBurnable pattern confirmed on USDC |
| Ethereum deployment | ✓ | High | Publicly documented |
| Base deployment | ✓ | High | Publicly documented |
| Arbitrum deployment | ❓ | Low | Not confirmed — check Circle docs |
| 7-day volume sufficient | ❓ | Medium | EURC market cap ~$300–600M; substantially lower volume than USDC |
| `historical_supply` gate | ✓ | High | Ethereum archive nodes support |
| Config fields complete | Requires creation | — | Need YAML config files |

### Config files required

```
configs/tokens/eurc.ethereum.yml
configs/tokens/eurc.base.yml
configs/tokens/eurc.arbitrum.yml  (if deployed)
```

### Risks not covered by on-chain audit

- MiCA reserve requirements (off-chain, attestation-based)
- EUR/USD FX exposure in reserves
- Cross-chain bridge accounting (same caveat as USDC)
- Issuer redemption risk

### Preliminary verdict

**Usable with config** — provided contract addresses are verified and 7-day transfer volume proves sufficient on at least one chain. EURC uses the same Circle contract pattern as USDC; transfer-audit schema should apply without code changes. Requires: (1) new YAML config files, (2) volume probe to confirm non-trivial 7-day log count.

---

## XSGD — StraitsX Singapore Dollar

### Profile

| Field | Value |
|-------|-------|
| Issuer | StraitsX (Xfers Pte. Ltd. / FAZZ Financial) |
| Fiat peg | Singapore Dollar (SGD) |
| Primary jurisdiction | Singapore (MAS Payment Services Act, Major Payment Institution licence) |
| Standard | ERC-20; original contract uses `mint` / `burn` functions |
| Token symbol | XSGD |
| Decimals | 6 |

### EVM deployments

| Chain | Contract address | Notes |
|-------|-----------------|-------|
| Ethereum | `0x70e8de73ce538da2beed35d14187f6959a8eca96` | **Verify before use** |
| Polygon | deployed | Outside current RPC config (no `ALCHEMY_POLYGON_URL`) |
| Base | not confirmed | Check StraitsX docs |
| Arbitrum | not confirmed | Check StraitsX docs |
| Zilliqa | original deployment | Not EVM — not auditable under this framework |

> Contract addresses must be verified. Zilliqa deployment is out of scope.

### Compatibility assessment

| Criterion | Expected | Confidence | Source |
|-----------|----------|-----------|--------|
| ERC-20 Transfer logs | ✓ | High | ERC-20 standard |
| `totalSupply()` available | ✓ | High | ERC-20 standard |
| Zero-address mint/burn | ❓ | Medium | StraitsX contract may use non-zero addresses for treasury; requires probe |
| Ethereum deployment | ✓ | High | Publicly documented |
| Base/Arbitrum deployment | ❓ | Low | Not confirmed |
| 7-day volume sufficient | ⚠ | Medium-low | XSGD total supply ~$30–80M; Ethereum volume may be very low |
| `historical_supply` gate | ✓ | Medium | Should work; verify with probe |
| Config fields complete | Requires creation | — | Need YAML config files |

### Key unknown: mint/burn convention

XSGD was not originally designed to the same Circle MintBurnable pattern. If the issuer mints to a non-zero treasury address rather than from `0x0`, the framework's mint/burn count will be zero and the supply invariant check will still hold (it does not depend on mint/burn count), but `mint_count` and `burn_count` will be incorrect. **This must be probed before treating XSGD as compatible.**

### Risks not covered by on-chain audit

- SGD reserves held at Singapore-regulated banks (off-chain)
- SGD/USD FX exposure
- Zilliqa ↔ Ethereum bridge supply relationship
- Low liquidity / thin orderbook (not in scope, but affects interpretability)

### Preliminary verdict

**Usable with config — conditional** on (1) verifying mint/burn convention via log probe, (2) confirming Ethereum 7-day transfer volume is sufficient (risk of **insufficient volume**), (3) creating YAML config files. If the zero-address mint/burn convention is not used, only supply-invariant and transfer-count metrics are valid; mint/burn counts and gross-to-net ratio would be unreliable.

---

## Probe commands

Run these probes **before** creating config files. All commands require `--asset` and chain-specific block ranges.

### Step 1: Metadata check

```bash
# Once eurc.ethereum.yml config exists:
cargo run --release -- metadata --asset EURC --chains ethereum
cargo run --release -- metadata --asset XSGD --chains ethereum
```

Expected output: contract address, name, symbol, decimals, deployer, totalSupply at window boundaries.

### Step 2: Historical totalSupply boundary check

```bash
cargo run --release -- resolve-window \
  --chains ethereum \
  --from 2026-05-12T00:00:00Z \
  --to   2026-05-19T00:00:00Z
```

Then run `metadata` with `--from-block` / `--to-block` to confirm `totalSupply()` returns non-zero values at both boundaries.

### Step 3: Transfer log volume probe

Use `eth_getLogs` on a 500-block sample to estimate 7-day volume:

```bash
# Documented probe — run manually or via the probe script below
# scripts/probe_token_volume.py --asset EURC --chain ethereum --sample-blocks 500
# scripts/probe_token_volume.py --asset XSGD --chain ethereum --sample-blocks 500
```

A probe script (`scripts/probe_token_volume.py`) is provided below to estimate 7-day transfer count without running a full audit.

### Step 4: Mint/burn convention check

Inspect a known mint transaction for each token to confirm `from == 0x0000...0000`:

```python
# In scripts/probe_token_volume.py, pass --check-mint-convention
# Look for Transfer(from=0x0) in recent mint logs
```

---

## Summary verdict table

| Asset | Chains available | Zero-addr mint/burn | Volume estimate | Config effort | Verdict |
|-------|-----------------|---------------------|-----------------|---------------|---------|
| EURC | Ethereum, Base (verify Arbitrum) | ✓ likely | Moderate | 2–3 YAML files | **Usable with config** |
| XSGD | Ethereum only (confirm) | ❓ must probe | Low–moderate | 1–2 YAML files + validation | **Usable with config — conditional** |

---

## Next steps (after USDC panel is complete)

1. Verify contract addresses from official issuer documentation
2. Add `ALCHEMY_ETHEREUM_URL` already exists; no new RPC env vars needed for Ethereum-only probe
3. Run Step 3 + Step 4 probes to confirm volume and mint/burn convention
4. If probes pass: create YAML config files and run a single test window (e.g. 7-day)
5. If XSGD mint/burn convention differs: document as a known limitation and suppress `mint_count` / `burn_count` / `gross_to_net_ratio` in findings

**Do not extend the geo-policy panel to EURC or XSGD until the USDC panel (all 6 windows) is complete.**
