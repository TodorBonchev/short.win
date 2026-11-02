# Implementation scope

The repository contains the Jupiter Router V2 pooled implementation.

| Surface | Source |
| --- | --- |
| On-chain program | `programs/shortwin-simple/src` |
| Pool accounting | `crates/pooled-core/src` |
| Jupiter header and isolation validation | `crates/jupiter-route/src` |
| Generated client and Node operator | `client/src`, `client/idl` |
| Trusted exact-quantity history | `services/src` |
| Development wallet frontend | `web/src` |
| Production proposals | `config` |

The root `Cargo.toml` and `Cargo.lock` pin the Rust workspace. The root pnpm workspace and lockfile pin the client and frontend. [.github/workflows/pooled.yml](../.github/workflows/pooled.yml) builds and checks these components.

`tests/runtime`, core accounting tests, client/service/frontend tests and `config/test_production_proposal.py` are test-only. Runtime tests load the actual program SBF with pinned historical Jupiter/CPMM bytes. Captured route tests also exercise Jupiter routes to the DEX programs present in their snapshots. DEX fixtures are test dependencies, not direct production venue adapters.

The tests explicitly use synthetic funding, clocks, local lookup tables, authenticated history and a labelled reserve shock. Their case counts do not establish live provider capacity, arbitrary multi-loan interleavings, universal route compatibility or deployed operation.

Mint admission accepts initialized plain SPL and plain/metadata-only Token-2022 mints without freeze authority, up to 4096 bytes. Transfer-changing, unknown, duplicate and malformed extensions are excluded. Token vaults and scratch accounts remain 165 bytes.

`config/*proposed.json` and `config/production-roles.template.json` are review inputs. They do not prove role assignment, funding, deployment approval, measured service capacity or delivered alerts. Mainnet commissioning remains separate from software qualification.
