//! Actual paid value only: fixed opening fee, whole debt exits, no rent receivable.
use crate::{
    errors::{CheckedExt, SimpleError},
    state::FinancialPolicy,
};
use anchor_lang::prelude::*;

pub fn ceil_bps(value: u64, bps: u32) -> Result<u64> {
    let n = u128::from(value).checked_mul(u128::from(bps)).checked()?;
    u64::try_from(n.checked_add(9_999).checked()? / 10_000)
        .map_err(|_| error!(SimpleError::Arithmetic))
}
pub fn floor_bps(value: u64, bps: u32) -> Result<u64> {
    u64::try_from(u128::from(value) * u128::from(bps) / 10_000)
        .map_err(|_| error!(SimpleError::Arithmetic))
}
#[derive(Debug, PartialEq, Eq)]
pub struct Opening {
    pub notional: u64,
    pub margin: u64,
    pub collateral: u64,
    pub fee: u64,
    pub lender_fee: u64,
    pub protocol_fee: u64,
}
pub fn opening(sale: u64, live_ask: u64, history_ask: u64, p: &FinancialPolicy) -> Result<Opening> {
    require!(
        sale > 0 && live_ask > 0 && history_ask > 0,
        SimpleError::BadReference
    );
    let notional = sale.max(live_ask).max(history_ask);
    let required = ceil_bps(notional, p.cr_open_bps)?;
    let margin = required.checked_sub(sale).checked()?;
    let collateral = sale.checked_add(margin).checked()?;
    let fee = ceil_bps(notional, u32::from(p.term_fee_bps))?;
    let protocol_fee = floor_bps(fee, u32::from(p.protocol_share_bps))?;
    Ok(Opening {
        notional,
        margin,
        collateral,
        fee,
        lender_fee: fee.checked_sub(protocol_fee).checked()?,
        protocol_fee,
    })
}
#[allow(clippy::too_many_arguments)]
pub fn check_caps(
    q: u64,
    cash: u64,
    principal: u64,
    booked: u64,
    wallet: u64,
    notional: u64,
    p: &FinancialPolicy,
) -> Result<()> {
    require!(q > 0, SimpleError::ZeroAmount);
    require_gte!(cash, q, SimpleError::InsufficientLiquidity);
    require_gte!(
        notional,
        p.min_notional_lamports,
        SimpleError::BelowMinNotional
    );
    require_gte!(
        p.position_notional_cap_lamports,
        notional,
        SimpleError::PositionLimit
    );
    require_gte!(
        p.market_notional_cap_lamports,
        booked.checked_add(notional).checked()?,
        SimpleError::MarketLimit
    );
    let wallet_limit = floor_bps(
        p.market_notional_cap_lamports,
        u32::from(p.wallet_notional_cap_bps),
    )?;
    require_gte!(
        wallet_limit,
        wallet.checked_add(notional).checked()?,
        SimpleError::WalletLimit
    );
    let debt = principal.checked_add(q).checked()?;
    let total = cash.checked_add(principal).checked()?;
    require!(
        u128::from(debt) * 10_000 <= u128::from(total) * u128::from(p.utilization_cap_bps),
        SimpleError::UtilizationLimit
    );
    Ok(())
}
pub fn unhealthy(collateral: u64, bid: u64, cr: u32) -> Result<bool> {
    require!(bid > 0, SimpleError::BadReference);
    Ok(u128::from(collateral) * 10_000 < u128::from(bid) * u128::from(cr))
}
pub fn keeper_reward(basis: u64, available: u64, p: &FinancialPolicy) -> Result<u64> {
    // Saturation here only implements the stated fee cap, never a ledger balance.
    let variable = floor_bps(basis, u32::from(p.keeper_bps))?;
    Ok(p.keeper_fixed_lamports
        .saturating_add(variable)
        .min(p.keeper_cap_lamports)
        .min(available))
}
pub fn buyback_cap(bref: u64, collateral: u64, p: &FinancialPolicy) -> Result<u64> {
    require!(bref > 0, SimpleError::BadReference);
    let price_limit = floor_bps(bref, 10_000 + u32::from(p.buyback_premium_bps))?;
    // Reserve the full configured fee cap; an unaffordable route waits for a full
    // repayment or for the deterministic grace-period default.
    Ok(price_limit.min(collateral.saturating_sub(p.keeper_cap_lamports)))
}
pub fn move_sol(from: &AccountInfo<'_>, to: &AccountInfo<'_>, amount: u64) -> Result<()> {
    require!(
        from.owner == &crate::ID && from.is_writable && to.is_writable && from.key != to.key,
        SimpleError::AccountMismatch
    );
    let debit = from.lamports().checked_sub(amount).checked()?;
    let credit = to.lamports().checked_add(amount).checked()?;
    **from.try_borrow_mut_lamports()? = debit;
    **to.try_borrow_mut_lamports()? = credit;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p() -> FinancialPolicy {
        FinancialPolicy {
            term_seconds: 3600,
            default_grace_seconds: 86400,
            term_fee_bps: 100,
            protocol_share_bps: 2000,
            cr_open_bps: 15000,
            cr_liq_bps: 12000,
            min_notional_lamports: 10,
            position_notional_cap_lamports: 1000,
            market_notional_cap_lamports: 10000,
            utilization_cap_bps: 8000,
            wallet_notional_cap_bps: 5000,
            keeper_fixed_lamports: 2,
            keeper_bps: 100,
            keeper_cap_lamports: 10,
            buyback_premium_bps: 500,
            reference_max_age_seconds: 30,
            reference_max_age_slots: 100,
            history_epoch_seconds: 60,
            minimum_history_epochs: 3,
            max_spread_bps: 1000,
            cr_confirmation_delay_seconds: 5,
            cr_confirmation_delay_slots: 5,
            price_policy_hash: [1; 32],
        }
    }
    #[test]
    fn fixed_paid_fee_and_conservative_margin() {
        let o = opening(100, 120, 110, &p()).unwrap();
        assert_eq!(
            o,
            Opening {
                notional: 120,
                margin: 80,
                collateral: 180,
                fee: 2,
                lender_fee: 2,
                protocol_fee: 0
            }
        );
        // Higher sale proceeds increase collateral requirements and do not create a free borrow.
        assert_eq!(opening(200, 120, 110, &p()).unwrap().margin, 100);
    }
    #[test]
    fn checked_bounds_and_exact_cr_boundary() {
        assert!(!unhealthy(120, 100, 12000).unwrap());
        assert!(unhealthy(119, 100, 12000).unwrap());
        assert!(opening(u64::MAX, u64::MAX, 1, &p()).is_err());
        assert!(check_caps(81, 100, 0, 0, 0, 100, &p()).is_err());
        assert!(check_caps(80, 100, 0, 0, 0, 100, &p()).is_ok());
        assert!(check_caps(1, 100, 0, 0, 4990, 100, &p()).is_err());
    }
    #[test]
    fn reward_and_budget_never_consume_rent_or_borrower_surplus() {
        assert_eq!(keeper_reward(100, 1, &p()).unwrap(), 1);
        assert_eq!(keeper_reward(10000, 10000, &p()).unwrap(), 10);
        assert_eq!(buyback_cap(100, 200, &p()).unwrap(), 105);
        assert_eq!(buyback_cap(100, 100, &p()).unwrap(), 90);
        assert_eq!(buyback_cap(100, 5, &p()).unwrap(), 0);
    }
}
