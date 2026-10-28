# Trusted VPS pricing and quote-default checkpoint — 2026-10-28

This is focused engineering evidence, **not mainnet readiness or deployment approval**.
The owner accepted trusted off-chain history and separately selected deadline-plus-24h
collateral default, including healthy borrower forfeiture. Current program paths
still expose no quote-native borrowing/adoption or general routed financial executor.

Current formatted runtime identities:

| Artifact | Bytes | SHA256 |
|---|---:|---|
| Release SBF | 2,064,552 | `43971efd45e22df7e2511e5439ff78281f4fa72c30b88bfc96e7f82740008ac1` |
| Test-venue SBF | 2,095,584 | `72dbc9e9cf5ce0771c18a25e7efe6be0dfc28e93c72cccff21a5d55e161f7a9f` |
| Generated IDL | 275,981 | `68ee467f08413f95d6404332143b90e392112bf300c067281799843d77534b48` |

The release trailing-zero-trimmed hash is
`4aabeb8b139535c4183a391dec2d906fa618d6e699d631bd8ca9d9ac6bad0613`.
The local trimming implementation reproduces the retained prior official-verifier
result; the official verifier CLI is unavailable locally. A new verifiable CI build
is still required. All 123 active runtime input hashes stayed constant during the
current build; unreferenced next-phase lender source is excluded.

Passed checks, separately scoped rather than added into an invented coverage total:

- Current-byte actual SBF: policy5, full engine94, quote repayment/default15; no failures/ignores.
- Native program107, math190, report/snapshot32, fuzzer units13, generated client34.
- Keeper196, indexer75, strict scoped Clippy; web18/typecheck/format checks.
- Actual PostgreSQL default2 after review fixes; separate legacy Repay1 and API6
  (384 responses/129 operation-status pairs) are dated in their own receipts.
- Local fixtures346 and live check at slot462217605; every pinned dependency matched.
- Root/services and nested benchmark metadata accepted frozen lockfiles; registry
  package versions/checksums in services remain unchanged.

Independent AI-assisted reviews have explicit author/reviewer scopes and limitations.
No external firm, accreditation or official security audit is claimed. Resolved
findings include SBF decoder stack overflow, protocol-credit overwrite, immutable
vesting, default vest flush and old-lender record-prefunding projection. Their
failing controls and passing evidence remain inside the archives.

`RETENTION.json` lists every archive/member size and hash. Original inner seals and
exact log bytes are preserved in lossless ZIP files; intermediate receipts retain
their own artifact identities. Extract an archive before checking its original
inner SHA256SUMS. `current-build-and-artifacts.zip` holds current binaries/IDL;
`preceding-build-and-client-checks.zip` is the earlier unformatted checkpoint,
not the current binary. The earlier [October27 archive](../trusted-history-2026-10-27/)
retains the prior green CI and initial service/snapshot evidence without rewriting it.

Complete routed execution, lender backend integration, snapshot debt hooks,
prospective cohort-carry reconciliation, real fuzzer actions/final24h campaign,
new full CI/verifier, actual operator authorities/funding/alerts/backups and the
concrete deployment packet remain required. No live Jupiter queries, mainnet
financial transactions, purchase, tag or deployment are claimed.
