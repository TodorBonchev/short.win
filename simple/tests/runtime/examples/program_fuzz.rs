//! Bounded/future-duration actual-SBF scenario runner. Never borrows older fuzz time.
//! One genuine opened loan per fresh VM; clocks, initial holder supply and the CR
//! market shock are explicit synthetic fixtures. Setup transactions are not part of
//! the per-scenario transaction counters. No live network or wallet sends occur.
use anchor_lang::prelude::Pubkey;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use shortwin_simple::state::{Exposure, LenderReceipt, Loan, PoolGeneration};
use shortwin_simple_runtime_tests::{
    deterministic_key, env as pda, exit_case as x, jupiter,
    position_case::{exposure, loan, PositionCase, Q},
    TxResult,
};
use solana_instruction::Instruction;
use solana_keypair::Keypair;
use solana_signer::Signer;
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Default)]
struct Counts {
    attempted: u64,
    succeeded: u64,
    rejected: u64,
    jupiter_inner_success: u64,
    setup_started: u64,
    setup_completed: u64,
    scenarios: BTreeMap<String, u64>,
    operations: BTreeMap<String, [u64; 3]>,
}
impl Counts {
    fn json(&self) -> Value {
        json!({"attempted_fixture_setups":self.setup_started,"successful_fixture_setups":self.setup_completed,"attempted_transactions":self.attempted,"successful_transactions":self.succeeded,"expected_rejected_transactions":self.rejected,"actual_cpmm_success_logs":self.jupiter_inner_success,"scenario_counts":self.scenarios,"operation_counts_attempt_success_reject":self.operations,"fixture_setup_transactions_instrumented":false})
    }
}
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut n = self.0;
        n ^= n << 13;
        n ^= n >> 7;
        n ^= n << 17;
        self.0 = n;
        n
    }
}
fn ensure(condition: bool, message: &str) {
    assert!(condition, "INVARIANT: {message}");
}
fn check(c: &PositionCase) {
    let p: PoolGeneration = c.env.read(&c.pool);
    shortwin_pooled_core::validate_pool(&p.core()).expect("INVARIANT: core pool");
    let receipt: LenderReceipt = c.env.read(&pda::lender(&c.pool, &c.lender.pubkey()));
    ensure(receipt.shares == p.shares, "one lender owns current shares");
    shortwin_pooled_core::validate_lender(&p.core(), &receipt.core()).expect("INVARIANT: receipt");
    ensure(
        c.env.token_amount(&pda::token_vault(&c.pool)) >= p.cash,
        "vault covers tracked cash",
    );
    ensure(
        c.env.lamports(&c.pool)
            >= c.env
                .svm
                .minimum_balance_for_rent_exemption(PoolGeneration::SPACE)
                + p.sol_reserve,
        "pool rent and paid income reserve",
    );
    let key = loan(c.pool, c.borrower.pubkey(), 0);
    let ex = exposure(c.pool, c.borrower.pubkey());
    if c.env.svm.get_account(&key).is_some() {
        let l: Loan = c.env.read(&key);
        let e: Exposure = c.env.read(&ex);
        ensure(
            (p.principal_out, p.loan_count, p.booked_notional_out) == (l.q, 1, l.opening_notional),
            "live principal/count/notional equals actual loan",
        );
        ensure(
            (e.q, e.notional) == (l.q, l.opening_notional),
            "wallet exposure equals live loan",
        );
        ensure(
            c.env.lamports(&key)
                >= c.env.svm.minimum_balance_for_rent_exemption(Loan::SPACE) + l.collateral,
            "loan rent and collateral funded",
        );
    } else {
        ensure(
            (p.principal_out, p.loan_count, p.booked_notional_out) == (0, 0, 0),
            "no stale aggregate loan debt",
        );
        ensure(
            c.env.svm.get_account(&ex).is_none(),
            "empty exposure rent closed",
        );
    }
}
fn keys(c: &PositionCase, keeper: &Keypair, source: Pubkey) -> Vec<Pubkey> {
    let mut keys = c.protected(0);
    keys.extend([
        keeper.pubkey(),
        source,
        c.lender.pubkey(),
        pda::lender(&c.pool, &c.lender.pubkey()),
        deterministic_key(102).pubkey(),
    ]);
    keys.sort();
    keys.dedup();
    keys
}
fn sums(c: &PositionCase, keys: &[Pubkey], source: Pubkey) -> (u128, u128) {
    let lamports = keys.iter().map(|k| u128::from(c.env.lamports(k))).sum();
    let tokens = [
        pda::token_vault(&c.pool),
        c.route.base_vault,
        source,
        deterministic_key(102).pubkey(),
    ]
    .iter()
    .map(|k| u128::from(c.env.token_amount(k)))
    .sum();
    (lamports, tokens)
}
#[allow(clippy::too_many_arguments)] // Explicit scenario assertion boundary, not a product API.
fn transact(
    c: &mut PositionCase,
    k: &Keypair,
    source: Pubkey,
    name: &str,
    instruction: Instruction,
    actor: &Keypair,
    expected: Option<&str>,
    counts: &mut Counts,
) {
    let keys = keys(c, k, source);
    let before = c.snapshot(&keys);
    let sums_before = sums(c, &keys, source);
    let result: TxResult = c.env.send_v0(&[instruction], &[actor]).0;
    counts.attempted += 1;
    let rows = counts.operations.entry(name.into()).or_default();
    rows[0] += 1;
    let metadata = match (result, expected) {
        (Ok(m), None) => {
            counts.succeeded += 1;
            rows[1] += 1;
            m
        }
        (Err(e), Some(code)) => {
            assert!(
                code == "*" || e.meta.logs.iter().any(|l| l.contains(code)),
                "EXPECTATION: {name} expected {code}, got {:?}\n{}",
                e.err,
                e.meta.logs.join("\n")
            );
            counts.rejected += 1;
            rows[2] += 1;
            ensure(
                c.snapshot(&keys) == before,
                "expected rejection rolls back full protected state",
            );
            e.meta
        }
        (Ok(_), Some(code)) => panic!("EXPECTATION: {name} succeeded instead of {code}"),
        (Err(e), None) => panic!(
            "EXPECTATION: {name} failed {:?}\n{}",
            e.err,
            e.meta.logs.join("\n")
        ),
    };
    if name == "underfill" {
        ensure(
            metadata
                .logs
                .iter()
                .any(|l| l == &format!("Program {} success", jupiter::CPMM_ID)),
            "underfill reaches genuine CPMM before refusal",
        );
    }
    counts.jupiter_inner_success += metadata
        .logs
        .iter()
        .filter(|s| s == &&format!("Program {} success", jupiter::CPMM_ID))
        .count() as u64;
    ensure(sums(c,&keys,source)==sums_before,"tracked actual lamports and token supply conserved across transaction (fee payer separate)");
    check(c);
}
#[allow(clippy::too_many_arguments)] // Explicit timestamp/slot and bounded trusted-price fixture.
fn publish(
    c: &mut PositionCase,
    k: &Keypair,
    source: Pubkey,
    ts: i64,
    slot: u64,
    bid: u64,
    ask: u64,
    counts: &mut Counts,
) {
    // Shared helper executes a real reporter instruction. Snapshot/check it too;
    // it changes an existing reference and does not create rent-bearing accounts.
    let keys = keys(c, k, source);
    let before = sums(c, &keys, source);
    x::clock(c, ts, slot);
    x::publish(c, ts, slot, bid, ask);
    counts.attempted += 1;
    counts.succeeded += 1;
    let row = counts
        .operations
        .entry("publish_reference".into())
        .or_default();
    row[0] += 1;
    row[1] += 1;
    ensure(
        sums(c, &keys, source) == before,
        "publication conserves balances",
    );
    check(c);
}
fn run_case(case_seed: u64, counts: &mut Counts) {
    let mut rng = Rng(if case_seed == 0 { 1 } else { case_seed });
    let scenario = rng.next() % 8;
    let label = [
        "close",
        "repay",
        "margin_repay",
        "expiry",
        "default_boundary",
        "cr_fill_veto",
        "cr_market_rise",
        "underfill",
    ][scenario as usize];
    *counts.scenarios.entry(label.into()).or_default() += 1;
    counts.setup_started += 1;
    let mut c = PositionCase::new();
    let keeper = x::keeper(&mut c);
    let source = x::source(&mut c, 112);
    let borrower = c.borrower.insecure_clone();
    check(&c);
    counts.setup_completed += 1;
    // An independently randomized rejected open, followed by a genuine sale.
    let bad = rng.next() % 4;
    if bad < 3 {
        let mut args = c.args();
        let code = if bad == 0 {
            args.max_term_fee = 0;
            "FeeAboveMax"
        } else if bad == 1 {
            args.max_margin = 0;
            "MarginAboveMax"
        } else {
            args.nonce = 7;
            "InvalidNonce"
        };
        let i = c.ix(args);
        transact(
            &mut c,
            &keeper,
            source,
            "rejected_open",
            i,
            &borrower,
            Some(code),
            counts,
        );
    }
    let i = c.ix(c.args());
    transact(&mut c, &keeper, source, "open", i, &borrower, None, counts);
    let l: Loan = c.env.read(&x::key(&c));
    let ts = pda::START_TS;
    let slot = jupiter::EXECUTION_SLOT;
    match scenario {
        0 => {
            let input = 2_500_000 + rng.next() % 10_000;
            let i = x::close(&c, input, Q);
            transact(&mut c, &keeper, source, "close", i, &borrower, None, counts);
        }
        1 => {
            let i = x::repay(&c, source);
            transact(&mut c, &keeper, source, "repay", i, &borrower, None, counts);
        }
        2 => {
            let i = x::add(&c, 1 + rng.next() % 100_000);
            transact(
                &mut c,
                &keeper,
                source,
                "add_margin",
                i,
                &borrower,
                None,
                counts,
            );
            let i = x::repay(&c, source);
            transact(&mut c, &keeper, source, "repay", i, &borrower, None, counts);
        }
        3 => {
            publish(
                &mut c,
                &keeper,
                source,
                l.deadline,
                slot + 20,
                2_200_000,
                2_400_000,
                counts,
            );
            let i = x::force(&c, &keeper, 2_500_000);
            transact(
                &mut c,
                &keeper,
                source,
                "expiry_force",
                i,
                &keeper,
                None,
                counts,
            );
        }
        4 => {
            x::clock(&mut c, l.default_at, slot + 10);
            for (name, i) in [
                ("late_repay", x::repay(&c, source)),
                ("late_close", x::close(&c, 3_000_000, Q)),
                ("late_margin", x::add(&c, 1)),
            ] {
                transact(
                    &mut c,
                    &keeper,
                    source,
                    name,
                    i,
                    &borrower,
                    Some("DefaultRequired"),
                    counts,
                );
            }
            let i = x::default_ix(&c, &keeper);
            transact(&mut c, &keeper, source, "default", i, &keeper, None, counts);
        }
        5 => {
            publish(
                &mut c,
                &keeper,
                source,
                ts + 1,
                slot + 1,
                10_000_000,
                10_100_000,
                counts,
            );
            let i = x::arm(&c, &keeper);
            transact(&mut c, &keeper, source, "arm", i, &keeper, None, counts);
            let i = x::force(&c, &keeper, 3_000_000);
            transact(
                &mut c,
                &keeper,
                source,
                "early_cr_force",
                i,
                &keeper,
                Some("ConfirmationPending"),
                counts,
            );
            publish(
                &mut c,
                &keeper,
                source,
                ts + 6,
                slot + 6,
                10_000_000,
                10_100_000,
                counts,
            );
            let i = x::force(&c, &keeper, 3_000_000);
            transact(
                &mut c,
                &keeper,
                source,
                "actual_fill_veto",
                i,
                &keeper,
                Some("HealthyFill"),
                counts,
            );
            let i = x::add(&c, 1 + rng.next() % 1000);
            transact(
                &mut c,
                &keeper,
                source,
                "margin_clear",
                i,
                &borrower,
                None,
                counts,
            );
            let i = x::repay(&c, source);
            transact(&mut c, &keeper, source, "repay", i, &borrower, None, counts);
        }
        6 => {
            let mut vault = c.env.svm.get_account(&c.route.wsol_vault).unwrap();
            let old = u64::from_le_bytes(vault.data[64..72].try_into().unwrap());
            vault.data[64..72].copy_from_slice(&(old + old / 2).to_le_bytes());
            vault.lamports += old / 2;
            c.env.svm.set_account(c.route.wsol_vault, vault).unwrap();
            publish(
                &mut c,
                &keeper,
                source,
                ts + 1,
                slot + 1,
                3_300_000,
                3_500_000,
                counts,
            );
            let i = x::arm(&c, &keeper);
            transact(&mut c, &keeper, source, "arm", i, &keeper, None, counts);
            publish(
                &mut c,
                &keeper,
                source,
                ts + 6,
                slot + 6,
                3_300_000,
                3_500_000,
                counts,
            );
            let i = x::force(&c, &keeper, 3_600_000);
            transact(
                &mut c, &keeper, source, "cr_force", i, &keeper, None, counts,
            );
        }
        _ => {
            let i = x::close(&c, 3_000_000, 100 * Q);
            transact(
                &mut c,
                &keeper,
                source,
                "underfill",
                i,
                &borrower,
                Some("*"),
                counts,
            );
            let i = x::close(&c, 3_000_000, Q);
            transact(&mut c, &keeper, source, "close", i, &borrower, None, counts);
        }
    }
    check(&c);
}
fn hash(path: &Path) -> String {
    format!(
        "{:x}",
        Sha256::digest(fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())))
    )
}
fn files(root: &Path) -> Vec<PathBuf> {
    let mut out = vec![];
    for e in fs::read_dir(root).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            out.extend(files(&p));
        } else {
            out.push(p);
        }
    }
    out.sort();
    out
}
fn copy_tree(src: &Path, dest: &Path, hashes: &mut BTreeMap<String, String>, prefix: &str) {
    for file in files(src) {
        let rel = file.strip_prefix(src).unwrap();
        let target = dest.join(rel);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(&file, &target).unwrap();
        hashes.insert(format!("{prefix}/{}", rel.display()), hash(&target));
    }
}
fn freeze(out: &Path) -> Value {
    let source_root = std::env::var_os("SHORTWIN_SIMPLE_SOURCE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.."))
        .canonicalize()
        .unwrap();
    let fixture_root = jupiter::repository().canonicalize().unwrap();
    let mut hashes = BTreeMap::new();
    let frozen = out.join("frozen-root");
    fs::create_dir_all(&frozen).unwrap();
    copy_tree(
        &fixture_root.join("tests/fixtures"),
        &frozen.join("tests/fixtures"),
        &mut hashes,
        "tests/fixtures",
    );
    let candidate = std::env::var_os("SHORTWIN_JUPITER_CANDIDATE_DIR").map_or_else(
        || fixture_root.join("target/jupiter-exec-probe/jupiter-candidate"),
        PathBuf::from,
    );
    copy_tree(
        &candidate,
        &frozen.join("target/jupiter-exec-probe/jupiter-candidate"),
        &mut hashes,
        "target/jupiter-exec-probe/jupiter-candidate",
    );
    let elf = pda::program_so_path();
    let program = frozen.join("shortwin_simple.so");
    fs::copy(&elf, &program).unwrap();
    hashes.insert("shortwin_simple.so".into(), hash(&program));
    let mut source = BTreeMap::new();
    for part in [
        "crates/shortwin-jupiter-route",
        "simple/crates/pooled-core",
        "simple/programs/shortwin-simple",
        "simple/tests/runtime",
    ] {
        for file in files(&source_root.join(part)) {
            if file.components().any(|x| x.as_os_str() == "target") {
                continue;
            }
            if file.extension().is_some_and(|e| e == "rs")
                || file.file_name().is_some_and(|n| n == "Cargo.toml")
            {
                let rel = file.strip_prefix(&source_root).unwrap();
                let dest = out.join("source").join(rel);
                fs::create_dir_all(dest.parent().unwrap()).unwrap();
                fs::copy(&file, &dest).unwrap();
                source.insert(rel.display().to_string(), hash(&dest));
            }
        }
    }
    for name in ["LICENSE", "simple/Cargo.toml", "simple/Cargo.lock"] {
        let file = source_root.join(name);
        let dest = out.join("source").join(name);
        fs::create_dir_all(dest.parent().unwrap()).unwrap();
        fs::copy(&file, &dest).unwrap();
        source.insert(name.to_string(), hash(&dest));
    }
    let runner = out.join("runner");
    fs::copy(std::env::current_exe().unwrap(), &runner).unwrap();
    // Set once before any LiteSVM instance exists. Every case reads these frozen copies.
    std::env::set_var("SHORTWIN_SIMPLE_SO", program);
    std::env::set_var("SHORTWIN_SIMPLE_FIXTURE_ROOT", &frozen);
    std::env::remove_var("SHORTWIN_JUPITER_CANDIDATE_DIR");
    json!({"artifact_hashes":hashes,"source_hashes":source,"runner_sha256":hash(&runner),"runner_path":"runner","source_repository":source_root,"fixture_scope":"all copied fixture files plus Jupiter candidate, own program, bundled SPL/T22 tied to runner/Cargo.lock","synthetic_fixtures":["initial token holder allocations","execution clocks","test-only active ALT","50% native reserve+lamport CR market shock"],"program_model":"new pooled, fixed paid upfront term fee, one actual loan per fresh VM"})
}
fn output(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("Actual new-program scenario runner (local LiteSVM only).\nUsage: program_fuzz [--seed N] [--cases N | --duration-seconds N | --replay-seed N] [--out NEW_DIRECTORY]\nDefaults: --seed 103410000 --cases 32 --out target/program-fuzz-smoke.\nDuration mode has no case limit and finishes the in-flight case.\nThe output directory must not exist; frozen inputs, summary, completion hashes and crash replay evidence are retained.\nThis help command does not create output or start a campaign.");
        return;
    }
    let mut seed = 103410000u64;
    let mut limit = Some(32u64);
    let mut duration = None;
    let mut replay = None;
    let mut out = PathBuf::from("target/program-fuzz-smoke");
    let mut explicit_cases = false;
    let mut i = 1;
    while i < args.len() {
        let flag = &args[i];
        i += 1;
        let v = args
            .get(i)
            .unwrap_or_else(|| panic!("missing value for {flag}"));
        match flag.as_str() {
            "--seed" => seed = v.parse().unwrap(),
            "--cases" => {
                limit = Some(v.parse().unwrap());
                explicit_cases = true;
            }
            "--duration-seconds" => duration = Some(v.parse::<u64>().unwrap()),
            "--replay-seed" => replay = Some(v.parse::<u64>().unwrap()),
            "--out" => out = PathBuf::from(v),
            _ => panic!("unknown argument {flag}"),
        };
        i += 1;
    }
    if duration.is_some() {
        assert!(!explicit_cases, "choose duration or cases");
        limit = None;
    }
    if replay.is_some() {
        assert!(!explicit_cases, "replay is one case; omit --cases");
        assert!(duration.is_none(), "replay is one case");
        limit = Some(1);
    }
    assert!(limit != Some(0) && duration != Some(0));
    assert!(!out.exists(), "refuse existing output directory");
    fs::create_dir_all(&out).unwrap();
    let out = out.canonicalize().unwrap();
    let frozen = freeze(&out);
    let started = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    output(
        &out.join("metadata.json"),
        &json!({"started_at_unix":started,"command":args,"seed":seed,"requested_duration_seconds":duration,"case_limit":limit,"frozen":frozen}),
    );
    let mut log = OpenOptions::new()
        .create_new(true)
        .append(true)
        .open(out.join("run.log"))
        .unwrap();
    let timer = Instant::now();
    let mut rng = Rng(if seed == 0 { 1 } else { seed });
    let mut counts = Counts::default();
    let mut completed = 0u64;
    let mut failure = None;
    loop {
        if limit.is_some_and(|n| completed >= n)
            || duration.is_some_and(|n| timer.elapsed().as_secs() >= n)
        {
            break;
        }
        let case_seed = replay.unwrap_or_else(|| rng.next());
        writeln!(log, "case_start index={completed} seed={case_seed}").unwrap();
        log.flush().unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_case(case_seed, &mut counts)
        }));
        match result {
            Ok(()) => {
                completed += 1;
                writeln!(
                    log,
                    "case_complete completed={completed} wall_seconds={:.6}",
                    timer.elapsed().as_secs_f64()
                )
                .unwrap();
            }
            Err(e) => {
                let message = e
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_else(|| "non-string panic".into());
                let crash = json!({"case_index":completed,"case_seed":case_seed,"error":message,"counts":counts.json()});
                output(&out.join("crash.json"), &crash);
                failure = Some(crash);
                break;
            }
        }
        log.flush().unwrap();
    }
    let wall = timer.elapsed().as_secs_f64();
    let summary = json!({"schema_version":1,"scope":"actual-new-SBF deterministic scenario runner; setup transactions not counted; no inherited duration","completed_cases":completed,"wall_seconds":wall,"failures":u64::from(failure.is_some()),"anomalies":u64::from(failure.is_some()),"setup_failures":counts.setup_started-counts.setup_completed,"invariant_failures":u64::from(failure.as_ref().is_some_and(|f|f["error"].as_str().is_some_and(|s|s.contains("INVARIANT:")))),"coverage":counts.json(),"requested_duration_seconds":duration,"finished_cleanly":failure.is_none()});
    writeln!(log, "summary {}", summary).unwrap();
    log.flush().unwrap();
    drop(log);
    output(&out.join("summary.json"), &summary);
    output(
        &out.join("completion.json"),
        &json!({"exit_code":if failure.is_some(){1}else{0},"elapsed_seconds":wall,"summary_sha256":hash(&out.join("summary.json")),"log_sha256":hash(&out.join("run.log"))}),
    );
    println!("{}", summary);
    if failure.is_some() {
        std::process::exit(1);
    }
}
