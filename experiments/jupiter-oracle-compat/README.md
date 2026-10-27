# Jupiter-derived oracle SDK compatibility probe

This is a standalone, manual native experiment. It does not join the onchain
workspace, import production short.win, create a feed, select an oracle queue or
trust policy, sign data, call oracle/RPC endpoints, build SBF or submit transactions.
The owner selected signed Jupiter-derived feeds; concrete settings still require review.

## What this probe tests

- Actual published `switchboard-on-demand=0.13.0`, default features disabled,
  `no-entrypoint,solana-v3`; exact Anchor1.2.0 and current root component versions.
- Direct AccountInfo type compatibility; actual QuoteVerifier instruction-sysvar
  reader type-checking; SDK packed-feed accessor ABI and malformed parser refusal.
- Pinned queue/owner/sysvar identities, fixed feed/mint/raw-amount mapping and
  synthetic post-verification freshness/quorum/slot-regression/integer-unit checks.
- Real Anchor instruction-data construction plus explicit byte/privilege preserving
  conversion of the SDK's Instruction type to the pinned caller Instruction type.

The SDK's feature name `solana-v3` permits a monolithic solana-program>=3 dependency.
This probe pins actual solana-program4.0.0 because its component ranges match root
AccountInfo3.1.1, Clock3.1.1, Instruction3.4.1 and Pubkey4.2.1. It does not claim an
actual3.x monolithic build. Unpinned5.x introduces different component versions.
The exact feature graph and resolved versions must be inspected from CI evidence.

## Evidence boundaries

There are ZERO successful valid-attestation tests: no private keys or real signatures
are available or requested. Synthetic helper tests never replace SDK verification.
The manual run will compile the complete reader function; no successful native
compile is claimed before its receipt. No happy-path onchain Ed25519 result, queue
deployment identity, oracle quorum, managed-update account or source freshness is
established here. Raw fixed-input output units are not Bhat(q) or executable cost bands.
Equal quote slots may be reused while fresh; the helper checks slot regression,
not a strict action nonce/replay policy. A signed minimum quorum must also be met.

Published0.13.0's `ParsedEd25519Instruction::to_instruction_bytes()` is an upstream
`unimplemented!` reconstruction helper. A test explicitly documents that expected
panic. It is never used as a signed-update builder. The existing JS managed-update
builder and its Kit instruction bridge need separate qualification; no homegrown
signer or replacement serializer is introduced here. Passing this experiment would
not complete oracle integration or qualify protocol release/deployment.

## Numeric unit boundary

The reader uses `PackedFeedInfo::feed_value()` as raw i128, never f64 or the SDK's
`value()` Decimal convenience accessor. It requires exact division by10^18 and checked
conversion to u64. A native test covers `u64::MAX *10^18`: it fits i128 but exceeds the
96-bit mantissa accepted by Decimal::from_i128_with_scale. SDK value() invokes that
constructor before normalize(), so it can panic for this valid raw-token quantity.
The test contrasts25SOL expressed as lamports*10^18, which fits96bits. No fixed USDC
fixture is extrapolated to every token unit or generic risk-cost representation.
See [Rust Decimal constructor source](https://docs.rs/rust_decimal/latest/src/rust_decimal/decimal.rs.html).
The actual resolved Decimal version/source is retained with the standalone lock.

## Run and retain

The manual-only `.github/workflows/jupiter-oracle-compat.yml` runs native checks on
a GitHub runner. It resolves an isolated ignored Cargo.lock, then uses --locked for
metadata, feature tree, test and clippy. It retains that lock, versions, raw output,
exact repository and probe source, published SDK archive, all selected downloaded
registry source archives, source hashes and diagnostics even on failure.

Initial resolution is not reproducibility evidence by itself. Review the retained
lock, source identity, actual feature graph and artifacts before reusing it. No root,
service or pnpm lockfile is changed; the script hashes protected files before/after.

Local source/config-only validation (no resolution/build):

```sh
python3 experiments/jupiter-oracle-compat/run_probe.py --validate-only
rustfmt --check --edition 2021 experiments/jupiter-oracle-compat/src/lib.rs
```

The workflow is prepared for owner/root review only. This change does not dispatch it.
GitHub requires a workflow_dispatch definition on the default branch before manual
feature-ref dispatch; registration/merge is a separate reviewed step. See the
[GitHub manual-run documentation](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/manually-run-a-workflow).
Do not add push/PR triggers to bypass that restriction.
