//! Shared setup of the keeper scenarios.

#![allow(dead_code)]

#[allow(unused_imports)]
pub use shortwin_keeper::ExitResult;
pub use shortwin_keeper::{Alert, ExitKind, KeeperConfig, TickReport};
pub use shortwin_keeper_litesvm::Harness;
pub use shortwin_litesvm_tests::engine::{Borrower, MarketSpec, MockMarket};
pub use shortwin_litesvm_tests::errors::ok;
pub use shortwin_litesvm_tests::{Env, mock};

/// Positions of this size fit every default cap (≈ 3 SOL of notional on the default pool).
pub const Q: u64 = 3_000_000_000_000;

/// A test-venue environment with `n` default mock markets and a keeper with `cfg`.
pub fn setup(n: usize, cfg: KeeperConfig) -> (Harness, Vec<MockMarket>) {
    let mut env = Env::test_venue();
    let markets = (0..n)
        .map(|_| env.mock_market(MarketSpec::default()))
        .collect();
    (Harness::new(env, cfg), markets)
}

/// Opens a position of `q` on `mm` for a new borrower (must succeed). A crank one second
/// before integrates the warm-up, so the keeper's first observation follows no gap (the
/// warm-up itself would otherwise count as a `CoverageGap` once a position is open).
pub fn open(h: &Harness, mm: &MockMarket, q: u64) -> (Borrower, u64) {
    h.with_env(|env| {
        env.crank(mm);
        env.warp(1);
        let mut b = env.borrower();
        let (res, nonce) = env.open(mm, &mut b, q);
        ok(res);
        (b, nonce)
    })
}

use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::Instruction;
use shortwin_litesvm_tests::{errors, pda, read};
use solana_keypair::Keypair;
use solana_signer::Signer;
use solana_transaction::Transaction;

/// An outside keeper that only simulates: the oracle of "would the program accept this
/// exit now" against which the keeper's timing is checked.
pub struct Oracle {
    pub kp: Keypair,
}

impl Oracle {
    /// A funded outside key.
    pub fn new(h: &Harness) -> Self {
        let kp = h.with_env(|env| env.funded_keypair());
        Self { kp }
    }

    /// Simulates `ix` (signed by this key) on the current state: `Ok` if the program accepts
    /// it, else the custom error code (if any).
    pub fn accepts(&self, h: &Harness, ix: Instruction) -> Result<(), Option<u32>> {
        h.with_env(|env| {
            let tx = Transaction::new_signed_with_payer(
                &[ix],
                Some(&self.kp.pubkey()),
                &[&self.kp],
                env.svm.latest_blockhash(),
            );
            match env.svm.simulate_transaction(tx) {
                Ok(_) => Ok(()),
                Err(f) => Err(errors::custom_code(&Err(f))),
            }
        })
    }
}

/// Whether the position `(borrower, nonce)` of `mm` is still open.
pub fn position_open(h: &Harness, mm: &MockMarket, borrower: &Pubkey, nonce: u64) -> bool {
    h.with_env(|env| {
        read::borsh::<shortwin::state::ShortPosition>(
            &env.svm,
            &pda::position(&mm.market(), borrower, nonce).0,
        )
        .is_some()
    })
}

/// The position account key.
pub fn position_key(mm: &MockMarket, borrower: &Pubkey, nonce: u64) -> Pubkey {
    pda::position(&mm.market(), borrower, nonce).0
}

/// The exits of `kind` that landed in `r`, for `position`.
pub fn landed_for<'a>(
    r: &'a TickReport,
    kind: ExitKind,
    position: &'a Pubkey,
) -> impl Iterator<Item = &'a shortwin_services_common::TxOutcome> + 'a {
    r.exits
        .iter()
        .filter(move |e| e.kind == kind && e.position == *position)
        .filter_map(|e| e.result.landed())
}
