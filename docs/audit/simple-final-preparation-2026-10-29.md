# Pooled final-identity preparation — 2026-10-29

The owner's request was to finish monitoring-capacity reuse and preparation for a
fresh 24-hour qualification of the Jupiter-only pooled implementation. Engineering
checks and the current deterministic route matrix pass locally. **The 24-hour
campaign gate remains OPEN and no long run has started.** A provider filter stopped
unfinished combined campaign tooling; that work is isolated and unqualified. The
existing historical-CPMM duration runner cannot stand in for the unfinished scope.
No tag, deployment, live publisher, financial transaction or provider purchase was
performed. This is preparation for the owner's mainnet-before-external-audit and
funded-bounty policy, with both activities still accurately reported as incomplete.

## Candidate identity and private custody

The owner selected generation of a dedicated local program key. The new public
program ID is `BCPiRMTMoLNY5EHm2RYxoxJChgyrttkFX2Qe2Rnbe3jh` and the independent
replacement's version is **1.1.0, untagged**. The program declaration, Anchor
mapping, IDL, generated client/PDA defaults and Rust wire fixture were rebuilt for
that identity. The dedicated secret remains outside the repository with private
permissions; no key bytes or seed words enter this evidence. The ignored local
build key reference resolves to that same private key. The previous local test key
is preserved separately. An owner-controlled private backup remains a deployment
prerequisite; a public address is not a backup or an upgrade-authority decision.

| Artifact | SHA256 |
| --- | --- |
| SBF v3 program, 786,864 bytes | `d4868d29c14f4a04abe1cdead451f197b2102286fec10abe34161d387f66c510` |
| Generated IDL | `52bdccd8a9f29739885e95a4c4595e70067edf3572e9d60c23aa69741b6abc15` |
| Primary v3 price model | `7e282d429cb65694c844f7f321483a1cd9bcdd5c4e1f32d70e8a9942016643b8` |
| Separate keyless v3 price model | `e610c336a9bdbe1fc155f21f4f655dcb7c6938b04c009c20711097c4db2bede2` |

The program key does not assign governance, emergency, reporter, treasury or keeper
roles. Those runtime assignments do not alter the declared program bytes, but
must be reviewed with the actual mainnet genesis and Jupiter loader instance
before initialization. The proposed role verifier deliberately fails its complete-
assignment gate while those fields are absent. Execution remains false. The
[financial/service proposal](../../simple/config/README.md) retains its existing
terms: one exact-q history slot initially, per-market borrowing limits and uncapped
deposits/market count/global exposure. No policy is activated by these files.

## Monitoring capacity reuse

A complete finalized bank snapshot now drives `reconcile-queue`. Its clock slot
must equal its account context slot, and its block timestamp must be locally fresh.
SQLite revision compare-and-swap protects concurrent enqueue/tick updates; exact
replay is idempotent only while the resulting revision remains unchanged.
`sync-loans` alone is now add/priority-only and cannot retire inactive work.

A scheduling identity retires only when absent from the complete active-loan set,
with no unresolved publication, no finite reference still within either time or
slot validity bound, and no unfulfilled warm-publication intent still inside its
TTL. Defaultable but unclosed loans remain active debt. Confirmed-but-unfinalized,
unknown and post-snapshot publication transactions retain capacity. Finalized
success must be included in the complete reference snapshot; failed-publication
proof requires an explicit valid finalized error and inclusion slot. A malformed
RPC result fails closed.

Retirement removes only the scheduling row and preserves evidence. Readmission
increments the generation; old observations and fulfillment acknowledgements
cannot warm or authorize the new generation. The model is v3 and v2 history
stores are rejected rather than migrated silently. The primary/keyless policy
hashes and sensitivity report were refreshed through the actual service checker.

The operator holds an OS-backed SQLite publisher lock for its entire pass. A
separate durable identity companion, history UUID/binding and operator-store
UUID/mirror bind the canonical database paths and program identity. The store's
write transaction serializes first claims even when two processes use different
history paths. Independent review reproduced that race before the fix; a
synchronized two-process regression now rejects the second claim. Persisting the
identity companion never drops the publisher lock. A crash can leave matching
one-sided initialization, which a matching retry completes; mismatched state is
never silently rebound.

Preserve the history and operator databases, the publisher identity companion and
lock files, including SQLite journals where present, with a stopped consistent
backup and tested restore. Four-address reference reservations and UTC-day fee
reservations remain intact across reuse and restart; retirement does not restore
financial allowances. Database loss/replacement or corruption after an identity is
recorded stops the pass for reviewed recovery. Deterministic defaults remain
independent of price settings, provider outages, Unknown prices and price pause
while the local identities are intact. A first boot with no history yet can process
defaults before configuring prices and bind its first history later; this exception
does not permit deleting previously recorded history.

## Current dependencies and real route checks

The [new dated route archive](simple-final-route-evidence-2026-10-29/README.md)
retains raw responses, actual accounts/lookup tables, source slots, reconstructed
snapshots, external loader review, replay logs and checksums. The selected-ID SBF
passed six positive full sell/whole-q buyback cases: BONK, JUP, RAY, active legacy
SPL Pump curve, graduated PumpSwap and an active metadata-only Token-2022 Pump
curve. Two separately retained Scorch routes failed with the exact expected
`Custom(5460)` / `0x1554` downstream refusal. All positive packets used actual
captured lookup tables and fit the 1,232-byte limit.

Pump openings consumed 2,199,360 / 1,844,400 / 1,844,400 lamports of separately
actor-funded native setup respectively. Each same-loan buyback signed and consumed
zero setup. Four explicit setup-budget refusals and post-successful-CPI failure
checks preserve rollback. These are retained deterministic functional cases, with
synthetic local signers, lending inventory/collateral and history attestations;
they do not prove live transaction landing or real production history.

A previously captured Token-2022 curve had all token supply inside its curve
account and only 56 lamports of real SOL. A cached quote did not make synthetic
outside lending inventory physically possible; its actual replay failed. The raw
counterexample remains archived and is excluded from the positive matrix. The
replacement current `8GV…` snapshot has enough outside supply for the retained
synthetic lending allocation and actually passed the new-ID replay. Other provider
and volatile-candidate failures remain evidence, not successful coverage.

The approved existing deterministic replayer was restored to its previously
reviewed source before rebuilding for the new identity and invoked in its normal
two-argument form. No isolated combined-campaign or seeded-variant WIP was used.
The current external comparison and its authority/account limits remain in the
archive; old dated Oct28 captures and reviews are unchanged. Repeat live identities
at deployment if upstream programs change. A changed pump-fees ELF would require
a renewed authority/account review before promotion.

## Verification and honest remaining gates

Local [command receipts and source hashes](simple-final-preparation-evidence-2026-10-29/local-verification.json)
retain successful format/Clippy, **70 Rust**, **42 service**, **six configuration**,
**39 client/operator/HTTP** and **nine web** tests, frozen-lockfile installation and
web build. An additional synchronized pairing-race regression passed. The SBF/IDL
were rebuilt with the pinned tools; generated client and wire checks agree.
`audit_scope.py check` and `git diff --check` pass. Independent AI review found and
rechecked the database-claim and lock-release fixes; no official external-firm
review is claimed. Hosted CI and independent artifact comparison are pending
until the candidate commit is published and its run completes.

The [provider-stop receipt](simple-final-preparation-evidence-2026-10-29/provider-stopped-status.json)
retains hashes of the unfinished combined campaign work. Its local copies and
patch are preserved under `simple/target/provider-stopped-final-campaign-20261029`;
new unfinished tooling was removed from active source and the two approved Rust
examples restored. Do not retry the stopped work, execute it or claim its results.
No 24-hour completion, duration, all-route stochastic coverage or final whole-system
freeze is claimed. Earlier 1.0.x duration and Oct28 smoke remain scoped to their
original inputs. This preparation therefore does **not** authorize starting a
substitute long run or tagging the candidate as stable.

The remaining route to release is concrete:

1. Obtain green CI for this candidate and independently compare the produced ELF,
   IDL and deterministic route results. Retain any further changes and their scope.
2. Keep the incomplete 24-hour campaign gate open; any qualifying record must
   truthfully identify its reviewed scope, frozen source/artifacts, elapsed time,
   readable zero-failure/anomaly summary and actual coverage. Resolve the unfinished
   preparation without bypassing the provider stop before starting a final run.
3. Assign public operational roles and loader/genesis values, privately back up the
   program key, and commission real free-key/RPC capacity with the actual worker,
   indexer and keeper artifacts. Fund bounded payer balances/reserves, test alert
   delivery, outage/restart/backup restore and one-slot admission/reuse. Code tests
   and offline arithmetic do not establish any of these operating conditions.
4. Present the exact qualified commit, ELF/IDL hashes, public configuration,
   route/long-run evidence and funded operating receipts for the owner's tag and
   deployment decision. The intended stable line is `v1.1.0`; never repoint a tag or
   borrow 1.0.3/1.0.4 qualification for this independent program. Only after that
   decision prepare/simulate deployment and initialization of the canonical bytes,
   verify chain state and running services, and enable the reviewed opening policy.

Archived review checksums and legacy finding dispositions remain unchanged. The
primary `/root/short.win` checkout and its untracked `fork_seed.rs` stay preserved.
