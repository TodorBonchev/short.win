//! The pump-curve adapter (M5; ARCH §2.4.1 PumpCurve row, §2.5, §3.2, §3.12, §4.3, §5.3, §5.4,
//! §8.2) on the mainnet fork: the release short.win trading against the **dumped** pump, pump-fees
//! and PumpSwap programs and the pump coins of `tests/fixtures/` (`shortwin_litesvm_tests::fork`,
//! `shortwin_litesvm_tests::pump`). Every instruction goes through `Env::send`, so the invariant
//! hooks run after each one — on curve markets with the harness's own venue view
//! (`invariants::venue_view`: I13 re-quotes, I16 route rules and records).
//!
//! | Module | What |
//! |---|---|
//! | `listing` | `create_market(Auto)` on every pump coin of the dump: the curve cache, the trader's base ATA and pump UVA, the price start; the list-only outcomes; every listing refusal |
//! | `flows` | open → close to the lamport on the curve (Token-2022 125- and 115-byte curves, a legacy SPL-Token 115-byte curve without a creator, the 151-byte extended curve), every delta against `shortwin-math` on the readers' view |
//! | `refusals` | every `open_short` refusal on a curve market, in the program's order |
//! | `structure` | I14: every ReadSet / TradeExtras slot of open, close, crank and `sol_settle` key-checked; any listed recipient; the creator vault follows the live creator; extras on the sell leg ride along (appended to the `sell_v2` CPI), a hostile set is bypassed with `previous` (G-H2); the curve and its ATAs on every extras site and the borrower on the keeper exits are `DeniedAccount` (M10 batch 3) |
//! | `rent` | T-RENT (curve): the creator-vault top-up on sales (out of `S`, `ert` refunded) and buys (inside `B`), the small sale it refuses, the UVA paid at listing; a short curve's growth to 141 bytes (the 2026-10-02 redeploy) out of `S` |
//! | `pda` | T-PDA-USER (curve): `buy_v2` / `sell_v2` with the trader PDA as `user` under CPI, read off the inner instructions |
//! | `straddle` | `CurveStraddle` above `rT`; `B̂_str` against the executed bundle (completing buy, `migrate_v2`, PumpSwap `buy`) on the real ELFs; the reader's synthetic fee on the mint's live supply (pump-final P1 correction (a)) |
//! | `graduation` | `PumpCurve → PumpMigrating` (a completing buyback, an external completion), the migration pause and S3 at 70 min, `migrate_v2` (the next crank moves the market onto its canonical pool, M6's `→ PumpSwap`; `pamm_it` runs it with positions), `→ None` (graduated elsewhere) and S3 on its detection set, `None → PumpCurve` re-detection; M4's carry-overs: a graduated-elsewhere market with a position open adopts its Raydium pool through the collection window (switch only if all fit, the deepest candidate, the position closed on the new pool), and the re-detection clears a running window and the route chains |
//! | `fees` | venue admin with a position open (M10): pump `FeeConfig` hikes between the caps and above the hard cap (G-G4: opens refused, S3 evidence, risk and FV at the locked cap, closes and liquidations at the live fee), a curve layout change (G-H3: not executable, S3 at the cap), the Emergency route-broken flag (G-D6: S3 after the timer, liquidations identical second by second, O15's race against an honest expiry) |
//! | `buckets` | M10 pump revalidation, on the 2026-10-02 redeploy: `buy_v3` / `sell_v3` keep their fees in the curve's buckets (@125/@133) outside `rQ` at the v2 quote (the 2026-09 build has no v3), short.win's open and close on a curve carrying buckets to the lamport, `migrate_v2` after v3 fees seeds the pool `B̂_str` prices and leaves the buckets; the new pool is 287 bytes |
//! | `exits` | expiry and liquidation on the curve, `sweep_trader`'s token leg, token donations during a leg, a `trader_wsol` donation surviving curve legs until `sweep_trader` (FUZZ-M7-1), the pump / pump-fees / PumpSwap latch bits |
//!
//! Opens and closes are sent with ARCH §6.1's PROPOSED 250 k CU limit (`VenueMarket::cu_limit`),
//! keeper exits with 300 k: curve opens measure 176–194 k CU, above the runtime's 200 k default
//! only in their variance.

// `TxResult` is LiteSVM's own `TransactionResult` (as in the harness library).
#![allow(clippy::result_large_err)]

mod buckets;
mod common;
mod exits;
mod fees;
mod flows;
mod graduation;
mod listing;
mod pda;
mod refusals;
mod rent;
mod straddle;
mod structure;

/// The curve fork E2E's machinery (`pump_curve_support.rs`): the keeper's view of a curve
/// position (`CurveCtx`: the curve's pricing at the live fee or a cap), for `fees`.
#[path = "../pump_curve_support.rs"]
mod curve_support;
