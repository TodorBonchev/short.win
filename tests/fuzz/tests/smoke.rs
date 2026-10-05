//! The fuzzer's bounded CI smoke (`cargo test -p shortwin-fuzz --release --test smoke`): fixed
//! seeds on every profile with the invariant hooks and the fuzzer's oracles, the harness's own
//! self-tests (replay determinism of the action stream, `tmin` on an injected failure).
//!
//! `FUZZ_SMOKE_SEEDS` (default 3 per profile) and `FUZZ_SMOKE_ACTIONS` (default 200) scale it. A
//! debug build (a local `cargo test --workspace`) runs 1 × 60 per profile and skips the coverage
//! assertions: LiteSVM is about 20× slower there.
//! Needs the program builds, as `shortwin-litesvm-tests` does (`SHORTWIN_SO`,
//! `SHORTWIN_TEST_VENUE_SO`, `MOCK_CPAMM_SO`).

use shortwin_fuzz::action::{Action, Kind, Profile, stream};
use shortwin_fuzz::runner::{self, Crash, Opts};
use shortwin_fuzz::stats::Stats;
use shortwin_fuzz::world::{CREDITED_SKEW, DEFICIT_BOUND_PPM, INJECT, INJECTED};

fn env_num(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// The smoke seeds: the same on every machine (the action streams are a pure function of
/// them).
const SEED_BASE: u64 = 0x5eed_0000;

/// Whether this is the full smoke (a release build, as CI runs it).
const FULL: bool = !cfg!(debug_assertions);

fn smoke(profile: Profile) -> Stats {
    let seeds = env_num("FUZZ_SMOKE_SEEDS", if FULL { 3 } else { 1 });
    let actions = env_num("FUZZ_SMOKE_ACTIONS", if FULL { 200 } else { 60 }) as usize;
    let mut total = Stats::default();
    for i in 0..seeds {
        let seed = SEED_BASE + 97 * i + profile as u64;
        let o = runner::run(profile, seed, None, actions, Opts::default());
        if let Some(f) = &o.failure {
            let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("fuzz-smoke");
            let path = runner::write_crash(&dir, &o, false);
            panic!(
                "{} seed {seed} failed at step {} ({}):\n{}\ncrash file: {}",
                profile.name(),
                f.step,
                f.signature,
                f.message.lines().take(60).collect::<Vec<_>>().join("\n"),
                path.display()
            );
        }
        total.merge(&o.stats);
    }
    println!(
        "{}: {} runs, {} actions, {} transactions, {} hook checkpoints; opens {:?}, events {:?}",
        profile.name(),
        total.runs,
        total.executed(),
        total.txs,
        total.hook_checkpoints,
        total.ix.get("open_short"),
        total.events
    );
    assert!(total.executed() > 0);
    assert!(
        !FULL || total.ix.get("open_short").is_some_and(|v| v[0] > 0),
        "{}: no open landed in the smoke seeds",
        profile.name()
    );
    total
}

#[test]
fn smoke_mock() {
    let s = smoke(Profile::Mock);
    let exits = [
        "Liquidate",
        "Expire",
        "SolSettle(S1)",
        "SolSettle(S2)",
        "SolSettle(S3)",
    ]
    .iter()
    .map(|k| s.events.get(*k).copied().unwrap_or(0))
    .sum::<u64>();
    assert!(!FULL || exits > 0, "no keeper exit landed on the mock");
}

#[test]
fn smoke_mixed() {
    let s = smoke(Profile::Mixed);
    // O17's role (M7): the keeper's degraded-mode pauses and the adversarial signers.
    let landed = s.ix.get("set_coverage_pause").map_or(0, |v| v[0]);
    let refused = s.ix.get("set_coverage_pause").map_or(0, |v| v[1]);
    println!("mixed: set_coverage_pause landed {landed}, refused {refused}");
    assert!(
        !FULL || landed > 0,
        "no set_coverage_pause landed in the smoke seeds"
    );
}

/// The corpus (`corpus/*.json`: minimised reproducers of reported findings) replays: an open
/// finding either still fails with its recorded signature or passes (fixed: then mark the file
/// `"status": "fixed"`) — never with another failure; a finding marked fixed must pass, so its
/// reproducer is a regression. A finding that was an anomaly, not a failure, names its class in
/// `"anomaly"` (its `"signature"` is empty): fixed, the replay must also not report that class.
#[test]
fn corpus_replays() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("corpus");
    let mut n = 0;
    for entry in std::fs::read_dir(&dir).expect("corpus dir") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let c = runner::read_crash(&path).expect("corpus file");
        let raw: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("read")).expect("json");
        let fixed = raw["status"].as_str() == Some("fixed");
        let o = runner::run(c.profile, c.seed, Some(&c.actions), 0, c.opts);
        if let Some(class) = raw["anomaly"].as_str() {
            let seen = o.stats.anomalies.get(class).map_or(0, |a| a.count);
            if fixed {
                assert_eq!(
                    seen,
                    0,
                    "{} is marked fixed but reports its anomaly `{class}` again:\n{}",
                    path.display(),
                    o.stats.anomalies[class]
                        .detail
                        .lines()
                        .take(30)
                        .collect::<Vec<_>>()
                        .join("\n")
                );
            } else {
                println!("{}: anomaly `{class}` × {seen}", path.display());
            }
        }
        match &o.failure {
            None => println!(
                "{}: passes ({})",
                path.display(),
                if fixed {
                    "fixed, regression"
                } else {
                    "fixed: mark it"
                }
            ),
            Some(f) if fixed => panic!(
                "{} is marked fixed but fails again ({}):\n{}",
                path.display(),
                f.signature,
                f.message.lines().take(30).collect::<Vec<_>>().join("\n")
            ),
            Some(f) => {
                assert_eq!(
                    f.signature,
                    c.signature,
                    "{} fails differently now:\n{}",
                    path.display(),
                    f.message.lines().take(30).collect::<Vec<_>>().join("\n")
                );
                println!("{}: still fails ({})", path.display(), f.signature);
            }
        }
        n += 1;
    }
    assert!(n > 0, "an empty corpus");
}

#[test]
fn smoke_curve() {
    smoke(Profile::Curve);
}

#[test]
fn smoke_pumpswap() {
    smoke(Profile::PumpSwap);
}

#[test]
fn smoke_amm_v4() {
    smoke(Profile::AmmV4);
}

#[test]
fn smoke_cpmm() {
    smoke(Profile::Cpmm);
}

/// M10 (G-G4): `fee_shock` reaches the pump venues. On the curve and PumpSwap profiles a shock to
/// level 5 — every tier's protocol fee in pump's or PumpSwap's `FeeConfig` at 1,000 bps, above
/// `venue_fee_hard_cap_bps` — held for 8 actions refuses the opens inside it `VenueFeeTooHigh`, and
/// `outage`'s fee branch runs (the keeper settling through it); every oracle holds and the
/// wind-down restores the configs. (Before M10 both returned "unsupported": the actions were
/// skipped.)
#[test]
fn fee_shocks_reach_the_pump_venues() {
    let acts: Vec<Action> = [
        "open 0 0 6000",
        "keeper_seconds 0 3 0",
        "fee_shock 0 12 7",
        "open 0 1 6000",
        "open 0 2 5000",
        "keeper_seconds 0 5 0",
        "close 0 0 0",
        "outage 0 0 1",
        "open 0 3 6000",
    ]
    .iter()
    .map(|a| a.parse().expect("action"))
    .collect();
    for profile in [Profile::Curve, Profile::PumpSwap] {
        let o = runner::run(profile, SEED_BASE + 7, Some(&acts), 0, Opts::default());
        if let Some(f) = &o.failure {
            panic!("{}: {}\n{}", profile.name(), f.signature, f.message);
        }
        let fee_shocks = o.stats.actions.get("fee_shock").copied().unwrap_or(0);
        let outages = o.stats.actions.get("outage").copied().unwrap_or(0);
        let refused = o
            .stats
            .errors
            .get("open_short:VenueFeeTooHigh")
            .copied()
            .unwrap_or(0);
        println!(
            "{}: fee_shock {fee_shocks}, outage {outages}, opens refused VenueFeeTooHigh {refused}",
            profile.name()
        );
        assert_eq!((fee_shocks, outages), (1, 1), "{}", profile.name());
        assert!(refused >= 2, "{}: {:?}", profile.name(), o.stats.errors);
    }
}

/// The action stream is a pure function of the profile and the seed, and a run of a given
/// list executes the same actions (the keys a run creates are random; the actions are not).
#[test]
fn a_seed_replays_its_actions() {
    let a = stream(Profile::Mock, 11, 80);
    assert_eq!(a, stream(Profile::Mock, 11, 80));
    let o1 = runner::run(Profile::Mock, 11, None, 80, Opts::default());
    let o2 = runner::run(Profile::Mock, 11, Some(&a), 0, Opts::default());
    assert!(o1.failure.is_none() && o2.failure.is_none());
    assert_eq!(o1.actions, o2.actions);
    assert_eq!(o1.stats.actions, o2.stats.actions);
    assert_eq!(o1.stats.skipped, o2.stats.skipped);
}

/// `tmin` shrinks a failing list to the actions the failure needs: an injected failure on the
/// first crank with a position open reduces to an open and a crank (plus what the open needs).
#[test]
fn tmin_minimises_an_injected_failure() {
    INJECT.with(|c| c.set(Some(Kind::Crank)));
    let mut failing = None;
    for seed in 0..20 {
        let opts = Opts {
            differential: false,
            avoid_known: true,
        };
        let o = runner::run(Profile::Mock, 900 + seed, None, 150, opts);
        if o.failure
            .as_ref()
            .is_some_and(|f| f.message.contains(INJECTED))
        {
            failing = Some(o);
            break;
        }
    }
    let o = failing.expect("an injected failure in 20 seeds");
    let before = o.actions.len();
    let c = Crash {
        profile: o.profile,
        seed: o.seed,
        opts: o.opts,
        actions: o.actions.clone(),
        signature: o.failure.as_ref().expect("failure").signature.clone(),
    };
    let m = runner::tmin(&c, 300, false);
    INJECT.with(|c| c.set(None));
    let f = m.failure.as_ref().expect("still failing");
    assert_eq!(f.signature, c.signature);
    println!(
        "tmin: {before} -> {} actions: {:?}",
        m.actions.len(),
        m.actions
    );
    assert!(
        m.actions.len() <= 4,
        "tmin left {} actions",
        m.actions.len()
    );
    assert!(m.actions.len() < before);
    assert_eq!(m.actions.last().map(|a| a.kind), Some(Kind::Crank));
}

/// M10: the deficit tripwire and the wind-down's second claim pass on FUZZ-M7-24H-3's reproducer
/// (a late depositor owing ≈ 1,044 SOL, its pro-rata share of a near-cap short's shortfall). The
/// deficit sits at its bound — the late depositor's share of the shortfall the exit's events and
/// the receivable show — within the dust, and the run fails the tripwire once the bound is scaled
/// to 99 %: a debit 1 % above the lender's share would be caught; a receivable over-credited by
/// 1 SOL fails its cross-check against the position's `credited_for`. The first pass's claim of the
/// seeding lender finds `lender_sol` short until the late depositor covers; the second pass pays
/// it, and nothing is left owed.
#[test]
fn deficit_tripwire_on_fuzz_m7_24h_3() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("corpus/fuzz-m7-24h-3-amm-v4-wind-down-deficit-unfunded.json");
    let c = runner::read_crash(&path).expect("corpus file");
    let o = runner::run(c.profile, c.seed, Some(&c.actions), 0, c.opts);
    if let Some(f) = &o.failure {
        panic!("{}\n{}", f.signature, f.message);
    }
    let d = o.stats.deficits;
    let ev = |k: &str| o.stats.events.get(k).copied().unwrap_or(0);
    println!(
        "deficits {d:?}; second claims paid {}, nothing {}; unpaid {} lender_sol {}; events {:?}",
        ev("wind-down-second-claim-paid"),
        ev("wind-down-second-claim-nothing"),
        o.stats.wind_down_unpaid_max,
        o.stats.wind_down_lender_sol_max,
        o.stats.events
    );
    assert!(d.largest > 1_000_000_000_000, "{d:?}");
    let excess = i128::from(d.largest) - d.largest_bound as i128;
    assert!(
        excess <= shortwin_fuzz::shortfall::DEFICIT_DUST as i128,
        "{d:?}"
    );
    assert!(d.tightest_ppm >= 990_000, "the bound is loose: {d:?}");
    assert!(ev("shortfall-exits-short") > 0);
    assert!(ev("wind-down-second-claim-paid") > 0);
    assert!(o.stats.wind_down_unpaid_max <= shortwin_fuzz::world::CLAIM_DUST);

    DEFICIT_BOUND_PPM.with(|c| c.set(990_000));
    let o = runner::run(c.profile, c.seed, Some(&c.actions), 0, c.opts);
    DEFICIT_BOUND_PPM.with(|c| c.set(1_000_000));
    let f = o.failure.expect("the tripwire fires on a 99 % bound");
    assert!(f.message.contains("deficit tripwire"), "{}", f.message);

    // A receivable 1 SOL over its positions' credit would widen the bound with the deficit; the
    // independent `credited_for` catches it at the exit.
    CREDITED_SKEW.with(|c| c.set(1_000_000_000));
    let o = runner::run(c.profile, c.seed, Some(&c.actions), 0, c.opts);
    CREDITED_SKEW.with(|c| c.set(0));
    let f = o.failure.expect("the credited cross-check fires");
    assert!(f.message.contains("took"), "{}", f.message);
}

/// M10 (FUZZ-M10-24H-A): the wind-down's second claim pass leaves exactly ECON §9.4's I-P6
/// residue — rounding and the SOL credited to rebase-orphan shares (§9.5) — and the final-tree
/// 24 h run's reproducers pass through their own branch of the oracle. On CPMM the last lender,
/// its shares all withdrawn, stays owed more than `CLAIM_DUST` of the accumulators' carry while
/// none of its realised SOL is left and the pot is empty (`wind-down-owed-rounding`); on the curve
/// `lender_sol` keeps more than the hooks' dust cap credited to orphan shares no lender holds
/// (`wind-down-orphan-credit`). Either run failed the pass's first oracle.
#[test]
fn wind_down_residue_on_fuzz_m10_24h_a() {
    use shortwin_fuzz::world::CLAIM_DUST;
    use shortwin_litesvm_tests::invariants::lender::DEFAULT_LIVENESS_DUST_CAP;
    for (file, event) in [
        (
            "corpus/fuzz-m10-24h-a-cpmm-carry-owed.json",
            "wind-down-owed-rounding",
        ),
        (
            "corpus/fuzz-m10-24h-a-curve-orphan-credit.json",
            "wind-down-orphan-credit",
        ),
    ] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(file);
        let c = runner::read_crash(&path).expect("corpus file");
        let o = runner::run(c.profile, c.seed, Some(&c.actions), 0, c.opts);
        if let Some(f) = &o.failure {
            panic!("{file}: {}\n{}", f.signature, f.message);
        }
        let ev = |k: &str| o.stats.events.get(k).copied().unwrap_or(0);
        println!(
            "{file}: unpaid {} lender_sol {}; events {:?}",
            o.stats.wind_down_unpaid_max, o.stats.wind_down_lender_sol_max, o.stats.events
        );
        assert!(ev(event) > 0, "{file}: no `{event}`");
        if event == "wind-down-owed-rounding" {
            assert!(o.stats.wind_down_unpaid_max > CLAIM_DUST);
        } else {
            assert!(o.stats.wind_down_lender_sol_max > DEFAULT_LIVENESS_DUST_CAP);
        }
    }
}

/// M10: `stale_receivable` (FUZZ-M7-24H-3's shape) reaches the deficit tripwire on both Raydium
/// programs: a near-cap short left unattended for days, a late depositor of three times the
/// pool, the price steered until `B̂_live` lies in `(V − R_due, V − K_s − ert]`, keeper seconds — a
/// liquidation realises a shortfall (`shortfall-exits-short`), the wind-down covers the late
/// depositor's deficit within its bound (each at ≥ 99.9 % of it: the bound is tight), and the
/// second claim pass leaves nothing owed. PumpSwap reaches the window too, but rarely (a probe:
/// its counts are printed, not asserted); on the mock and the curve it stays empty.
#[test]
fn stale_receivable_reaches_the_tripwire() {
    for profile in [Profile::AmmV4, Profile::Cpmm, Profile::PumpSwap] {
        let mut checked = 0;
        for k in 0..2u32 {
            let acts: Vec<Action> = [
                format!("stale_receivable 0 {} {}", k * 7 + 2, k * 3002 + 2),
                "keeper_seconds 0 5 0".into(),
            ]
            .iter()
            .map(|a| a.parse().expect("action"))
            .collect();
            let o = runner::run(
                profile,
                SEED_BASE + 0x31 + u64::from(k),
                Some(&acts),
                0,
                Opts::default(),
            );
            if let Some(f) = &o.failure {
                panic!("{}: {}\n{}", profile.name(), f.signature, f.message);
            }
            let ev = |k: &str| o.stats.events.get(k).copied().unwrap_or(0);
            let d = o.stats.deficits;
            println!(
                "{} +{k}: deficits {d:?}, in window {}, shortfall exits {}, unpaid {}",
                profile.name(),
                ev("stale-receivable-in-window"),
                ev("shortfall-exits-short"),
                o.stats.wind_down_unpaid_max
            );
            assert!(d.checked == 0 || d.tightest_ppm >= 999_000, "{d:?}");
            checked += d.checked;
        }
        assert!(
            !FULL || checked > 0 || profile == Profile::PumpSwap,
            "{}: no deficit reached",
            profile.name()
        );
    }
}
