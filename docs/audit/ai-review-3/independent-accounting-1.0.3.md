# Independent AI accounting fix review: draft 1.0.3

**Follow-up:** [AI3-FR01's deposit quarantine](ai3-fr01-fix-1.0.3.md) now blocks the
legacy-state capture reproduced here, without reallocating or erasing historical value.
This review and its source/binary/probe evidence describe the preceding candidate.

Review date: 2026-10-19 (Europe/Kiev). This is a separate AI code review and local test run, not an external or human audit, release approval, or deployment verification.

## Identity and scope

Question reviewed: Q1 / AI3-F06 in [the fix packet](fix-review-1.0.3.md).
Baseline: `b815269224ba0f5d946f49de7e113220c4067041`.
Candidate: `9b910602ed076a6d4359c496960b3ffc0e7c6af8`.
The evidence checkout began at `b4abcaa69e43a680b68fa5996d42fa7415a7c6fb` and advanced during parallel integration to `708746eb0085016c37f4d73e2aea02031bd6efc9`. Native Git at that HEAD returned no difference from the candidate in the five implementation/regression files below. Later metadata, documentation and packaging changes are outside this conclusion.

Read directly, rather than relying on implementation claims:

- `crates/shortwin-math/src/pool.rs`: crediting, touch/pot normalization, last-share withdrawal, claim, deposit and atomic-copy wrappers.
- `programs/shortwin/src/instructions/withdraw.rs`: production instruction order.
- `docs/design/sim/pool_model.py`: matching withdrawal rule and touch behavior.
- `crates/shortwin-math/tests/pool_carry.rs`, shared harness and vector replay.
- `tests/litesvm/tests/lender_it/adversarial.rs`; also the prior `crates/shortwin-math/tests/ai2_accounting.rs` pot regression.

No AGENTS.md was found in the review tree or the checked /root ancestor search.
All Git operations used native WSL Git. Verified source/cache paths were
`/root/short.win-audit-review` and `/root/short.win/target`. Only this evidence file was authored in the review tree by this reviewer; additional probes and baseline controls live under /tmp.

## Disposition

**Q1: conditionally accepted for fresh state and future withdrawals executed by the candidate; upgrade migration is not accepted.** The code implements the stated forward correction: a final withdrawal clears both carry fields, pays their whole lamports under the documented final-holder rule, and normalizes realised surplus before the production claim. Tested mismatches preserve the earlier zero-share claimant. No additional implementation defect was found within that forward scope.

**AI3-FR01 is open:** a pool that became empty under the preceding code can already contain historical carries, and the candidate's first new deposit does not migrate them. Exact deployment inventory and the treatment of affected persisted states require a separate decision. No production accounts were inspected or modified, and existence of such deployed markets is not established. Complete Q1/release signoff is withheld.

This conclusion also does not close historical pot ownership (AIR2-P2-02) or repeated-accrual truncation (KI-68 / AI3-L23), and does not itself accept the economic allocation policy.

## Mechanical assessment

`withdraw_in` releases the due vest, touches/checks the lender, handles the lock, and flushes the remaining vest to the last holder before burning shares (pool.rs:1829-1907). Only when the burn makes total_shares zero does it add carry/WAD to pending and carry_r/WAD to real, call `to_pot`, check the narrow integer conversions, record residues in diagnostic dust, clear both carries, and reset sweep bits (1908-1926). A partial burn skips this block.

Although the entitlement index is signed, `credit_pos` uses floor division with a nonnegative remainder below the divisor. Rust truncating division by positive WAD therefore agrees with Python floor division on reachable carry values. The signed rent true-up test exercises a negative index adjustment and the requirement to cover negative pending before the final burn.

`to_pot` caps unclaimed realised allocation at max(pending,0), moving only surplus to unmatched. It does not overwrite another lender's pending or real. When final entitlement carry exceeds final realised carry, production `claim` may draw the pot up to the remaining deficit; in the reverse mismatch the final realised surplus first restores the pot. This preserves allocated backing in the tested resolved-receivable scenarios, not a general historical allocation guarantee.

The on-chain handler runs prologue accrual, pool withdrawal, min_tokens and LastSharesBorrowed checks, then claim and min_sol before storing math state and performing transfers. Event pot_draw compares the state after withdrawal to that after claim, so carry-surplus normalization is not reported as a claim draw. Verified-ELF transaction tests cover event accounting, actual signed withdrawals, closure, the sweep barrier, and the next cohort.

The math wrapper computes on copies and writes back only after success, including record-remainder commits. The existing overflow test covers pending exceeding i64. A separate injected near-limit unmatched test exercises overflow inside the new final `to_pot`; every math input remained unchanged. This is a rollback test, not a claim that u64::MAX unmatched is economically reachable. Program transaction rollback also covers later failures.

Each index's residue is below WAD, so the new block adds less than 2*WAD to diagnostic dust per final exit. These counters describe entitlement and realised bookkeeping for the same vault; their sum is not two distinct physical SOL losses or a global lifetime bound. Per-touch losses are not recovered by clearing global carries.

The normative Python withdrawal follows the same flush/touch, burn, final allocation, surplus normalization and clear order. The 13-test recorded model-vector replay passed; this reviewer did not regenerate fixtures or rerun the entire Python generator.

## Remaining allocation boundaries

### Historical carry within a cohort

An independent public-math-API probe: A holds 2e18 raw tokens, receives a 1,000-lamport sweep, and waits the complete default 21,600-second vest. Both carries become 1,000*WAD. B then deposits one raw token. A exits and claims zero; B exits last and claims all 1,000 lamports.

This follows the selected final-holder rule but does not reconstruct A's historical fraction. It differs from preservation of already allocated zero-share claims. For a credit at supply S, the index remainder is below S, in lamports*WAD units, so its whole-lamport value can approach S/WAD; extreme raw share supplies do not have a universal one-lamport bound. The correction deliberately assigns that amount to the final holder instead of letting it survive a future empty transition.

### AI3-FR01: pre-fix empty-state carry survives first new deposit

**Status: open.** The correction executes only on candidate withdrawals. `deposit_in` does not migrate carries when total_shares is already zero.

A standalone test compiled with the actual baseline pool.rs uses only public APIs to execute: deposit A=2e18 tokens, sweep 1,000, wait/release the complete vest, withdraw all A shares, claim A. It yields:
S=0, T=0, A shares/pending/real=0, carry=carry_r=1,000*WAD, lender_sol=1,000, sweep bits cleared, A payout=0.
The test persists the full harness canonical state.

A second test against candidate math reconstructs the persisted fields and asserts that its entire canonical state equals the actual baseline output. It then uses public candidate operations: sweep 0, B deposits one token, sweep 1, B withdraws all and claims. B receives **1,001**, including the **1,000 historical lamports**; A still receives zero. Both old carries survive B's deposit.

This demonstrates a reachable predecessor state and candidate behavior after loading it, not a production upgrade transaction or deployed exposure. If no affected markets exist, the migration condition is vacuous; otherwise the inventory and treatment must be resolved before claiming complete cross-cohort repair. This review prescribes no production fix or ownership/migration choice. See the two executable probe sources below.

### Historical pot/receivable allocation (AIR2-P2-02)

The existing focused regression was independently rerun and still reproduces: two old lenders earn 200 SOL; an equal-total-sized later depositor enters; only 100 SOL is repaid; 100 SOL stays receivable; 50 SOL surplus reaches the pot. An old lender has pending 100 SOL and own real 25 SOL. The first old claimant gets **75 SOL** and the other **25 SOL**. Current-share subtraction still permits the same 50 SOL draw. After the tested partial withdrawal and remaining unpaid-rent reversal, pending is **+21, +43 and -64 SOL**.

The carry patch changes none of this. It is a demonstrated cash-allocation race with historical dependencies, not proof of net economic profit after complete borrower funding. Historical receivable/realised entitlement, cover obligations and claim ordering remain a separate design decision.

### Repeated-accrual truncation (KI-68 / AI3-L23)

Current lenders' `touch_tail` still adds floor(shares*delta_acc/WAD), and a corresponding realised floor, per touch, then advances snapshots without a per-lender fractional remainder. Permissionless pot-only touches of current lenders take this same path.

With two equal one-token lenders, release one lamport per integer second and touch A after every release. For an n-lamport vest lasting n seconds:

| n | No intermediate touch: A/B paid | Touch A every release: A/B paid | Vault left after touched run |
|---|---|---|---|
| 2 | 1 / 1 | 0 / 1 | 1 |
| 1,000 | 500 / 500 | 0 / 500 | 500 |
| 21,600 (default vest duration) | 10,800 / 10,800 | 0 / 10,800 | 10,800 |

Final global carries and diagnostic dust are zero in every case. These are public native-math/harness probes, not on-chain transaction-cost or profitability measurements. The minimal trace loses one lamport; the default-duration trace shows accumulation with the number of fractional touches.

For fixed shares and monotone positive index increments, n floors lose less than n lamports compared with exact fractions, and at most n-1 whole lamports relative to one combined floor, independently for the two indices. Equal losses in both indices are the same unpaid vault value, not additive physical loss. There is no constant one-lamport lifetime bound. Negative true-ups, changing shares, rolls and forward chains need a share-change-safe remainder design. The existing cumulative-floor forward-chain path does not make the current-lender path cumulative.

## Independently executed checks

Commands ran through `wsl.exe -d ubuntu-24.04 -- bash -lc ...`; host Cargo was 1.98.1.
All commands cited as passes below executed nonzero test sets. From the review root:

```sh
CARGO_TARGET_DIR=/root/short.win/target cargo test -p shortwin-math --locked --test pool_carry -- --nocapture
CARGO_TARGET_DIR=/root/short.win/target cargo test -p shortwin-math --locked --test ai2_accounting pass2_pot_claims_can_use_receivables -- --nocapture
CARGO_TARGET_DIR=/root/short.win/target cargo test -p shortwin-math --locked --test pool_vectors -- --nocapture
```

Results: **7 passed** in carry (six behavior tests, including its 256-case property, plus checksum); **1 passed / 15 filtered** for historical pot; **13 passed** in vectors. Replay includes both 60,000-line continuous traces, 40 stranding seeds, and lazy/rebase/forward-chain profiles.

The cache's existing SBF files were measured and rejected as candidate evidence:
release SHA256 `72c82879accac8323af6ef0ce777baea6b40c1ce312cefaf65aaa152a3f21429`;
test-venue `fd46da2c6b907486fe0021a06b5a075965b0db0d59981749bd4ed85b2ca89db7`.
The parent supplied downloaded exact-ref CI artifacts, whose file hashes this reviewer independently measured:

| Artifact under target/ci-39144291500 | SHA256 |
|---|---|
| deploy/shortwin.so | b88aa520c24301522b45871e73622a4d498603dc03a8f34ff8bbb557d7fa0647 |
| test-venue/deploy/shortwin.so | 994ad12c468f465937af21080ec3c48678bb1f197ab143ab44f91f98ca56132b |
| deploy/mock_cpamm.so | 4cf1700761782aece1a4e3463ce0b2b9f2afaed2ecf3d0a6362f115faf9c7ab9 |

```sh
CARGO_TARGET_DIR=/root/short.win/target \
SHORTWIN_SO=/root/short.win-audit-review/target/ci-39144291500/deploy/shortwin.so \
SHORTWIN_TEST_VENUE_SO=/root/short.win-audit-review/target/ci-39144291500/test-venue/deploy/shortwin.so \
MOCK_CPAMM_SO=/root/short.win-audit-review/target/ci-39144291500/deploy/mock_cpamm.so \
cargo test -p shortwin-litesvm-tests --locked --test lender_it last_exit -- --nocapture
```

Result: **3 passed / 53 filtered**, covering the empty-sweep gate, both vest branches of the signed swept-carry two-cohort scenario, and mismatched-carry pot-draw accounting. An earlier mistaken `--test it last_exit` invocation ran zero tests and supplies no evidence. This is candidate-ELF testing, not a new verifiable build or full CI claim for the merged checkout.

### Baseline negative control and legacy-state generation

Outside the review tree, copied current math src/Cargo.toml and carry tests/harness into /tmp/shortwin-independent-accounting-103-baseline, then replaced only pool.rs:

```sh
git -C /root/short.win-audit-review show b815269224ba0f5d946f49de7e113220c4067041:crates/shortwin-math/src/pool.rs > /tmp/shortwin-independent-accounting-103-baseline/src/pool.rs
cd /tmp/shortwin-independent-accounting-103-baseline
CARGO_TARGET_DIR=/tmp/shortwin-independent-accounting-103-baseline-target cargo test --offline --test pool_carry -- --nocapture
```

Expected failure, Cargo exits 101: **1 checksum passed, all 6 behavioral tests failed**. Last exit paid zero instead of 1,000; final residues survived; mismatch/signed final entitlement did not pay; the new-credit overflow was absent; the property shrank to a one-lamport realised carry surviving S=0. This is an isolated source control, not a complete old workspace build; its standalone lockfile was resolved offline.

The following file was added as /tmp/shortwin-independent-accounting-103-baseline/tests/legacy.rs and run:

```sh
CARGO_TARGET_DIR=/tmp/shortwin-independent-accounting-103-baseline-target cargo test --offline --locked --test legacy -- --nocapture
```

Result: **2 passed**, baseline public-API state generator plus checksum. Source:

```rust
#[path = "pool_harness.rs"] mod pool_harness;
use pool_harness::{Market,LUMP_VEST_S};

#[test]
fn baseline_public_api_produces_legacy_empty_carry() {
    let mut m = Market::new();
    let a = m.deposit(0,2_000_000_000_000_000_000).unwrap();
    m.sweep(1000).unwrap();
    m.now += i64::from(LUMP_VEST_S);
    m.release().unwrap();
    m.withdraw(0,a).unwrap();
    assert_eq!(m.claim(0),Ok(0));
    assert_eq!((m.pool.total_shares,m.pool.tokens_free,m.pool.carry,m.pool.carry_r,m.vault.lender_sol),
               (0,0,1000_000_000_000_000_000_000,1000_000_000_000_000_000_000,1000));
    std::fs::write("/tmp/shortwin-independent-accounting-103-baseline-empty.canonical",m.canonical()).unwrap();
    println!("baseline public-API state persisted to /tmp/shortwin-independent-accounting-103-baseline-empty.canonical");
}

```

### Additional candidate probes

The standalone /tmp/shortwin-independent-accounting-103 package has this manifest:

```toml
[package]
name = "independent-accounting-103-probes"
version = "0.0.0"
edition = "2024"
[dependencies]
shortwin-math = { path = "/root/short.win-audit-review/crates/shortwin-math" }
```

Run after the baseline state generator above:

```sh
cd /tmp/shortwin-independent-accounting-103
CARGO_TARGET_DIR=/root/short.win/target cargo test --offline --locked -- --nocapture
```

Final result: **5 passed**, four behavioral probes plus checksum, 0 failed/ignored.
An initial exploratory expectation that the ordinary math API allowed last borrowed-share withdrawal was disproved by InsufficientInventory and removed; it is not a candidate failure or bypass evidence.
Final executed src/lib.rs follows:

```rust
#[cfg(test)]
#[path = "/root/short.win-audit-review/crates/shortwin-math/tests/pool_harness.rs"]
mod pool_harness;

#[cfg(test)]
mod probes {
    use super::pool_harness::{Market, LOCK_GRACE_S, LUMP_VEST_S};
    use shortwin_math::{pool, num::WAD};

    #[test]
    fn historical_carry_goes_to_a_late_final_holder() {
        let mut m = Market::new();
        let a = m.deposit(0, 2_000_000_000_000_000_000).unwrap();
        m.sweep(1000).unwrap();
        m.now += i64::from(LUMP_VEST_S);
        m.release().unwrap();
        assert_eq!(m.pool.carry, 1000 * WAD as i128);
        let b = m.deposit(1, 1).unwrap();
        m.withdraw(0, a).unwrap();
        assert_eq!(m.claim(0), Ok(0));
        m.withdraw(1, b).unwrap();
        assert_eq!(m.claim(1), Ok(1000));
        assert_eq!((m.pool.carry, m.pool.carry_r), (0,0));
        println!("historical carry: A funded all shares during income, paid=0; B deposited 1 token after full release, final-holder paid=1000");
        m.invariants().unwrap();
    }

    fn repeated_touch(n: u32, split: bool) -> (u64,u64,u64,i128) {
        let mut m = Market::with_params(n, LOCK_GRACE_S);
        let a = m.deposit(0,1).unwrap();
        let b = m.deposit(1,1).unwrap();
        m.sweep(u64::from(n)).unwrap();
        for t in 1..=n {
            m.now = i64::from(t);
            m.release().unwrap();
            if split { m.touch_pot(0).unwrap(); }
        }
        m.withdraw(0,a).unwrap();
        let ap = m.claim(0).unwrap();
        m.withdraw(1,b).unwrap();
        let bp = m.claim(1).unwrap();
        assert_eq!((m.pool.carry,m.pool.carry_r), (0,0));
        m.invariants().unwrap();
        (ap,bp,m.vault.lender_sol,m.pool.dust)
    }

    #[test]
    fn repeated_touch_remains_outside_final_carry_fix() {
        for n in [2,1000,21_600] {
            let unsplit = repeated_touch(n,false);
            let split = repeated_touch(n,true);
            assert_eq!(unsplit, (u64::from(n/2),u64::from(n/2),0,0));
            assert_eq!(split,(0,u64::from(n/2),u64::from(n/2),0));
            println!("n={n}: one touch each {unsplit:?}; third-party touch A each release {split:?}; tuple=(A paid,B paid,lender_sol left,diagnostic dust)");
        }
    }


    #[test]
    fn legacy_empty_pool_carry_is_not_migrated_by_first_deposit() {
        let mut m = Market::new();
        let a = m.deposit(0, 2_000_000_000_000_000_000).unwrap();
        m.sweep(1000).unwrap();
        m.now += i64::from(LUMP_VEST_S);
        m.release().unwrap();
        let carry = m.pool.carry;
        let carry_r = m.pool.carry_r;
        // Simulate the pre-fix empty state using all existing fields:
        // old withdraw had the same token/share burn and did not assign/clear carries.
        m.withdraw(0,a).unwrap();
        m.lenders[0].1.pending = 0;
        m.lenders[0].1.real = 0;
        m.pool.carry = carry;
        m.pool.carry_r = carry_r;
        // Compare the entire reconstructed state against a real baseline public-API run.
        let legacy = std::fs::read_to_string("/tmp/shortwin-independent-accounting-103-baseline-empty.canonical").unwrap();
        assert_eq!(m.canonical(),legacy);
        assert_eq!((carry,carry_r), (1000 * WAD as i128,1000 * WAD));
        m.sweep(0).unwrap();
        let b = m.deposit(1,1).unwrap();
        assert_eq!((m.pool.carry,m.pool.carry_r), (carry,carry_r));
        m.sweep(1).unwrap();
        m.withdraw(1,b).unwrap();
        assert_eq!(m.claim(1),Ok(1001));
        assert_eq!(m.claim(0),Ok(0));
        println!("legacy empty-pool state: carry survives first candidate deposit; new cohort paid1001 including1000 historical carry");
        let _ = a;
        m.invariants().unwrap();
    }

    #[test]
    fn final_exit_pot_overflow_rolls_back_copies() {
        let mut m = Market::new();
        let a = m.deposit(0,2_000_000_000_000_000_000).unwrap();
        m.borrow(1,1).unwrap();
        m.accrue(0,3000).unwrap();
        let b = m.deposit(1,3_000_000_000_000_000_000).unwrap();
        m.repay(1,3000,0,3000,0).unwrap();
        m.withdraw(0,a).unwrap();
        m.vault.unmatched = u64::MAX;
        let before = m.clone();
        let result = pool::withdraw(&mut m.pool,&mut m.vault,&mut m.lenders[1].1,&mut m.records,m.now,m.lock_grace_s,b);
        assert_eq!(result,Err(shortwin_math::MathError::Overflow));
        assert_eq!(m,before);
        println!("mismatched final real carry causes to_pot overflow: all math inputs unchanged");
    }
}

```

No sustained 24-hour fuzz run, production inventory, complete release gate, or owner economic-policy acceptance was performed by this reviewer. Q2-Q4 are outside this accounting review.
