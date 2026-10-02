//! The fuzzer's bounded CI smoke (`cargo test -p shortwin-fuzz --release --test smoke`): fixed
//! seeds on every profile with the invariant hooks and the fuzzer's oracles, the harness's own
//! self-tests (replay determinism of the action stream, `tmin` on an injected failure).
//!
//! `FUZZ_SMOKE_SEEDS` (default 3 per profile) and `FUZZ_SMOKE_ACTIONS` (default 200) scale it. A
//! debug build (a local `cargo test --workspace`) runs 1 × 60 per profile and skips the coverage
//! assertions: LiteSVM is about 20× slower there.
//! Needs the program builds, as `shortwin-litesvm-tests` does (`SHORTWIN_SO`,
//! `SHORTWIN_TEST_VENUE_SO`, `MOCK_CPAMM_SO`).

use shortwin_fuzz::action::{Kind, Profile, stream};
use shortwin_fuzz::runner::{self, Crash, Opts};
use shortwin_fuzz::stats::Stats;
use shortwin_fuzz::world::{INJECT, INJECTED};

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
