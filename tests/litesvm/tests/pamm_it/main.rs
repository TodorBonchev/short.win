//! The PumpSwap adapter and graduation (M6; ARCH §2.4.1 PumpSwap row, §2.5 "On → PumpSwap", §3.2,
//! §3.4, §3.9, §3.12, §5.3–§5.5, §8.2) on the mainnet fork: the release short.win trading against
//! the **dumped** PumpSwap, pump and pump-fees programs and the canonical pools of
//! `tests/fixtures/` (groups `pamm_state`, `pamm_synthetic`, `pump_state`; `shortwin_litesvm_tests::
//! pamm`). Every instruction goes through `Env::send`, so the invariant hooks run after each one —
//! on PumpSwap markets with the harness's own venue view (`invariants::venue_view`: I13 re-quotes,
//! I16 route rules, I18 the post-change confirmation, the graduation's effects).
//!
//! | Module | What |
//! |---|---|
//! | `listing` | `create_market(Auto)` of a PumpSwap-class coin (SPL Token and Token-2022): the PumpSwap cache, the trader's base ATA and pAMM UVA, no price until the first PumpSwap observation, which starts it marked for the warm-up; a `None →` PumpSwap re-detection |
//! | `flows` | open → close to the lamport on every pool role (Token-2022 301 B: fresh after its boost window, mature, cashback; SPL Token 261 B and 211 B), every delta against `shortwin-math` on the readers' view; expiry and liquidation after a real pump on an SPL and a Token-2022 pool; `sol_settle`'s no-op and S1 after a held ×10 pump (S3 under the buy halt is `gates`') |
//! | `refusals` | every PumpSwap open refusal in the program's order: the warm-up, `disable_flags` (`VenueFlagged`, the buy bit `NoRoute`), the boost gate, the latch, the existence requirement, account substitution |
//! | `gates` | T-DISABLE, T-ATA, T-PDA-USER (pAMM buy), T-RENT (pAMM), T-CASHBACK (pAMM) |
//! | `negvqr` | the synthetic negative-VQR pool: pricing on `vault + VQR`, trades to the lamport, the tier inputs at `min(real, effective)` |
//! | `lock` | ★T-PAMM-LOCK (M6R-1, the owner's decision of 2026-09-30): the pool's locked depth `D` from its burned LP (the LP mint P6), key-checked; third-party `deposit` / `withdraw` against the size cap, the tier, the reference cap and the warm-up's `L_slow ≤ D` (`attacks/pumpswap_lp.py` parts 2 and 3); FITS-2 (a) and FITX-1 on a pool with third-party LP; M6D-1's pin (part 3 (e): a position sized on a held deposit gaps into S1 when the deposit is pulled at a genuine pump; an open owner item) |
//! | `graduation` | a curve market with open positions through completion, `PumpMigrating`, `migrate_v2`, `→ PumpSwap`, the warm-up and exits on the pool; a graduation without positions, then trading on the pool. The atomic `[buy_v2(rT), migrate_v2]` is `m5_sec_curve_it::m5_sec_4_an_atomic_complete_and_migrate_graduates_the_market_onto_its_pool`, a borrower's self-made straddle and the bundle `straddle_it::a_self_made_straddle_is_liquidated_on_the_pool_after_the_bundle`, the real `migrate_v2` scenarios `pump_curve_e2e` |
//!
//! Opens and closes are sent with a 300 k CU limit (`VenueMarket::cu_limit`), keeper exits with
//! 400 k.

// `TxResult` is LiteSVM's own `TransactionResult` (as in the harness library).
#![allow(clippy::result_large_err)]

mod common;
mod creator;
mod flows;
mod gates;
mod graduation;
mod listing;
mod lock;
mod negvqr;
mod refusals;

/// The graduation E2E's machinery (`graduation_support.rs`): the keeper's view of a PumpSwap
/// position (`pool_status`), for `lock`.
#[path = "../graduation_support.rs"]
mod support;
