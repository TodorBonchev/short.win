# Integrated release-preparation evidence

These are retained copies of the actual October 22 checks, public devnet
observations and authorized program-only deployment. The earlier
integration-check source is
`bf166672efee578b6a2a1835e0dd53729c548671`; program checks ran on initial merge
`b31c02a091008da18c39d7836c8e0e9d6d018973`, and the later commit changes only the
independently reviewed web configuration fix. The reports state their exact
scope, output identities and qualifications.

The selected program source is now `7b11cc7f85503474c6b87e0ee8b2fedd51bbc249`;
the additive byte-rebuild evidence below documents its comment correction and
canonical artifact equality. Copied build reports retain their original wording:
references to `artifacts/` inside that evidence folder mean the external directory
recorded in `byte-comparison.json`, not a compiled artifact checked into Git.

- [Program build, IDL, host tests and byte comparison](program/FRESH_RESULTS.md).
- [Install, client, focused browser, keeper, WASM and web build checks](services/focused-checks.md).
- [Initial official-verifier setup failure](verifier/readiness.md): Docker socket
  access was denied; no verifier build ran in that attempt. The later
  [Docker access receipt](verifier/docker-access.json) records the authorized group
  and daemon repair and successful server access; the environment blocker is closed.
- [Actual devnet account observations](devnet/devnet-rpc-summary.json) and
  [rent/oracle observations](devnet/devnet-cost-oracle-summary.json).
- [Proposed program-only command packet](devnet/devnet-config-runbook-proposal.md).
  This was not an executed transaction or an authorization request when prepared.
- [Devnet wallet public metadata](devnet/devnet-wallet-metadata.json) and
  [failed faucet request plus zero-balance recheck](devnet/faucet-attempt.json).
- [Later confirmed funding update](devnet/funding-2026-10-22.json): 25 SOL at
  slot 513,807,141. This additive administrative observation closes funding only;
  it is separate from the earlier retained/reviewed evidence freeze.
- [Retention manifest](retention-manifest.json) records exact original and
  retained hashes and sizes. Original `.log` files are retained here as `.log.gz`;
  decompress them to compare the report's original log hash.
- [Later exact-byte rebuild](byte-rebuild/SUMMARY.md): commit `7b11cc7` returns to
  canonical release bytes in fresh native and pinned-Docker builds. Test artifacts
  and unchanged fuzzer/harness/fixture source inputs are compared explicitly.
  Its own manifest/whitelist covers the additive evidence; compiled SO copies and
  isolated build checkouts remain outside Git at the recorded external paths.
  [Retained-file checksums](byte-rebuild/retained-files.json) list exactly what
  is stored in Git. The separate [prior-fuzz receipt review](byte-rebuild/prior-fuzz-receipt-review.json)
  verifies the completed run's retained evidence and attribution without running
  fuzz or re-reviewing its cases.
- [Concrete deployment packet](deployment-packet/deployment-packet.md), prepared
  after canonical artifact acceptance: exact addresses, allocation, costs,
  command and successful unsigned buffer preparation. At packet preparation no
  signing had occurred; the subsequent first attempt is recorded separately
  below. Its [retained-file list](deployment-packet/retained-files.json) excludes
  the ELF and keys; the copied report's ELF link and original SHA256SUMS refer to
  the external packet directory in `deployment-packet.json`.
  CLI help is gzip-compressed to preserve its exact original whitespace; its
  retained-file entry includes both compressed and original checksums.
- [First approved upload attempt receipts](deployment-packet/failed-attempt-retained-files.json):
  the CLI stopped at `Max retries exceeded` after buffer creation and 14 confirmed
  writes. The program was still absent after that attempt; its original receipt
  does not claim a second upload or buffer-close transaction.
- [Same-buffer recovery proposal](deployment-packet/recovery-packet.md) and
  [retained public evidence](deployment-packet/recovery-retained-files.json):
  an unsigned simulation of the first missing write passed. The exact second
  command and its changed transport/retry flags were prepared but unexecuted at
  that checkpoint; the approved execution is recorded below.
  The full buffer response, ELF and all keys remain outside Git.
- [Completed devnet deployment and evidence index](deployment-packet/README.md):
  the separately approved same-buffer resume finalized transaction
  [`2twpM5sUnQeAJLVTeKo9gHeDPfpU3iNitPZNg9UVm7At6zMyQCn6DATh9azQRk9rd1DzCi315W8HKZLbDpGhVm4j`](https://explorer.solana.com/tx/2twpM5sUnQeAJLVTeKo9gHeDPfpU3iNitPZNg9UVm7At6zMyQCn6DATh9azQRk9rd1DzCi315W8HKZLbDpGhVm4j?cluster=devnet)
  at slot `513919880`. The on-chain ELF exactly matches full SHA-256
  `d6f4e092dd15639c502535d68bcdb6c428c423b66d49d08f4f89e9409af538bc`.
  The program is executable but uninitialized; see the
  [deployment result](../../../deployment/DEVNET_1_0_3_DEPLOYMENT_2026_10_22.md).
- [Jupiter mainnet administrative pin review](jupiter-mainnet-pin-2026-10-22.md):
  retains the current ProgramData/ELF/IDL identity and scoped 1.0.3 interface
  disposition. The new [live-check log](jupiter-live-check-after-pin-2026-10-22.log)
  passes without changing program source or the deployed ELF. The earlier
  failed aggregate check remains historical evidence.
- [Final documentation/provenance review](independent/final-doc-review.json)
  checks the current claims against the retained build and historical fuzz evidence.

Commands, source paths and public addresses are metadata. Private keys, seeds,
credential-bearing endpoint files, compiled target caches and excluded audit
fixtures are not included. Earlier raw devnet responses contain public account
prefixes, not complete executable data. The later deployment evidence verifies
the full on-chain ELF hash without storing that executable in Git.

The completed ≥24-hour run retains its original `d6f4…538bc` artifact attribution.
The intermediate `d8d0…e182` build was superseded by the line-count-preserving
comment correction: fresh `7b11cc7` builds exactly reproduce the canonical bytes,
so the old result applies to the same runtime inputs. The separate fresh fuzz
attempt was interrupted after a provider safety block, as recorded in the
[root stop receipt](fuzz-provider-stop.json). It remains stopped and contributes
no result. No broader audit completion is claimed. See the
[current readiness record](../../release-wrap-up-1.0.3-2026-10-22.md).
