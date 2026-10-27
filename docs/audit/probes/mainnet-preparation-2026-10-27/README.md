# Mainnet preparation observations — 2026-10-27

These are read-only preflight and local preparation results for the owner's
[unaudited mainnet policy](../../../release/MAINNET-1.0.3.md), not a mainnet
deployment or a promoted upstream fixture. Source base is main
`7f2af5b5787d62978c79e7630f5b0e1a3310d94e` with the policy/service changes under review.

`receipts.json` records UTC start/end, exact commands, exit codes and original
output hashes. Scope, whitespace and all 344 static fixture entries pass. The
aggregate live check **failed** at slot **461,877,333** with Token-2022 as its only
drift; `03.log` retains the failure. The unchanged venues are not represented as
an aggregate pass.

## Current Token-2022 identity

The independent finalized read at slot **461,878,038** confirms deploy slot
**460,497,271**, versus the tested fixture's **427,147,035**. Upgrade authority and
ProgramData allocation match, but executable bytes differ:

| Identity | Tested fixture | Observed mainnet |
| --- | --- | --- |
| ELF bytes | 632,560 | 611,056 |
| Full SHA-256 | `9bbf90b30e06778ca0feca100b29f0eeb9be576ae024f6323cc207308f51a5d1` | `3dde54d202caa2d10514fd58a154df5f26f07643f9f5fd0402ff08084f0cb954` |
| Trimmed SHA-256 | `7ea94a027005b39196fa08e6d0ddcd55ec32eb43266179921a834a1ba2eb1947` | `5e888b294bd1941bfc47107a73d5e40bb24fde0d639f69099e7920ad8b6413d9` |
| Embedded self-reported source release | `program@v11.0.0` | `program@v11.1.0` |

The embedded release text is a provenance lead, not verification that deployed
bytes reproduce that source. `token-2022/identity-comparison.json`, exact account
headers, raw public RPC response and observed ELF are retained. The two large
original files are gzip-compressed without altering decompressed bytes;
`retention.json` gives both original and compressed hashes. The observed ELF is
**diagnostic evidence only**, under the original Apache-2.0 licence. It is not in
`tests/fixtures`, no manifest or pin was changed, and no completed run used it.

This new upstream change needs scoped source/interface/layout/transfer/extension
compatibility review and applicable fork/profile qualification before promotion.
The earlier frozen fuzz remains evidence for its own inputs. Audit/bounty timing
acceptance does not close this dependency gate or justify a mainnet-ready tag.

## Program identity and costs

`mainnet-program-readonly.json` records the correct mainnet genesis and a null
finalized account at the compiled program address
`5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK` (slot **461,879,034**). No mainnet
loader/configuration transaction was sent. Minimum rent observations were
**8.352520760 SOL** for a single-sized 1,644,024-byte ProgramData plus header,
and **8.352480120 SOL** for a same-size upload buffer plus header. These are
individual rent queries, not a complete deployment quote: actual max length,
program account, temporary funding, recoveries, signatures and priority fees
must be included in the concrete transaction packet. No payer was guessed or
private key read.

## Local web/client checks

Frozen pnpm install passed the existing 316-entry supply-chain lock validation.
Web typechecking passed. All 30 generated-client checks passed, including the
304-byte/full-SHA-256 guard. The first focused Biome check found formatting in
the changed security-page prose; its failing log is preserved. Applying the
formatter to that one file and rechecking passed (`js-00-after-format.log`).
The change was formatting only; no test result is credited to a failed command.
New service logging/host dependency changes require their fresh CI; they are not
tested by the completed on-chain fuzz. The separately retained
[dependency review](../../dependency-advisories-2026-10-27.md) explains the failed
main CI and narrow remediation.

`SHA256SUMS` checks every file retained in this directory except itself. Historical
review archives and frozen evidence were preserved; they are not rewritten to
match these new observations.

The readable first Biome console log has trailing display whitespace normalized; `console-log-retention.json` maps its hashes to `js-00.log.original.gz`, preserving the exact original output used by the receipt.

## Subsequent scoped source/provenance review

[Token-2022 source review](token-2022-source-review/REPORT.md) identifies the new
publisher asset byte-for-byte, the exact tagged commit, a separately retained
public verifier response and relevant layout/transfer/initialization/memo changes.
No adapter change is identified by that static review within the existing mint
allowlist. Tests, benches and a fresh qualifying campaign against the new upstream
baseline remain pending; current CI still uses the old fixture. This advances the
disposition without claiming the live gate passed or promoting fixture bytes.
