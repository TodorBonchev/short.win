# short.win

Jupiter Router V2 pooled lending, with SOL collateral, fixed-term token debt and paid SOL income for lenders. The Rust, Python and TypeScript workspaces run from this repository root.

The software version is **1.1.1** and the program ID is `BCPiRMTMoLNY5EHm2RYxoxJChgyrttkFX2Qe2Rnbe3jh`. Deployment and funded operation remain to be commissioned. Package names, generated instruction identities and the program ID are preserved across the repository layout change.

- One active pooled vault per supported token, token-denominated lender shares and separately claimable paid SOL income.
- Jupiter-only sale and whole-debt buyback; borrowers can also repay the exact token debt directly.
- Fixed borrowing fee paid upfront; early close does not refund it.
- Trusted off-chain exact-quantity price history, with public registration and bounded automatic warm-up.
- Confirmed pre-maturity liquidation, maturity buyback and collateral default at deadline plus 24 hours.
- Plain SPL and metadata-only Token-2022 mints, with borrowing, wallet and utilization limits. Deposits and market count are uncapped.

Withdrawals depend on available token liquidity. Default can lose token principal and return SOL compensation. Read the [specification](docs/SPEC.md), [scope](docs/SCOPE.md), [operations guide](docs/OPERATIONS.md) and [production proposals](config/README.md).

## Repository

| Path | Purpose |
| --- | --- |
| `programs/shortwin-simple` | On-chain pooled lending program |
| `crates/pooled-core` | Checked pool accounting |
| `crates/jupiter-route` | Jupiter scalar-header and account-isolation checks |
| `client` | Generated instruction client, transaction helpers, operator and HTTP bridge |
| `services` | Trusted Python/SQLite price history and queue lifecycle |
| `web` | Development wallet interface and public recorded showcase |
| `tests/runtime` | Actual-SBF ledger, route and financial fuzz tests |
| `tests/fixtures` | Pinned Jupiter execution inputs and captured route cases |
| `config` | Non-executable operating and policy proposals |
| `scripts` | [Capture, replay, build and release tools](scripts/README.md) |
| `tools/social-video` | Editable social workflow films and verified exports |

## Local checks

Use the toolchain versions in `rust-toolchain.toml` and `mise.toml`. Run from the repository root:

```sh
python3 scripts/materialize-jupiter-probe.py
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
python3 -m unittest discover -s services/tests -v
python3 -m unittest discover -s config -p 'test_*.py' -v
python3 config/verify-production-proposal.py
cargo build-sbf --manifest-path programs/shortwin-simple/Cargo.toml --tools-version v1.57 --arch v3 --sbf-out-dir target/deploy -- --locked
cargo test --workspace --locked
pnpm install --frozen-lockfile
pnpm check
pnpm build
```

The [runtime guide](tests/runtime/README.md) describes seeded fuzzing and captured-route replay. Tests use pinned snapshots, synthetic history, clocks and initial allocations; they do not send transactions. Qualification and duration results apply to their exact frozen source, runner, program and inputs. A campaign running from a separate frozen checkout retains that checkout's identity.

The [capped-mainnet release guide](docs/MAINNET_RELEASE.md) prepares a hash-bound, non-executable packet for the exact qualified ELF. Role assignment, funding, alerts, operating capacity and final activation checks remain required before launch.

The [security policy](SECURITY.md) explains private reporting. [LICENSE](LICENSE), [GRANTS.md](GRANTS.md), [REUSE.toml](REUSE.toml) and [third-party notices](THIRD_PARTY_NOTICES.md) define the applicable terms; a software tag does not authorize deployment or financial transactions.
