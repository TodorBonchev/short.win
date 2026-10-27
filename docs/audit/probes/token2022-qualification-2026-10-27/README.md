# Token-2022 qualification evidence — 2026-10-27

This is preparation and partial execution evidence, not a release approval or a
new 24-hour result. The source/provenance review is retained separately in the
[mainnet preparation record](../mainnet-preparation-2026-10-27/token-2022-source-review/REPORT.md).
The candidate and remaining gates are in the [qualification record](../../token2022-qualification-1.0.3-2026-10-27.md).

- `preflight/` retains exact scope, whitespace, 346-entry static fixture and live
  check outputs, UTC timestamps and hashes. The live check passed at slot
  **461,913,288**, including the new Token-2022 deployment at 460,497,271.
- `ci-40440067371/` pins actual GitHub artifacts to source
  `4c26706cc60b6fce9a2f2eb0e93dd824c2575e91`. Anchor, verifier and runtime-bundle
  program bytes all equal canonical SHA-256
  `d6f4e092dd15639c502535d68bcdb6c428c423b66d49d08f4f89e9409af538bc`.
  The new native fuzzer hash is
  `3ffb12b803f01c1cc60bd82bcc386834e8f148ac0e8fb934df533882953769c3`.
  All ZIP digests, runtime checksums, 2,533 source blobs and **337 tracked fixture
  files** were independently verified. The managed fixture-entry count and file
  count describe different inventories. The compressed workflow checkpoint is
  deliberately partial/failed, not a successful full gate. Raw successful bench,
  service, indexer and smoke logs, exact measured budget tables, and the failed
  browser log/diagnosis are retained compressed. Larger diagnostic trace
  archives remain in ignored `target/`; the review receipt records their identity.
  Reviews here are independent AI-assisted checks, not an external firm's audit.
- `benchmarks/` records exact reconstruction of 13 generated reports from the six
  successful CI job diffs (M1/M2/M3/M5/M6/M7), including before/after Git blobs,
  raw-output bindings and independent review. All 254 printed instruction cases
  stayed within their existing budgets with no Mollusk/LiteSVM mismatch. Every
  delta is an observed CI measurement, not an isolated causal estimate for the
  upstream upgrade. Depth reports were unchanged.
- `keeper-cost/` retains the refreshed deterministic cost-model output and checks
  against actual CI measurements. Only five measured CU inputs changed; model
  functions, fee scenarios and economic/default assumptions did not. Existing
  ECON tables remain historical, with a dated note linking this current output.
  This arithmetic is not a funded-operation or measured mainnet burn receipt.
- `scheduling/` retains exact-source scheduling inputs and independently computed
  Python/Rust draws: 29 clean completed ordinary runs guarantee selection of all
  six profiles; 36 conservatively guarantee one worker's whole all-six prefix.
  This needs the specified two-worker command, seed, successful setup/run/wind-down
  assumptions and actual launch identity. It proves neither equal allocation nor
  24 hours for each profile. No new campaign has started.
- `devnet-policy/` retains the successful manual website deployment and served
  security-route byte/hash observations. Run
  [40442618297](https://github.com/TodorBonchev/short.win/actions/runs/40442618297)
  deployed main `a23a34283c9c83f6c35104867a0409baa7035617`. The live lazy-loaded
  policy discloses no external audit/funded bounty, a 25 SOL per-market opening-
  notional limit, uncapped deposits/market count and intended protocol-fee funding.
  Its HTML still selects devnet RPC; no mainnet deployment is claimed.

The old completed frozen run, metadata and review archives remain unchanged. Its
old duration is not credited to this changed harness/Token-2022 baseline.
