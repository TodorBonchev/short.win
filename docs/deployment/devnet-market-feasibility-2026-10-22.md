# Devnet initialized-market feasibility, 2026-10-22

**Disposition: hold signed initialization and market transactions.** The
program-only 1.0.3 deployment is complete, but the public devnet dependencies
do not yet support a verified production-governance rehearsal or a justified
live Pump/PumpSwap market packet. This is a read-only assessment, not a failed
simulation or a claim that the Pump adapters are incompatible.

## Current public-chain evidence

The read-only endpoint was `https://api.devnet.solana.com`, confirmed
commitment, genesis `EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG`.
A `getMultipleAccounts` read at context slot **513925255** found the short.win
Program executable and its ProgramData still at deploy slot **513919880**.
`GlobalConfig` `ALyVdw8YE71nVstrmqnn4Preww99ZDbvAtYfxDbcQRuW`,
`VenueGuard` `DZJN1doY9r5BUSybJz4NibiLQfMKtGvhrxxME4QY2Sfp` and
`OracleCache` `HfeS818hsgNEC2TZ8SYN3PEcUsmyMLrwszDKUYitdZzP` were absent.
The [deployment receipt](DEVNET_1_0_3_DEPLOYMENT_2026_10_22.md) proves the
full on-chain short.win ELF hash independently.

The same read showed Squads v4 ProgramData
`Fy3YMJCvwbAXUgUM5b91ucUVA3jYzwWLHL3MwBqKsh8n` owned by the upgradeable
loader at deploy slot **446638068**, with a present upgrade authority. The
previous [full public-input survey](DEVNET_1_0_3_READINESS_2026_10_22.md#public-devnet-observations)
decoded that authority as
`HM5y4mz3Bt9JY9mr1hkyhnvqxSH4H2u2451j7Hc2dtvK` and found the canonical
Raydium v4, CPMM and Jupiter program IDs nonexecutable on devnet. The current
read found Pump, PumpSwap and pump-fees ProgramData at deploy slots
**510657843**, **508659436** and **508659783** respectively. These are
observations at particular slots; recheck before a later packet.

Full read-only `getAccountInfo` responses supplied the venue ProgramData
bytes. For each, the calculation was
`sha256(program_data[45:].rstrip(b"\0"))`, matching the repository's
[`trimmed_elf`](../../crates/shortwin-venue-state/src/program_data.rs) rule.
The corresponding pinned mainnet fixture was hashed after the same trimming.

| Executable | Devnet sample slot | Devnet trimmed length / SHA-256 | Pinned fixture trimmed length / SHA-256 |
|---|---:|---|---|
| Pump | 513925688 | 1,899,433 B / `e202096978d8f20935153a4271938fd9f0a421d64c1749986b4c5c20ca2a52be` | 1,934,785 B / `756c04ab1ac76122dcdcf2eeb0073bb139087b23d58d3f0d6c895510ab59d582` (`pump_2026_10_21.so`) |
| PumpSwap | 513925689 | 1,233,449 B / `3ffd5e3b5a31ccc86b434405e8a1876926da1e8685f891dc964b7f919c172ee1` | 1,268,489 B / `4d5fc41918374b24b16d8372d14a1ac64c768c83a2971bfeef1c8660dad649e7` (`pump_amm_2026_10_21.so`) |
| Pump-fees | 513925729 | 853,513 B / `9701b698bf970005ceb3f5358ff691bf8008cd9f9348440beb1c35fc47230f6a` | 873,241 B / `8ad2ca93c7ae97a7fb0de8a99de4d8825ccfdf89f73138cd49bda948e124b8b8` (`pump_fees_2026_10_12.so`) |

The fixture ELF files are in
[`tests/fixtures/programs`](../../tests/fixtures/programs). The difference
proves the local mainnet-binary replay is **not** execution against the current
devnet binaries. It does not prove an interface mismatch. The [official Pump
program documentation](https://github.com/pump-fun/pump-public-docs/blob/main/docs/PUMP_PROGRAM_README.md)
and [official PumpSwap documentation](https://github.com/pump-fun/pump-public-docs/blob/main/docs/PUMP_SWAP_README.md)
identify the same program IDs on mainnet and devnet; they do not attest that
these different executable hashes implement the exact accounts, layouts and
trade semantics required by the pinned
[`pump_curve`](../../programs/shortwin/src/venues/pump_curve.rs) and
[`pump_amm`](../../programs/shortwin/src/venues/pump_amm.rs) adapters.

A coherent `getMultipleAccounts` read of the Clock sysvar and the configured
Pyth `PriceUpdateV2` at slot **513925489** found a `Full` SOL/USD update with
the configured feed ID `ef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d`.
The chain Clock was **2026-10-22 15:11:39 UTC**, publish time **15:11:05
UTC**, so the sampled age was **34 seconds**, inside the current 120-second
limit. The prior 134-second sample was transient; freshness must be checked
again at execution. The read also found the account at the expected Pyth
receiver owner and 134-byte layout. It did not send an oracle crank.

A bounded recent-transaction crawl against the public RPC returned
`429 Too many requests for a specific RPC call` for some `getTransaction`
requests. Several returned transactions showed current Pump `CreateV2` or
`BuyExactQuoteInV3`/`SellV2` logs, but that partial sample cannot select a
supported SOL-quoted, non-cashback test mint or establish its liquidity and
counterparty control. Do not turn the 429s into an unbounded retry or call
those samples a market-compatibility test.

## Governance and capital gates

The program's [`initialize_config`](../../programs/shortwin/src/instructions/initialize_config.rs)
checks its own current ProgramData upgrade-authority signature and distinct
nonzero public authority keys; it does **not** check that Squads is immutable.
The repo's [`shortwin-gov verify`](../../services/ops/src/verify.rs) explicitly
fails when Squads v4 has an upgrade authority, as it does on this devnet. The
official [Squads security description](https://docs.squads.so/main/basics/security)
describes the immutable production program; it does not change the observed
devnet ProgramData. The owner cannot make the existing third-party devnet
Squads program immutable. One could initialize with an explicitly labelled
devnet-only authority layout and still leave the verifier failing; that would
rehearse narrower program behavior, **not** the production authority topology.
No such layout, member identities, create keys or funded fee vault exists in
this packet. Four separate devnet-only service/smoke identities were
[staged later](devnet-operator-plan-2026-10-22.md#selected-topology-and-identities),
including a coverage-pauser public key, but it is not installed as protocol
authority and remains unfunded. The repo's `shortwin-gov init-config` also requires a funded fee
vault and either actual live guard deploy slots or reviewed configured values
for missing ProgramData; recording stale mainnet slots as if they were devnet
evidence would not close this gate.

Confirmed devnet `getMinimumBalanceForRentExemption` returned the following
deterministic minimums for the intended 3-of-5 Governance and 2-of-4 Emergency
layout and short.win's initialized accounts:

| Phase | Lamports | SOL | Derivation |
|---|---:|---:|---|
| Governance multisig | 2,159,000 | 0.002159000 | 297 B, Squads `Multisig::size(5)` |
| Emergency multisig | 1,991,360 | 0.001991360 | 264 B, Squads `Multisig::size(4)` |
| Fee vault required by ops | 890,880 | 0.000890880 | `shortwin-gov` prefund requirement; above the bare 0-byte rent minimum |
| `initialize_config` PDAs | 146,004,280 | 0.146004280 | Config 8,006,080 + guard 2,194,560 + four extras at 33,731,200 each + oracle 878,840 |
| **Prerequisite subtotal** | **151,045,520** | **0.151045520** | Excludes network fees and governance proposals |

The first market itself requires at least Market rent **16,418,560**,
MarketVault rent **11,826,240**, pool token-account rent **1,488,440** and
the current **5,000,000-lamport** market creation fee: **34,733,240 lamports
(0.034733240 SOL)** before Pump-side user accumulator/token accounts, ALT if
needed, lender token inventory, borrower's SOL margin, keeper/coverage funding,
venue buyback liquidity and transaction fees. These amounts are not a total
spend ceiling. Exact signed steps and capital can be priced only for a chosen
market and authority layout, then simulated and reviewed transaction by
transaction. The remaining devnet payer balance after the loader deployment
was 16.638061120 SOL at the prior post-deployment read; it is not evidence
that separate service and smoke wallets are funded.

Before an initialization packet, select and publish the devnet-only governance
trust/scope decision, real distinct public member identities, install and fund
the selected coverage identity,
actual fee-vault and current guard-slot inputs, and build/source identities for
the ops client. A `shortwin-gov verify` failure from mutable Squads must remain
visible; do not weaken its immutability rule or mark production governance
verified. Before a market packet, additionally prove the exact devnet Pump,
PumpSwap and pump-fees accounts and CPI interface used by the adapters; select
an actual SOL-quoted non-cashback mint and valid curve or canonical pool;
check reserves, fees, owner, LP burn/lock where applicable and a fresh oracle;
then set a finite capital ceiling and fund and monitor keeper, reader and
test participants. Keep Raydium/Jupiter and ordinary users disabled. A
local/fork rehearsal using the pinned immutable mainnet Squads and fixture
venues remains the honest route for production-topology behavior until the
public-devnet differences have a separately reviewed disposition.
