# Final 1.0.3 frozen fuzz evidence (2026-10-22 Europe/Kyiv)

The run completed naturally at 2026-10-21T22:21:37.094749Z, exit 0. Supervisor
elapsed 86,428.073717718 seconds; runner wall time 86,426.897880876 seconds.
38,563 completed runs/wind-downs, 7,330,667 actions, 87,632,843 transactions and
158,970,339 hook checkpoints; failures `[]`, failure_counts `{}`, anomalies `{}`.

`completion.json`, `summary.json` and `run.log` preserve exact final bytes. The
supervisor's final summary/log hashes match independent rehashing; `SHA256SUMS`
checks every retained file. `metadata.json` is the unchanged launch record, anchored
at SHA-256 `3cb472ecc4a911394e4e6cbe40299ca6486a9f780c6790d87bf8ff57bc772e4c`.
`independent-final-verification.json` checks fixed identity anchors, all four frozen
binaries, source archive, all 334 fixture files, source/CI byte equivalence and the
source-pinned six-profile selection witness. Original binaries, source.tar and
read-only fixtures remain at the frozen local directory named in metadata.

`monitor-verification-notes.md` and `running-identity-witness.json` are dated
pre-completion observations; their partial counters are not final totals. The notes
prove all six profiles by deterministic seed scheduling (29 completed runs suffice;
36 is a conservative sufficient bound). The final 38,563 clean runs exceed both.
There are no explicit per-profile totals or equal-time/per-profile-24h claims.

Coverage proxies are 28/30 landed instructions and 68/111 returned error codes;
full instruction/event/error maps are retained in the summary. initialize_config
and create_market execute during setup before Stats collection. LegacyPoolCarry
was not returned naturally; its explicit matrix/signed regression and old-code/ELF
negative controls provide its targeted coverage. Refused transactions are expected,
not invariant failures. The recorded wind-down economic residuals remain: the
largest present amount owed against absent-owner coverage is 524,012,267 lamports,
and one absent-deficit market remains. A clean oracle does not promise eventual
claim payment, keeper profitability, principal protection or funded operation.

This evidence closes the final frozen-runtime fuzz requirement. Supported venue
inputs and short.win runtime are unchanged; later audit/docs and package-age-policy
cleanup do not extend this result to changed runtime/fuzzer/fixtures. Release
preflight, owner review and any future upstream promotion remain separate.

## Incoming publication update

The source/CI identity checks above were performed before remote `56f660027` was
integrated. `incoming-source-equivalence.json` and `remote-56-review/` record the
later keeper/indexer/build changes and new CI separately. Frozen program/fuzzer/
fixture inputs are unchanged; old service/workflow equality claims are dated
snapshots, not claims about the incoming files. The two skipped new-CI jobs include
mutants; actual 19-kill evidence remains from the earlier manual `ff85a1b0` run.
