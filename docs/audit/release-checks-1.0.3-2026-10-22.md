# 1.0.3 release checks and owner-review plan (2026-10-22 Europe/Kyiv)

> Historical predeployment snapshot. The program-only devnet deployment later
> [completed](../deployment/DEVNET_1_0_3_DEPLOYMENT_2026_10_22.md), and the
> [Jupiter administrative pin drift](probes/release-wrap-2026-10-22/jupiter-mainnet-pin-2026-10-22.md)
> was dispositioned with a later passing live check. Its original failed check
> and unfulfilled operating/audit gates remain accurately recorded below.

**Technical fix gates passed; first-deployment readiness remains pending.** This is
an unreleased preparation record. No tag, deployment, governance/financial transaction
or media publication was performed. The owner confirmed no previous deployment and
delegated the documented policies; no old-state migration or external-firm report is
claimed. The accurately attributed AI-assisted batch review route remains as written
in [AUDIT §12](../AUDIT.md#12-logistics-and-change-policy).

## Frozen runtime, final evidence and technical closure

Current integrated code commit: `56f660027f88ee60a33b9d2b5da516daccc9c9b9`.
Earlier complete mutation-gated batch: `ff85a1b0c8452cb9fad32767f7083d22377e90a3`.
Frozen runtime source: `e04a6776f1c2f5e02e581d76dd1255c416270275`.
Between frozen `e04a6776` and `ff85a1b0`, only the standalone AIR2-01 math test
changed. Incoming `56f660027` changes keeper/indexer services and build controls,
separately reviewed below; the on-chain program, fuzzer/harness and fixtures remain
identical. Audit records and package-age cleanup change none of those frozen inputs.
No previous run's duration is combined with it.

[Full CI 39509811932](https://github.com/TodorBonchev/short.win/actions/runs/39509811932)
remains completed/success on exact `ff85a1b0`, freshly rechecked through GitHub's API.
The retained full-gate record has 25 passing jobs, one intentional nightly skip,
19 selected mutants killed and verifier/Anchor/frozen byte identity. Source/CI
comparisons are in the [final independent verification](probes/fuzz-final-1.0.3-2026-10-22/independent-final-verification.json).
No new full program suite was dispatched for documentation and an exclusion removal;
the affected dependency installation, client checks and web tests below were run.

The final all-profile fuzz naturally finished at **2026-10-21T22:21:37.094749Z**:
exit **0**, supervisor **86,428.073717718 s**, runner **86,426.897880876 s**;
**38,563 runs/wind-downs, 7,330,667 actions, 87,632,843 transactions,
158,970,339 hook checkpoints; zero invariant failures and zero anomalies**.
All frozen inputs, source archive and 334 fixtures match, as do exact summary/log
hashes. [Final retained files, coverage witness and checksums](probes/fuzz-final-1.0.3-2026-10-22/README.md).

The six-profile selection witness uses the unchanged two-worker seed schedule;
29 completed clean runs suffice (36 is a conservative bound). It supplies no
per-profile counters, equal allocation or 24-hour duration for each profile.
Actual coverage is 28/30 landed instructions, 68/111 returned error codes and the
retained event maps. Setup initialization is outside Stats; LegacyPoolCarry is
covered by its direct matrix/signed regression and old-code/ELF controls rather
than a natural hit in this random run.

The [findings tracker](findings.md) closes 19 technical rows: AIR2-01–05, 07–09 and
P2-01; AI3-F01–06 and L01/L03/L04; AI3-FR01. AI3-L12 follows AIR2-07 in triage.
Acknowledged fee/quote, cashback, zero keeper reward, pot-priority, final-holder
carry and renewable deposit-denial consequences remain acknowledged/deferred,
not fixed by this evidence. Largest present claims against absent-owner coverage
were 524,012,267 lamports; one absent-deficit market remains. Zero oracle anomalies
does not guarantee eventual claims, keeper profitability or funded operation.
The original 90-file review archive remains checksum-identical.

| Canonical artifact | File SHA-256 | Bytes |
|---|---|---:|
| Release shortwin.so | `d6f4e092dd15639c502535d68bcdb6c428c423b66d49d08f4f89e9409af538bc` | 1644024 |
| Test-venue shortwin.so | `0a405deba60a521798820985cd4cec1a0e61e46ed9d0ee829fd037b010565a01` | 1675056 |
| Mock CPMM | `4cf1700761782aece1a4e3463ce0b2b9f2afaed2ecf3d0a6362f115faf9c7ab9` | 281240 |
| Fuzzer | `5b6971e3fd11f8c8d3166d010631b40e0dd57b797f311ba97a348a35e1036b48` | 11686272 |

Only the release shortwin.so is a deployment artifact; the others are test evidence.

## Concurrent service/build integration

The first publication push detected remote commit
`56f660027f88ee60a33b9d2b5da516daccc9c9b9` after the previously observed `f894b810`.
It was preserved through a clean rebase, without force-pushing or altering the
primary checkout. It authenticates keeper logged events against the active runtime
invocation, preserves a newly observed indexer stream's checkpoint/membership for
duplicate transactions, pins container/install inputs and enforces locked builds.

[CI 39556305501](https://github.com/TodorBonchev/short.win/actions/runs/39556305501)
completed success on exact `56f660027`: **24 jobs passed, zero failed, two skipped**.
`fuzz-nightly` is an intentional scheduling skip; `mutants` is an intentional
push-event skip because that job runs on manual dispatch (or opted-in schedule).
This run did not kill 19 mutants. The earlier `ff85a1b0` dispatch's actual 19-kill
results remain separately attributed to unchanged program/math/mutant/fuzzer/fixture
inputs. Changed services/build controls use the new CI and focused peer review.
[Incoming source equivalence](probes/fuzz-final-1.0.3-2026-10-22/incoming-source-equivalence.json)
distinguishes unchanged frozen program inputs from the changed services/workflow.

[Retained incoming CI, byte comparison and service review](probes/fuzz-final-1.0.3-2026-10-22/remote-56-review/README.md)
record matching new Anchor release/test/mock and verifier bytes, both ZIP digests,
unchanged full-digest IDL, independent checkpoint review, 96 passing keeper units and
two genuine-gap/recovery SVM regressions. Earlier whole-service/workflow
identity observations in the dated operator/final-verification snapshots apply to
pre-integration `f894b810`, not to these changed files. The completed frozen on-chain
fuzz is not presented as testing the new keeper/indexer implementation. No fresh
24-hour duration is needed for identical program/fuzzer/fixture bytes; any change
to those inputs would require a new qualifying run.

## Preparation checks and actual live result

Timestamped commands, exact output and output SHA-256 records are retained in
[the preflight evidence](probes/fuzz-final-1.0.3-2026-10-22/preflight/).

| Check | Result |
|---|---|
| `python3 scripts/audit_scope.py check` | Passed; watched files classified. |
| `git diff --check` | Passed; repeated after final edits. |
| `python3 scripts/dump-fixtures.py check` | Passed; 344 fixture entries match files, pins and checksums (334 copied files in the frozen run is a separate count). |
| `python3 scripts/dump-fixtures.py check --live` | **Exit 1: Jupiter deploy drift only**, slot 459,802,698, 2026-10-21T22:45:04–22:45:16Z. Full aggregate preflight is not green. |
| Mobile package-age exclusions | Removed only the two matured `@solana-mobile` exclusions on October 22 local; exact protocol/Kit version overrides retained; lockfile unchanged. |
| `pnpm install --frozen-lockfile` | Passed supply-chain validation for all 316 locked entries, pnpm 12.7.0. |
| `pnpm --filter @shortwin/client check` | Typecheck and all 30 tests passed, including 304-byte guard/full digest roundtrip. |
| Web guard command | `pnpm --filter web test -- src/features/short/venueGuard.test.ts` ran the whole suite under the package script: 1,075 tests passed, one intentional skip; 91 files passed, one skipped. Guard's two tests passed. Expected negative-test stderr is retained; exit 0. |
| Indexer destination 3 and current client/IDL | New exact-56 CI passed duplicate-stream checkpoint/rollback, destination-3, recording decode/replay and 304-byte client tests; its actual Anchor IDL equals the committed IDL. Running identities are separate below. |
| Stale service runbook | Old 1.0.2 V2-builder retention paragraph marked historical/superseded, replaced with current first-deployment prerequisites. |

The live check reports pump 458,921,440, PumpSwap 458,921,517, pump-fees 456,132,171,
AMM v4, CPMM, Token-2022, Squads, memo and Squads config unchanged. Pump-fees bytes
still match the retained R-PFEE analysis; re-read authority/accounts if those bytes
change later. The dated result is not a guarantee of future live state.

**Jupiter:** deployment slot **459,479,624** differs from baseline **454,465,850**.
There is no current Jupiter adapter/VenueKind; current execution masks exclude its
guard bit 5. The keeper still watches/flags it and configuration tooling checks
all six explicitly configured slots, so it is not wholly absent. In particular,
`shortwin-gov init-config` can refuse a stale configured Jupiter slot. No baseline
was changed, no ELF was promoted and no initialization slot was omitted to bypass
review. Supported trade-path fuzz remains valid on unchanged inputs; the aggregate
live check and deployment configuration disposition remain pending. Before changing
this reference pin, retain new ELF/header identity and the required scoped interface,
mask, configuration and compatibility review under AUDIT §12. Do not activate a new
Jupiter execution/latch scope through this administrative follow-up.

## Operating evidence and release sequence for owner review

Available local evidence does not establish a real configured fee payer, covered
markets, funded balance/top-up procedure, working metrics/alerts or running
keeper/indexer/client artifact identities. Only example/reference configurations
were found; no service in the inspected local WSL/Windows namespace or default
local listener demonstrates production operation. A service may run elsewhere;
its evidence was not supplied. No key material was read and no guessed payer was
queried. [Redacted compatibility and operator inventory](probes/fuzz-final-1.0.3-2026-10-22/operator-compatibility-review.md).
Code tests do not close these requirements. The ordinary keeper already attempts
eligible zero-reward exits; it requires an actually funded operator backstop.

The proposed sequence is:

1. Resolve/disposition Jupiter's observed redeploy with retained upstream and
   initialization-configuration review. Repeat the full read-only live check and
   record every result; promote changed inputs only after their required checks.
2. Supply the actual operator's redacted public configuration, covered market set,
   payer public key/current balance, funding/top-up ownership and alert drill.
   Verify ordinary keeper execution is enabled (`dry_run = false`), and identify
   actual running indexer/client artifacts against reviewed 304-byte/destination-3
   source. Stage the compatible indexer and readers before the program.
3. Review the prepared `audit-1.0.3-gates` branch with these records, canonical
   artifact hashes and owner-accepted residuals. Integrate the approved revision
   into main, requiring source-equivalent green gates or new gates for any relevant
   source changes. Recheck archive/scope and current live identities at that point.
4. Obtain the owner's final approval of that concrete revision/artifact and
   deployment configuration. Only then create `v1.0.3` at the approved main commit
   and carry out the reviewed first-deployment/governance procedure. Rehash the
   actual release ELF before signing, preserve transaction/config identities, and
   verify funded keeper/monitoring and indexer/client behavior after deployment.

These are proposed steps, not executed actions or current tag/deployment authority.
The monitor stops after publishing this preparation record because actual operating
evidence and final owner review require user action; it will not repeat notifications.
