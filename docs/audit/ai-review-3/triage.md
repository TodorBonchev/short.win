# AI review 3: implementation triage

This index assigns stable IDs to the eight findings and 23 leads in the immutable
[`report.md`](report.md). IDs follow its finding and lead order. Canonical keys and
project relationships come from [`project-issue-comparison.json`](project-issue-comparison.json),
which maps each key to raw candidate IDs, prior records and comparison reasons.
All eight findings have confidence **75**, which is not severity. Leads are unscored.

**Draft 1.0.3 triage (2026-10-18).** The immutable archive records the scan of
`b815269`; this register records subsequent decisions. Implementation and validation
evidence is in [`../findings.md`](../findings.md#ai-review-3-draft-103-2026-10-18).
“In progress” means an implemented correction whose required release gates remain open.
The [current readiness record](../release-readiness-1.0.3-2026-10-21.md) links the
owner-authorized, accurately attributed AI review route and current CI/fuzz status;
no external-firm acceptance is claimed.
An audit patch passing `git apply --check` is not an implemented or tested fix.

Relationships describe only the comparison with records predating this scan:

- **Exact known** (21): the same causal mechanism appears in a precise prior record. This
  does not mean accepted, fixed or runtime-confirmed; some matches are earlier raw leads.
- **Related, distinct** (7): a prior issue is related, but a different component, trigger or
  remaining boundary needs its own disposition.
- **New to register** (3): no exact prior mechanism was found in the inspected records.
  This does not prove novelty beyond those records or establish exploitability.

The report's `NEW` label means absence from the initially empty rust-auditor ledger.
It does not replace these project relationships. Prior issue IDs below are comparison
context; the full comparison preserves exact source references where no formal ID exists.

## Findings

| ID | Canonical key | Prior relationship / references | Implementation |
|---|---|---|---|
| <a id="ai3-f01"></a>AI3-F01 | `shortwin-keeper\|evaluate\|wrong-window-floor-reference` | Related, distinct; KI-17 | In progress — candidate-specific `min(EMA, spot)` floor; paired with F02. |
| <a id="ai3-f02"></a>AI3-F02 | `shortwin-keeper\|plan\|false-window-floor-monotonicity` | Related, distinct; KI-17 | In progress — continue across floor failures in proposal and existing-chain planning. |
| <a id="ai3-f03"></a>AI3-F03 | `shortwin-keeper\|resolve-entry\|unsupported-coin-creator-placeholder` | Related, distinct | In progress — resolve placeholder 4 from the cached creator for current and previous extras. |
| <a id="ai3-f04"></a>AI3-F04 | `shortwin-keeper\|score\|rounded-lock-share-ranking` | Related, distinct; AIR2-09 | In progress — rank with the raw LP fraction instead of rounded basis points. |
| <a id="ai3-f05"></a>AI3-F05 | `shortwin-keeper\|score\|stale-lp-counter` | New to register | In progress — refresh pool counter, WSOL vault and mint together; 33 pools / 99 keys per RPC group. |
| <a id="ai3-f06"></a>AI3-F06 | `shortwin-math\|withdraw-in\|carry-crosses-empty-pool` | Exact known | In progress — pay whole-lamport carries to the final lender; clear both before a new cohort. |
| <a id="ai3-f07"></a>AI3-F07 | `shortwin\|apply-config\|unsafe-authority-transfer` | Exact known; KI-70, AIR-04, AIR2-06 | Duplicate of KI-70 / AIR-04 / AIR2-06 — retain the owner’s 2026-10-12 decision; no new bound. |
| <a id="ai3-f08"></a>AI3-F08 | `shortwin\|flag-venue-upgraded\|unhashed-slot-upgrade-latch` | Exact known; KI-69, AIR-01 | Duplicate of KI-69(a) / AIR-01 — retain hashed bootstrap and revalidation runbook; live exposure not established. |

## Leads

| ID | Canonical key | Prior relationship / references | Implementation |
|---|---|---|---|
| <a id="ai3-l01"></a>AI3-L01 | `shortwin-common\|parse-lookup-table\|hardening-truncated-metadata-accepted` | New to register | In progress — complete metadata header required; negative control fails, all 10 common tests pass. |
| <a id="ai3-l02"></a>AI3-L02 | `shortwin-keeper\|check-foreign\|held-liquidity-freeze-residual` | Exact known; KI-23 | Duplicate of KI-23 within the stated held-low mechanism; no new settlement or profit sequence proved. |
| <a id="ai3-l03"></a>AI3-L03 | `shortwin-keeper\|decode-zc\|hardening-version-unchecked` | Related, distinct | In progress — enforce supported versions at typed GlobalConfig, Market, VenueExtras and VenueGuard reads. |
| <a id="ai3-l04"></a>AI3-L04 | `shortwin-keeper\|next-rank-batch\|ranked-batch-lost-on-rpc-error` | New to register | In progress — dequeue only after every grouped RPC read succeeds; continue with cached candidates during rank failure. Sustained outage and short-response regressions in the [internal handoff](../handoff-1.0.3-2026-10-20.md); full CI on the follow-up still pending. |
| <a id="ai3-l05"></a>AI3-L05 | `shortwin-keeper\|prepare-pamm-trader\|stale-external-account-readiness` | Related, distinct | Deferred — reproduce an actual recipient ATA closure and failed repair before changing readiness caching. |
| <a id="ai3-l06"></a>AI3-L06 | `shortwin-math\|pump-curve\|hardening-fixed-supply-fee-basis` | Exact known; KI-77 | Deferred under KI-77 — multi-tier schedule/burn boundary required; no current schedule verified in this scan. |
| <a id="ai3-l07"></a>AI3-L07 | `shortwin-venue-state\|read-base\|hardening-mint-type-unchecked` | Exact known | Deferred hardening — current readers use authenticated mint keys; no current token-account substitution path. |
| <a id="ai3-l08"></a>AI3-L08 | `shortwin\|assess\|manipulable-liquidity-history` | Exact known; KI-23 | Duplicate of KI-23 for the stated mechanism; a new production sequence beyond its bound remains unproved. |
| <a id="ai3-l09"></a>AI3-L09 | `shortwin\|close-short\|hardening-position-status-unchecked` | Exact known | Deferred hardening — current writers create OPEN then close; no persistent non-open status or second exit shown. |
| <a id="ai3-l10"></a>AI3-L10 | `shortwin\|deposit\|donation-extends-deposit-hold` | Related, distinct; AIR2-04, AIR2-05, AIR-02 | Acknowledged by delegated owner decision — actual-vest income isolation retained despite renewable new-deposit denial. Production probe: 219,140 seconds added hold for 78 donated +155,000 fee lamports; no universal reopening/cost bound. [Decision and evidence](../owner-decisions-1.0.3-2026-10-21.md). |
| <a id="ai3-l11"></a>AI3-L11 | `shortwin\|deposit\|hardening-pool-vault-fields-unchecked` | Exact known | Deferred hardening — canonical vault creation and stored PDA binding block the hypothesized importer path today. |
| <a id="ai3-l12"></a>AI3-L12 | `shortwin\|elf-fingerprint-of\|hardening-truncated-elf-digest` | Exact known; AIR2-07, KI-69 | In progress — full 32-byte SHA-256 stored/compared in a 304-byte first-deployment guard (`f5a8ccef`); no old-account migration. Final CI/fuzz gates pending. |
| <a id="ai3-l13"></a>AI3-L13 | `shortwin\|exit-risk-cap-bps\|live-supply-risk-fee` | Exact known; KI-72, AIR-10 | Acknowledged under the owner-selected capped-risk/live-execution policy; supported supply/tier changes may affect decisions, with no new numerical loss or exit-time guarantee. [Decision](../owner-decisions-1.0.3-2026-10-21.md). |
| <a id="ai3-l14"></a>AI3-L14 | `shortwin\|init-uva\|hardening-existing-uva-validation` | Exact known | Deferred hardening — canonical venue-owned UVA keys; no present wrong-user creation path established. |
| <a id="ai3-l15"></a>AI3-L15 | `shortwin\|inputs\|transaction-controlled-settlement-legs` | Exact known; KI-75, AIR-20 | Duplicate of KI-75 / AIR-20 within its stated boundary; loss beyond that residual is unproved. |
| <a id="ai3-l16"></a>AI3-L16 | `shortwin\|math-tiers\|unbounded-tier-allocation` | Exact known | Deferred resource hardening under the owner-selected venue-configuration dependency: fixed pinned accounts fit ≤100 combined tiers, but ExtendFeeConfig exists and the supported global maximum is unestablished. Do not impose an arbitrary rejection cap. [Decision](../owner-decisions-1.0.3-2026-10-21.md). |
| <a id="ai3-l17"></a>AI3-L17 | `shortwin\|parse\|hardening-vault-authority-unchecked` | Exact known | Deferred hardening — adoption authenticates authority; no current public mutation path proved for an adopted vault. |
| <a id="ai3-l18"></a>AI3-L18 | `shortwin\|pricing-inputs\|hardening-fixed-supply-fee-tier` | Exact known; KI-77 | Deferred under KI-77 — same fixed/live supply dependency as L06; requires a supported multi-tier schedule. |
| <a id="ai3-l19"></a>AI3-L19 | `shortwin\|pricing-inputs\|live-supply-synthetic-risk-fee` | Exact known; KI-77 | Acknowledged under the selected fee/supply risk and execution distinction; retained pinned evidence is not a universal harmlessness proof. [Decision](../owner-decisions-1.0.3-2026-10-21.md). |
| <a id="ai3-l20"></a>AI3-L20 | `shortwin\|pricing-inputs\|synthetic-fee-gate-basis-mismatch` | Exact known; KI-77 | Acknowledged under the selected fee/supply risk and execution distinction; no unsupported synthetic/gate tier-crossing bound is claimed. [Decision](../owner-decisions-1.0.3-2026-10-21.md). |
| <a id="ai3-l21"></a>AI3-L21 | `shortwin\|read-state-boxed-for-exit\|curve-risk-fee-at-spot` | Exact known; AIR-10, KI-77 | Acknowledged current-fee risk/history dependency; retain manipulation protection and actual execution fees without a universal decision/liveness bound. [Decision](../owner-decisions-1.0.3-2026-10-21.md). |
| <a id="ai3-l22"></a>AI3-L22 | `shortwin\|sweep-trader\|external-cashback-recovery-dependency` | Exact known | Acknowledged upstream cashback claim-availability dependency; no universal recovery deadline, and canonical source gates are retained. [Decision](../owner-decisions-1.0.3-2026-10-21.md). |
| <a id="ai3-l23"></a>AI3-L23 | `shortwin\|sync-lender\|repeated-accrual-truncation` | Exact known; KI-68 | Original per-touch trace is a duplicate of KI-68 within its accepted ≤1 lamport per index per intervening touch. Aggregate strategies, changing shares and any remainder redesign remain open; [exact comparison](../release-decisions-1.0.3-2026-10-20.md#exact-comparison-for-ai3-l23). |

For each completed triage row, preserve the canonical key and distinguish the audit
claim from the current decision: implemented with linked checks; deferred with the exact
bound or prerequisite; rejected with the failing premise; or pending further reproduction.
In particular, an exact-known label alone is not a disposition.
