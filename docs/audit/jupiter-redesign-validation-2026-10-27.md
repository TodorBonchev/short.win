# Jupiter redesign engineering checkpoint — 2026-10-27

This is development evidence for the owner-selected signed-feed redesign. Mainnet
is postponed. Neither the route/math prototypes nor the standalone oracle experiment
is linked into production instructions or admits new markets. No final version,
release tag, live feed, new signing authority, financial transaction or deployment
is approved or performed by this checkpoint.

## Verified work

- Pure no_std route-header/fixed-meta and exact-quantity math checks passed, including
  the independently retained slippage/interval exhaustive cases. Header validation
  is not route authorization; candidate upper costs are not forced-action lower references.
- Manual native CI40480763900 on3a35d6ee1827285901acb9fb01a5634e21b3d40c
  passed24 Rust tests, two runner fault tests, formatting and strict Clippy. Artifact,
  exact source/modes, all280 active registry archives, SDK features and unchanged
  pinned/production locks were independently verified. The24 count already includes
  the10 wire groups; it is not34. Successful authentic-attestation and SBF tests remain0.
- The candidate SDK reader always returns ProviderVerifierQuarantined and invokes
  no unsafe SDK verifier. The earlier native ABI pass is retained with its limited
  scope; the source-level SDK safety findings remain qualification blockers.
- Initial prototype CI's Anchor and reproducible verifier build both produce the
  canonical release ELF SHA256
  d6f4e092dd15639c502535d68bcdb6c428c423b66d49d08f4f89e9409af538bc,
  size1644024bytes. Test-venue/mock and IDL hashes are retained in the artifact receipt.
  This proves that prototype batch's bytes; it does not qualify future integration.
- The fresh full dispatch's indexer job122115063321 passed. Its actual log shows
  inbound_floods_are_refused_then_closed passing at09:42:26Z. The corrected test sends
  exactly11messages, preserving fivepongs/twoerrors/close1008 assertions and avoiding
  an extra unread frame during teardown. This is a test change, not a rate-policy relaxation.
- Scope, whitespace, evidence SHA256 manifests, gzip-decoded source bytes and new
  local documentation links passed. The primary checkout's fork_seed.rs and frozen
  run metadata retain their fixed preservation hashes.

## Full CI checkpoint

Observed UTC: 2026-10-27T09:54:03.218659+00:00.

- Run40476629768 on21db9fadb888e0a29f999e3346f18dc603b9b999: in_progress, conclusion=None; 22success/1failed/2skipped. Active: program-tests.
- Run40480767468 on3a35d6ee1827285901acb9fb01a5634e21b3d40c: in_progress, conclusion=None; 21success/0failed/1skipped. Active: services, web-e2e, mutants, program-tests.

The original PR run's WebSocket failure is retained; it is not called green. The
new manual full run includes the repaired test and selected mutation checks. Its
running jobs are pending evidence. See the [dated API snapshots](probes/jupiter-redesign-2026-10-27/ci-checkpoint/).
Later documentation/evidence-only commits after3a35do not change its tested executable
sources; this is a scoped equivalence statement, not exact-head full-green status.

## Provider and release work still open

Switchboard is excluded after its [shutdown announcement](https://switchboardxyz.medium.com/important-announcement-54dd6dbe9379).
The [screening record](probes/jupiter-redesign-2026-10-27/provider-screening/) discloses
cached primary extraction, failed fresh retrieval and unsupported operating claims.
Its native/queue/ELF research is preserved; remaining on-chain accounts do not prove
an operating provider or source provenance.

The [maintained-provider inquiry](../design/jupiter/PROVIDER_CAPABILITY_REQUEST.md)
is concrete and unsent. Outreach requires owner authorization and chosen sender/reply
contact. No private code/audit packet, purchase or business commitment is included.
No alternative provider or protocol-operated signer set is selected by this record.
The [trust worksheet](../design/jupiter/PROVIDER_TRUST_REVIEW.md) and
[quantity producer contract](../design/jupiter/QUOTE_PRODUCER_CONTRACT.md) identify
required authentic fields, lower-reference methodology, quantity/history coverage,
provider/queue/rotation/cost settings and failure rules before activation.

Actual bounded Jupiter SBF execution, adversarial rollback/resource tests,
provider authentication/qualification, complete risk/history/admission/settlement
and keeper/client/indexer integration, fresh final-runtime regression/review/fuzz
and funded monitored operation remain open. No historical fuzz duration is credited
to a new program or fixture baseline. No protocol audit finding is closed here.
