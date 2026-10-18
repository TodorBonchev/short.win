# shortwin-venue-state

Length-tolerant, bytes-in / values-out readers of the Solana account state that the
[short.win](https://github.com/TodorBonchev/short.win) program prices, routes and guards on: Raydium AMM v4 and CPMM pools, pump
bonding curves, PumpSwap pools and configs, SPL / Token-2022 mints and token accounts, the
BPF upgradeable loader's `ProgramData` header and Pyth `PriceUpdateV2`.

- `#![no_std]`, `#![forbid(unsafe_code)]`, zero dependencies: keys are `[u8; 32]`, so it
  works on-chain (SBF), on the host and in WebAssembly alike.
- Each reader takes the account's owner and data as plain bytes, checks the owner, the length
  and the discriminator where one exists, and zero-pads reads past the end of a shorter
  account. Key checks (is this the right pool?) are the caller's job.
- The `test-venue` feature adds a TEST-ONLY mock venue reader; never enable it in production.

```toml
[dependencies]
shortwin-venue-state = "0.1"
```

The module table in the crate docs lists every reader. The same code runs inside the
short.win program, its keeper and its web frontend.

## Licence

Business Source License 1.1 (`LICENSE` in this package; Change Date 2030-10-01, Change
License Apache-2.0). Production use is permitted solely to interact with the Official
Deployment, as set out in the Additional Use Grant and in
[GRANTS.md](https://github.com/TodorBonchev/short.win/blob/main/GRANTS.md).
