# USDC Initial Probe Results

These results were collected during the pre-implementation probe to validate that the RPC strategy is viable before committing to windowed fetch logic.

## Probe Configuration

- Block window: ~8,000 blocks on Ethereum, ~5,000 blocks on Base and Arbitrum
- Queries run: metadata calls, current totalSupply, historical totalSupply, Transfer log count estimates, control event queries

## Results by Chain

### Ethereum

| Item | Result |
|---|---|
| Contract | `0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48` |
| Transfer log count (probe window ~8k blocks) | ~568,000 |
| Metadata calls (name/symbol/decimals/totalSupply) | PASS |
| Historical totalSupply at start-1 and end | PASS |
| Control event query | PASS |
| EIP-1967 proxy implementation slot | Unknown (not blocking) |

### Base

| Item | Result |
|---|---|
| Contract | `0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913` |
| Transfer log count (probe window ~5k blocks) | ~306,000 |
| Metadata calls | PASS |
| Historical totalSupply | PASS |
| Control event query | PASS |
| EIP-1967 proxy implementation slot | Unknown (not blocking) |

### Arbitrum

| Item | Result |
|---|---|
| Contract | `0xaf88d065e77c8cC2239327C5EDb3A432268e5831` |
| Transfer log count (probe window ~5k blocks) | ~9,200 |
| Metadata calls | PASS |
| Historical totalSupply | PASS |
| Control event query | PASS |
| EIP-1967 proxy implementation slot | Unknown (not blocking) |

## Conclusions

1. All metadata and historical totalSupply calls pass on all three chains.
2. Log volumes on Ethereum (~568k in ~8k blocks) and Base (~306k in ~5k blocks) are too large for a single `eth_getLogs` request. Arbitrum volume is lower but still significant.
3. **Windowed (paginated) fetch mode is required for v0.1.** Log fetching must be split into sub-ranges of at most a few hundred blocks per request.
4. The EIP-1967 proxy implementation slot was not resolved during the probe. This does not affect supply accounting and is deferred to a later milestone.
5. The RPC strategy (Alchemy HTTP endpoints, alloy `RootProvider<Http<Client>>`) is confirmed viable.
