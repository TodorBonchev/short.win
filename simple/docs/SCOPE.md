# Scope of the replacement

Production on-chain: `programs/shortwin-simple/src/*.rs`, `crates/pooled-core/src/*.rs`, and the internal stateless header/isolation crate `crates/jupiter-route/src/*.rs`. Their Cargo manifests and the standalone `Cargo.lock` pin the build graph. There is no dependency on the legacy `shortwin` program or venue readers.

Production off-chain: `client/src/` (generated IDL client, address/route/transaction helpers and Node-only operator), `client/idl/`, `services/src/` (trusted Python history), and `web/src/` (development frontend). The operator's signing and networking are outside the on-chain program; private history is explicitly trusted. A built browser bundle is not an operating receipt.

Test-only: `tests/runtime/`, `crates/pooled-core/tests/`, `crates/pooled-core/examples/model_fuzz.rs`, `client/test/`, `services/tests/`, and `web/test/`. Runtime tests use a hash-pinned historical real Jupiter/CPMM witness with explicitly synthetic starting allocations, clocks, reserve changes and local lookup table. No test-only route driver or mock is compiled into the production program.

Documentation/build: `README.md`, `docs/`, `.github/workflows/simple-pooled.yml` (repository root), `Anchor.toml`, JS manifests/lockfile and client generator config. Existing legacy scope records, audit findings and archived checksums retain their original meaning. This new scope must receive its own final freeze and qualification before tagging.
