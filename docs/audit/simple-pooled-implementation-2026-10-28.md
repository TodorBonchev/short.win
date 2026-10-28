# Jupiter-only pooled implementation checkpoint — 2026-10-28

Status: initial implementation and focused validation on `f4377093e`, **not mainnet release approval**. The subsequent [build-isolation follow-up](simple-pooled-reproducibility-2026-10-28.md) supersedes this checkpoint's runtime identity for the final candidate. Branch `codex/jupiter-only-pooled` replaces the old financial program with the owner-selected [pooled model](../../simple/docs/SPEC.md). The 2026-10-28 owner choice is a fixed upfront SOL term fee, without an early-close refund. No legacy state migration is required because the owner reports that the program was never deployed.

## Implemented scope

A separate standalone workspace implements public eligible-mint registration, per-token pool generations and lender receipts; paid SOL income; fixed-fee whole-token shorts through Jupiter; direct full repayment, margin addition, borrower buyback, confirmed CR liquidation, expiry buyback and deadline-plus-24h default. Default sends all economic collateral less the capped keeper fee to lenders; borrower rent/untracked donations are separate. The old accrued-rent/vesting/venue/graduation machinery is not imported.

The generated client and small browser interface use the new IDL. Final strict TypeScript/client/operator/bridge checks (19 tests), browser logic checks (9 tests), 13 Python service tests, frozen-lockfile installation, production web build and regeneration consistency passed. The in-house SQLite history service, bounded unsigned HTTP bridge and Node-only operator implement exact-quantity warm-up/publication, finalized event retention, keeper planning and explicit opt-in submission. Local/mock provider checks are not evidence of provisioned roles, payer funding, API throughput, live wallets or delivered monitoring alerts. The operator is dry-run by default.

## Evidence and limits

The initial local SBF v3 ELF SHA256 is `49f11f900aa9a368638dcb6a624685a85d59909c805086403ae6765bd539a54a` (770,800 bytes). The generated IDL SHA256 is `83748440336bcfeb41bcb37ae6d1f7083df3341ab8e0b9199655ca5a436cc60c`. Explicit platform-tools v1.57/arch v3 compilation passed the stack check. This is local build evidence; a clean independent/verifiable release build remains part of final qualification.

The [retained evidence directory](simple-pooled-evidence-2026-10-28/) contains exact logs and JSON receipts. Final Rust formatting, strict workspace Clippy, locked workspace tests and the shared Router V2 parser's tests passed. The workspace tests include 16 ledger unit tests plus a randomized model regression, 8 program unit tests and 22 actual-SBF runtime tests; the stateless parser has 13 additional tests. Runtime tests exercise both plain token programs for lender flows and the frozen real Jupiter-to-CPMM route for trading. Historical state, initial lender allocations, test lookup table, clocks and selected reserve shocks are explicitly synthetic. This does not establish every DEX/mint, current mainnet dependency compatibility or live landing.

A 200,000-transition pure-core model smoke passed separately. The new **actual-program smoke** completed 32 cases in 22.048472413 seconds: 147 driver transactions (98 success, 49 expected rejection), 65 actual CPMM success log observations, and all eight reported scenario families. Setup transactions are not included in those transaction counts. Failures, anomalies, setup failures and invariant failures were all zero.

Its fixed metadata SHA256 is `4e2a82f62f7a82586a42bcd4751d05732ad6045fd66bc16f1c50fc44b8806667`; summary SHA256 `bb364ad727a8d579f428969cb4defcdbff6b851beb6d8d71cef1d5165200ccd7`; exact log SHA256 `e776e31f02b6917706081048d0548fa5f698c99564de2dee1ec56e23a1ff00b9`. Frozen native runner, program, 342 artifact files and 37 source files were independently checked locally; they remain under the run's ignored local frozen directory. This short smoke is **not 24-hour evidence**, has no equal-duration-per-family claim, and inherits no duration from the previous program. The completion and summary share the runner's timer; a future qualifying campaign also needs independent start/process identity and an externally retained initial metadata anchor.

## Read-only API envelope follow-up

Two keyless HTTP-only schema probes on 2026-10-28 exposed the distinction between Jupiter's route-account budget and the full instruction envelope: a `maxAccounts=48` request returned 55 total accounts, while `maxAccounts=32` returned 25. The live probes used USDC only as an API transport fixture; its freeze authority is outside this program's mint eligibility. There was no protocol simulation, wallet signature, chain submission or current-ELF compatibility claim. Exact raw responses, timestamps and hashes are retained beside the receipts.

The service and SDK now request a 32-account routing budget while retaining the program's hard 48-total-account and 768-byte limits. Oversized responses remain unavailable instead of bypassing limits. The routing profile is included in the trusted service model hash; the development example hash is `496c0f963b0862c08c771aafcb1a256cc6bd2f7b561a7aa278670966b9a68637`. The service and client regressions cover the request budget, strict envelope refusal and hash drift. This changes no on-chain program, fuzzer or fixture bytes, and the short program smoke remains accurately attributed to the identical runtime. Final whole-system/provider qualification is still pending.

## Peer review

Separate AI-agent passes within the same development session reviewed the financial, account, operator, frontend and freeze contracts. This is an AI-assisted engineering peer review, not an external firm report or accreditation. Concrete issues corrected during implementation included delayed source-clock confirmation, reporter-epoch arm invalidation, debt-normalized actual-fill liquidation checks, operator publication/queue/rotation starvation, clean-CI fixture materialization, complete frozen dependency sources, archived-runner path handling and cancellation/account-change guards across quote, simulation, wallet reply and pre-broadcast checks. Regressions cover the corrected behavior. Historical finding statuses and archived review checksums are preserved.

## Open release work

- Review concrete financial settings, role identities, exact current Jupiter/token-program compatibility and additional current route families; retain reproducible final-source program bytes.
- Run a fresh independently supervised >=24h campaign against frozen final inputs; require readable zero-anomaly/zero-failure evidence and actual scenario coverage. No long run has started here.
- Commission the operator, HTTP service and web against one network/program identity with real funding, request/rent/fee budgets, tested alert delivery and measured coverage. The low-cost example has bounded exact-quantity capacity; it does not promise instant coverage of all mints or unlimited simultaneous quantities.
- Review the concrete release/tag/deployment plan after those receipts. A distinct version line (proposed 1.1.0) fits the incompatible accounts and economics; no stable 1.0.3/1.0.4 tag is created for this replacement.

The owner's mainnet-before-external-audit/funded-bounty policy remains unchanged. Future fee revenue is not represented as a funded audit or bounty, and lender assets are not designated for those expenses. No deployment, live transaction, public service, purchase or stable tag was made by this implementation.
