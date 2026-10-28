//! Quote-native rent and shared lender-ledger bookkeeping. No price or recovery authority.
//!
//! Public activation requires complete backend dispatch; these helpers never set a
//! market flag or enable admission. Whole-q token delivery and authorized exit
//! eligibility are the instruction's obligations. Only the rent math changes; pool
//! realization, receivable retirement, vesting and share epochs keep their existing rules.
use anchor_lang::prelude::*;
use shortwin_math::num::WAD;
use shortwin_math::pool::{self, Pool, Resolution, Vault};
use shortwin_math::quote_rent::{self, persistence};
use shortwin_math::rent;
use shortwin_math::waterfall::Payout;

use crate::engine::lender;
use crate::errors::{CheckedExt, MathResultExt, ShortwinError};
use crate::state::quote_finance::{
    QuoteFinanceExposure, QuoteFinanceMarket, QuoteFinancePosition, MARKET_FLAG_QUOTE_NATIVE,
};
use crate::state::Market;

/// A deliberately narrow new-backend start, not an old-state migration. Existing
/// lenders, receivables, fractional carry, historic share epochs, locks, vest and
/// unswept source obligations must first use a separately reviewed reconciliation.
/// Protocol fees can remain: their ownership is unchanged and they are not adopted.
pub fn assert_pristine_adoption(market: &Market, vault: &Vault) -> Result<()> {
    let pool = market.pool();
    require!(
        !market.has_flag(MARKET_FLAG_QUOTE_NATIVE)
            && market.open_positions == 0
            && market.total_notional_open == 0
            && market.borrowed_q_by_tier == [0; 4]
            && market.rent_basis.iter().all(|value| value.get() == 0)
            && market.rent_aggr.iter().all(|value| value.get() == 0)
            && market.lender_credit_rem.get() == 0
            && market.max_q_hwm == 0
            && market.min_open_fee_ppm == 0
            && market.min_open_hard_cap_bps == 0
            && market.deposit_hold_until == -1
            && pool.total_shares == 0
            && pool.tokens_free == 0
            && pool.borrowed_q == 0
            && pool.receivable == 0
            && pool.acc == 0
            && pool.acc_r == 0
            && pool.carry == 0
            && pool.carry_r == 0
            && pool.dust == 0
            && pool.max_release == 0
            && pool.vest_bal == 0
            && pool.share_epoch == 0
            && !pool.is_wipe_live()
            && pool.wipe_swept == (pool::WIPE_TRADER_SWEPT | pool::WIPE_REWARDS_CLAIMED)
            && vault.lender_sol == 0
            && vault.unmatched == 0
            && vault.pi == WAD
            && vault.lock_norm == 0
            && vault.b_min == 0
            && vault.pi_epoch == 0
            && vault.lock_cal.iter().all(|term| *term == 0),
        ShortwinError::AccountMismatch
    );
    Ok(())
}

/// Advance rent using real elapsed time and pre-action pool utilization. Clamp a
/// backward Solana clock to the last persisted accrual, as the common pool does.
/// No observation roll or off-chain snapshot is needed to accrue or repay debt.
#[allow(clippy::too_many_arguments)]
pub fn accrue(
    config: &Pubkey,
    market_key: &Pubkey,
    market: &mut Market,
    finance: &mut QuoteFinanceMarket,
    pool: &mut Pool,
    vault: &mut Vault,
    clock_now: i64,
) -> Result<i64> {
    let mut checkpoint = finance.restore(config, market_key, market)?;
    require!(*pool == market.pool(), ShortwinError::AccountMismatch);
    let now = clock_now.max(market.last_accrual_ts);
    let timestamp = u64::try_from(now).map_err(|_| error!(ShortwinError::InvalidParams))?;
    let utilization = rent::utilization_bps(pool.tokens_free, pool.borrowed_q).math()?;
    let kink = checkpoint
        .aggregate()
        .kink_bps()
        .unwrap_or(market.util_kink_bps);
    let credit = checkpoint
        .accrue_through(timestamp, utilization, kink)
        .math()?
        .lender_lamports;
    let mut bytes = [0; persistence::MARKET_ENCODED_LEN];
    persistence::encode_market(&checkpoint, &mut bytes)
        .map_err(|_| error!(ShortwinError::AccountMismatch))?;
    // Prepare legacy hold compatibility before the common pool mutation. It is not
    // a conversion of any legacy rent state or unallocated fraction.
    let mut next_pool = *pool;
    let mut deposit_hold = market.deposit_hold_until;
    if deposit_hold == 0 {
        next_pool.wipe_swept = 0;
        deposit_hold = now.max(next_pool.vest_end).max(1);
    }
    if deposit_hold < 0
        && next_pool.total_shares > 0
        && lender::rewards_required(market)
        && next_pool.wipe_swept & pool::WIPE_REWARDS_CLAIMED == 0
    {
        deposit_hold = now.max(next_pool.vest_end).max(1);
    }
    // pool::accrue is atomic on refusal, including the boxed vault's calendar.
    pool::accrue(&mut next_pool, vault, now, credit).math()?;
    *pool = next_pool;
    finance.rent_state = bytes;
    market.last_accrual_ts = now;
    market.deposit_hold_until = deposit_hold;
    market.set_pool(pool);
    Ok(now)
}

/// Book exactly one new real quote-native debt after pool::borrow succeeded in
/// the same instruction. This does not prove a qualified sale, margin, cap, release
/// horizon or recovery policy; no public opening path is supplied here.
pub fn register_position(
    config: &Pubkey,
    market_key: &Pubkey,
    finance_key: &Pubkey,
    market: &mut Market,
    finance: &mut QuoteFinanceMarket,
    position: &QuoteFinancePosition,
    pool_after_borrow: &Pool,
) -> Result<()> {
    let checkpoint = finance.restore(config, market_key, market)?;
    let rent_position = position.restore(finance_key, finance, &checkpoint)?;
    require!(
        position.t_open == market.last_accrual_ts
            && rent_position.opening_basis() == checkpoint.basis()
            && pool_after_borrow.max_release >= position.default_at,
        ShortwinError::AccountMismatch
    );
    require!(
        pool_after_borrow.borrowed_q == market.borrowed_q.checked_add(position.q).or_math()?
            && pool_after_borrow.tokens_free.checked_add(position.q) == Some(market.tokens_free),
        ShortwinError::AccountMismatch
    );
    let mut aggregate = checkpoint.aggregate();
    aggregate.add(&rent_position).math()?;
    let next = persistence::MarketCheckpoint::from_parts(
        checkpoint.basis(),
        aggregate,
        checkpoint.carry(),
        checkpoint.accrued_through(),
    )
    .math()?;
    let mut bytes = [0; persistence::MARKET_ENCODED_LEN];
    persistence::encode_market(&next, &mut bytes)
        .map_err(|_| error!(ShortwinError::AccountMismatch))?;
    let count = market.open_positions.checked_add(1).or_math()?;
    let notional = market
        .total_notional_open
        .checked_add(position.n_open)
        .or_math()?;
    let tier = usize::from(position.tier);
    let tier_quantity = market
        .borrowed_q_by_tier
        .get(tier)
        .ok_or(ShortwinError::AccountMismatch)?
        .checked_add(position.q)
        .or_math()?;
    market.open_positions = count;
    market.total_notional_open = notional;
    market.borrowed_q_by_tier[tier] = tier_quantity;
    market.max_q_hwm = market.max_q_hwm.max(position.q);
    market.set_pool(pool_after_borrow);
    finance.rent_state = bytes;
    Ok(())
}

// Copy the large lock calendar between referenced/heap objects without a 2 KiB
// value temporary in an SBF frame. Common pool operations remain unchanged.
fn copy_vault(target: &mut Vault, source: &Vault) {
    target.lock_cal.copy_from_slice(&source.lock_cal);
    target.pi = source.pi;
    target.lock_norm = source.lock_norm;
    target.lender_sol = source.lender_sol;
    target.unmatched = source.unmatched;
    target.protocol_fees = source.protocol_fees;
    target.b_min = source.b_min;
    target.cal_day = source.cal_day;
    target.pi_epoch = source.pi_epoch;
    target._pad = source._pad;
}

/// Retire rent and actual debt after a whole-q token repayment. The instruction
/// must first verify the new net q vault credit and all payout transfers atomically.
/// This function grants no liquidation/default permission and pays no cash default.
#[allow(clippy::too_many_arguments)]
pub fn book_token_repayment(
    config: &Pubkey,
    market_key: &Pubkey,
    finance_key: &Pubkey,
    market: &mut Market,
    finance: &mut QuoteFinanceMarket,
    pool: &mut Pool,
    vault: &mut Vault,
    now: i64,
    position: &QuoteFinancePosition,
    payout: &Payout,
) -> Result<u64> {
    let checkpoint = finance.restore(config, market_key, market)?;
    require!(
        *pool == market.pool() && now == market.last_accrual_ts,
        ShortwinError::AccountMismatch
    );
    let rent_position = position.restore(finance_key, finance, &checkpoint)?;
    let adjustment =
        quote_rent::true_up(&rent_position, &checkpoint.basis(), payout.rent).math()?;
    require!(
        payout.total().math()? == position.v
            && payout.rent_protocol == adjustment.protocol_rent
            && payout.rent_lenders == adjustment.lender_paid
            && payout.lenders == payout.rent_lenders.checked_add(payout.lump).or_math()?
            && payout.protocol >= payout.rent_protocol,
        ShortwinError::AccountMismatch
    );
    let mut aggregate = checkpoint.aggregate();
    aggregate.remove(&rent_position).math()?;
    let next = persistence::MarketCheckpoint::from_parts(
        checkpoint.basis(),
        aggregate,
        checkpoint.carry(),
        checkpoint.accrued_through(),
    )
    .math()?;
    let mut bytes = [0; persistence::MARKET_ENCODED_LEN];
    persistence::encode_market(&next, &mut bytes)
        .map_err(|_| error!(ShortwinError::AccountMismatch))?;
    let count = market.open_positions.checked_sub(1).or_math()?;
    let notional = market
        .total_notional_open
        .checked_sub(position.n_open)
        .or_math()?;
    let tier = usize::from(position.tier);
    let tier_quantity = market
        .borrowed_q_by_tier
        .get(tier)
        .ok_or(ShortwinError::AccountMismatch)?
        .checked_sub(position.q)
        .or_math()?;
    let mut next_pool = *pool;
    let mut next_vault = Box::new(Vault::new());
    copy_vault(&mut next_vault, vault);
    let lender_rent = pool::repay(
        &mut next_pool,
        &mut next_vault,
        now,
        position.lump_vest_seconds,
        &Resolution {
            q: position.q,
            credited: adjustment.credited_for,
            lump: payout.lump,
        },
        payout.rent,
        rent_position.protocol_bps(),
    )
    .math()?;
    require_eq!(
        lender_rent,
        adjustment.lender_paid,
        ShortwinError::AccountMismatch
    );
    // pool::repay may itself credit protocol-owned rent when there is no
    // eligible lender. Add the payout cut to its resulting balance; never
    // replace it with a pre-operation snapshot.
    next_vault.protocol_fees = next_vault
        .protocol_fees
        .checked_add(payout.protocol)
        .or_math()?;
    *pool = next_pool;
    copy_vault(vault, &next_vault);
    market.open_positions = count;
    market.total_notional_open = notional;
    market.borrowed_q_by_tier[tier] = tier_quantity;
    if count == 0 {
        market.max_q_hwm = 0;
        market.min_open_fee_ppm = 0;
        market.min_open_hard_cap_bps = 0;
    }
    market.set_pool(pool);
    finance.rent_state = bytes;
    Ok(lender_rent)
}

/// Explicit collateral foreclosure after the owner-selected locked grace. Token
/// claims disappear; this must never be emitted/indexed as a token repayment.
#[derive(Clone, Debug)]
pub struct OverdueDefaultBooking {
    pub keeper_payment: u64,
    pub lender_amount: u64,
    pub rent_due: u64,
    pub credited_retired: u64,
    pub vested: u64,
    pub vest_flushed: u64,
    pub record: Option<pool::ShareEpoch>,
}

/// Book collateral-only default with the common loss/share-epoch machinery. The
/// actual runtime clock, not the accrual clamp or a reporter assertion, authorizes
/// this terminal transition. All fields are preflighted before commit. Position
/// closure and actual SOL transfers must occur in the same atomic instruction.
#[allow(clippy::too_many_arguments)]
pub fn book_overdue_default(
    config: &Pubkey,
    market_key: &Pubkey,
    finance_key: &Pubkey,
    market: &mut Market,
    finance: &mut QuoteFinanceMarket,
    pool: &mut Pool,
    vault: &mut Vault,
    actual_clock_now: i64,
    position: &QuoteFinancePosition,
) -> Result<OverdueDefaultBooking> {
    require_gte!(
        actual_clock_now,
        position.default_at,
        ShortwinError::OverdueDefaultNotReady
    );
    let checkpoint = finance.restore(config, market_key, market)?;
    require!(
        *pool == market.pool() && pool.total_shares > 0,
        ShortwinError::AccountMismatch
    );
    let rent_position = position.restore(finance_key, finance, &checkpoint)?;
    let now = market.last_accrual_ts;
    require!(now >= actual_clock_now, ShortwinError::AccountMismatch);
    let rent_due = quote_rent::due(&rent_position, &checkpoint.basis()).math()?;
    let credited = quote_rent::credited_for(&rent_position, &checkpoint.basis()).math()?;
    let credited_retired = credited.min(pool.receivable);
    let keeper_payment =
        shortwin_math::risk::k_s(position.v, &position.default_keeper_terms()).math()?;
    let lender_amount = position.v.checked_sub(keeper_payment).or_math()?;
    let mut aggregate = checkpoint.aggregate();
    aggregate.remove(&rent_position).math()?;
    let next = persistence::MarketCheckpoint::from_parts(
        checkpoint.basis(),
        aggregate,
        checkpoint.carry(),
        checkpoint.accrued_through(),
    )
    .math()?;
    let mut bytes = [0; persistence::MARKET_ENCODED_LEN];
    persistence::encode_market(&next, &mut bytes)
        .map_err(|_| error!(ShortwinError::AccountMismatch))?;
    let count = market.open_positions.checked_sub(1).or_math()?;
    let notional = market
        .total_notional_open
        .checked_sub(position.n_open)
        .or_math()?;
    let tier = usize::from(position.tier);
    let tier_quantity = market
        .borrowed_q_by_tier
        .get(tier)
        .ok_or(ShortwinError::AccountMismatch)?
        .checked_sub(position.q)
        .or_math()?;
    let mut next_pool = *pool;
    let mut next_vault = Box::new(Vault::new());
    copy_vault(&mut next_vault, vault);
    let vest_before = next_pool.vest_bal;
    // Every SOL after the keeper payment compensates lenders. There is no price
    // estimate, buyback cost or discretionary protocol penalty. The common pool
    // retires receivables, realizes actual compensation, vests only any rent top-up,
    // debits the q token claim and emits a share-epoch record when required.
    let settled = pool::settle(
        &mut next_pool,
        &mut next_vault,
        now,
        position.lump_vest_seconds,
        &Resolution {
            q: position.q,
            credited,
            lump: 0,
        },
        lender_amount,
        rent_due,
    )
    .math()?;
    let vest_flushed = if settled.flushed {
        vest_before.checked_add(settled.vested).or_math()?
    } else {
        0
    };
    require!(
        next_pool.tokens_free == pool.tokens_free
            && next_pool.borrowed_q == pool.borrowed_q.checked_sub(position.q).or_math()?,
        ShortwinError::AccountMismatch
    );
    *pool = next_pool;
    copy_vault(vault, &next_vault);
    market.open_positions = count;
    market.total_notional_open = notional;
    market.borrowed_q_by_tier[tier] = tier_quantity;
    if count == 0 {
        market.max_q_hwm = 0;
        market.min_open_fee_ppm = 0;
        market.min_open_hard_cap_bps = 0;
    }
    if settled.hold {
        market.deposit_hold_until = market.deposit_hold_until.max(pool.vest_end);
    }
    market.set_pool(pool);
    finance.rent_state = bytes;
    Ok(OverdueDefaultBooking {
        keeper_payment,
        lender_amount,
        rent_due,
        credited_retired,
        vested: settled.vested,
        vest_flushed,
        record: settled.rec,
    })
}

pub fn add_exposure(exposure: &mut QuoteFinanceExposure, q: u64, notional: u64) -> Result<()> {
    require!(q > 0 && notional > 0, ShortwinError::InvalidParams);
    let quantity = exposure.q_open.checked_add(q).or_math()?;
    let booked = exposure.notional_open.checked_add(notional).or_math()?;
    exposure.q_open = quantity;
    exposure.notional_open = booked;
    Ok(())
}
pub fn remove_exposure(exposure: &mut QuoteFinanceExposure, q: u64, notional: u64) -> Result<()> {
    require!(q > 0 && notional > 0, ShortwinError::InvalidParams);
    let quantity = exposure.q_open.checked_sub(q).or_math()?;
    let booked = exposure.notional_open.checked_sub(notional).or_math()?;
    require!(
        (quantity == 0) == (booked == 0),
        ShortwinError::AccountMismatch
    );
    exposure.q_open = quantity;
    exposure.notional_open = booked;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::arithmetic_side_effects)]
mod tests {
    use super::*;
    use crate::state::quote_finance::QUOTE_FINANCE_VERSION;
    use bytemuck::Zeroable;
    use shortwin_math::pool::Lender;
    use shortwin_math::quote_rent::{LockedRates, NotionalPosition, OpeningNotional};

    fn blank_market() -> Box<Market> {
        let mut market = Box::new(Market::zeroed());
        market.set_pool(&Pool::new());
        market.deposit_hold_until = -1;
        market.util_kink_bps = 8000;
        market.last_accrual_ts = 100;
        market
    }
    fn setup() -> (
        Box<Market>,
        QuoteFinanceMarket,
        Box<Vault>,
        Pubkey,
        Pubkey,
        Pubkey,
    ) {
        let mut market = blank_market();
        market.set_flag(MARKET_FLAG_QUOTE_NATIVE, true);
        let mut pool = market.pool();
        let mut vault = Box::new(Vault::new());
        let mut lender = Lender::new(&pool);
        pool::deposit(
            &mut pool,
            &mut vault,
            &mut lender,
            &mut [],
            100,
            10_000_000,
            false,
        )
        .unwrap();
        market.set_pool(&pool);
        let (config, key, finance_key) = (
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
        );
        let mut finance = QuoteFinanceMarket {
            version: QUOTE_FINANCE_VERSION,
            bump: 254,
            market: key,
            config,
            model_version: 2,
            model_epoch: 1,
            policy_hash: [7; 32],
            rent_state: [0; persistence::MARKET_ENCODED_LEN],
        };
        finance
            .store(&persistence::MarketCheckpoint::empty(100))
            .unwrap();
        (market, finance, vault, config, key, finance_key)
    }
    fn position(
        finance_key: Pubkey,
        finance: &QuoteFinanceMarket,
        now: i64,
        minimum_seconds: u64,
    ) -> QuoteFinancePosition {
        let checkpoint = persistence::decode_market(&finance.rent_state).unwrap();
        let rent = NotionalPosition::lock(
            OpeningNotional::new(1_000_000_007).unwrap(),
            &checkpoint.basis(),
            LockedRates::new(10000, 37500, 550000),
            1500,
            1000,
            8000,
            minimum_seconds,
        )
        .unwrap();
        let mut pos = QuoteFinancePosition {
            version: QUOTE_FINANCE_VERSION,
            borrower: Pubkey::new_unique(),
            market: finance.market,
            finance_market: finance_key,
            nonce: 3,
            bump: 253,
            tier: 2,
            q: 1_000_000,
            s: 1_000_000_000,
            m: 3_000_000_000,
            v: 4_000_000_000,
            n_open: 1_000_000_007,
            fee_open: 0,
            t_open: now,
            deadline: now + 2000,
            default_at: now + 2000 + crate::state::QUOTE_DEFAULT_GRACE_SECONDS,
            settle_keeper_fixed: 10_000_000,
            settle_keeper_bps: 100,
            settle_keeper_cap: 250_000_000,
            lump_vest_seconds: 86_400,
            model_version: finance.model_version,
            model_epoch: finance.model_epoch,
            policy_hash: finance.policy_hash,
            rent_state: [0; persistence::POSITION_ENCODED_LEN],
        };
        pos.store_rent(&rent).unwrap();
        pos
    }
    fn open(
        market: &mut Market,
        finance: &mut QuoteFinanceMarket,
        config: &Pubkey,
        key: &Pubkey,
        finance_key: &Pubkey,
        pos: &QuoteFinancePosition,
    ) {
        let mut borrowed = market.pool();
        pool::borrow(&mut borrowed, pos.t_open, pos.q, pos.default_at).unwrap();
        register_position(config, key, finance_key, market, finance, pos, &borrowed).unwrap();
    }
    #[test]
    fn pristine_adoption_refuses_every_legacy_fraction_and_epoch_obligation() {
        let vault = Vault::new();
        assert!(assert_pristine_adoption(&blank_market(), &vault).is_ok());
        for mutate in [
            |m: &mut Market| m.lender_credit_rem.set(1),
            |m: &mut Market| m.rent_basis[0].set(1),
            |m: &mut Market| m.rent_aggr[1].set(1),
            |m: &mut Market| m.carry.set(1),
            |m: &mut Market| m.carry_r.set(1),
            |m: &mut Market| m.dust.set(1),
            |m: &mut Market| m.receivable = 1,
            |m: &mut Market| m.share_epoch = 1,
            |m: &mut Market| m.total_shares.set(1),
            |m: &mut Market| m.vest_bal = 1,
            |m: &mut Market| m.deposit_hold_until = 0,
            |m: &mut Market| m.set_flag(MARKET_FLAG_QUOTE_NATIVE, true),
            |m: &mut Market| m.set_flag(crate::state::MARKET_FLAG_WIPE_TRADER_SWEPT, false),
        ] {
            let mut market = blank_market();
            mutate(&mut market);
            assert!(assert_pristine_adoption(&market, &vault).is_err());
        }
        for mutate in [
            |v: &mut Vault| v.lender_sol = 1,
            |v: &mut Vault| v.unmatched = 1,
            |v: &mut Vault| v.lock_norm = 1,
            |v: &mut Vault| v.lock_cal[0] = 1,
            |v: &mut Vault| v.pi_epoch = 1,
        ] {
            let mut modified = vault;
            mutate(&mut modified);
            assert!(assert_pristine_adoption(&blank_market(), &modified).is_err());
        }
        let mut protocol_owned = vault;
        protocol_owned.protocol_fees = 123;
        assert!(assert_pristine_adoption(&blank_market(), &protocol_owned).is_ok());
    }
    #[test]
    fn accrue_and_full_repayment_use_actual_common_pool_without_any_price() {
        let (mut market, mut finance, mut vault, config, key, finance_key) = setup();
        assert_eq!(market.ema_price.get(), 0);
        assert_eq!(market.ring_count, 0);
        let pos = position(finance_key, &finance, 100, 3600);
        open(&mut market, &mut finance, &config, &key, &finance_key, &pos);
        assert_eq!(
            (market.open_positions, market.total_notional_open),
            (1, pos.n_open)
        );
        let mut pool = market.pool();
        let now = accrue(
            &config,
            &key,
            &mut market,
            &mut finance,
            &mut pool,
            &mut vault,
            101,
        )
        .unwrap();
        assert_eq!(now, 101);
        assert!(pool.receivable > 0);
        let checkpoint = finance.restore(&config, &key, &market).unwrap();
        let rent = pos.restore(&finance_key, &finance, &checkpoint).unwrap();
        let due = quote_rent::due(&rent, &checkpoint.basis()).unwrap();
        let credited = quote_rent::credited_for(&rent, &checkpoint.basis()).unwrap();
        let carry = checkpoint.carry();
        assert_ne!(carry.numerator(), 0);
        let payout = shortwin_math::waterfall::close(pos.v, 0, due, rent.protocol_bps()).unwrap();
        let realized = book_token_repayment(
            &config,
            &key,
            &finance_key,
            &mut market,
            &mut finance,
            &mut pool,
            &mut vault,
            now,
            &pos,
            &payout,
        )
        .unwrap();
        assert_eq!(realized, payout.rent_lenders);
        assert_eq!(pool.tokens_free, 10_000_000);
        assert_eq!(pool.borrowed_q, 0);
        assert_eq!(pool.receivable, 0);
        assert_eq!(pool.vest_bal, payout.rent_lenders.saturating_sub(credited));
        assert_eq!(vault.lender_sol, payout.rent_lenders);
        assert_eq!(vault.protocol_fees, payout.protocol);
        assert_eq!(market.borrowed_q_by_tier, [0; 4]);
        assert_eq!((market.open_positions, market.total_notional_open), (0, 0));
        let checkpoint = finance.restore(&config, &key, &market).unwrap();
        assert_eq!(checkpoint.aggregate().positions(), 0);
        assert_eq!(checkpoint.carry(), carry);
        let saved = checkpoint.carry();
        accrue(
            &config,
            &key,
            &mut market,
            &mut finance,
            &mut pool,
            &mut vault,
            200,
        )
        .unwrap();
        assert_eq!(
            finance.restore(&config, &key, &market).unwrap().carry(),
            saved
        );
    }
    #[test]
    fn clock_step_back_is_clamped_and_wrong_pool_or_binding_is_refused() {
        let (mut market, mut finance, mut vault, config, key, finance_key) = setup();
        let pos = position(finance_key, &finance, 100, 0);
        open(&mut market, &mut finance, &config, &key, &finance_key, &pos);
        let mut pool = market.pool();
        let bytes = finance.rent_state;
        assert_eq!(
            accrue(
                &config,
                &key,
                &mut market,
                &mut finance,
                &mut pool,
                &mut vault,
                99
            )
            .unwrap(),
            100
        );
        assert_eq!(finance.rent_state, bytes);
        let mut wrong = pool;
        wrong.tokens_free += 1;
        let saved_vault = *vault;
        assert!(accrue(
            &config,
            &key,
            &mut market,
            &mut finance,
            &mut wrong,
            &mut vault,
            101
        )
        .is_err());
        assert_eq!(*vault, saved_vault);
        assert_eq!(finance.rent_state, bytes);
        finance.model_epoch += 1;
        let checkpoint = finance.restore(&config, &key, &market).unwrap();
        assert!(pos.restore(&finance_key, &finance, &checkpoint).is_err());
        finance.model_epoch -= 1;
        market.total_notional_open += 1;
        assert!(finance.restore(&config, &key, &market).is_err());
    }
    #[test]
    fn early_minimum_rent_and_underpaid_exit_true_up_remain_distinct() {
        let (mut market, mut finance, mut vault, config, key, finance_key) = setup();
        let pos = position(finance_key, &finance, 100, 0);
        open(&mut market, &mut finance, &config, &key, &finance_key, &pos);
        let mut pool = market.pool();
        let now = accrue(
            &config,
            &key,
            &mut market,
            &mut finance,
            &mut pool,
            &mut vault,
            1000,
        )
        .unwrap();
        let checkpoint = finance.restore(&config, &key, &market).unwrap();
        let rent = pos.restore(&finance_key, &finance, &checkpoint).unwrap();
        let due = quote_rent::due(&rent, &checkpoint.basis()).unwrap();
        let adjustment = quote_rent::true_up(&rent, &checkpoint.basis(), due / 2).unwrap();
        // The caller already authorized a full-q exit; underpayment of rent is a
        // ledger shortfall only, not permission to extinguish debt without tokens.
        let payout = Payout {
            rent: due / 2,
            rent_protocol: adjustment.protocol_rent,
            rent_lenders: adjustment.lender_paid,
            protocol: adjustment.protocol_rent,
            lenders: adjustment.lender_paid,
            borrower: pos.v - due / 2,
            ..Payout::default()
        };
        let before_acc = pool.acc;
        book_token_repayment(
            &config,
            &key,
            &finance_key,
            &mut market,
            &mut finance,
            &mut pool,
            &mut vault,
            now,
            &pos,
            &payout,
        )
        .unwrap();
        assert!(pool.acc < before_acc);
        assert_eq!(pool.receivable, 0);
        assert_eq!(pool.vest_bal, 0);
        assert_eq!(pool.borrowed_q, 0);
    }
    #[test]
    fn protocol_owned_rent_from_common_pool_is_not_overwritten() {
        let (mut market, mut finance, mut vault, config, key, finance_key) = setup();
        let pos = position(finance_key, &finance, 100, 3600);
        open(&mut market, &mut finance, &config, &key, &finance_key, &pos);
        // Controlled common-pool boundary: no live share owner. This does not
        // authorize such a market transition or bypass its separate ledger rules.
        // pool::repay must retain its explicit no-lender distribution unchanged.
        let mut pool = market.pool();
        pool.total_shares = 0;
        market.set_pool(&pool);
        let checkpoint = finance.restore(&config, &key, &market).unwrap();
        let rent = pos.restore(&finance_key, &finance, &checkpoint).unwrap();
        let due = quote_rent::due(&rent, &checkpoint.basis()).unwrap();
        assert!(due > 0);
        let payout = shortwin_math::waterfall::close(pos.v, 0, due, rent.protocol_bps()).unwrap();
        book_token_repayment(
            &config,
            &key,
            &finance_key,
            &mut market,
            &mut finance,
            &mut pool,
            &mut vault,
            100,
            &pos,
            &payout,
        )
        .unwrap();
        assert_eq!(vault.lender_sol, 0);
        assert_eq!(vault.protocol_fees, payout.rent);
    }
    #[test]
    fn late_protocol_fee_overflow_preserves_pool_vault_and_debt_checkpoint() {
        let (mut market, mut finance, mut vault, config, key, finance_key) = setup();
        let pos = position(finance_key, &finance, 100, 3600);
        open(&mut market, &mut finance, &config, &key, &finance_key, &pos);
        let checkpoint = finance.restore(&config, &key, &market).unwrap();
        let rent = pos.restore(&finance_key, &finance, &checkpoint).unwrap();
        let due = quote_rent::due(&rent, &checkpoint.basis()).unwrap();
        let payout = shortwin_math::waterfall::close(pos.v, 0, due, rent.protocol_bps()).unwrap();
        vault.protocol_fees = u64::MAX;
        let mut pool = market.pool();
        let saved = (pool, *vault, finance.rent_state);
        assert!(book_token_repayment(
            &config,
            &key,
            &finance_key,
            &mut market,
            &mut finance,
            &mut pool,
            &mut vault,
            100,
            &pos,
            &payout,
        )
        .is_err());
        assert_eq!((pool, *vault, finance.rent_state), saved);
        assert_eq!(
            (
                market.open_positions,
                market.borrowed_q,
                market.total_notional_open
            ),
            (1, pos.q, pos.n_open)
        );
    }
    #[test]
    fn overdue_default_requires_actual_clock_and_distributes_all_collateral_without_price() {
        let (mut market, mut finance, mut vault, config, key, finance_key) = setup();
        let pos = position(finance_key, &finance, 100, 3600);
        open(&mut market, &mut finance, &config, &key, &finance_key, &pos);
        let mut pool = market.pool();
        accrue(
            &config,
            &key,
            &mut market,
            &mut finance,
            &mut pool,
            &mut vault,
            pos.default_at,
        )
        .unwrap();
        let saved = (pool, *vault, finance.rent_state);
        assert!(book_overdue_default(
            &config,
            &key,
            &finance_key,
            &mut market,
            &mut finance,
            &mut pool,
            &mut vault,
            pos.default_at - 1,
            &pos,
        )
        .is_err());
        assert_eq!((pool, *vault, finance.rent_state), saved);
        let carry = finance.restore(&config, &key, &market).unwrap().carry();
        let booked = book_overdue_default(
            &config,
            &key,
            &finance_key,
            &mut market,
            &mut finance,
            &mut pool,
            &mut vault,
            pos.default_at,
            &pos,
        )
        .unwrap();
        assert_eq!(booked.keeper_payment, 50_000_000);
        assert_eq!(booked.keeper_payment + booked.lender_amount, pos.v);
        assert_eq!(pool.tokens_free, 9_000_000); // Lost q is NOT returned.
        assert_eq!(pool.borrowed_q, 0);
        assert_eq!(pool.receivable, 0);
        assert_eq!(vault.protocol_fees, 0); // No default protocol penalty/rent cut.
        assert_eq!(vault.lender_sol, booked.lender_amount);
        assert_eq!(market.open_positions, 0);
        assert_eq!(market.total_notional_open, 0);
        assert_eq!(
            finance.restore(&config, &key, &market).unwrap().carry(),
            carry
        );
    }
    #[test]
    fn overdue_full_wipe_retains_old_lender_record_and_fractional_rent_carry() {
        let (mut market, mut finance, mut vault, config, key, finance_key) = setup();
        let mut pos = position(finance_key, &finance, 100, 0);
        pos.q = market.tokens_free;
        open(&mut market, &mut finance, &config, &key, &finance_key, &pos);
        let mut pool = market.pool();
        let shares = pool.total_shares;
        accrue(
            &config,
            &key,
            &mut market,
            &mut finance,
            &mut pool,
            &mut vault,
            pos.default_at,
        )
        .unwrap();
        let carry = finance.restore(&config, &key, &market).unwrap().carry();
        let booked = book_overdue_default(
            &config,
            &key,
            &finance_key,
            &mut market,
            &mut finance,
            &mut pool,
            &mut vault,
            pos.default_at,
            &pos,
        )
        .unwrap();
        let record = booked.record.unwrap();
        assert!(record.is_full_wipe());
        assert_eq!(record.s_old, shares);
        assert_eq!(pool.share_epoch, 1);
        assert_eq!(pool.total_shares, 0);
        assert!(pool.is_wipe_live());
        assert_eq!(
            (pool.tokens_free, pool.borrowed_q, pool.receivable),
            (0, 0, 0)
        );
        assert_eq!(vault.lender_sol, booked.lender_amount);
        assert_eq!(
            finance.restore(&config, &key, &market).unwrap().carry(),
            carry
        );
    }
    #[test]
    fn recovery_terms_are_locked_checked_and_no_lender_foreclosure_fails_closed() {
        let (mut market, mut finance, mut vault, config, key, finance_key) = setup();
        let mut pos = position(finance_key, &finance, 100, 0);
        pos.lock_recovery(shortwin_math::risk::SettleKeeperTerms {
            fixed: 1,
            bps: 2,
            cap: 3,
        })
        .unwrap();
        assert_eq!(
            pos.default_at,
            pos.deadline + crate::state::QUOTE_DEFAULT_GRACE_SECONDS
        );
        let saved = pos.clone();
        assert!(pos
            .lock_recovery(shortwin_math::risk::SettleKeeperTerms {
                fixed: 1,
                bps: 10001,
                cap: 3
            })
            .is_err());
        assert_eq!(pos, saved);
        let mut overflow = saved.clone();
        overflow.deadline = i64::MAX;
        assert!(overflow
            .lock_recovery(saved.default_keeper_terms())
            .is_err());
        open(&mut market, &mut finance, &config, &key, &finance_key, &pos);
        let mut pool = market.pool();
        accrue(
            &config,
            &key,
            &mut market,
            &mut finance,
            &mut pool,
            &mut vault,
            pos.default_at,
        )
        .unwrap();
        pool.total_shares = 0;
        market.set_pool(&pool);
        let saved = (pool, *vault, finance.rent_state);
        assert!(book_overdue_default(
            &config,
            &key,
            &finance_key,
            &mut market,
            &mut finance,
            &mut pool,
            &mut vault,
            pos.default_at,
            &pos,
        )
        .is_err());
        assert_eq!((pool, *vault, finance.rent_state), saved);
    }
    #[test]
    fn changed_global_vesting_cannot_rewrite_a_positions_opening_policy() {
        let (mut market, mut finance, mut vault, config, key, finance_key) = setup();
        let pos = position(finance_key, &finance, 100, 3600);
        let opening_policy_vesting = 86_400;
        let mut changed_global = crate::state::GlobalConfig::zeroed();
        changed_global.params.lump_vest_s = 3600;
        assert_ne!(pos.lump_vest_seconds, changed_global.params.lump_vest_s);
        open(&mut market, &mut finance, &config, &key, &finance_key, &pos);
        let checkpoint = finance.restore(&config, &key, &market).unwrap();
        let rent = pos.restore(&finance_key, &finance, &checkpoint).unwrap();
        let due = quote_rent::due(&rent, &checkpoint.basis()).unwrap();
        let payout = shortwin_math::waterfall::close(pos.v, 0, due, rent.protocol_bps()).unwrap();
        let mut pool = market.pool();
        book_token_repayment(
            &config,
            &key,
            &finance_key,
            &mut market,
            &mut finance,
            &mut pool,
            &mut vault,
            100,
            &pos,
            &payout,
        )
        .unwrap();
        assert!(pool.vest_bal > 0);
        assert_eq!(
            pool.vest_end - pool.vest_last,
            i64::from(opening_policy_vesting)
        );
    }
    #[test]
    fn booked_exposure_is_atomic_and_cannot_leave_one_sided_zero() {
        let mut exposure = QuoteFinanceExposure {
            market: Pubkey::new_unique(),
            wallet: Pubkey::new_unique(),
            q_open: 0,
            notional_open: 0,
            bump: 1,
        };
        add_exposure(&mut exposure, 5, 7).unwrap();
        let saved = exposure.clone();
        assert!(remove_exposure(&mut exposure, 5, 6).is_err());
        assert_eq!(exposure, saved);
        assert!(remove_exposure(&mut exposure, 6, 7).is_err());
        assert_eq!(exposure, saved);
        remove_exposure(&mut exposure, 5, 7).unwrap();
        assert_eq!((exposure.q_open, exposure.notional_open), (0, 0));
    }
}
