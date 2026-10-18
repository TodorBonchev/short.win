//! Future VenueGuard versions fail closed at every reader, before trading or pool selection.
#![allow(clippy::result_large_err)]

use shortwin::errors::ShortwinError as E;
use shortwin::state::VenueGuard;
use shortwin_litesvm_tests::engine::{MarketSpec, create_mock_market_ix};
use shortwin_litesvm_tests::errors::assert_err;
use shortwin_litesvm_tests::pool_select::{execute_pool_switch_raw, propose_pool_raw};
use shortwin_litesvm_tests::{Env, fixtures, mock, pda, read};
use solana_signer::Signer;

#[test]
fn all_venue_guard_readers_refuse_an_unknown_version() {
    let mut env = Env::test_venue();
    let mm = env.mock_market(MarketSpec::default());
    let mut borrower = env.borrower();
    let spec = MarketSpec::default();
    let pool = mock::new_pool(&mut env, spec.program, spec.decimals, spec.fee_ppm);
    mock::seed(&mut env, &pool, spec.base, spec.quote);
    let creator = env.funded_keypair();

    // An upgrade-owned fixture: today's instruction writers cannot produce this version.
    let mut guard: VenueGuard = read::zc(&env.svm, &pda::venue_guard()).unwrap();
    guard.version += 1;
    fixtures::set_zc(&mut env.svm, &pda::venue_guard(), &guard, 0);

    assert_err(
        &env.open(&mm, &mut borrower, 3_000_000_000_000).0,
        E::ConfigVersion,
    );
    assert_err(
        &env.send(&[propose_pool_raw(&mm.mint(), 3, &[])], &[]),
        E::ConfigVersion,
    );
    assert_err(
        &env.send(&[execute_pool_switch_raw(&mm.mint(), &[])], &[]),
        E::ConfigVersion,
    );
    let create = create_mock_market_ix(&creator.pubkey(), &pool.keys);
    assert_err(&env.send(&[create], &[&creator]), E::ConfigVersion);
}
