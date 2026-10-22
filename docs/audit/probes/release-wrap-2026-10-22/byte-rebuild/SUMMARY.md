# Deterministic release artifact comparison, 2026-10-22

Status: PASS for ordinary release builds and byte equivalence. No fuzz run,
security review, deployment or protocol transaction was performed by this task.

Both isolated clones were checked out at
`7b11cc7f85503474c6b87e0ee8b2fedd51bbc249`. Their tracked files remain clean after
building. No preexisting target outputs were copied into either build checkout.
The clones contain committed inputs only; primary-checkout target/key files were
not mounted into Docker.

## Measured build results

- Fresh host `anchor build --tools-version v1.57 --arch v3 --ignore-keys --no-idl -- -- --locked`:
  exit 0, 83.021582531 seconds. This builds the normal release and mock artifacts.
- Real Docker `solana-verify build --library-name shortwin --arch v3 --cargo-build-sbf-args="--tools-version v1.57"`:
  exit 0, 138.173376603 seconds. It logged the Agave 4.1.2 image and compiled the
  unique `programs/shortwin/Cargo.toml` library. Digest-pinned image identity was
  pulled and independently inspected. Container host Rust is 1.95.0; the SBF
  compiler comes from requested platform-tools v1.57, with SBPF v3 selected.
- Fresh `bash scripts/build-test-venue.sh`: exit 0, 74.765012096 seconds. This is a
  test-only artifact build; no test or fuzz executable was run.

| Artifact | File SHA-256 | Bytes |
| --- | --- | ---: |
| Host and Docker release | `d6f4e092dd15639c502535d68bcdb6c428c423b66d49d08f4f89e9409af538bc` | 1644024 |
| Host test-venue | `0a405deba60a521798820985cd4cec1a0e61e46ed9d0ee829fd037b010565a01` | 1675056 |
| Host mock | `4cf1700761782aece1a4e3463ce0b2b9f2afaed2ecf3d0a6362f115faf9c7ab9` | 281240 |

All are SBPF v3. Full byte comparisons against the preexisting canonical
release/test/mock files passed. Fresh host and Docker release bytes also compare
identically. The release executable hash, after trimming trailing zero bytes, is
`f3d8dd7a5c62385abc6306e2da395ff666a1f69234ea50f2e7eb0523ab686250`.
The verifier build's printed hash and a separate `get-executable-hash` command
agree with the independently computed trimmed hash.

The deployment candidate is `artifacts/release/shortwin.so` in this evidence
folder. `artifacts/verifier/shortwin.so` is identical. Files under
`artifacts/test-only/` must never be deployed as the release.

## Source relationship to completed historical fuzz

The existing completed run in the owner's supplied record
`docs/audit/release-checks-1.0.3-2026-10-22.md` used frozen runtime source
`e04a6776f1c2f5e02e581d76dd1255c416270275` and the same three program artifact
hashes measured above. Git object comparisons establish unchanged `tests/fuzz`,
`tests/litesvm` (including fixtures), `tests/mock_cpamm`, runtime math source,
Cargo.lock, Cargo.toml, Anchor.toml and rust-toolchain.toml.

Between those commits, program sources differ only in two module-documentation
lines in `add_margin.rs`; the corrected wording now preserves original line
counts. `crates/shortwin-math/tests/ai2_accounting.rs` has a previously integrated
standalone host-test addition. The test-venue shell script has the previously
integrated argument-forwarding/`--locked` correction. Actual newly built bytes,
not an assumption about comments or scripts, establish program artifact identity.

These measurements resolve the intermediate article-comment diagnostic-byte
mismatch and support applying the already completed historical on-chain fuzz
result to the same runtime artifacts and unchanged fuzzer/harness/fixture inputs.
They do not constitute a new 24-hour run, extend its duration, add coverage, test
changed keeper/indexer/client code, or close outstanding review/deployment gates.
No fuzzer executable was rebuilt or run here.

## Tool and receipt limits

Host versions were Anchor CLI 1.2.0, Agave 4.1.2, solana-verify 0.5.2 and Rust
1.98.1. The installed Anchor binary and AVM wrapper hashes are retained; this task
did not establish that the installed Anchor CLI was built from CI's exact
`84a63f9f7112b23581816436fbcf3f6c515779f9` source revision. The separately pinned
real-Docker release build and exact byte comparison supply independent artifact
reproducibility evidence.

Docker image:
`solanafoundation/solana-verifiable-build@sha256:2e0b78f44ee76612e9260c7c988570c5e14de6fbd93e0ab07115ec7054473b4f`.
Docker access used a `newgrp docker` child; no socket permissions or sudo policy
were changed in this task. The standard wrapper's post-build `sudo chown` was not
run: direct equivalent verifier checks were performed in an isolated clone.
Root-owned container build output remains isolated and was copied read-only into
user-owned artifact copies. The verifier command did not pass `--locked`; the
post-build Cargo.lock SHA and Git blob equal committed Cargo.lock in both clones,
and both complete tracked working trees remain clean.

See `byte-comparison.json`, `source-input-identity.json`,
`build-checkout-identity.json`, `docker-image-identity.json`,
`host-tool-identity.json` and the command receipts/logs. `manifest.json` hashes
these retained evidence files and artifact copies; clone targets are intentionally
outside that manifest.
