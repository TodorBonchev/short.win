# Independent AIR2 fix-review evidence — 2026-10-21

This sparse evidence bundle supports the [consolidated attributed review](../../independent-air2-fix-review-1.0.3-2026-10-21.md). It contains source reports, small execution logs/check records and independently authored control/probe sources. It contains no compiled artifacts, copied math crate, venue fixtures, SBF ELF, build cache or dependency tree. Fifteen logs retain their original EOF blank lines inside deterministic lossless gzip (level 9, timestamp zero, empty stored filename). Other logs remain raw. No original log bytes were trimmed or normalized.

Production reviewed: `e04a6776f1c2f5e02e581d76dd1255c416270275`. The AIR2-01 regression was strengthened separately during review, with the final test-file hash recorded below and in the accounting checks. Publishing these reports and that test-only addition does not change the production identity reviewed. CI and the current-profile ≥24-hour fuzz completion were pending when the review records closed; consult the [release-readiness record](../../release-readiness-1.0.3-2026-10-21.md) for subsequent gate status.

## Attributed original reports

All three files below are copied **byte-for-byte**. They retain their original wording, attribution disclosures, limitations and original execution paths. Their byte identity is indexed in [SOURCE_INDEX.json](SOURCE_INDEX.json) and [SHA256SUMS](SHA256SUMS).

| Reviewer | Scope | Original report |
| --- | --- | --- |
| `reader_fix_review` | AIR2-01, AIR2-P2-01 | [Accounting](records/air2-independent-accounting-2026-10-21.md) |
| `keeper_fix_review` | AIR2-02, AIR2-03, AIR2-08 | [Observation and settlement](records/air2-independent-observation-2026-10-21.md) |
| `remote_scope_check` | AIR2-04, AIR2-05, AIR2-09 | [Vesting and locked depth](records/air2-independent-vesting-depth-2026-10-21.md) |

These are separate AI agents within the same coordinated review, not external firms or separate human auditors. Each original report states its actual production/test overlap. The accounting reviewer authored the newly strengthened regression after detecting that the L39 cap masked the old AIR2-01 allowance-only assertion; that test author overlap is explicit.

## Evidence map and use

The `records/` subtree mirrors the names and layout originally produced under repository `target/`, so original execution paths remain traceable. The unchanged source report's historical raw-log links are mapped explicitly to gzip files below; use the canonical report's adapted links for retained log access. The small `docs/audit/` routing note preserves the observation report's original relative integration-report link without editing that source report.

- [Accounting commands/checks and log hashes](records/air2-independent-accounting-controls-2026-10-21/checks.json), [control driver](records/air2-independent-accounting-controls-2026-10-21.py), [reviewer edge probe](records/air2-independent-accounting-controls-2026-10-21/controls/tests/reviewer_edges.rs), [old-formula loss replay](records/air2-independent-accounting-controls-2026-10-21/controls/tests/pre_fix_loss.rs).
- [Observation/settlement native probe](records/air2-independent-native-probe/src/lib.rs) and [minimal Cargo manifest](records/air2-independent-native-probe/Cargo.toml); its five focused logs use the original filenames alongside the reports.
- [Vesting/depth machine results and log hashes](records/air2-independent-vesting-depth/results.json); its ten command logs are retained in that same directory.

The original controls/manifests retain historical machine-specific paths. They are evidence, not a standalone copied project. Re-execution requires the reviewed checkout/source and matching runtime hashes specified by the reports; review and adapt local checkout/cache paths before running them. The accounting driver reconstructs its isolated `target/` math/test copies from repository source and applies each negative-control formula only inside that copy. The bundle deliberately omits those copied crates and tracked regression/harness duplicates. Its retained control manifest alone is therefore not an independent build tree.

The accounting driver uses fresh source writes before each candidate/variant build. `cached-copy-diagnostic.log` is an explicitly uncredited review-harness stale-cache diagnostic, not a production failure. `floor-lock-existing-regression-masked.log` demonstrates the original weaker test; later `floor-lock-strengthened-regression-negative-control.log` demonstrates the repaired test rejects the old lock floor. Other repeated runs and incidental harness CRC checks are not counted as new unique protocol tests.

Final strengthened `crates/shortwin-math/tests/ai2_accounting.rs` SHA-256: `1638c8b9ba4a058c283545f0d8e6588fd384369b44dee711b01d8d9bc5e2eaa4`. Production `crates/shortwin-math/src/pool.rs` SHA-256: `3d270b521f777b314a3e86048bb5c4396bc78129d4f47eb5898942e30ad55312`.

## Integrity

From this directory, run `sha256sum -c SHA256SUMS`. The checksum index covers every stored bundle file except itself, including compressed bytes. [SOURCE_INDEX.json](SOURCE_INDEX.json) additionally records the original source path, retained destination/encoding, original and retained byte lengths, and original and retained SHA-256 for every source file. The `records/` name avoids the repository's ignored `**/target/` pattern. The three original reports were byte-compared with their source. Every log was either compared directly or decompressed and byte-compared with its original source before indexing. The consolidated report is outside this bundle and has its own published file hash.

## Byte-preserving compressed logs

[COMPRESSION_MAP.json](COMPRESSION_MAP.json) records the complete original-to-retained mapping, uncompressed and compressed byte lengths/hashes, and deterministic gzip parameters. The source reports and machine check records remain byte-for-byte originals and therefore retain their historical `.log` names/hashes. The canonical report adapts active links to `.log.gz`. To inspect a log, use `gzip -dc <retained.log.gz>`; to verify its historical digest, use `gzip -dc <retained.log.gz> | sha256sum`. The following raw-path references are historical names, not additional uncompressed files.

| Historical retained name | Stored log | Original/decompressed SHA-256 |
| --- | --- | --- |
| `records/air2-independent-extra-native-2026-10-21.log` | [gzip](records/air2-independent-extra-native-2026-10-21.log.gz) | `07e8e1d0c9ba4f141af7b3a1ad909158a1073f0f51204405cc7a6998e519e700` |
| `records/air2-independent-pumpswap-2026-10-21.log` | [gzip](records/air2-independent-pumpswap-2026-10-21.log.gz) | `06213a17e7e5edcbbeb5078e7028a6781ef72a1890fdaf8761dc3bf094fe6587` |
| `records/air2-independent-settle-2026-10-21.log` | [gzip](records/air2-independent-settle-2026-10-21.log.gz) | `a6281795657d99a977b11010e471bb029cf6641c8910d943b909bfb154fc0baf` |
| `records/air2-independent-stale-2026-10-21.log` | [gzip](records/air2-independent-stale-2026-10-21.log.gz) | `d75c7f4aeabaa048e33e3d3d953fb86de4f564aa0ca0ce37a937aaf3314f20d2` |
| `records/air2-independent-switch-2026-10-21.log` | [gzip](records/air2-independent-switch-2026-10-21.log.gz) | `98749ec77e83c6e6cf394be827e9cedc2fda46aab40a5e97a0876b0cba3b1256` |
| `records/air2-independent-vesting-depth/math-air2-05.log` | [gzip](records/air2-independent-vesting-depth/math-air2-05.log.gz) | `cd6becc3bc91c53aea2e29095c07bad398d2fc5d8476a36a5c57828e8a0808ea` |
| `records/air2-independent-vesting-depth/math-air2-09.log` | [gzip](records/air2-independent-vesting-depth/math-air2-09.log.gz) | `c7148abe94c6035dee09b808ba2605a885f8d36940e7b53070704b685c0764ed` |
| `records/air2-independent-vesting-depth/math-depth-cap.log` | [gzip](records/air2-independent-vesting-depth/math-depth-cap.log.gz) | `1f8d8610d15b89a6d80a4bed9a5f408d696fac52c8ef6eab90b1276cf3abcd39` |
| `records/air2-independent-vesting-depth/math-depth-properties.log` | [gzip](records/air2-independent-vesting-depth/math-depth-properties.log.gz) | `41c1cb5dcfb8c861a5959f44627fa6f20424bdf5dbbd8760e9334d03041895e0` |
| `records/air2-independent-vesting-depth/math-split-settle.log` | [gzip](records/air2-independent-vesting-depth/math-split-settle.log.gz) | `234de5b5ba00c9d896d2b43199a38ac1d89fcca2fa8d8cf95891c47149d76600` |
| `records/air2-independent-vesting-depth/math-vest-hold.log` | [gzip](records/air2-independent-vesting-depth/math-vest-hold.log.gz) | `0fda7ef3204f0379b63530e3a9f9621b1633cf25865e348e65c3e151ce2e96a9` |
| `records/air2-independent-vesting-depth/svm-cpmm-lp.log` | [gzip](records/air2-independent-vesting-depth/svm-cpmm-lp.log.gz) | `17567491d2a7cb2360055d929e7b281b216fb252040fffb0fd42c2ab6bba35f4` |
| `records/air2-independent-vesting-depth/svm-dust-split.log` | [gzip](records/air2-independent-vesting-depth/svm-dust-split.log.gz) | `86a3293bb2bbd61472efee5df7e2fe1a3ad93dd0030b521785daefd620a9b112` |
| `records/air2-independent-vesting-depth/svm-halted-s3.log` | [gzip](records/air2-independent-vesting-depth/svm-halted-s3.log.gz) | `a8437f35079cbafb34457cb34b7fc1fbbcd993438f0055d9ade2d093a67779f9` |
| `records/air2-independent-vesting-depth/svm-pamm-lp.log` | [gzip](records/air2-independent-vesting-depth/svm-pamm-lp.log.gz) | `e9e2f27a64eb87d54ffa3f396b7cdb93fd720bed677463446a44c01e472e74e2` |
