# Installed capability before fresh build

Checked 2026-10-22 on clean consolidated worktree `/home/todor_dev/Projects/short.win-release-devnet`, HEAD `b31c02a091008da18c39d7836c8e0e9d6d018973`.

- `NO_DNA=1 rustc --version --verbose`: rustc 1.98.1, commit `48a229ceaefd4985c50990b14116b6d856af0985`, LLVM 22.1.8.
- `NO_DNA=1 cargo --version --verbose`: cargo 1.98.1, commit `797e8a9bca276c1c9f9f738d2a20f484fa4eea9d`.
- `NO_DNA=1 solana --version`: solana-cli 4.1.2, src 182084b8, feature c763ae0a, Agave.
- `NO_DNA=1 anchor --version`: anchor-cli 1.2.0.
- `NO_DNA=1 cargo-build-sbf --version`: cargo-build-sbf 4.1.0, **default** platform-tools v1.54/rustc 1.89.0. Explicit build flags select v1.57 and SBPF v3 as required.
- `/home/todor_dev/.cache/solana/v1.57/platform-tools/rust/bin/rustc --version --verbose`: rustc 1.95.0-dev, commit `ae660768afd467d131776918a1627e18920b230c`, LLVM 22.1.2. v1.57 version.md names cargo revision `bbbd3a76017677319ff85d096a7c9bee9813c3db` and newlib `f0f8003daa3f71a1a3e2f067f8d5bb46e2756514`.
- Cached v1.57 tools 1.7 GB, Cargo registry 1.4 GB and Git sources 155 MB; 162 GB free on /home. Isolated worktree initially had no target directory.

Installed binary SHA-256:

| Binary | SHA-256 |
|---|---|
| `.cargo/bin/anchor` (AVM symlink) | `f53377785c9c51627db7a605b6d3a16afe3c8e70a423483e8e866183b9ac7a57` |
| `.avm/bin/anchor-1.2.0` | `0c9c41a3292c281cc6eadb78d6e1c8224d8324a34b0736a89d640fd314db05b7` |
| active Agave `cargo-build-sbf` | `e74835c5f7b9147151f8d1970e4433a3e5c6f95fc9b1c1bcaffdb423b54678bd` |
| v1.57 cached rustc | `f7a4ec1b62e27fa990de4c9bd3c38eb7a5ba0a4f41b2e0929a0631c68c9c6e78` |

**Provenance qualification:** `.cargo/.crates.toml` records AVM 1.2.0 installed from `solana-foundation/anchor#2b4b2fd22a881228e02c41b26bd23addb5b8b26f`. This is the AVM router receipt, not proof of the separately downloaded `anchor-1.2.0` binary's exact source commit. CI pins `otter-sec/anchor#84a63f9f7112b23581816436fbcf3f6c515779f9`. Installed CLI version matches; exact installed CLI source provenance is unestablished by the available receipt. Actual fresh output/expected-byte comparison must be recorded separately. No tool installation or network/config changes were performed.

Source SHA-256 before build:

| Path | SHA-256 |
|---|---|
| `Cargo.lock` | `0732c477fe1e6b38fd74d558e23b8e3ebda8790705a6ddff1de6fdaca72b316b` |
| `programs/shortwin/Cargo.toml` | `02ae56bcfc61f0b1d6a83313afb65c4f7b4950ed3f5dc0fe87001b0b0d5feccb` |
| `programs/shortwin/src/instructions/add_margin.rs` | `172a646ead5d8e08ffbffe5e696a241f5067160be31dbc7f334b14e74d7636c1` |
| `clients/js/idl/shortwin.json` | `5b94b1e98f5b32cf6d3c501d1998965bbbab80d6e2853fa6eeb8f3ba5470a003` |
| `.github/workflows/ci.yml` | `720e58ed6010a0da59bb2dfbab020472df8468be78d62e9ee0828a80015008db` |
| `rust-toolchain.toml` | `1e977a72694dfd8a4b1b5d2253f22ce5dad4e09bf522c902236476b1801dd803` |

`run-recorded.py` is external orchestration: it records exact argv, cwd, selected env, HEAD/status, start/end, exit, wall time and output hash, bounds process time, and writes full logs here. Its fixed target is the isolated worktree's `target/program-readiness-2026-10-22`; it never points at primary or fuzz-cache products. All target invocations are locked/offline, NO_DNA=1, two jobs; no signing/deployment is performed.
