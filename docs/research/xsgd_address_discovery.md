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

---

### Polygon — `0xDC3326e71D45186F113a2F448984CA0e8D201995`

Not probed — no `ALCHEMY_POLYGON_URL` in current RPC config. Polygon is the primary native XSGD chain per StraitsX documentation and likely has the highest volume. **Requires adding Polygon RPC support before probing.**

---

### Avalanche — `0xb2F85b7AB3c2b6f62DF06dE6aE7D09c010a5096E`

Not probed — no Avalanche RPC config. Lower priority; probe after Polygon.

---

## Summary verdict table

| Chain | Address | Status | Note |
|-------|---------|--------|------|
| Ethereum | `0x70e8de73...` | **Insufficient volume** | ~80/7d; too sparse |
| Polygon | `0xDC3326e7...` | **Needs probe** | Primary chain; no RPC config yet |
| Arbitrum | `0xE333e775...` | **Inactive** | Zero activity in 50K-block sample |
| Base | `0x0A4C9cb2...` | **Active** ✓ | ~13.7K/7d; mint convention confirmed |
| Avalanche | `0xb2F85b7A...` | **Needs probe** | No RPC config yet |
| Polygon bridged | `0x769434dC...` | **Wrong address** | Deprecated; do not use |

---

## Recommended next steps

1. **Add Base to audit config** — `configs/tokens/xsgd.base.yml` — address and mint/burn convention confirmed.
2. **Add Polygon RPC** — `ALCHEMY_POLYGON_URL` in `.env` + `configs/tokens/xsgd.polygon.yml` — then re-probe to confirm volume and mint/burn convention before auditing.
3. **Skip Ethereum and Arbitrum** for now — insufficient volume and inactive respectively.
4. **Do not add XSGD to main audit config** until at least one chain is fully probed and a test window completes.

**Do not extend the geo-policy panel to XSGD until the USDC policy-conditioned panel analysis is complete.**
