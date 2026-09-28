//! Raydium pool selection (M4; ARCH §2.5 `None`, §2.6, §3.9, §4.4 `rewrite_liquidity`, §8.2
//! T-RAY-SWITCH / T-VENUE-SWITCH, §8.3 I15) on the mainnet fork: the release short.win against
//! the dumped AMM v4 and CPMM programs, the dumped pools and **planted** candidate pools
//! (`shortwin_litesvm_tests::pool_select::{plant_v4, plant_cpmm}`: real pool accounts cloned
//! onto fresh keys, traded by the dumped ELFs). Every instruction goes through `Env::send`, so
//! the invariant hooks run after each one — I15 (`invariants::pool_select`) included.
//!
//! | Module | What |
//! |---|---|
//! | `planted` | the planted pools themselves: the dumped ELFs swap on them exactly as `shortwin-math` quotes, either SOL side, both programs |
//! | `listing` | a list-only (`Auto`) market adopts its Raydium pool (M3-SEC-3 a): exact state, O13 / O13b ages, D1 precedence, base/quote order; the listing window (M3-SEC-3 b): a thin listing replaced by a deeper pool at the 5 % margin, a chain of proposals never extends the window |
//! | `zero_position` | adoption without a failure: the 25 % margin, `PositionsOpen` while a position is open and the adoption once it closed, AMM v4 ↔ CPMM switches with trades on the new pool |
//! | `fast_path` | the `pool_fail` chain (halt, fee above the hard cap; never the Emergency flag), 120 s, the 60 s collection window keeping the deepest candidate (not the first, V2-7), the execution after the window with open positions (locked terms, EMA continuity, the ring rewrite and a ≤ 1 % box from the first second, exits on the new pool), cancellations |
//! | `refusals` | every `propose_pool` / `execute_pool_switch` refusal: account checks, candidate kinds, eligibility (the locked-depth floor — locker-LP, thin —, fee, halted, other mint; λ under λ_min no refusal, M4-SEC-1), the incumbent itself, the latch (the candidate's program, M4-SEC-8), margins, windows |
//! | `attacks` | T-RAY-SWITCH's attacks: a fake deep pool of unlocked liquidity, an LP pull, proposal spam, an atomic price push around a proposal |
//! | `hooks` | the I15 hook (and I17's M4 part) fire on violating states |
//! | `trace_cu` | T-TRACE / T-CU: every path of both instructions makes only its event CPI (2 trace entries) within ARCH §6.1's 100 k CU (`-- --nocapture` prints the figures) |

// `TxResult` is LiteSVM's own `TransactionResult` (as in the harness library).
#![allow(clippy::result_large_err)]

#[allow(unused_imports)]
#[path = "../ray_it/common.rs"]
mod common;

mod attacks;
mod fast_path;
mod hooks;
mod listing;
mod planted;
mod refusals;
mod support;
mod trace_cu;
mod zero_position;
