//! Invariant checks run after **every** instruction (ARCH §8.3; M1 exit criteria: I1, I2, I4,
//! I8, I17).
//!
//! Hook API:
//! - [`check_all`]`(svm, market)`: the per-market state invariants (I1, I2, I8). [`crate::Env`]
//!   runs it for every tracked market after every transaction (and, for a transaction of
//!   several instructions, after each prefix of it on a clone of the VM).
//! - [`snapshot`] + [`check_ix`]: the instruction-level invariants that compare the state
//!   **just before an instruction** with the state just after it (I4, the lender bounds) or read
//!   **that instruction's** events and signers (I17, I4's "admin-signed", the lender bounds).
//!   [`crate::Env::send`] snapshots before the transaction and after each proper prefix (on a
//!   clone of the VM), so instruction `k` of a transaction is checked against the state after
//!   instructions `1..k−1`, never against the transaction's start (M1-FID-3, M1-FID-5): a
//!   latch set by an earlier instruction of the same transaction counts for a later open, and a
//!   lender's own withdrawal is not blamed on an admin who signed another instruction.
//!
//! Status (M1): each check implements what can be checked on the stored state; the lender pool
//! (I1 token claims in both directions, the whole of I8 on the state the program sees, the
//! instruction-level allowance, claim, conservation and lock bounds) is [`lender`]; the parts marked `TRACK:` are M2's (positions in
//! I2/I4, opens in I17). Every check here works on **stored** values, which is sound where
//! noted: an untouched lender's stored `real` only lags its true value, so a stored-value
//! upper bound implies the true one.

use anchor_lang::AccountDeserialize;
use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::Instruction;
use litesvm::LiteSVM;
use litesvm::types::TransactionMetadata;
use shortwin::events::{ClaimSol, DeficitCovered, Deposit, LenderSynced, Open, Withdraw};
use shortwin::state::{LenderPosition, Market, MarketVault, ShortPosition, VenueGuard, VenueKind};
use shortwin_math::pool::{R_MAX, VIRTUAL_ASSETS, VIRTUAL_SHARES};

use crate::{events, pda, read};

pub mod lender;

/// A failed invariant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Violation {
    /// `"I1"`, `"I2"`, …
    pub invariant: &'static str,
    /// What failed, with the values.
    pub detail: String,
}

impl core::fmt::Display for Violation {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{} violated: {}", self.invariant, self.detail)
    }
}

fn fail(invariant: &'static str, detail: String) -> Result<(), Violation> {
    Err(Violation { invariant, detail })
}

fn ensure(
    ok: bool,
    invariant: &'static str,
    detail: impl FnOnce() -> String,
) -> Result<(), Violation> {
    if ok {
        Ok(())
    } else {
        fail(invariant, detail())
    }
}

/// Every `LenderPosition` of `market`.
pub fn lenders_of(svm: &LiteSVM, market: &Pubkey) -> Vec<(Pubkey, LenderPosition)> {
    read::all_of::<LenderPosition>(svm)
        .into_iter()
        .filter_map(|(k, a)| {
            LenderPosition::try_deserialize(&mut a.data.as_slice())
                .ok()
                .filter(|l| l.market == *market)
                .map(|l| (k, l))
        })
        .collect()
}

/// Every `ShortPosition` of `market`, with its lamports.
pub fn positions_of(svm: &LiteSVM, market: &Pubkey) -> Vec<(Pubkey, ShortPosition, u64)> {
    read::all_of::<ShortPosition>(svm)
        .into_iter()
        .filter_map(|(k, a)| {
            ShortPosition::try_deserialize(&mut a.data.as_slice())
                .ok()
                .filter(|p| p.market == *market)
                .map(|p| (k, p, a.lamports))
        })
        .collect()
}

/// The per-market state invariants (I1, I2, I8). A market that does not exist passes.
pub fn check_all(svm: &LiteSVM, market: &Pubkey) -> Result<(), Violation> {
    let Some(m) = read::zc::<Market>(svm, market) else {
        return Ok(());
    };
    let Some(v) = read::zc::<MarketVault>(svm, &m.market_vault) else {
        return fail("I2", format!("market {market} has no MarketVault"));
    };
    i1_token_claims(svm, market, &m)?;
    i2_vault_lamports(svm, market, &m, &v)?;
    i8_lender_pool(svm, market, &m, &v)?;
    lender::check_market(svm, market, &m, &v)?;
    Ok(())
}

/// I1: `T + ΣB_Q` = total lender token claims (SPEC §10); `pool_vault.amount ≥ T` (donations
/// ignored).
///
/// The claims side (Σ over the lenders on the current epoch of `⌊shares·(T + ΣB_Q + VA)/(S +
/// VS)⌋ ≤ T + ΣB_Q`, every lender touched) is [`lender::check_market`]'s.
pub fn i1_token_claims(svm: &LiteSVM, market: &Pubkey, m: &Market) -> Result<(), Violation> {
    let amount = read::token_amount(svm, &m.pool_vault).unwrap_or(0);
    ensure(amount >= m.tokens_free, "I1", || {
        format!(
            "market {market}: pool_vault.amount {amount} < T {}",
            m.tokens_free
        )
    })?;
    let by_tier: u128 = m.borrowed_q_by_tier.iter().map(|&q| u128::from(q)).sum();
    ensure(by_tier == u128::from(m.borrowed_q), "I1", || {
        format!(
            "market {market}: Σ borrowed_q_by_tier {by_tier} != ΣB_Q {}",
            m.borrowed_q
        )
    })
}

/// I2: `MarketVault.lamports ≥ rent_min + lender_sol + protocol_fees`; every `ShortPosition`
/// of the market has `lamports ≥ rent_min + v` (ARCH §2.3.1 A4).
pub fn i2_vault_lamports(
    svm: &LiteSVM,
    market: &Pubkey,
    m: &Market,
    v: &MarketVault,
) -> Result<(), Violation> {
    let lamports = read::lamports(svm, &m.market_vault);
    let rent = svm.minimum_balance_for_rent_exemption(MarketVault::SPACE);
    let need = u128::from(rent) + u128::from(v.lender_sol) + u128::from(v.protocol_fees);
    ensure(u128::from(lamports) >= need, "I2", || {
        format!(
            "market {market}: MarketVault lamports {lamports} < rent {rent} + lender_sol {} + \
             protocol_fees {}",
            v.lender_sol, v.protocol_fees
        )
    })?;
    let pos_rent = svm.minimum_balance_for_rent_exemption(ShortPosition::SPACE);
    for (key, p, lamports) in positions_of(svm, market) {
        ensure(
            u128::from(lamports) >= u128::from(pos_rent) + u128::from(p.v),
            "I2",
            || {
                format!(
                    "position {key}: lamports {lamports} < rent {pos_rent} + v {}",
                    p.v
                )
            },
        )?;
    }
    Ok(())
}

/// I8 (= ECON I-P1–I-P7), the parts checkable on stored state:
/// - claim side, strict per lender: `claimed ≤ real`; `Σ(real − claimed) + unmatched + vest_bal
///   ≤ lender_sol` (stored `real` lags the true value, so this is implied by the true one);
/// - the calendar: `lock_norm = Σ buckets`;
/// - shares: `S + VS ≤ R_MAX·(T + ΣB_Q + VA)` or `S = 0`; Σ shares of the lenders on the
///   current epoch ≤ `S`;
/// - `receivable = 0` and `A_j = 0` with no open position.
///
/// The rest of I8 — the entitlement side after touching every lender, `Σ allowances ≤ T +
/// dust` with each lender touched alone (`pool::allowance_view`, T-POT) and with everyone
/// touched, no lock effective at or after its `lock_until`, the calendar against the live
/// locks, and the wind-down liveness — is [`lender::check_market`]'s.
pub fn i8_lender_pool(
    svm: &LiteSVM,
    market: &Pubkey,
    m: &Market,
    v: &MarketVault,
) -> Result<(), Violation> {
    let lenders = lenders_of(svm, market);
    let mut owed: u128 = 0;
    let mut current_shares: u128 = 0;
    for (key, l) in &lenders {
        ensure(l.claimed <= l.real, "I8", || {
            format!("lender {key}: claimed {} > real {}", l.claimed, l.real)
        })?;
        owed += u128::from(l.real - l.claimed);
        if l.epoch == m.share_epoch {
            current_shares += l.shares;
        }
    }
    let claim_side = owed + u128::from(v.unmatched) + u128::from(m.vest_bal);
    ensure(claim_side <= u128::from(v.lender_sol), "I8", || {
        format!(
            "market {market}: Σ(real − claimed) {owed} + unmatched {} + vest_bal {} > \
             lender_sol {}",
            v.unmatched, m.vest_bal, v.lender_sol
        )
    })?;
    let buckets: u128 = v.lock_cal.iter().map(|b| b.get()).sum();
    ensure(buckets == v.lock_norm.get(), "I8", || {
        format!(
            "market {market}: lock_norm {} != Σ calendar buckets {buckets}",
            v.lock_norm.get()
        )
    })?;
    let s = m.total_shares.get();
    let claims = u128::from(m.tokens_free) + u128::from(m.borrowed_q) + VIRTUAL_ASSETS;
    ensure(
        s == 0 || s.saturating_add(VIRTUAL_SHARES) <= R_MAX.saturating_mul(claims),
        "I8",
        || format!("market {market}: S {s} + VS > R_MAX·(T + ΣB_Q + VA) = R_MAX·{claims}"),
    )?;
    ensure(current_shares <= s, "I8", || {
        format!("market {market}: Σ current-epoch shares {current_shares} > S {s}")
    })?;
    if m.open_positions == 0 {
        let aggr_zero = m.rent_aggr.iter().all(|a| a.get() == 0);
        ensure(m.receivable == 0 && aggr_zero, "I8", || {
            format!(
                "market {market}: no open position but receivable {} / A_j {:?}",
                m.receivable, m.rent_aggr
            )
        })?;
    }
    Ok(())
}

/// One market's balances before a transaction (I4).
#[derive(Clone, Debug)]
pub struct MarketSnapshot {
    pub market: Pubkey,
    pub pool_vault_amount: u64,
    pub market_vault_lamports: u64,
    pub protocol_fees: u64,
    pub positions: Vec<(Pubkey, u64)>,
}

/// The state just before an instruction, which the instruction-level checks compare against
/// (named for the transaction because a one-instruction transaction's is its start).
#[derive(Clone)]
pub struct TxSnapshot {
    pub markets: Vec<MarketSnapshot>,
    pub guard: Option<VenueGuard>,
    pub fee_recipient: Option<(Pubkey, u64)>,
    /// The lender-pool state of the tracked markets ([`lender::check_ix`]).
    pub lender: lender::LenderSnapshot,
}

/// Snapshots `markets` (and the latch and the fee recipient) before an instruction.
pub fn snapshot(svm: &LiteSVM, markets: &[Pubkey]) -> TxSnapshot {
    let snaps = markets
        .iter()
        .filter_map(|market| {
            let m = read::zc::<Market>(svm, market)?;
            let v = read::zc::<MarketVault>(svm, &m.market_vault)?;
            Some(MarketSnapshot {
                market: *market,
                pool_vault_amount: read::token_amount(svm, &m.pool_vault).unwrap_or(0),
                market_vault_lamports: read::lamports(svm, &m.market_vault),
                protocol_fees: v.protocol_fees,
                positions: positions_of(svm, market)
                    .into_iter()
                    .map(|(k, _, lamports)| (k, lamports))
                    .collect(),
            })
        })
        .collect();
    let config = read::zc::<shortwin::state::GlobalConfig>(svm, &pda::config());
    TxSnapshot {
        markets: snaps,
        guard: read::zc::<VenueGuard>(svm, &pda::venue_guard()),
        fee_recipient: config.map(|c| (c.fee_recipient, read::lamports(svm, &c.fee_recipient))),
        lender: lender::snapshot(svm, markets),
    }
}

/// The events of one top-level instruction that the instruction-level checks read.
#[derive(Default)]
pub struct IxEvents {
    pub withdraws: Vec<Withdraw>,
    pub claims: Vec<ClaimSol>,
    pub deposits: Vec<Deposit>,
    pub covers: Vec<DeficitCovered>,
    pub syncs: Vec<LenderSynced>,
    pub opens: Vec<Open>,
}

impl IxEvents {
    /// The events top-level instruction `ix` (0-based) of the transaction emitted.
    pub fn of(meta: &TransactionMetadata, ix: usize) -> Self {
        Self {
            withdraws: events::of_ix(meta, ix),
            claims: events::of_ix(meta, ix),
            deposits: events::of_ix(meta, ix),
            covers: events::of_ix(meta, ix),
            syncs: events::of_ix(meta, ix),
            opens: events::of_ix(meta, ix),
        }
    }
}

/// `true` if one of `admins` signs `ix` itself (a signer meta of that instruction; I4 is decided
/// per instruction, M1-FID-5: an admin co-signing another instruction of the transaction
/// does not make a lender's own withdrawal "admin-signed").
pub fn is_admin_signed(ix: &Instruction, admins: &[Pubkey]) -> bool {
    ix.accounts
        .iter()
        .any(|m| m.is_signer && admins.contains(&m.pubkey))
}

/// The instruction-level invariants (I4, I17, and the lender bounds of [`lender::check_ix`])
/// for one instruction: `pre` is the state just before it, `svm` the state just after it.
/// `admin_signed`: an admin key (upgrade authority, governance, Emergency, coverage
/// authority) signs this instruction ([`is_admin_signed`]).
pub fn check_ix(
    svm: &LiteSVM,
    pre: &TxSnapshot,
    admin_signed: bool,
    ev: &IxEvents,
) -> Result<(), Violation> {
    if admin_signed {
        i4_admin_moves_no_funds(svm, pre)?;
    }
    i17_no_open_on_flagged_venue(pre, &ev.opens)?;
    lender::check_ix(svm, &pre.lender, ev)
}

/// I4: no admin-signed instruction decreases `pool_vault`, `ShortPosition` or `MarketVault`
/// balances, except `sweep_protocol_fees` by ≤ `protocol_fees` to `config.fee_recipient`.
/// (No instruction can change a venue program id: they are compile-time constants, C0-8.)
///
/// TRACK (M7/O17): `coverage_authority` can change only `paused_coverage`.
pub fn i4_admin_moves_no_funds(svm: &LiteSVM, pre: &TxSnapshot) -> Result<(), Violation> {
    let recipient_gain = pre
        .fee_recipient
        .map(|(k, before)| read::lamports(svm, &k).saturating_sub(before))
        .unwrap_or(0);
    for s in &pre.markets {
        let Some(m) = read::zc::<Market>(svm, &s.market) else {
            continue;
        };
        let amount = read::token_amount(svm, &m.pool_vault).unwrap_or(0);
        ensure(amount >= s.pool_vault_amount, "I4", || {
            format!(
                "admin-signed tx decreased pool_vault of {} from {} to {amount}",
                s.market, s.pool_vault_amount
            )
        })?;
        let lamports = read::lamports(svm, &m.market_vault);
        let drop = s.market_vault_lamports.saturating_sub(lamports);
        ensure(
            drop == 0 || (drop <= s.protocol_fees && drop <= recipient_gain),
            "I4",
            || {
                format!(
                    "admin-signed tx took {drop} lamports from MarketVault of {} (protocol fees \
                     {}, recipient gained {recipient_gain})",
                    s.market, s.protocol_fees
                )
            },
        )?;
        for (key, before) in &s.positions {
            let after = read::lamports(svm, key);
            ensure(after >= *before || after == 0, "I4", || {
                format!("admin-signed tx decreased position {key} from {before} to {after}")
            })?;
        }
    }
    Ok(())
}

/// I17: no open is accepted while `venue_guard.upgraded` has a bit of the market's venue mask
/// set (A7; masks v0.4.1). Reads one instruction's `Open` events against the latch **just
/// before that instruction** (so `[flag_venue_upgraded, open_short]` in one transaction is
/// checked against the flagged latch, and `[revalidate_venue, open_short]` against the cleared
/// one; M1-FID-3); vacuous until `open_short` exists (M2).
pub fn i17_no_open_on_flagged_venue(pre: &TxSnapshot, opens: &[Open]) -> Result<(), Violation> {
    // M2: cross-check the market's post-state venue as well.
    let Some(guard) = pre.guard else {
        return Ok(());
    };
    for open in opens {
        let mask = VenueKind::from_u8(open.venue).map_or(0, VenueKind::guard_mask);
        ensure(!guard.blocks(mask), "I17", || {
            format!(
                "open {} accepted on venue {} while VenueGuard.upgraded = {:#b}",
                open.position, open.venue, guard.upgraded
            )
        })?;
    }
    Ok(())
}
