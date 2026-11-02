# Jupiter execution fixtures

This directory contains only inputs consumed by the active runtime tests, captured-route replayer and configuration verifier. It contains no live provider settings or signing keys.

- `manifest.json`, `accounts/` and `programs/raydium_cpmm_2026_10.so` bind ten historical accounts and the downstream CPMM program used by a Jupiter Router V2 route. Account data hashes, loader headers, slots and available source provenance remain in the manifest. The runtime reconstructs and verifies ProgramData before loading it.
- `jupiter-2026-10-27/` retains the hash-pinned raw program/ProgramData/IDL RPC capture and decoded IDL. Run `python3 scripts/materialize-jupiter-probe.py` from the repository root to create the checked runtime input under `target/`.
- `current-routes/` contains the eight cases selected by `tests/runtime/current-routes.json`. Each case retains its qualification manifest, account manifest and exact raw quote/build/RPC replies. Large replies remain gzip-compressed. The materializer checks raw-response hashes and reconstructs account bindings before the actual-SBF replayer executes them.
- `reference-rent/` retains the 205-byte reference-account rent observation and its request/hash receipt used by the production proposal verifier. This is a dated observation.

`SHA256SUMS` covers the retained corpus files. Check it from this directory with `sha256sum -c SHA256SUMS`. Raw captures preserve their original bytes; embedded program binaries and public account data keep their own terms as described in [third-party notices](../../THIRD_PARTY_NOTICES.md).

Tests supply synthetic initial funding, history, clocks and test lookup tables as stated in each runner. Passing captured cases establishes compatibility for those exact inputs; it does not establish live route availability or provider capacity. Duration evidence belongs to the exact frozen runner, program, source and corpus used for that run.
