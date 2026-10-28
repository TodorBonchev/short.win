# In-house exact-quantity report state

This dependency-free `no_std` crate is a **financially unwired prerequisite**, not a production oracle or mainnet approval. It implements structural report acceptance, authorization policy, bounded history and a canonical state codec. It performs no HTTP requests, Solana account-boundary authentication, signature verification, transaction execution, financial health calculation or settlement. No production risk settings, keys or deployment are selected here.

## Selected trusted-history mode (version 2)

The owner explicitly accepted off-chain VPS history and on-demand authenticated
snapshots. `trusted_snapshot::SnapshotState` uses a distinct `SWPSNAP2` codec
(1,533 bytes), account namespace/version and `SnapshotReport`; it never decodes as
version-1 public history. The on-chain wrapper is `PriceSnapshot` (1,624 bytes, including the immutable per-feed producer commitment).
An allowed runtime Signer supplies live marks and up to eight chronological
completed private-history buckets. No previous on-chain updates or periodic
publication are required. This is deliberately **operator-attested history**:
structural checks cannot prove past sample truth, collection time or omitted gaps.

The source epoch anchors the completed history; a still-fresh snapshot can land
or be read in a later runtime epoch. `evidence_at(now, minimum_sequence)` requires
latest sequence at least the caller's bound, actual runtime freshness in both
slots/seconds, finite marks and configured warm history. Replays, future/stale
windows, wrong bindings, noncanonical storage and Unknown are refused or invalidate
finite use. `Unknown` clears history. Publisher/model policy remains immutable in
the wrapper; safe rotation and actual outstanding-debt hooks remain integration
requirements. A zero counter in an unwired wrapper proves no real debt absence.

Sections below describe the retained version-1 public-history prototype. Its
same-runtime-epoch publication and on-chain cadence requirements are not the active
version-2 transport. Neither mode itself enables financial admission or defaults.

## Scope and trust boundary

Each `ReportState` binds one exact positive debt quantity `q` to the chain genesis, program, configuration account, market, base/quote mints, token-program identities, decimals, model/version/policy commitment and configuration epoch. The quantity and domain never change through reconfiguration. Use separately derived, independently validated state for each required quantity; this does not impose a fixed lot size or certify aggregate market liquidity, concurrent liquidation capacity or exposure limits.

`CallerAuthenticatedSigner::from_verified_runtime_signer` is a named caller boundary, not cryptographic authentication. Before constructing it, the production instruction must verify the actual runtime account's signer flag and public key. The instruction must also authenticate account ownership/PDA and trusted configuration/governance, obtain the actual runtime clock, validate mint/token-program/decimals and canonical quote-native units, and reject unintended accounts or report payloads. This crate cannot discover whether any supplied identity is the real network, program, market or mint.

The checked publisher list admits alternative individual writers. It does **not** implement a quorum or a claim of source independence. An accepted signed report remains an assertion by a trusted project operator; signatures, a nonzero observation digest and ordered cost marks do not prove fair prices, best routing, executable liquidity or resistance to source manipulation. Operational separation, key custody/rotation/revocation, raw-source retention, independent monitoring and a reviewed source/search policy remain required.

The `policy_hash` and `observation_digest` are nonzero commitments supplied by the caller. This crate neither computes them nor defines canonical encodings. The production producer and program must agree on a versioned canonical encoding binding all settings, raw evidence identities and observations. Reusing a digest does not establish independent observations. `Domain` checks nonzero identities and distinct base/quote mints only; mint safety and token-program allowlists belong to admission.

## Accepted reports

The initial status is `Unknown`. Version 1 accepts only:

- `Finite(CostMarks)`, structurally requiring `0 < lower <= central <= upper` in quote-native raw units for the exact `q`;
- `Unknown`, which clears history and prevents finite risk evidence until recovery and warmup.

There is no caller-supplied `liquidatable` flag, signed no-route assertion, infinity sentinel or cash/default trigger. API errors, transport failures, rate limits and unsupported routes cannot be promoted into no-route or S3 authorization through this API. A producer with only a feasible route upper bound lacks the lower/central information needed for `Finite` and must not invent it.

Each update binds the entire expected domain, quantity, model and configuration epoch. It requires an authorized authenticated signer matching the payload publisher and the exact next sequence number. Runtime clocks cannot regress, source observation end slots strictly increase, source end time cannot regress and arithmetic does not wrap. All fallible acceptance operations complete before mutation; any returned error preserves the full state, including sequence, prices, history and outstanding count.

Source windows must be ordered, bounded, not in the future, no earlier than configuration and fresh from their **earliest** observation in both slots and seconds. Signing time alone cannot make old observations fresh. Both source endpoints must belong to the epoch derived from the current runtime clock, so a new signature or sequence cannot backfill historical epochs. These checks constrain asserted metadata; source truth still depends on the authorized reporter and its retained evidence.

## Completed history and outages

All `HistoryPolicy` settings are explicit caller-reviewed configuration. The constants of eight publishers and eight history buckets are resource ceilings, not recommended quorum, cadence, duration, risk or borrowing settings. A state can retain only the most recent eight consecutive completed buckets.

An epoch can contribute history only if configuration predates its start and observation/publication begins within the configured time allowance from that start. Both source-end and actual-publication gaps must remain within configured slot and time cadence. A prior epoch becomes completed only when a timely observation in the next adjacent epoch demonstrates its timely end and continuity. The still-open current bucket is never exposed as completed history.

Missing cadence or an epoch resets the completed prefix. An `Unknown` sample clears completed history, cancels any pending completion at the rollover and taints that whole epoch; later finite samples in the same epoch do not repair its historical qualification. Reconfiguration also clears current prices and history. These rules intentionally require a new consecutive warmup after an outage or rotation instead of silently borrowing old history.

`CompletedBucket` exposes raw lower minimum, last central, upper maximum, first/last source observations and sample count. Those statistics do not select an EMA, median, historical economic reference, collateral ratio, liquidation threshold or payout rule. In particular, a bucket minimum does not prove a lower economic bound. The production program must calculate reviewed health and action bounds from the qualified observations/history; a reporter cannot authorize a liquidation by asserting it.

`risk_evidence_at` requires current finite freshness, current cadence and the configured count of completed epochs. Its borrowed view rechecks those conditions on each data access at the supplied actual-use clock; a numerically fresh quote alone does not preserve historical continuity after cadence expiry. `live_marks_at` alone establishes neither completed history nor permission for any risk-increasing action. The caller must use the appropriate qualified path for every action and must not use a cached `CostMarks` value after its eligibility expires.

## Debt lifecycle and configuration

`register_outstanding` and `release_outstanding` are production-owned internal accounting hooks, not reporter powers. They must be paired atomically with a real admitted open and actual full repayment or separately authorized default debt accounting. Admission, position ownership, full-quantity accounting, borrowing caps and any cash/default eligibility remain outside the crate. The per-quantity count is not an aggregate exposure or liquidity certificate. Account closure must be refused unless `can_retire()` and the independently checked real positions agree that no obligations remain.

Configuration epochs advance exactly once, and sequence numbers remain continuous across reconfiguration. Domain and `q` remain immutable even after debt reaches zero; use a separately identified state for a different binding. While outstanding debt is registered, model or temporal/history policy changes are refused. When debt is zero, a changed policy requires a model change and an exactly incremented model epoch. Production policy commitments must include every locked economic/search setting, not merely temporal settings stored here.

Governance can rotate publishers while debt remains. This preserves quantity, model, policy and outstanding count, but clears prices/history and starts a new warmup. That creates an explicit report outage: new risk and ordinary report-dependent liquidation cannot continue as if history survived. Production integration must preserve separately authorized safe token repayment, margin addition and other qualified risk-reducing paths throughout this interval; this crate does not implement them or decide default/SOL settlement. Rotation must not become an implicit economic-policy change for existing positions.

## Canonical persistence and the unadmitted account boundary

`codec::STATE_BYTES` is exactly 1,488 bytes. Version 1 begins with the eight-byte `SWPRSTAT` magic and a little-endian u16 codec version; all fixed integer fields use little endian. Option/status tags are 0/1, inactive payloads and unused publisher/history slots must be zero, and history is serialized oldest to newest with a canonical ring reconstruction. Rust object layout and physical stale ring entries are never serialized. Decoding refuses nonexact lengths (including appended bytes), unsupported versions, invalid tags/padding, malformed configuration and inconsistent clocks, sequence, pending/live or completed-history state. This is structural persistence validation, not authentication of source truth or historical signatures.

The initial program wrapper exposes `initialize_price_report` (actual GlobalConfig governance signer) and `publish_price_report` (actual runtime publisher Signer plus Clock). Its `PriceReport` account is 1,547 bytes including discriminator, with a version/bump/market/quantity/model-epoch prefix and the fixed codec payload. PDA seeds are `price_report`, market, quantity u64 LE and model epoch u64 LE. The wrapper compares every restored binding with its real program/config/market identity, canonical WSOL/SPL Token/9 quote units and stored base mint/program/decimals; a publish payload also carries the full domain/model/quantity/epoch/sequence/window/digest/status and asserted publisher. `PriceReport::restore(&self, ...)` validates decoded bindings but cannot check the outer account data length; every future financial consumer must independently require the actual 1,547-byte account length, owner and PDA, as the publish instruction does. Governance supplies the nonzero genesis hash; Solana exposes no runtime genesis-hash sysvar, so an external operator check must verify that configured value against the actual network.

Initialization accepts explicit policy/model settings and at most eight publishers through a fixed array plus checked count. These are testable structural parameters, not approved mainnet risk settings. Publication neither mutates Market nor grants open, liquidation, S3, cash-settlement or borrowing authority. No report event is emitted in this phase; confirmed account/instruction evidence is the persistence source. The wrapper exposes **no reconfigure, rotation, close or retirement instruction**, because real debt hooks and admission are not wired. The stored outstanding count's initial zero is not evidence that a market has no real positions. Creating another unused model-epoch account does not migrate existing financial obligations.

Actual account size, rent, compute, stack, BPF/SBF behavior and transaction-size limits must qualify alongside program integration. In particular, the first isolated SBF build exposed a 5,312-byte decoder frame despite returning exit zero; the diagnostic was retained and the decoder was split into bounded frames. An exit-zero compiler result with any stack diagnostic cannot qualify an artifact.

The parent workspace can add `crates/shortwin-price-report` to its Cargo members and first-party lock entries after review, then add its package tests to relevant CI and classify the new source in the audit scope. The crate itself has no third-party dependencies. The first persistence instructions enforce runtime signer/clock/account/governance checks. Financial admission and every risk handler still need canonical policy/evidence commitments, authoritative debt lifecycle hooks, reviewed key-rotation/outage behavior, source policy and financial/settlement logic with end-to-end adversarial tests. The first wrapper deliberately has no reconfiguration/close paths. No existing fuzz duration qualifies changed program bytes.

Host verification can run without resolving the root workspace:

```sh
rustfmt --edition 2024 --check crates/shortwin-price-report/src/lib.rs
rustc --edition=2024 --test -D warnings crates/shortwin-price-report/src/lib.rs -o target/price-report-unit-tests
target/price-report-unit-tests
rustc --edition=2024 --crate-name shortwin_price_report --crate-type lib -D warnings crates/shortwin-price-report/src/lib.rs -o target/libshortwin_price_report.rlib
```

The focused unit tests also cover canonical persistence roundtrips, wrapped history, corruption/trailing/tag/padding refusals and full binding/authentication-policy and stale/window/sequence refusal, failure atomicity, current-only source epochs, completion and cadence, Unknown taint, ring retention, debt-preserving publisher rotation and policy locks, and integer boundaries. An independent epoch model checks all 4,096 finite/Unknown patterns of twelve consecutive updates (49,152 accepted updates, each canonically encoded, restored and reencoded). These are state-policy tests, not proof of source honesty, on-chain authorization or production financial safety. Numeric policy values and simulated slot rates in tests are fixtures only.
