# Operator and release compatibility review - 2026-10-22 local

Prepared by Codex agent remote_scope_check as a read-only review of available operator evidence and retained CI artifacts. This is AI-assisted evidence, not an external firm's audit, deployment approval or proof that an operator is funded. The reviewer previously implemented the Rust full-digest VenueGuard change; the client/browser and indexer checks below are compatibility observations, not a new independent audit of that implementation.

## Result

Code compatibility is supported by the green exact-source CI and the actual retained IDL artifact. Actual operator readiness and running deployment identities are not established. The current live check fails solely for Jupiter; that failure must remain visible and receive an explicit scoped disposition before deployment. No tag, deployment, transaction, fund movement or service start was performed.

The review first read current TODO.md, AUDIT.md section 12, the readiness record, findings tracker and owner-decision record. These were still at the pre-completion documentation checkpoint when read; the coordinating agent is updating final fuzz evidence separately. This report does not independently accept or re-compute the 24-hour gate.

## Available operational evidence

A filename-only inventory of the review and primary checkouts found only the tracked keeper.example.toml, indexer.example.toml, reference deploy/indexer.toml, governance.example.toml and web .env.example. The targeted top-level /root operator-config filename search found no additional config. Untracked configuration contents, non-example .env contents, private key files, process command lines, process environments and database credentials were not read.

No executable named shortwin-keeper, shortwin-indexer or shortwin-gov was found running in the current WSL process namespace. A Windows process-name check found none of those names. The WSL default metrics/API ports 9464, 8080 and 8081 were not listening. These are local, bounded observations; other hosts, renamed processes, nondefault ports or inaccessible container namespaces are not ruled out. Docker was not available on the inspected WSL PATH.

| Requirement | Evidence available | Status |
|---|---|---|
| Real keeper configuration and covered markets | Reference keeper config says markets = "all" and dry_run = false. It is an example, not a running operator's selected config. | Unverified |
| Payer funding and top-up plan | No real payer public key, dated balance, funding receipts, top-up records, budget or responsible operator was supplied. Test/fixture keys are not operational identities. No guessed balance query was made. | Unverified |
| Monitoring and alert delivery | Source supports optional --metrics and log alerts. No actual scrape target, alert receiver, acknowledged alert drill, escalation rota or uptime evidence was available. Logs emitted by code do not prove a pager receives them. | Unverified |
| Running indexer/client identities | Reference compose and Dockerfile are present; CI parses the reference deployment. No deployed image digest, running binary hash, public deployment URL/build revision or client bundle identity was supplied. | Unverified |
| Zero-reward exits | Owner-selected rent-first policy requires a funded and monitored backstop. Existing keeper code attempts eligible exits without a profit test; that code behavior does not establish service operation or funding. | Prerequisite remains |

No new real operator was created or configured during this review. The owner's never-deployed statement is treated as the recorded owner statement, not as an independently completed chain inventory.

## Compatibility tied to green CI ff85a1b0

Retained CI 39509811932 has completed success on ff85a1b0c8452cb9fad32767f7083d22377e90a3: 25 successful jobs and one intentional scheduled-fuzz skip. This review inspected the recorded exact-head job and step results and the workflow commands, rather than rerunning unchanged full suites.

- Codama job 118929925978: successful IDL-copy check, generated-client diff check, client typecheck and unfiltered client tests. The client layout regression checks 304 bytes, all four 32-byte digests through byte 303, suffix changes outside the former prefix and refusal of a truncated 224-byte account.
- Web job 118928174736: successful typecheck, unfiltered browser tests and production build. The browser guard regression reads the current304 account, retains all digest suffixes/status fields and refuses truncated/foreign data.
- Indexer job 118929926212: successful complete indexer test step, replay harness and validator ingest/finality step. services/indexer/src/ingest/project.rs maps both destination 0 and 3 to lender income. programs/shortwin/src/events.rs defines SWEEP_TO_LENDERS = 3. The existing security_ingest regression a_dust_flush_sweep_credits_lenders_instead_of_protocol_fees checks real writer projections, and the committed-session decoder/replay tests cover current account/event layouts.
- The actual downloaded shortwin-program.zip contains idl/shortwin.json. Its bytes equal clients/js/idl/shortwin.json exactly, SHA-256 5b94b1e98f5b32cf6d3c501d1998965bbbab80d6e2853fa6eeb8f3ba5470a003. VenueGuard's guard_elf field is [[u8; 32]; 4]. The ZIP itself matches its retained declared SHA-256.
- The relevant program, generated-client, browser, indexer and workflow files were compared with ff85 and have no differences. Later audit documentation does not extend the identity of the tested code. Frozen e04a6776 source remains separately linked to ff85 by the retained source-equivalence evidence.

The companion operator-compatibility-evidence.json records exact inspected paths, hashes and job/step results. Existing green CI is sufficient to reconfirm these unchanged code compatibility claims; no redundant full suite or production service run was executed here. Deployment must still identify which binaries/bundles actually run.

## Jupiter-only live drift

The coordinating agent's retained read-only preflight log, target/monitor-1.0.3-release/preflight/fixture-live.log, observes context slot 459802698 and reports Jupiter deploy slot 459479624 versus the pinned reference 454465850. Every other pinned program/account is reported unchanged, including pump-fees. This review read and hashed that log; it did not perform or rewrite the live query.

Jupiter is reserved and watched, but there is no current Jupiter execution adapter or VenueKind:

- state/market.rs defines current real venues None, PumpCurve, PumpMigrating, PumpSwap, RaydiumV4 and RaydiumCpmm.
- constants.rs current guard masks are 0b00111 for curve, 0b00110 for PumpSwap and 0b11000 for Raydium. None includes guard index 5 (Jupiter).
- engine/guard.rs uses these masks for opens and current pool-selection candidates. The current venues module has no Jupiter adapter.
- The keeper still watches all six VenueGuard programs and can flag Jupiter. Governance recognizes a future Jupiter enable bit, even though no current market maps to it. The reference governance venue list enables only the four supported venues.
- services/README.md already discloses that Jupiter has no fixture, pinned ELF hash or fork suite, and that its latch blocks no current market. Its code identity and hash-path resource maximum have not been reviewed here.

The new Jupiter slot therefore does not by itself demonstrate a change to the tested supported trade paths. It also does not turn the aggregate live check green or establish Jupiter compatibility. Keep the failed check and its reason in the release record. Do not silently change frozen fixture/baseline constants, suppress Jupiter in the checker or describe this as an all-program pass. Enabling or adopting a future Jupiter route requires its own reviewed bytes, interface/resource checks and supported-path tests.

For first initialization, services/ops/src/cli.rs checks all six explicitly configured guard slots against live ProgramData and refuses a mismatch. Stale example slots must not be copied into a launch config. The on-chain initialize_config handler itself stores the supplied guard_slot array; it does not perform that off-chain comparison. A review of the actual launch layout and its observed inactive Jupiter slot is therefore still required; omitting configured slots merely to bypass the mismatch is not a substitute for that review.

Pump-fees is reported unchanged in the live log. The retained R-PFEE analysis applies only while those bytes remain identical. Any new pump-fees ELF requires renewed withdraw-account/authority review; no new R-PFEE acceptance is manufactured by this report.

## Concrete first-deployment prerequisites and tag plan

1. Finish and independently retain the final fuzz acceptance, final source/byte equivalence and audit-record updates. Preserve economic acknowledgments and open/deferred lead scopes; no official external-firm review is claimed.
2. Resolve and record the Jupiter-only failed live preflight explicitly, then repeat the read-only check close to actual deployment. Do not promote a new active dependency without the venue-upgrade review policy. No Jupiter baseline/source change was made here.
3. Obtain the real operator host/process and selected non-secret config, covered-market policy, public payer key, dated public balance and funding/top-up budget/ownership evidence. Review the operator's actual binary/image digest against the accepted source. Do not copy a key file into audit evidence.
4. Verify the actual metrics scrape, alert receiver, receipt/acknowledgment of a safe alert drill, escalation ownership and funded backstop for uneconomic exits. A dry run or synthetic test alone cannot close this requirement.
5. Identify intended indexer image/binary and browser/client bundle revisions. Confirm destination 3 and current304 readers are the deployed versions before opening the protocol to users. Compatibility tests do not establish this rollout.
6. Prepare a reviewable tag/deployment plan naming the final commit, unchanged reviewed runtime artifacts, exact hashes, governance layout/authority and current slot choices, client/indexer order and operator evidence. The explicit owner record requires final review before tagging/deployment; this report does not authorize either action.

Only target/monitor-1.0.3-release/operator-compatibility-review.md and its companion JSON were written for this review. No tracked file, program, fuzzer input, fixture, source policy or deployed state was modified.
