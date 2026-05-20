# XSGD address discovery

**Purpose:** Identify active, auditable XSGD contract addresses across EVM chains before adding XSGD to the geo-policy-conditioned audit config.

**Scope:** Address discovery and on-chain probing only. No reserve adequacy claim, payment-volume conclusion, safety ranking, or causality statement.

**Probed:** 2026-05-20

---

## Issuer

| Field | Value |
|-------|-------|
| Issuer | StraitsX (Xfers Pte. Ltd. / FAZZ Financial Group) |
| Fiat peg | Singapore Dollar (SGD) |
| Jurisdiction | Singapore — MAS Payment Services Act, Major Payment Institution licence |
| Token standard | ERC-20 with zero-address mint/burn pattern (confirmed on Base — see below) |
| Decimals | 6 |

---

## Official addresses

Sources: [straitsx.com/xsgd](https://www.straitsx.com/xsgd), StraitsX blog announcements, Etherscan/Arbiscan verified contracts.

| Chain | Contract address | Source | Probe result |
|-------|-----------------|--------|--------------|
| Ethereum | `0x70e8de73ce538da2beed35d14187f6959a8eca96` | Etherscan verified + StraitsX domain | **Insufficient volume** |
| Polygon | `0xDC3326e71D45186F113a2F448984CA0e8D201995` | StraitsX blog + PolygonScan verified | **Not probed** (no RPC config) |
| Arbitrum | `0xE333e7754a2DC1E020a162Ecab019254b9DaB653` | StraitsX blog + Arbiscan | **Inactive** |
| Base | `0x0A4C9cb2778aB3302996A34BeFCF9a8Bc288C33b` | StraitsX domain | **Active** ✓ |
| Avalanche | `0xb2F85b7AB3c2b6f62DF06dE6aE7D09c010a5096E` | StraitsX domain | **Not probed** (no RPC config) |

### Deprecated address (do not use)

| Chain | Address | Status |
|-------|---------|--------|
| Polygon (bridged) | `0x769434dcA303597C8fc4997Bf3DAB233e961Eda2` | **Wrong address** — unsupported bridged version; StraitsX explicitly warns against use |

---

## Probe results

### Ethereum — `0x70e8de73ce538da2beed35d14187f6959a8eca96`

```
sample: 5,000 blocks  |  Transfer logs: 8
estimated 7-day count: ~80
zero-address mint: 0 observed  |  zero-address burn: 0 observed
```

**Verdict: insufficient volume.** ~80 transfers per 7-day window is too sparse for a meaningful supply invariant check. The contract is confirmed correct and live (Etherscan verified), but XSGD activity has migrated to other chains. Mint/burn convention could not be confirmed in this sample — extend to a historical window with known issuance activity if needed.

---

### Arbitrum — `0xE333e7754a2DC1E020a162Ecab019254b9DaB653`

```
sample: 50,000 blocks  |  Transfer logs: 0
```

**Verdict: inactive.** Zero Transfer events in a 50,000-block sample. The address is officially listed on StraitsX but has no observable activity on Arbitrum at this time. Do not add to audit config.

---

### Base — `0x0A4C9cb2778aB3302996A34BeFCF9a8Bc288C33b`

```
sample: 50,000 blocks  |  Transfer logs: 2,266
estimated 7-day count: ~13,704
zero-address mint: 1 observed  |  zero-address burn: 0 observed
```

**Verdict: active.** Moderate volume (~13.7K transfers/7d), sufficient for a meaningful audit window. Zero-address mint convention confirmed. Burn not observed in sample — consistent with infrequent burn events relative to sample size, not a compatibility concern.

#### First audit result — `xsgd_7d_20260513_20260520`

| Field | Value |
|-------|-------|
| Blocks | 45920527 – 46222926 |
| Transfer events | 15,309 |
| Active senders | 275 |
| Active recipients | 341 |
| Mint count | 5 |
| Burn count | 0 |
| sum_mints_raw | 307,553,000,000 (= +307,553.00 XSGD) |
| sum_burns_raw | 0 |
| net_mint_raw | 307,553,000,000 |
| Supply at start | 6,617,005.00 XSGD |
| Supply at end | 6,924,558.00 XSGD |
| discrepancy_raw | 0 |
| supply_invariant_pass | **true** |
| All QA gates | **PASS** |
| gross_to_net_ratio | 1.0000 (burns = 0) |

All five QA gates pass. Supply invariant holds exactly (discrepancy = 0). Benchmark published to `docs/benchmarks/xsgd_7d_20260513_20260520/`.

---

### Polygon — `0xDC3326e71D45186F113a2F448984CA0e8D201995`

```
sample: 50,000 blocks  |  Transfer logs: 524
estimated 7-day count: ~3,169
zero-address mint: 2 observed  |  zero-address burn: 0 observed
```

**Verdict: active.** Moderate volume (~3.2K transfers/7d), lower than Base (~15K) but sufficient for a meaningful audit window. Zero-address mint convention confirmed. Burn not observed in sample — consistent with infrequent burn events. Config created at `configs/tokens/xsgd.polygon.yml`; ready to run a transfer-audit window.

---

### Avalanche — `0xb2F85b7AB3c2b6f62DF06dE6aE7D09c010a5096E`

Not probed — no Avalanche RPC config. Lower priority; probe after Polygon.

---

## Summary verdict table

| Chain | Address | Status | Note |
|-------|---------|--------|------|
| Ethereum | `0x70e8de73...` | **Insufficient volume** | ~80/7d; too sparse |
| Polygon | `0xDC3326e7...` | **Active** ✓ | ~3.2K/7d; mint convention confirmed |
| Arbitrum | `0xE333e775...` | **Inactive** | Zero activity in 50K-block sample |
| Base | `0x0A4C9cb2...` | **Audited** ✓ | 15,309 transfers/7d; all gates PASS; benchmark published |
| Avalanche | `0xb2F85b7A...` | **Needs probe** | No RPC config yet |
| Polygon bridged | `0x769434dC...` | **Wrong address** | Deprecated; do not use |

---

## Recommended next steps

1. ~~**Add Base to audit config**~~ — **Done.** `configs/tokens/xsgd.base.yml` created; first audit window (`xsgd_7d_20260513_20260520`) published with all gates PASS.
2. ~~**Add Polygon RPC**~~ — **Done.** Probe confirmed ~3.2K/7d, mint convention compatible. `configs/tokens/xsgd.polygon.yml` created. Ready to run a 7-day transfer-audit window.
3. **Skip Ethereum and Arbitrum** for now — insufficient volume and inactive respectively.
4. **Do not add XSGD to main audit config** until the USDC policy-conditioned panel analysis is complete.

**Do not extend the geo-policy panel to XSGD until the USDC policy-conditioned panel analysis is complete.**
