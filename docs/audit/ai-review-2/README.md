# AI review 2: short.win 1.0.1 (stopped after pass 1)

An AI-assisted security review of short.win 1.0.1, run on 2026-10-17 with the
[`rust-auditor`](https://github.com/TodorBonchev/rust-auditor) Claude Code skill and Claude Opus 5.5.

**This is not a formal audit.** The owner stopped the scan after its first full pass. The
review is therefore incomplete. This folder gives the auditors what it found, so that they can
continue from there.

## Status

| | |
|---|---|
| **Code** | short.win 1.0.1: this repository's `main` at `eb84737`. The scan read a copy at `e035f2e`; its 109 in-scope files are byte-identical to `eb84737`'s. The release build of this code is `shortwin.so` `72c82879…1429` ([AUDIT.md](../../AUDIT.md) §0, 1.0.1). |
| **Tool** | rust-auditor `6c41e1d` with Claude Opus 5.5. Options: 3 passes, memory on, `--poc`. |
| **Passes** | **1 of 3.** Pass 1 ran all 84 agents and was judged. Pass 2 stopped after 34 of 84 agents, and it was **not judged**. Pass 3 did not run. |
| **Result of pass 1** | 13 findings: 4 at confidence 90 or more, 8 at 75 to 89, 1 below 75. 85 leads. |
| **Proofs of concept** | For the 4 findings at confidence 90 or more: 3 CONFIRMED, 1 UNVERIFIED. |
| **Triage** | Broader targeted triage of all 85 leads and a 330-block/74-key pass-2 register are in [`../findings.md`](../findings.md) and the evidence files below (2026-10-18). Draft 1.0.2 fixes and new reproductions are recorded separately; open claims and release gates remain. Comparison sources: [`known-issues.md`](../known-issues.md) and [`ai-review/`](../ai-review/README.md). Finding 6 is the existing accepted KI-70. |

## Files

| File | Contents |
|---|---|
| [`report.md`](report.md) | The skill's report, byte for byte as its assembler wrote it: the scope, the 13 findings with their fixes, and the 85 leads |
| [`poc/`](poc/) | The proof-of-concept tests and the record of each run |
| [`pass-2-unjudged.md`](pass-2-unjudged.md) | The raw reports of pass 2's 34 agents. Nothing in it was deduplicated or judged. |
| [`triage-math-2026-10-18.md`](triage-math-2026-10-18.md) | Accounting and arithmetic reproductions, draft fixes, rejected patches and unresolved economics |
| [`triage-venues-2026-10-18.md`](triage-venues-2026-10-18.md) | Venue/program source review, real cashback reproduction, migration handling and remaining questions |
| [`triage-services-2026-10-18.md`](triage-services-2026-10-18.md) | Keeper, ALT, governance preflight and event/indexer integration |
| [`triage-pass2-2026-10-18.md`](triage-pass2-2026-10-18.md) | Every pass-2 block mapped to an exact key and current disposition |
| [`account-map.md`](account-map.md) | The skill's account map of the program: each instruction's accounts and constraints, and its review leads |

## Findings

The following list preserves the original scan result; current dispositions and regressions are in the triage files above. The list is from [`report.md`](report.md), which has the description and the fix of each.
Confidence is the judge's confidence that the attack path works. It is not a severity. The
report marks every item "NEW", because this was the first scan with the skill's memory on. "NEW"
does not mean that nobody reported the item before.

| # | Confidence | Title | Function | PoC |
|---|---|---|---|---|
| 1 | 90 | A rebase gives a locked lender a share worth all free tokens | `shortwin-math::convert_steps` | UNVERIFIED |
| 2 | 90 | open_short does not check the age of P_ema. | `shortwin::open_short` | CONFIRMED |
| 3 | 90 | The PumpSwap price start keeps P_ema at a pushed-down spot | `shortwin::run` | CONFIRMED |
| 4 | 90 | A deposit after a halted-venue S3 takes the old lenders' vested SOL | `shortwin::sol_settle` | CONFIRMED |
| 5 | 80 | After a dust flush a depositor takes the pre-wipe cashback | `shortwin-math::settle` | |
| 6 | 75 | apply_config installs a new governance key that never signed. | `shortwin::apply_config` | |
| 7 | 75 | Twelve-byte ELF fingerprint lets a venue hide an upgrade | `shortwin::flag_venue_upgraded` | |
| 8 | 75 | A clock armed before a venue restart stays live after the warm-up. | `shortwin-math::evaluate` | |
| 9 | 75 | The whole-bps floor of λ lets a deposit lower locked_depth. | `shortwin-math::locked_depth` | |
| 10 | 75 | A keeper raises the PumpSwap risk fee in its own liquidation transaction. | `shortwin::read_state` | |
| 11 | 75 | Correctness: sol_settle tests S1 at a lower fee than liquidate's buyback | `shortwin::sol_settle` | |
| 12 | 75 | Any caller redirects the PumpSwap cashback to a WSOL account nobody sweeps | `shortwin::sweep_trader` | |
| 13 | 55 | Liquidation pays the keeper nothing when rent takes the whole remainder. | `shortwin-math::liquidate` | |

One overlap is known: finding 6 restates KI-70 (AIR-04), which the owner accepted on
2026-10-12. The other findings have an initial comparison with the register in
[`../findings.md`](../findings.md); their open claims still need adjudication.

## Proofs of concept

The skill runs a proof of concept (PoC) on each finding at confidence 90 or more. One agent
wrote a LiteSVM test against the built program. A second agent then ran the test again and
checked that it models the attack faithfully. Each test asserts the safe behaviour that the
finding's fix restores. So it fails on 1.0.1 when the bug is real, and it passes after the fix.

| # | Label | Test | What the run shows |
|---|---|---|---|
| 1 | UNVERIFIED | — | No test ran. A safety classifier stopped the PoC agent before it wrote a test. The reviewer read `pool.rs:1243-1248` and saw the rounding mismatch that the finding describes, but reading the code is not a PoC. |
| 2 | CONFIRMED | [`poc_02_open_short.rs`](poc/02_open_short/) | `open_short` accepts a 60-second-old `P_ema`. The position then holds 9.05 SOL, but the borrowed tokens cost 27.35 SOL at the live price, so the lenders can lose 18.3 SOL. The test runs on the test-venue build with the mock venue; the missing age check is in the venue-independent open guard. |
| 3 | CONFIRMED | [`poc_03_run.rs`](poc/03_run/) | On the dumped mainnet PumpSwap pool, one transaction (sell, `crank_price`, buy back) starts the price at a pushed-down spot. After the eight warm-up cranks, `P_ema` is still that spot: 11 % of the pool's price. The test stops at the open gate, so the lenders' loss is derived, not measured. |
| 4 | CONFIRMED | [`poc_04_sol_settle.rs`](poc/04_sol_settle/) | On the release build, after an S3 on a Raydium pool that its admin halted, a plain deposit gets its tokens back and takes 5.87 of the old lender's 11.71 vested SOL. |

Each folder under [`poc/`](poc/) has a `result.md`: the run's output, the PoC agent's account
of how each step maps to the attack, and the reviewer's verdict.

To run a test:

1. Check out `eb84737`, or any commit with the same program sources.
2. Build the programs as [`tests/litesvm/README.md`](../../../tests/litesvm/README.md) says:
   `anchor build --tools-version v1.57 --arch v3 --ignore-keys` (as CI runs it) and
   `scripts/build-test-venue.sh`.
3. Copy the test into `tests/litesvm/tests/`, and run it from the repository root, for example:
   `cargo test -p shortwin-litesvm-tests --test poc_04_sol_settle -- --nocapture`.

The immutable PoC copies fail on audited 1.0.1. Working-tree regressions now live in
`tests/litesvm/tests/ai2_*_it.rs` and pass with the draft fixes; release gates remain open.

## How the scan worked

rust-auditor runs 12 specialist agents. Nine of them each read the code from one angle: math
precision, access control, economic security, execution trace, invariants, periphery, first
principles, asymmetry and account validation. Three gap hunters look at numbers, trust and
flow. A judge step merges duplicates. It then tests each candidate against four gates:
execution, reachability, trigger and impact. It scores the candidates that remain from 1 to
100. A finding at 75 or more gets a fix. A trail that the agents could not complete is a lead.

This scan changed the skill's procedure as follows:

1. **Scope.** Filename mode with 109 files: `programs/shortwin/src` (71 files),
   `crates/shortwin-math/src` (25), `crates/shortwin-venue-state/src` (11), and the keeper's
   two deploy tools `services/keeper/src/bin/build_alt.rs` and `v1_probe.rs`. The keeper core,
   which [AUDIT.md](../../AUDIT.md) §2 has in scope, was not scanned.
2. **Slices.** The 2 MB of source does not fit in one agent's context. The scan split it into
   seven slices: lending, positions, settlement, pump venues, Raydium venues, governance and
   markets. Each slice got all 12 specialists, so one pass ran 84 agents. Each agent could read
   every other in-scope file.
3. **Judging.** Judge agents did the dedup, the four gates and the lead promotion, in batches
   of up to five (program, function) groups. One more agent did the folds across functions.
   The skill's shell steps ran as written: the run file, the ledger, and the assembler that
   writes [`report.md`](report.md).
4. **Account map.** The skill's map generator left 54 helper locations to complete by hand.
   Four agents completed them; their notes are at the end of [`account-map.md`](account-map.md).
5. **Proofs of concept.** The skill runs the PoC step on the final pass. Pass 1 became the final
   pass, so the PoCs ran on its findings. They ran in a scratch copy of this repository, because
   that copy has the pump fixtures. The tests use the repository's LiteSVM harness, not a
   separate crate. A second agent ran each test again and checked it.

All agents only read the code that they reviewed. All builds and tests ran in separate copies.

## What remains

- **Pass 2 was not judged.** [`pass-2-unjudged.md`](pass-2-unjudged.md) has its 330 raw
  blocks. 50 of them have keys that pass 1 did not record, and 3 are FINDINGs on keys that pass
  1 judged as leads. Pass 2 covered the lending and positions slices, and 9 of 12 specialists on
  settlement. It did not reach the pump venues, the Raydium venues, governance or markets.
- **Pass 3 did not run.**
- **Finding 1 lacked an original PoC.** The follow-up reproduces and fixes the lock-rounding
  mismatch; the original all-free-inventory magnitude remains unverified.
- **Broader triage is recorded.** All 85 leads and 330 pass-2 blocks have dispositions and
  evidence in [`../findings.md`](../findings.md). Open claims still need the specified
  reproduction or design decision; targeted follow-up is not an independent full audit.
- **Fixes.** The changes in [`report.md`](report.md) are proposals. The broader draft 1.0.2
  batch contains verified targeted fixes and explicit unresolved findings; it does not blindly
  apply the report's patches. Under the audit freeze, a program fix goes out in a 1.0.x fix batch
  ([AUDIT.md](../../AUDIT.md) §12).

## The earlier AI review

[`ai-review/`](../ai-review/README.md) reviewed v1.0.0 (`55b3e23`) with the same skill:
AIR-01 … AIR-24. Its fixes are in 1.0.1, so this scan read them too.
