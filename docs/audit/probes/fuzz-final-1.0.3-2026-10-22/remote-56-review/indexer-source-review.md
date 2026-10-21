# Independent indexer/checkpoint and deployment source review

Reviewed source 56f660027f88ee60a33b9d2b5da516daccc9c9b9 against f894b810 (the same tested code base ff85 plus audit records). Reviewer remote_scope_check did not author the incoming writer/deployment changes. This is a read-only source and actual-CI-evidence review, not an external firm report. No production write, transaction or source edit was performed.

## Scoped conclusion

No blocking defect was identified in the incoming checkpoint fix or deployment configuration diff. Current304 VenueGuard and sweep destination 3 compatibility remain supported. The new CI's actual focused test results are retained in new-ci-review.json; they are not inferred merely from test source.

## Duplicate stream checkpoint

Previously, an ingest_txs signature conflict returned Duplicate before recording a later-discovered stream. A transaction first seen by the program stream could therefore be absent from its pool stream's membership/cursor data.

The new duplicate branch reads the existing row's canonical slot and tx_index within the same database transaction, then applies the existing cursor/membership helper for each newly observed stream. It commits those observations before returning Duplicate. It does not reinsert invocations/events, reapply projections or emit new-event notifications. The original insertion path uses the same helper with its raw canonical position.

The existing single-writer advisory lock and database transaction remain the concurrency/atomicity boundary. ingest_tx_streams retains ON CONFLICT DO NOTHING; cursor bounds retain their existing monotone slot rules. Rollback already resets all cursors associated with the signature using membership records, so recording the previously missing stream restores the information that rollback needs. This fix does not claim to redesign same-slot cursor tie ordering or transaction commitment promotion.

The added a_duplicate_transaction_still_checkpoints_a_new_stream test executes first program-stream ingestion, later pool-stream duplicate, exact newest/oldest cursor values and canonical membership position, one ingest_txs row, both stream memberships and rollback clearing both streams' bounds/membership. This would fail against the old early-return path when the second cursor is read. Its actual new-run CI log reports ok. The pre-existing duplicate regression also checks that repeat writes leave one event and invocation; the occupied-position/finalized collision test remains present.

No additional test is required to substantiate this demonstrated correction after its successful exact-source CI. Useful later boundary coverage would send the same signature with a different noncolliding incoming slot/index and assert that the new membership uses the stored position, plus repeat the same new-stream observation with pre-existing older/newer bounds. Those are limited coverage suggestions, not a reproduced bug or a claim that current rollback semantics are broken.

## Current account and income compatibility

The incoming diff does not change generated client/IDL, browser guard reader/tests, indexer projection classification, short.win event definitions or the decoder layouts. New CI codama explicitly passes the 304-byte layout/full-suffix regression; browser venueGuard.test.ts passes its two cases. The new Anchor IDL equals the committed client IDL byte-for-byte and declares guard_elf as [[u8; 32]; 4].

The projection still classifies to=0 or 3 as lender income; SWEEP_TO_LENDERS is 3. The exact new CI log reports a_dust_flush_sweep_credits_lenders_instead_of_protocol_fees, the committed-session current-layout decoder check and replay consistency check all ok. This supports code/package compatibility, not a verified rollout of those versions.

## Deployment and CI changes

The Dockerfile pins both base-image digests. The reference compose file pins Postgres and uses pull_policy: build for the local indexer source image, preventing a default pull of the mutable shortwin-indexer:latest tag instead of building this checkout. The CI reference-compose parse step succeeded. This is not evidence that an image was built/deployed on the operator's host or that a running image has the accepted source identity. The apt ca-certificates package remains resolved at build time, so do not claim a fully byte-reproducible container from these pins alone.

The incoming CI pins Anchor's commit, verifies both Agave installer/archive hashes, uses new cache keys and adds locked flags to release/IDL/test-venue builds and dependency checks. Their new outputs were downloaded and compared, rather than assuming new flags/tool setup preserve program bytes. Actual release/test/mock bytes and current IDL match the recorded accepted inputs. This source review did not independently establish upstream binary provenance beyond the retained CI downloads/hash validation.

The new push CI has no fresh mutation execution. The prior ff85 mutant result is available only through the explicit unchanged-input comparison in new-ci-review.json. No gate or result should be described as newly run on 56 when it was skipped.
