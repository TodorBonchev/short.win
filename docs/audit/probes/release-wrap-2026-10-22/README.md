# Integrated release-preparation evidence

These are retained copies of the actual October 22 checks and read-only public
devnet observations. The selected runtime/client source is
`bf166672efee578b6a2a1835e0dd53729c548671`; program checks ran on initial merge
`b31c02a091008da18c39d7836c8e0e9d6d018973`, and the later commit changes only the
independently reviewed web configuration fix. The reports state their exact
scope, output identities and qualifications.

- [Program build, IDL, host tests and byte comparison](program/FRESH_RESULTS.md).
- [Install, client, focused browser, keeper, WASM and web build checks](services/focused-checks.md).
- [Actual official-verifier setup blocker](verifier/readiness.md): Docker socket
  access denied and noninteractive sudo requires a password; no verifier build ran.
- [Actual devnet account observations](devnet/devnet-rpc-summary.json) and
  [rent/oracle observations](devnet/devnet-cost-oracle-summary.json).
- [Proposed program-only command packet](devnet/devnet-config-runbook-proposal.md).
  This is not an executed transaction or an authorization request.
- [Devnet wallet public metadata](devnet/devnet-wallet-metadata.json) and
  [failed faucet request plus zero-balance recheck](devnet/faucet-attempt.json).
- [Retention manifest](retention-manifest.json) records exact original and
  retained hashes and sizes. Original `.log` files are retained here as `.log.gz`;
  decompress them to compare the report's original log hash.

Commands, source paths and public addresses are metadata. Private keys, seeds,
credential-bearing endpoint files, compiled target caches and excluded audit
fixtures are not included. The raw devnet responses contain public account
prefixes, not complete executable data. They prove the dated observations only.

The old completed ≥24-hour run belongs to its original `d6f4…538bc` artifact.
The fresh `d8d0…e182` candidate differs by one diagnostic line-number byte.
These checks neither complete its final-byte fuzz gate nor close the incomplete
whole-repo audit. The subsequent fresh run was interrupted after a provider
safety block, as recorded in the [root stop receipt](fuzz-provider-stop.json).
No later launch review or completed qualification is claimed. See the
[current readiness record](../../release-wrap-up-1.0.3-2026-10-22.md).
