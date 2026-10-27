# Dependency advisory disposition — 2026-10-27

This is a usage-scoped dependency review for the current release candidate, by
separate AI engineering and review agents. It is not an external audit and does
not declare the affected upstream packages fixed. The retained failed CI remains
failed; fresh CI must validate the candidate after the actual host patches and
explicit, temporary nonapplicability dispositions below.

[CI 40345534308](https://github.com/TodorBonchev/short.win/actions/runs/40345534308)
on `7f2af5b5787d62978c79e7630f5b0e1a3310d94e` completed with 22 passing jobs,
two failed jobs (`deny`, `services`) and two intentional skips. Both failures
were dependency advisories; they were not web test failures. Retained
[run summary, failed log, authoritative advisory records and exact-source
checks](probes/dependency-advisories-2026-10-27/) identify their original result.
RustSec advisory database revision is
`3279c99911354c400e1d41e47c7d60e39258b147`.

## Actual host dependency patches

Only `services/Cargo.lock` changes dependency resolution:

| Package | Before | After | Advisory |
| --- | --- | --- | --- |
| Tokio | 1.53.1 | 1.53.3 | RUSTSEC-2026-0386: unsafe I/O descriptor registration |
| ctutils | 0.4.2 | 0.4.4 | RUSTSEC-2026-0385: constant-time mask/comparison operations |

The version/checksum blocks are the complete lockfile delta. Unrelated Windows
resolver changes were discarded. Locked no-dependency Cargo metadata passes;
full offline metadata stops at an absent `Inflector` download after the earlier
cache cleanup. That is not a successful local compilation or service test. The
new services build/tests and dependency scans require fresh CI. New service
artifacts must be built and identified before VPS operation; old devnet binaries
do not acquire these patches merely because the lockfile changes.

Services **do not receive a ctutils exception**: their graph also contains
potentially keyed `digest`/HMAC consumers, including SQLx MySQL support in the
resolved graph. The narrower root argument below is not sufficient for services.

The root/standalone benchmark lockfiles, program, math, fuzzer and fixtures are
unchanged. Prior frozen-runtime evidence retains its original source/input
attribution; no new fuzz duration or byte reproduction is claimed here.

## Temporary nonapplicability, not patched-package claims

Every disposition below expires at **2026-11-27 00:00 UTC**. The CI expiry check
fails on that date unless the exceptions are removed or a new scoped review is
retained. It checks expiry and policy metadata, not arbitrary future source
semantics. A changed caller, dependency/feature graph or deployment architecture
requires review immediately, even before that date.

| ID and scope | Evidence and boundary |
| --- | --- |
| RUSTSEC-2026-0387, root/benchmark and services test graphs | `crypto-bigint 0.5.5` enters through `elliptic-curve 0.13.8` → `k256 0.13.4` → `solana-secp256k1-recover 3.3.0` → Agave/LiteSVM. The actual host syscall wrapper uses `VerifyingKey::recover_from_prehash` on the supplied public message hash, signature and recovery ID. These paths hold no secp256k1 signing secrets; the package is absent from the deployed short.win runtime. Signing examples in upstream documentation are not application execution. This exception is revoked by secret scalar arithmetic, ECDSA signing/key generation or another consumer. |
| RUSTSEC-2026-0385, root/benchmark only | Root `ctutils 0.4.2` is reached through `digest 0.11.3`, whose root direct consumers are SHA-2 0.11.0 and BLAKE3 1.8.7. Anchor's `anchor-syn 1.2.0` hashes public source strings with SHA-256; Solana message/hash wrappers use unkeyed `Hasher::new`/default, update and finalize on public serialized messages. The advisory's Choice mask/comparison operations are used by `digest`'s MAC/CtOutput APIs, not these ordinary Digest calls. **The `mac` feature does compile through BLAKE3 traits-preview**; feature absence is not the argument. New MAC/CtOutput/Choice, keyed hash, KDF or secret-comparison use revokes this disposition. Services instead use the actual ctutils patch above. |
| RUSTSEC-2026-0391, native Linux services | Locked zstd 0.13.3's only direct callers are Solana account-decoder and account-decoder-client-types 4.2.2. Encoding creates a fresh `stream::write::Encoder::new(Vec::new(), 0)`, writes and finishes it; decoding creates a fresh read decoder. Neither caller borrows/reuses a context with `with_context`, the affected API. Context pooling/reuse revokes this disposition. |
| RUSTSEC-2026-0398, native Linux services | zstd-safe 7.3.0 is referenced only by zstd 0.13.3, whose dependency disables defaults and enables `std`; its forwarded default features are legacy/arrays/zdict_builder. zstd exposes no seekable forwarding feature, and no tracked workspace manifest directly requests zstd-safe/sys or seekable. The affected `Seekable`/`AdvancedSeekable` API is not compiled. Any seekable feature/caller revokes this disposition. |
| RUSTSEC-2026-0403, native Linux services | Solana's Base64Zstd input uses `zstd::stream::read::Decoder::new(...).read_to_end(...)`. The authoritative advisory explicitly excludes the read decoder; the affected `stream::write::Decoder` path is absent. Legacy support remains enabled, so feature absence is not claimed. Adding the affected write decoder revokes this disposition. |
| RUSTSEC-2026-0410, native Linux services | The advisory's affected architecture is `wasm32`; production services are native Linux binaries. The browser math/WASM workspace does not depend on zstd-sys. A WASM build of these services/dependencies revokes this disposition. |

Exact crate archives were downloaded from crates.io and SHA-256 checked against
the committed lockfiles; retained selected source, archive checksums and independent
verification make the API and feature arguments inspectable. Complete lockfile
reverse-edge checks, not just a search of short.win imports, identified zstd's
only direct callers. Local source has no direct keyed/MAC, ctutils, k256 or zstd
API additions that broaden these paths. These statements apply to this reviewed
graph and source, not to every potential use of the upstream crates.

Compatible fixes are preferred over exceptions. The current `elliptic-curve 0.13`
line constrains crypto-bigint to 0.5, while the advisory fixes start at 0.7.6.
Agave account-decoder 4.2.2 constrains zstd to 0.13, while its fix starts at 0.14.3
(and zstd-safe 8.2.0). Do not silently substitute incompatible major versions.

## Follow-up and invalidation

- The release maintainer must recheck upstream Solana/Agave and RustCrypto
  dependency updates **by 2026-11-27**, remove these exceptions as compatible fixes
  become available, and retain new focused tests/CI for changed host dependencies.
- Prefer patching root ctutils in the next deliberately qualified harness/build
  baseline. Do not credit the unchanged frozen run to changed root locked inputs
  without the required evidence and explicit attribution.
- Re-run this review on a new dependency caller or target. In particular, no secret
  secp256k1 arithmetic, keyed hash/MAC/CtOutput path, borrowed zstd context, seekable
  API or zstd WASM artifact is covered by these exceptions.
- `deny.toml`, `services/deny.toml` and CI's explicit `cargo audit --ignore` IDs must
  agree. All unrelated advisories continue to fail normally; no blanket warning or
  advisory suppression was added.
- An expiry/applicability disposition cannot close other release gates. The
  independently observed new Token-2022 ELF requires its own compatibility review;
  the capped-mainnet owner decision does not silently promote new upstream bytes.
