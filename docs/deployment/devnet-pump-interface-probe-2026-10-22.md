# Devnet Pump/PumpSwap interface probe, 2026-10-22

**Disposition: no specific CPI incompatibility was proven, but this does not
qualify a live market smoke.** The current public devnet executables dispatch
the exact four trade discriminators emitted by short.win without reporting a
data-decoding error before their first missing-account check.
Their canonical configuration accounts also have the owners, lengths and
discriminators that short.win expects. A successful trade through short.win,
including its full account list and conditional remaining accounts, has not
been simulated or executed.

This is a bounded, read-only check of `https://api.devnet.solana.com` at
`confirmed` commitment. The probes used unsigned transactions with zero-filled
signature placeholders and RPC `simulateTransaction` with `sigVerify: false`
and `replaceRecentBlockhash: true`. No keypair was loaded, no transaction was
signed or sent, and no account state was changed. No market mint was selected.

## Published interface and short.win calls

Pump's published IDLs were pinned to upstream commit
[`639b87fb45c3adeffa6f7c4a1bf0cb85e392bf22`](https://github.com/pump-fun/pump-public-docs/tree/639b87fb45c3adeffa6f7c4a1bf0cb85e392bf22):
[`pump.json`](https://github.com/pump-fun/pump-public-docs/blob/639b87fb45c3adeffa6f7c4a1bf0cb85e392bf22/idl/pump.json)
(SHA-256 `7616681ea557975f9f623ac211080f22a93060781eaf4765b0a488d980997207`)
and [`pump_amm.json`](https://github.com/pump-fun/pump-public-docs/blob/639b87fb45c3adeffa6f7c4a1bf0cb85e392bf22/idl/pump_amm.json)
(SHA-256 `b7d8c57a4d9c4dd0109a9ab893052352333252d4eedced10ac091d4d66cab89b`).
The upstream [curve v3 trade guide](https://github.com/pump-fun/pump-public-docs/blob/639b87fb45c3adeffa6f7c4a1bf0cb85e392bf22/docs/instructions/TRADE_V3.md)
permits omission of the optional `partial_fill` byte. The upstream
[PumpSwap v2 guide](https://github.com/pump-fun/pump-public-docs/blob/639b87fb45c3adeffa6f7c4a1bf0cb85e392bf22/docs/instructions/PUMP_SWAP_TRADE_V2.md)
says its legacy v1 trades remain available. These are published interfaces,
not an attestation of the current devnet ELF.

| Short.win adapter | Direct CPI | Published fixed accounts and data | Short.win encoding |
|---|---|---|---|
| [`pump_curve.rs`](../../programs/shortwin/src/venues/pump_curve.rs) | `buy_v3` / `sell_v3` | 17 accounts each; discriminators `07051dc4f5176550` / `1c92de7726c469d5`; two `u64` arguments; buy has an optional final `OptionBool` | 17 accounts in published order; 24 data bytes; buy omits `partial_fill` |
| [`pump_amm.rs`](../../programs/shortwin/src/venues/pump_amm.rs) | v1 `buy` / `sell` | 23 / 21 fixed accounts; discriminators `66063d1201daebea` / `33e685a4017f83ad`; two `u64` arguments; buy has an optional final `OptionBool` | Published fixed-account order, followed by conditional remaining accounts; 25 / 24 data bytes, with `track_volume = false` on buy |
| Both adapters | `init_user_volume_accumulator` during listing | Six accounts, no arguments, discriminator `5e06ca73ff60e8b7` in both published IDLs | Same six account roles and eight data bytes; **not simulated here** |

The PumpSwap v1 IDL describes its fixed accounts but does not establish the
adapter's conditional trailing order for cashback, the `pool-v2` marker and
buyback recipients. Those branches require a real account-populated test.
`claim_cashback` is not a direct CPI from either adapter; the caller may bundle
it before short.win's `sweep_trader` separately.

## Current public devnet state

A bounded `getMultipleAccounts` read at context slot **513937544** found these
upgradeable-loader ProgramData headers. `solana program dump --url devnet`
read the three executable byte arrays without a transaction; the CLI dump has
no context slot. The displayed hash uses
`sha256(dumped_elf.rstrip(b"\0"))`, the same trailing-zero rule as the
repository's [`trimmed_elf`](../../crates/shortwin-venue-state/src/program_data.rs).
The hashes agree with the earlier full RPC reads recorded in the
[market-feasibility check](devnet-market-feasibility-2026-10-22.md#current-public-chain-evidence),
which supplies context slots for those byte samples.

| Program | ProgramData address | Deploy slot at sample | Trimmed devnet ELF length / SHA-256 |
|---|---|---:|---|
| Pump `6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P` | `B5MvUwXdiW1NMM6QFFD3ssPKBujD4zMohncbM73Z2BQu` | 510657843 | 1,899,433 B / `e202096978d8f20935153a4271938fd9f0a421d64c1749986b4c5c20ca2a52be` |
| PumpSwap `pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA` | `6naEzKeUuFh1Jeeu51NXQgr5qkXgXtc9WKNct4xynVJc` | 508659436 | 1,233,449 B / `3ffd5e3b5a31ccc86b434405e8a1876926da1e8685f891dc964b7f919c172ee1` |
| Pump-fees `pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ` | `75Uu23mqWBb8LM8vDppqC1mQAnCcBuLXhVaDezVMQLRw` | 508659783 | 853,513 B / `9701b698bf970005ceb3f5358ff691bf8008cd9f9348440beb1c35fc47230f6a` |

All three ProgramData headers named the present upgrade authority
`GUYCUEpxSkm1ccDo3LSKMR4Xpe5bgMnEZaFygWUSEVH3`. The devnet ELF hashes
differ from the pinned mainnet fixture hashes in the market-feasibility check.
The common program IDs therefore establish address availability only, not
binary identity or behavioral equivalence. A canonical `anchor idl fetch` on
devnet returned `Account not found` for each of the three programs; the
published IDLs were used as references rather than on-chain devnet IDLs.

A single full-data `getMultipleAccounts` read at context slot **513937303**
found:

| Canonical account | Owner | Size / discriminator | Selected decoded state |
|---|---|---|---|
| Pump `Global` `4wTV1YmiEkRvAtNtsSGPtUrqRYQMe5SKy2uB4Jjaxnjf` | Pump | 1,088 B / `a7e8e8b1c86c727f` | Initialized and migration enabled; first buyback recipient nonzero |
| Pump `FeeConfig` `8Wf5TiAheLUqBrKXeYg2JtAFFMWtKdG2BSFgqUcPVwTt` | Pump-fees | 4,097 B / `8f3492bbdb7b4c9b` | One fee tier encoded at offset 65 |
| PumpSwap `GlobalConfig` `ADyA8hdefvWN2dbGGWFotbzWxrAvLW83WG6QCVXvJKqw` | PumpSwap | 949 B / `95089ccaa0fcb0d9` | `disable_flags = 0`; first buyback recipient nonzero |
| PumpSwap `FeeConfig` `5PHirr8joyTMp9JMm6nW7hNDVyEYdkzDqazxPD7RaTjx` | Pump-fees | 4,097 B / `8f3492bbdb7b4c9b` | 25 fee tiers encoded at offset 65 |

These owner, length and discriminator checks align with
[`shortwin-venue-state`'s Pump reader](../../crates/shortwin-venue-state/src/pump.rs)
and [PumpSwap reader](../../crates/shortwin-venue-state/src/pump_amm.rs).
They do not decode a particular market's curve or pool, nor prove all fee
arithmetic or account mutations match the pinned mainnet build.

## Unsigned dispatch simulations

Each probe supplied exactly Short.win's instruction discriminator and argument
bytes, but intentionally supplied **zero venue account metas**. Thus the
expected result was successful instruction/data dispatch followed by
`AccountNotEnoughKeys` on the first account. No success path was exercised.

| Instruction | Devnet simulation slot | Data bytes | Observed instruction log | Error and first missing account |
|---|---:|---:|---|---|
| Pump `buy_v3` | 513937472 | 24 | `Instruction: BuyV3` | Anchor 3005 `AccountNotEnoughKeys`, `global` |
| Pump `sell_v3` | 513937472 | 24 | `Instruction: SellV3` | Anchor 3005 `AccountNotEnoughKeys`, `global` |
| PumpSwap `buy` | 513937473 | 25 | `Instruction: Buy` | Anchor 3005 `AccountNotEnoughKeys`, `pool` |
| PumpSwap `sell` | 513937473 | 24 | `Instruction: Sell` | Anchor 3005 `AccountNotEnoughKeys`, `pool` |

The simulation logs establish discriminator dispatch and no early
data-decoding error for these payloads. They do **not**
establish that later account checks, conditional remaining accounts, token
transfers, pricing, fee routing, synthetic migration or graduation work with
Short.win. No full short.win open/exit transaction was simulated.

## Next bounded check

Keep the [market-feasibility hold](devnet-market-feasibility-2026-10-22.md)
in place. Select one actual SOL-quoted noncashback mint and its valid live curve
or canonical pool using bounded account reads. Recheck the three ProgramData
deploy slots and executable hashes, that market's owners, reserves, flags and
fee config, and a fresh configured oracle. Build fully populated short.win
open and exit packets with the exact account order, all conditional PumpSwap
remaining accounts and a finite devnet capital limit; simulate each complete
packet and inspect balance deltas and logs. The devnet governance, initialized
market, funded keeper and coverage gates in the market-feasibility check must
also be resolved before any live market transaction.
