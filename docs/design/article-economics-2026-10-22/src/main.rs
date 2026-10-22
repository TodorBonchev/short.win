use shortwin_math::{
    num::WAD,
    params::Params,
    rent::{self, Basis, PosRent},
    risk::ExpiryKeeperTerms,
    tier::Tier,
    waterfall::{self, ExitInputs},
};

fn rent_observation() {
    let tier = Tier::D.defaults();
    let cfg = Params::DEFAULT;
    let q = 1_000_000_000;
    let r_min = rent::min_rent(q, WAD, &tier.rate, 8_000, cfg.util_kink_bps, tier.min_rent_s)
        .expect("production minimum rent");
    let pos = PosRent {
        q,
        j_open: [0; 3],
        terms: tier.rate,
        rent_protocol_bps: cfg.rent_protocol_bps,
        r_min,
    };
    let original_pos = pos;
    let mut basis = Basis::default();
    let apr80 = rent::rate_bps(&pos.terms, 8_000, cfg.util_kink_bps).expect("APR at 80%");
    let apr90 = rent::rate_bps(&pos.terms, 9_000, cfg.util_kink_bps).expect("APR at 90%");
    assert_eq!((apr80, apr90), (40_000, 95_000));
    assert_eq!(r_min, 1_826_485);
    rent::accrue(&mut basis, WAD, 8_000, cfg.util_kink_bps, 86_400)
        .expect("production first accrual");
    let first_due = rent::due(&pos, &basis).expect("production first debt");
    assert_eq!(first_due, 10_958_905);
    let mut separately_changed_tier = tier;
    separately_changed_tier.rate.base_bps = 0;
    separately_changed_tier.rate.slope1_bps = 0;
    separately_changed_tier.rate.slope2_bps = 0;
    rent::accrue(&mut basis, WAD, 9_000, cfg.util_kink_bps, 86_400)
        .expect("production second accrual");
    let second_due = rent::due(&pos, &basis).expect("production second debt");
    assert_eq!(second_due, 36_986_302);
    assert_eq!(pos, original_pos);
    assert_ne!(pos.terms, separately_changed_tier.rate);
    println!(
        "RENT q={} p_ema={} coefficients={:?} kink={} minimum={} day1_util=8000 day1_apr_bps={} day1_due={} day2_util=9000 day2_apr_bps={} day2_cumulative_due={} day2_increment={} same_position_unchanged=true separate_new_tier_does_not_reprice_snapshot=true",
        q, WAD, pos.terms.coefficients(), cfg.util_kink_bps, r_min,
        apr80, first_due, apr90, second_due, second_due - first_due,
    );
}

fn expiry_observation() {
    let old_cfg = Params::DEFAULT;
    let old_snapshot = old_cfg.expiry_keeper();
    assert_eq!(old_snapshot, ExpiryKeeperTerms::DEFAULT);
    assert_eq!(old_snapshot.fixed, 1_000_000);
    let mut what_if_cfg = old_cfg;
    what_if_cfg.expiry_keeper_fixed = 10_000_000;
    let what_if_snapshot = what_if_cfg.expiry_keeper();
    assert_eq!(old_cfg.expiry_keeper_fixed, 1_000_000);
    assert_eq!(old_snapshot, old_cfg.expiry_keeper());
    assert_ne!(old_snapshot, what_if_snapshot);
    println!(
        "EXPIRY_CONFIG current={:?} what_if={:?} existing_snapshot_unchanged=true default_helper_consistent=true",
        old_snapshot, what_if_snapshot,
    );
    let cases = [
        ("full", 2_000_000_000, (2_000_000, 968_000_000), (11_000_000, 959_000_000)),
        ("low_5m", 1_035_000_000, (2_000_000, 3_000_000), (5_000_000, 0)),
        ("low_1m", 1_031_000_000, (1_000_000, 0), (1_000_000, 0)),
        ("zero", 1_030_000_000, (0, 0), (0, 0)),
    ];
    for (case, v, old_expected, what_if_expected) in cases {
        let inputs = ExitInputs {
            v,
            b: 1_000_000_000,
            r_due: 10_000_000,
            b_live: 1_000_000_000,
            b_ref: 1_000_000_000,
            rent_protocol_bps: old_cfg.rent_protocol_bps,
        };
        for (schedule, terms, expected) in [
            ("current", old_snapshot, old_expected),
            ("what_if_10m", what_if_snapshot, what_if_expected),
        ] {
            let payout = waterfall::expire(&inputs, old_cfg.expiry_fee_bps, &terms)
                .expect("production expiry waterfall");
            assert_eq!((payout.keeper, payout.borrower), expected);
            assert_eq!(payout.buyback, 1_000_000_000);
            assert_eq!(payout.rent, 10_000_000);
            assert_eq!(payout.expiry_fee, 20_000_000);
            assert_eq!(payout.protocol, 1_500_000);
            assert_eq!(payout.lenders, 28_500_000);
            assert_eq!(payout.total().expect("production conserved total"), v);
            println!(
                "EXPIRY case={} schedule={} v={} b={} r_due={} b_live={} b_ref={} expiry_fee_bps={} rent_protocol_bps={} buyback={} rent_paid={} expiry_fee={} keeper={} protocol={} lenders={} borrower={} total={} conserved=true",
                case, schedule, v, inputs.b, inputs.r_due, inputs.b_live, inputs.b_ref,
                old_cfg.expiry_fee_bps, inputs.rent_protocol_bps, payout.buyback,
                payout.rent, payout.expiry_fee, payout.keeper, payout.protocol,
                payout.lenders, payout.borrower, payout.total().expect("same total"),
            );
        }
    }
}

fn main() {
    println!("CURRENT_LIBRARY name=shortwin-math version={}", shortwin_math::VERSION);
    rent_observation();
    expiry_observation();
    println!("RESULT all_declared_current_library_assertions_passed=true");
}
