//! Additive invariants for distinct quote-native debt. No legacy hook is disabled.
use super::positions::{IxFacts, MarketPositions};
use super::{Violation, ensure, fail};
use crate::{pda, read};
use anchor_lang::AccountDeserialize;
use anchor_lang::prelude::Pubkey;
use litesvm::LiteSVM;
use shortwin::state::{
    Market, QuoteFinanceExposure, QuoteFinanceMarket, QuoteFinancePosition, WalletExposure,
};
use shortwin_math::quote_rent::{self, NotionalAggregate, persistence};
use std::collections::BTreeMap;

pub fn positions_of(svm: &LiteSVM, market: &Pubkey) -> Vec<(Pubkey, QuoteFinancePosition, u64)> {
    read::all_of::<QuoteFinancePosition>(svm)
        .into_iter()
        .filter_map(|(key, account)| {
            QuoteFinancePosition::try_deserialize(&mut account.data.as_slice())
                .ok()
                .filter(|position| position.market == *market)
                .map(|position| (key, position, account.lamports))
        })
        .collect()
}
pub(super) fn finance(
    svm: &LiteSVM,
    market: &Pubkey,
) -> Result<(Pubkey, QuoteFinanceMarket), Violation> {
    let key =
        Pubkey::find_program_address(&[QuoteFinanceMarket::SEED, market.as_ref()], &shortwin::ID).0;
    let state = read::borsh::<QuoteFinanceMarket>(svm, &key).ok_or_else(|| Violation {
        invariant: "I6",
        detail: format!("quote-native market {market} has no typed finance sidecar"),
    })?;
    Ok((key, state))
}
fn checkpoint(
    svm: &LiteSVM,
    market_key: &Pubkey,
    market: &Market,
) -> Result<persistence::MarketCheckpoint, Violation> {
    let (_, finance) = finance(svm, market_key)?;
    finance
        .restore(&pda::config(), market_key, market)
        .map_err(|error| Violation {
            invariant: "I6",
            detail: format!("quote-native market {market_key}: invalid sidecar {error:?}"),
        })
}

pub fn check_market(svm: &LiteSVM, market_key: &Pubkey, market: &Market) -> Result<(), Violation> {
    ensure(
        super::positions_of(svm, market_key).is_empty(),
        "I6",
        || format!("quote-native market {market_key} contains legacy ShortPosition debt"),
    )?;
    ensure(
        read::all_of::<WalletExposure>(svm).iter().all(|(key, _)| {
            read::borsh::<WalletExposure>(svm, key)
                .is_none_or(|exposure| exposure.market != *market_key)
        }),
        "I6",
        || format!("quote-native market {market_key} retains legacy wallet exposure"),
    )?;
    let (finance_key, finance) = finance(svm, market_key)?;
    let checkpoint = checkpoint(svm, market_key, market)?;
    let mut aggregate = NotionalAggregate::empty();
    let mut quantity = 0u128;
    let mut tiers = [0u128; 4];
    let mut notional = 0u128;
    let positions = positions_of(svm, market_key);
    let mut wallets = BTreeMap::<Pubkey, (u128, u128)>::new();
    for (key, position, lamports) in &positions {
        ensure(position.is_pda(key), "I6", || {
            format!("quote position {key} has wrong PDA")
        })?;
        let rent = position
            .restore(&finance_key, &finance, &checkpoint)
            .map_err(|error| Violation {
                invariant: "I6",
                detail: format!("quote position {key}: invalid binding/rent {error:?}"),
            })?;
        aggregate.add(&rent).map_err(|error| Violation {
            invariant: "I6",
            detail: format!("quote position {key}: aggregate error {error:?}"),
        })?;
        quantity += u128::from(position.q);
        notional += u128::from(position.n_open);
        tiers[usize::from(position.tier)] += u128::from(position.q);
        let wallet = wallets.entry(position.borrower).or_default();
        wallet.0 += u128::from(position.q);
        wallet.1 += u128::from(position.n_open);
        ensure(
            u128::from(*lamports)
                >= u128::from(svm.minimum_balance_for_rent_exemption(QuoteFinancePosition::SPACE))
                    + u128::from(position.v),
            "I2",
            || format!("quote position {key} escrow below tracked V plus account rent"),
        )?;
        ensure(market.max_release >= position.default_at, "I6", || {
            format!("quote position {key}: pool release horizon precedes locked default time")
        })?;
        ensure(market.max_q_hwm >= position.q, "I6", || {
            format!("quote position {key} exceeds market q high-water mark")
        })?;
    }
    ensure(
        quantity == u128::from(market.borrowed_q)
            && notional == u128::from(market.total_notional_open)
            && positions.len() == market.open_positions as usize
            && aggregate == checkpoint.aggregate(),
        "I6",
        || {
            format!(
                "quote-native market {market_key}: real account debt/count/N/rent aggregate mismatch"
            )
        },
    )?;
    ensure(
        tiers
            .iter()
            .zip(market.borrowed_q_by_tier)
            .all(|(sum, stored)| *sum == u128::from(stored)),
        "I6",
        || {
            format!(
                "quote-native market {market_key}: tier quantities differ from actual positions"
            )
        },
    )?;
    for (key, account) in read::all_of::<QuoteFinanceExposure>(svm) {
        let exposure = QuoteFinanceExposure::try_deserialize(&mut account.data.as_slice())
            .map_err(|error| Violation {
                invariant: "I6",
                detail: format!("quote exposure {key}: decode error {error:?}"),
            })?;
        if exposure.market != *market_key {
            continue;
        }
        let expected = Pubkey::create_program_address(
            &[
                QuoteFinanceExposure::SEED,
                market_key.as_ref(),
                exposure.wallet.as_ref(),
                &[exposure.bump],
            ],
            &shortwin::ID,
        );
        ensure(expected == Ok(key), "I6", || {
            format!("quote exposure {key} has wrong PDA")
        })?;
        ensure(
            wallets.remove(&exposure.wallet)
                == Some((
                    u128::from(exposure.q_open),
                    u128::from(exposure.notional_open),
                ))
                && exposure.q_open > 0
                && exposure.notional_open > 0,
            "I6",
            || format!("quote exposure {key}: actual wallet q/N mismatch or zero account retained"),
        )?;
    }
    ensure(wallets.is_empty(), "I6", || {
        format!("quote-native market {market_key} has a borrower without exposure")
    })
}

pub fn check_ix(
    svm: &LiteSVM,
    before: &MarketPositions,
    ix: &IxFacts<'_>,
) -> Result<(), Violation> {
    let after_market = read::zc::<Market>(svm, &before.market).ok_or_else(|| Violation {
        invariant: "I6",
        detail: format!("quote-native market {} disappeared", before.market),
    })?;
    let after_checkpoint = checkpoint(svm, &before.market, &after_market)?;
    let old_finance = before.quote_finance.as_ref().ok_or_else(|| Violation {
        invariant: "I6",
        detail: format!("quote-native market {} lacked prior sidecar", before.market),
    })?;
    let mut expected_checkpoint = old_finance
        .restore(&pda::config(), &before.market, &before.state)
        .map_err(|error| Violation {
            invariant: "I6",
            detail: format!("invalid prior quote checkpoint {error:?}"),
        })?;
    let utilization =
        shortwin_math::rent::utilization_bps(before.state.tokens_free, before.state.borrowed_q)
            .map_err(|error| Violation {
                invariant: "I6",
                detail: format!("invalid prior utilization {error:?}"),
            })?;
    let kink = expected_checkpoint
        .aggregate()
        .kink_bps()
        .unwrap_or(before.state.util_kink_bps);
    let step = expected_checkpoint
        .accrue_through(
            u64::try_from(after_market.last_accrual_ts).map_err(|_| Violation {
                invariant: "I6",
                detail: "negative quote accrual clock".into(),
            })?,
            utilization,
            kink,
        )
        .map_err(|error| Violation {
            invariant: "I6",
            detail: format!("quote rent accrual error {error:?}"),
        })?;
    ensure(
        after_checkpoint.basis() == expected_checkpoint.basis()
            && after_checkpoint.carry() == expected_checkpoint.carry(),
        "I6",
        || "quote-native time/util basis or fractional carry changed incorrectly".to_owned(),
    )?;
    let mut expected_receivable = before
        .state
        .receivable
        .checked_add(step.lender_lamports)
        .ok_or_else(|| Violation {
            invariant: "I6",
            detail: "quote receivable overflow".into(),
        })?;
    let after_positions = positions_of(svm, &before.market);
    for (key, position, lamports) in &before.quote_positions {
        let repayments: Vec<_> = ix
            .repays
            .iter()
            .filter(|event| event.position == *key)
            .collect();
        let defaults: Vec<_> = ix
            .overdue_defaults
            .iter()
            .filter(|event| event.position == *key)
            .collect();
        ensure(repayments.len() + defaults.len() <= 1, "I9", || {
            format!("quote position {key}: duplicate terminal events")
        })?;
        let after_lamports = read::lamports(svm, key);
        if repayments.is_empty() && defaults.is_empty() {
            ensure(after_lamports >= *lamports, "I3", || {
                format!("quote position {key} lost lamports without an exit")
            })?;
            let after = read::borsh::<QuoteFinancePosition>(svm, key).ok_or_else(|| Violation {
                invariant: "I3",
                detail: format!("quote position {key} disappeared without a recognized exit"),
            })?;
            let mut locked = after.clone();
            locked.v = position.v;
            locked.m = position.m;
            ensure(locked == *position, "A26", || {
                format!("quote position {key}: locked default/rent/identity terms changed")
            })?;
            ensure(
                after.v.checked_sub(position.v) == after.m.checked_sub(position.m)
                    && after.v >= position.v
                    && after.v - position.v <= after_lamports - lamports,
                "I3",
                || format!("quote position {key}: margin/vault growth mismatch"),
            )?;
            continue;
        }
        ensure(
            read::borsh::<QuoteFinancePosition>(svm, key).is_none() && after_lamports == 0,
            "I3",
            || format!("quote position {key}: terminal action did not close account"),
        )?;
        let rent =
            persistence::decode_position(&position.rent_state).map_err(|error| Violation {
                invariant: "I9",
                detail: format!("quote position {key}: stored rent invalid {error:?}"),
            })?;
        let due = quote_rent::due(&rent, &after_checkpoint.basis()).map_err(|error| Violation {
            invariant: "I9",
            detail: format!("quote position {key}: due error {error:?}"),
        })?;
        let credited =
            quote_rent::credited_for(&rent, &after_checkpoint.basis()).map_err(|error| {
                Violation {
                    invariant: "I9",
                    detail: format!("quote position {key}: credited error {error:?}"),
                }
            })?;
        let retired = credited.min(expected_receivable);
        expected_receivable -= retired;
        if let Some(event) = repayments.first() {
            ensure(
                event.market == before.market
                    && event.borrower == position.borrower
                    && event.q == position.q,
                "I9",
                || format!("quote position {key}: repayment identity/q mismatch"),
            )?;
            ensure(
                event.rent == due
                    && u128::from(event.rent) + u128::from(event.borrower_payout)
                        == u128::from(position.v),
                "I9",
                || format!("quote position {key}: full rent/conservation mismatch"),
            )?;
            ensure(
                u128::from(event.rent_protocol)
                    == u128::from(event.rent) * u128::from(rent.protocol_bps()) / 10000,
                "I9",
                || format!("quote position {key}: locked rent split mismatch"),
            )?;
        }
        if let Some(event) = defaults.first() {
            let now = svm.get_sysvar::<solana_clock::Clock>().unix_timestamp;
            let keeper = shortwin_math::risk::k_s(position.v, &position.default_keeper_terms())
                .map_err(|error| Violation {
                    invariant: "I9",
                    detail: format!("quote default {key}: keeper error {error:?}"),
                })?;
            ensure(
                event.market == before.market
                    && event.borrower == position.borrower
                    && event.q_lost == position.q
                    && event.default_at == position.default_at
                    && now >= position.default_at
                    && event.ts == now,
                "I9",
                || {
                    format!(
                        "quote default {key}: identity, actual-clock or nominal-token loss mismatch"
                    )
                },
            )?;
            ensure(
                event.collateral == position.v
                    && event.keeper_payment == keeper
                    && event.borrower_amount == 0
                    && u128::from(event.lender_amount) + u128::from(event.keeper_payment)
                        == u128::from(position.v)
                    && event.position_account_refund == lamports - position.v
                    && event.rent_due == due
                    && event.credited_retired == retired
                    && event.share_epoch_after == after_market.share_epoch,
                "I9",
                || {
                    format!(
                        "quote default {key}: collateral/keeper/lender/rent/loss accounting mismatch"
                    )
                },
            )?;
            ensure(
                after_market.tokens_free == before.state.tokens_free
                    && read::token_amount(svm, &before.state.pool_vault)
                        == Some(before.pool_vault_tokens),
                "I9",
                || format!("quote default {key} falsely returned token principal"),
            )?;
        }
    }
    ensure(after_market.receivable == expected_receivable, "I6", || {
        "quote exit receivable retirement mismatch".to_owned()
    })?;
    for event in ix
        .repays
        .iter()
        .filter(|event| event.market == before.market)
    {
        ensure(
            before
                .quote_positions
                .iter()
                .any(|(key, _, _)| *key == event.position),
            "I9",
            || format!("quote repayment names unknown position {}", event.position),
        )?;
    }
    for event in ix
        .overdue_defaults
        .iter()
        .filter(|event| event.market == before.market)
    {
        ensure(
            before
                .quote_positions
                .iter()
                .any(|(key, _, _)| *key == event.position),
            "I9",
            || format!("quote default names unknown position {}", event.position),
        )?;
    }
    let repaid: u128 = ix
        .repays
        .iter()
        .filter(|event| event.market == before.market)
        .map(|event| u128::from(event.q))
        .sum();
    if repaid > 0 {
        ensure(
            read::token_amount(svm, &before.state.pool_vault)
                .and_then(|amount| amount.checked_sub(before.pool_vault_tokens))
                .map(u128::from)
                == Some(repaid),
            "I9",
            || {
                format!(
                    "quote repayment on {} lacks exact new net token q",
                    before.market
                )
            },
        )?;
    }
    // No quote-native public opening/route exits exist yet. A later implementation
    // must add their actual event/authorization and economic rules before admission.
    if after_positions
        .iter()
        .any(|(key, _, _)| !before.quote_positions.iter().any(|(old, _, _)| key == old))
    {
        return fail(
            "I3",
            format!(
                "quote-native market {} created debt before its opening invariant contract exists",
                before.market
            ),
        );
    }
    Ok(())
}
