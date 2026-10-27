# CI integration repairs on the Jupiter foundation checkpoint

Run40557150355 on b7fcfc3e7437bc00d0abe34a241e13636de154c0 exposed two
stale integration catalogs. Neither program bytes nor invariant checks were
weakened to address them.

- Fuzzer statistics named only30instructions; the program now exposes33.
  Added the actual InitializePriceReport/PublishPriceReport/RepayShort Anchor
  discriminator mappings and Repay event. The existing coverage assertion
  remains; a direct discriminator regression also passes.13library tests,
  strict all-target Clippy and all21smoke/corpus tests passed locally using
  that run's verified SBF artifacts. Catalog support does not prove execution
  coverage of the new instructions or a qualifying24h campaign.
- The committed ingest-session inventory lacked Repay, although the newly
  generated scenario had passed live model/DB replay. Regenerated only
  tests/data/sessions/mock_lifecycle.jsonl through the actual-SBF recorder.
  The unchanged original PostgreSQL ingest test now passes all15fixtures,
  including strict event-catalog, zero-quarantine, chain and ledger checks.
  All other14fixture hashes, including the historical Raydium signature
  packet, remain unchanged. Exact source/artifact/fixture hashes are in the
  retained repair receipt.

Fresh ordinary CI on these corrections is required. These are integration
repairs, not complete production financial integration or final release fuzz.
