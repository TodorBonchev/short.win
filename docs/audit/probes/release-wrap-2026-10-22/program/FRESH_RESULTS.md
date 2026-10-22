# Focused fresh program/math verification — 2026-10-22

Root authorized focused native build/verification after the read-only readiness inventory. Tested consolidated source: `/home/todor_dev/Projects/short.win-release-devnet`, HEAD **`b31c02a091008da18c39d7836c8e0e9d6d018973`**. All build/test commands used **NO_DNA=1, CARGO_BUILD_JOBS=2, CARGO_NET_OFFLINE=true, RUST_TEST_THREADS=2, PROPTEST_CASES=2000**, with isolated `CARGO_TARGET_DIR=/home/todor_dev/Projects/short.win-release-devnet/target/program-readiness-2026-10-22`. No primary/fuzz-cache product was overwritten, no source edit was made by this agent, no deployment/signing/network config/private-key action occurred. No stopped audit fixture/review was reconstructed or retried. The normal SBF workspace command also builds the test-only mock as CI does; the release shortwin artifact never enabled `test-venue`.

## Fresh outcomes

| Command | Exit | Wall time | Evidence |
|---|---:|---:|---|
| `anchor build --tools-version v1.57 --arch v3 --ignore-keys --no-idl -- -- --locked` | 0 | 124.76s | `sbf-build.json`, full `sbf-build.log` |
| `anchor idl build -p shortwin -o target/program-readiness-2026-10-22/idl/shortwin.json -t target/program-readiness-2026-10-22/types/shortwin.ts -- --locked` | 0 | 37.30s | `idl-build.{json,log}` |
| `diff -u clients/js/idl/shortwin.json target/program-readiness-2026-10-22/idl/shortwin.json` | 0 | 0.006s | `idl-diff.{json,log}`; log empty |
| `cargo fmt --all --check` | 0 | 1.52s | `fmt-check.{json,log}` |
| `cargo test -p shortwin-math -p shortwin-math-wasm -p squads-v4-ix --locked` | 0 | 52.94s | **427 passed, 0 failed**; math386, WASM facade28, Squads13; `math-tests.{json,log}` |
| `cargo clippy -p shortwin-math --all-targets --locked -- -D warnings` | 0 | 8.23s | `math-clippy.{json,log}` |
| `python3 scripts/check-a49.py --has-id target/program-readiness-2026-10-22/deploy/mock_cpamm.so --no-id target/program-readiness-2026-10-22/deploy/shortwin.so` | 0 | 0.064s | `a49-check.{json,log}` |
| `python3 scripts/check-security-txt.py --forbid-placeholders --source-release v1.0.3 target/program-readiness-2026-10-22/deploy/shortwin.so short.win` | 0 | 0.064s | `security-txt-check.{json,log}` |
| External byte/section/hash/marker comparison | 0 | 0.114s | `artifact-comparison.{json,log}`, `compare-artifacts.py` |

Source remained clean during SBF/IDL/format/artifact checks. During math tests, root-authorized concurrent work added changes to `apps/web/src/config/parseConfig.ts` and `parseConfig.test.ts`. `git diff --exit-code b31c02a -- programs crates Cargo.toml Cargo.lock Anchor.toml rust-toolchain.toml clients/js/idl` returned0 afterward: all source relevant to this assignment remained the tested revision. This is not a full CI claim, whole-program SBF execution claim, new fuzz result or audit closure.

## Fresh products and exact difference

| Product | Size | SHA-256 |
|---|---:|---|
| Fresh release `target/program-readiness-2026-10-22/deploy/shortwin.so` | 1,644,024 | **`d8d08784b3b3081d5fe1cf7aaff319e7e9e063733c308bd2e9e7ed8981bee182`** |
| Fresh mock `target/program-readiness-2026-10-22/deploy/mock_cpamm.so` | 281,240 | `4cf1700761782aece1a4e3463ce0b2b9f2afaed2ecf3d0a6362f115faf9c7ab9` |
| Fresh IDL `target/program-readiness-2026-10-22/idl/shortwin.json` | — | `5b94b1e98f5b32cf6d3c501d1998965bbbab80d6e2853fa6eeb8f3ba5470a003` |
| Fresh TS type `target/program-readiness-2026-10-22/types/shortwin.ts` | — | `dbbaaa8fd290c9bb542fe38a6ed74701027d1b9db1e6f770368371c4aba74360` |

Fresh release trimmed executable hash: **`8531f4958aaff2c5050d478ec8752a7b947e70224da3d0a08e27dedfa5107112`**. ELF flags3, one valid v1.0.3 security.txt, no mock ID and no test-venue marker. Fresh mock equals prior canonical mock. Fresh IDL equals the committed IDL byte-for-byte, including1.0.3 metadata/304-byte guard layout; JS agent received this output path.

The existing primary release ELF was read-only hashed first and matches the retained canonical `d6f4e092dd15639c502535d68bcdb6c428c423b66d49d08f4f89e9409af538bc`, so it is a valid old-byte comparison source rather than a fresh-build claim.

**Exactly one byte differs across the complete 1,644,024-byte files:** zero-based file offset **330428**, old72/new73 (hex48→49). `.rodata`, section layout and all other bytes are identical. The change is within `.text` instruction **0x100046d70**, `stw [r10 - 0xf98], 0x48`→`0x49`. Adjacent immediate0x17bc is error6076 (`ZeroAmount`, `programs/shortwin/src/errors.rs:458`). The unchanged `require!(amount > 0, ShortwinError::ZeroAmount)` moved from old source line72 to current73 after the module-doc edit (`programs/shortwin/src/instructions/add_margin.rs:73`). Anchor1.2.0 `require!` expands to `error!`; `anchor-attribute-error` builds `ErrorOrigin::Source { filename: file!(), line: line!() }` (installed locked crate `src/lib.rs:109–111`). This attributes the single changed immediate to error-origin diagnostic line metadata, with no different economics or control-flow bytes found.

Old and fresh disassembly command/exit/full text are retained in `canonical-disassembly.{json,log}` and `fresh-disassembly.{json,log}`. Actual complete-file difference/section SHA records are in `artifact-comparison.log`.

**This is a new candidate artifact, not prior canonical byte identity.** No comment manipulation or fuzz/source-gate waiver was made. The explanation narrows the change; it does not turn the old24h run into a run on d8d08784.

## Remaining meaningful gates

- The earlier completed24h run is valid evidence for its old frozen bytes. Current d8d08784 has not had a fresh qualifying final-byte run. Under the written AUDIT§12/current release policy, freeze the chosen current source, program/fuzzer/fixtures and run/retain the required final-byte gate before stable release; do not combine old duration, change source only to inherit hashes, or call this completed build a new fuzz result.
- Exact current candidate G-full and reproducible-verifier byte comparison remain unrun here. `scripts/verifiable-build.sh --compare <fresh-deploy-dir>` should reproduce the actual chosen release artifact. Toolchain inventory caveat remains: installed AnchorCLI1.2.0 exact source revision is not proven by AVM's receipt; CI pins otter-sec84a63f9. This successful build used matching versions and cached pinned compiler, not a freshly installed/proven pinned CLI. Preserve these distinctions if assigning CI/verifier.
- Any newly implemented material finding fix still needs its own fail-before/pass-after regression, green full gate, independent fix review and final-byte fuzz per policy. No new finding/source fix was established here.
- Live upstream/config disposition, actual target-cluster venue/oracle availability, operator funding/alerts/running compatible304-byte/destination3 identities, and final reviewed deployment authority remain as documented in READINESS.md. Seven unresolved candidates/23undisposed entries remain unresolved; neither full audit nor excluded tasks are complete.

## Uninitialized devnet shell: narrow source assessment

`programs/shortwin/src/instructions/initialize_config.rs:33–35` requires `authority: Signer`. Lines92–101 require the **canonical short.win ProgramData PDA** derived under `bpf_loader_upgradeable::ID`, with its stored `upgrade_authority_address == Some(authority.key())`; the typed ProgramData account also supplies Anchor's owner/state validation. An arbitrary public signer cannot front-run or claim protocol initialization. Config/guard/extras are `init` PDAs, so initialization is one-shot (`:37–90`). The handler validates role keys/distinctness, timelock, params and oracle nondefault key (`:105–129`, `state/config.rs:769`); these checks do not prove actual operating governance/oracle availability.

Retaining a deployed upgradeable shell without configuration can support address/loader-authority/ELF verification. It cannot demonstrate functional protocol E2E: ordinary deposit requires a valid initialized GlobalConfig AccountLoader and expected version (`instructions/deposit.rs:54–59`), and no markets/config/operator readiness is supplied by shell existence. Removing the short.win upgrade authority **before initialization** makes `Some(signer)` impossible, so the shell could no longer initialize through this interface. No such action was performed or proposed.

The loader deployment and program initialization are separate operations. Immutable-Squads verification in governance tooling (`services/ops/src/verify.rs:273–289`) is not an on-chain permissionless-init gap and does not prevent the loader from holding an uninitialized short.win ELF. A shell-only devnet stage is therefore limited artifact preflight under explicit root/user deployment authority; it does not bypass or close the reviewed initialization/governance blocker, does not make the app usable, and does not establish stable release readiness. No bypass, source/key/config changes or transaction were attempted by this agent.
