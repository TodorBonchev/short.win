# Pooled workspace build isolation — 2026-10-28

This follow-up supersedes the runtime identity of the [initial implementation checkpoint](simple-pooled-implementation-2026-10-28.md). No stable tag, deployment or 24-hour campaign is approved by this record.

## Cause and controlled experiment

[CI 40728412575](https://github.com/TodorBonchev/short.win/actions/runs/40728412575) passed all checks on `f4377093e1058c2aaf0c9b26865b092d7cec641c`: Rust/runtime/SBF/stack/IDL, Python 13, client/operator/bridge 19, web 9 and its 32-case program smoke. Its ELF was `6330465280e6f27bfb6c6a3b21c853932ff385e7025b16c3d05ffb26770cbbe7`, while two clean local package rebuilds consistently produced `49f11f900aa9a368638dcb6a624685a85d59909c805086403ae6765bd539a54a`.

Both files were 770,800 bytes with identical headers and read-only data. The difference was exactly a 912-byte allocator-helper block versus a 1,528-byte entry block changing order, plus 201 relative call relocations; undoing that permutation and relocation mapping left zero residual text differences. This analysis did not treat functional equivalence as byte equality.

The pure Jupiter parser dependency lived outside the `simple/` Cargo workspace. Cargo 1.95's stable source hashing strips the workspace prefix for internal path packages, but retains an external path; dependency metadata then feeds the consuming crate's identity. See the retained [independent source review](simple-pooled-repro-evidence-2026-10-28/independent-cargo-anchor-review.md). The controlled experiment archived the same committed source and built it locally at CI's exact checkout path. It reproduced CI's `633046…` file byte for byte, using the same local compiler/cache. The pooled-core and Anchor dependency identities stayed the same; only the external parser identity and consuming program metadata changed. This establishes the build-path cause; no Anchor fork or economic change was needed.

## Self-contained correction

The parser's Rust files are copied byte for byte into `simple/crates/jupiter-route`, an explicit workspace member. The program now depends on that internal path. The manifest retains the repository license; the original component remains preserved. The copied README has an added provenance preface. The fuzz runner freezes the actual internal dependency and license.

The first corrected local SBF v3 is `b87598a00c788c30a64b4e232f5f2040c0c8eb333c0cce5fb09403d27159544c` (770,848 bytes). Workspace tests passed (13 parser, 16 ledger plus randomized regression, 8 program unit and 22 runtime tests). The new program smoke completed 32 cases in 21.800449815 seconds, with 147 driver transactions (98 success, 49 expected rejection), all eight reported families, 65 CPMM success logs and zero failures/anomalies/setup/invariant failures. The later README provenance preface is documentation-only and changes no runtime bytes.

The corrected source commit `571b4171c4e98cfe136fab27daf0e173c444d7ab` was then archived and rebuilt at the separate CI checkout path. It produced the exact same `b87598…59544c` ELF, 770,848 bytes, SBF v3; direct byte comparison passed. The archive SHA256 is `47121c930173fb36901c1596df0d99976fd1cc42c759e20c885df4e1ede134cc`. This closes the observed checkout-path identity defect. Corrected-candidate hosted CI is still pending at this point; its terminal result will be appended after verification. The [exact evidence](simple-pooled-repro-evidence-2026-10-28/) retains the earlier successful CI identity, the controlled experiment and corrected local checks. The CI workflow also keeps frozen smoke inputs in runner temporary storage, outside the Cargo cache scanner.

## Remaining release gates

A new release line (proposed 1.1.0), production program/role/settings review, final-source dependency compatibility and additional routed cases, an independently supervised >=24h frozen campaign, and funded measured operator/HTTP/web/monitoring commissioning remain open. Neither successful short smoke nor the old program's duration closes these gates. Mainnet-before-external-audit/bounty timing remains the owner's decision; no external-firm report is claimed.
