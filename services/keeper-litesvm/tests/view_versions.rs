//! Unsupported account layouts must not enter the keeper's typed view, even when their
//! owner, discriminator and length still match the current account types.

use anchor_lang::prelude::Pubkey;
use shortwin::constants::{CONFIG_VERSION, MARKET_VERSION, VENUE_EXTRAS_VERSION};
use shortwin::state::{EXTRAS_IX_BUY, Market, VenueExtras, VenueKind};
use shortwin_keeper::{KeeperConfig, pda, view};
use shortwin_keeper_litesvm::{Harness, block_on};
use shortwin_litesvm_tests::engine::{MarketSpec, MockMarket, mock_extras_key};
use shortwin_litesvm_tests::{Env, fixtures, read};

fn setup(n: usize) -> (Harness, Vec<MockMarket>) {
    let mut env = Env::test_venue();
    let markets = (0..n)
        .map(|_| env.mock_market(MarketSpec::default()))
        .collect();
    (Harness::new(env, KeeperConfig::default()), markets)
}

/// All three versioned account types put the u16 layout version immediately after their
/// discriminator. Change only those bytes, preserving ownership, length and other fields.
fn set_layout_version(h: &Harness, key: Pubkey, version: u16) {
    h.with_env(|env| {
        let mut account = env.svm.get_account(&key).expect("versioned account");
        account.data[8..10].copy_from_slice(&version.to_le_bytes());
        env.svm.set_account(key, account).expect("updated version");
    });
}

#[test]
fn global_config_requires_the_supported_layout_version() {
    let (h, _) = setup(0);
    assert_eq!(
        block_on(view::load(h.chain(), &[]))
            .expect("current config")
            .config
            .version,
        CONFIG_VERSION,
    );

    for version in [CONFIG_VERSION - 1, CONFIG_VERSION + 1] {
        set_layout_version(&h, pda::config(), version);
        let error = block_on(view::load(h.chain(), &[]))
            .err()
            .expect("unsupported config must stop the load");
        assert!(
            error.to_string().contains(&format!(
                "unsupported GlobalConfig layout version {version}"
            )),
            "{error:#}",
        );
    }

    set_layout_version(&h, pda::config(), CONFIG_VERSION);
    assert!(block_on(view::load(h.chain(), &[])).is_ok());
}

fn unsupported_market_is_skipped(kind: VenueKind) {
    let (h, markets) = setup(2);
    let key = markets[0].market();
    let other = markets[1].market();
    h.with_env(|env| {
        let mut market = read::zc::<Market>(&env.svm, &key).expect("market");
        market.venue = kind as u8;
        fixtures::set_zc(&mut env.svm, &key, &market, 0);
    });
    let current = block_on(view::load(h.chain(), &[key, other])).expect("current markets");
    assert_eq!(current.markets.len(), 2);
    assert!(current.skipped.is_empty());
    assert_eq!(
        current.list_only.len(),
        usize::from(kind == VenueKind::None)
    );

    for version in [MARKET_VERSION - 1, MARKET_VERSION + 1] {
        set_layout_version(&h, key, version);
        let loaded = block_on(view::load(h.chain(), &[key, other])).expect("other market loads");
        assert_eq!(loaded.markets.len(), 1);
        assert_eq!(loaded.markets[0].key, other);
        assert!(loaded.list_only.is_empty());
        assert_eq!(loaded.skipped.len(), 1);
        assert_eq!(loaded.skipped[0].0, key);
        assert!(
            loaded.skipped[0]
                .1
                .contains(&format!("unsupported Market layout version {version}")),
        );
    }

    set_layout_version(&h, key, MARKET_VERSION);
    let restored = block_on(view::load(h.chain(), &[key, other])).expect("restored market");
    assert_eq!(restored.markets.len(), 2);
    assert!(restored.skipped.is_empty());
    assert_eq!(
        restored.list_only.len(),
        usize::from(kind == VenueKind::None)
    );
}

#[test]
fn unsupported_market_layout_is_skipped_without_losing_supported_markets() {
    unsupported_market_is_skipped(VenueKind::Mock);
}

#[test]
fn unsupported_market_layout_never_enters_the_list_only_pool_view() {
    unsupported_market_is_skipped(VenueKind::None);
}

#[test]
fn unsupported_extras_layout_is_unavailable_to_the_market() {
    let (h, markets) = setup(1);
    let key = markets[0].market();
    let extras_key = mock_extras_key();
    // The publication version is independent of the account layout version.
    h.with_env(|env| {
        let mut extras = read::zc::<VenueExtras>(&env.svm, &extras_key).expect("extras");
        extras.sets[EXTRAS_IX_BUY].current.version = u32::MAX;
        fixtures::set_zc(&mut env.svm, &extras_key, &extras, 0);
    });
    let current = block_on(view::load(h.chain(), &[key])).expect("current extras");
    let extras = current.markets[0]
        .extras
        .as_ref()
        .expect("supported extras");
    assert_eq!(extras.version, VENUE_EXTRAS_VERSION);
    assert_eq!(extras.sets[EXTRAS_IX_BUY].current.version, u32::MAX);

    for version in [VENUE_EXTRAS_VERSION - 1, VENUE_EXTRAS_VERSION + 1] {
        set_layout_version(&h, extras_key, version);
        let loaded = block_on(view::load(h.chain(), &[key])).expect("market still loads");
        assert_eq!(loaded.markets.len(), 1);
        assert_eq!(loaded.markets[0].key, key);
        assert!(loaded.markets[0].extras.is_none());
        assert!(loaded.skipped.is_empty());
    }

    set_layout_version(&h, extras_key, VENUE_EXTRAS_VERSION);
    let restored = block_on(view::load(h.chain(), &[key])).expect("restored extras");
    assert!(restored.markets[0].extras.is_some());
}
