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


## Follow-up: explicit unsound-API warnings in the next CI

In [CI 40432351023](https://github.com/TodorBonchev/short.win/actions/runs/40432351023)
on `f5aef15cfaf6bbad0a1a25dbbf25a798760aaeca`, the completed services job passed
formatting, Clippy and `cargo deny`, then `cargo audit --deny warnings` failed on
ten additional `zstd-safe 7.3.0` **unsound** advisories. Those warnings were exposed
after the earlier advisory errors were dispositioned; they are not demonstrated
new flaws in short.win's executed paths. Service unit/scenario tests after that
step did not run. The retained job log and snapshot preserve this failure without
calling the whole still-running workflow complete.

Two separate agents reviewed the exact locked crate sources, sole callers and
features; the implementing agent checked their conclusions against all ten pinned
RustSec records. There is no compatible patched zstd-safe 7.x release at this
read: the fixes start at 8.2.0, outside zstd 0.13's 7.x dependency range. These are
additional **usage-scoped nonapplicability** exceptions with the same November 27
expiry, not a declaration that zstd-safe is sound or repaired.

| Advisory | Current-path boundary |
| --- | --- |
| RUSTSEC-2026-0395 | `StableInBuffer(true)` requires the absent `experimental` feature and explicit enabling. The current encoder requests neither. |
| RUSTSEC-2026-0396 | All affected Seekable/SeekTable methods require the absent `seekable` feature. |
| RUSTSEC-2026-0397 | No Rust source in the exact zstd 0.13.3 archive calls `OutBuffer::around_pos`. Its writer clears its private Vec then uses `OutBuffer::around` at position zero; its reader wraps an initialized mutable byte slice with `around` at zero. The vulnerable constructor only checks capacity, not initialized length; it is **not fixed** and must not be introduced. |
| RUSTSEC-2026-0399 | The deprecated `init_using_cdict`/`init_using_ddict` methods require absent `experimental`; normal fresh constructors load an empty dictionary instead. |
| RUSTSEC-2026-0400 | The sole Solana callers own fresh streaming encoder/read-decoder contexts; they never mix one-shot calls with unfinished streams on the same context. |
| RUSTSEC-2026-0401 | `zdict_builder` is compiled by default. Nonapplicability is based on absence of dictionary-training calls/sample-size input in either Solana consumer, **not** absence of the feature or assumed checked addition in the vulnerable API. |
| RUSTSEC-2026-0402 | `CCtx::try_clone` requires absent `experimental`; current consumers also do not request `zstdmt`/positive `NbWorkers` or abandoned multithreaded frames. |
| RUSTSEC-2026-0404 | `DCtx::try_clone` requires absent `experimental`; no current caller clones raw contexts. |
| RUSTSEC-2026-0405 | The affected API is specifically `seekable::SeekableCStream::compress_stream`, absent with the seekable feature. This is not a blanket claim that every raw InBuffer API checks bounds. Current wrappers construct ordinary streaming InBuffer at position zero. |
| RUSTSEC-2026-0406 | The trigger needs unfinished multithreaded jobs plus context/dictionary mutation. Current contexts are single-threaded, created fresh and configured before their first frame. `set_parameter`/`load_dictionary` **do execute**, but without positive `NbWorkers`, unfinished workers or mutation of their dictionary. API absence is not the argument. |

Any new raw buffer manipulation, experimental/seekable feature, dictionary
training, context reuse/clone, mixed one-shot/streaming operation or multithreaded
worker configuration invalidates the corresponding disposition immediately.
Retained source excerpts and independent archive checks are in
[`policy-buffer-proof.json`](probes/dependency-advisories-2026-10-27/policy-buffer-proof.json)
and [`vps-source-feature-proof.json`](probes/dependency-advisories-2026-10-27/vps-source-feature-proof.json).
The feature proof uses exact manifest forwarding and complete lockfile reverse
edges; it is not a successful full Cargo feature-resolution command or a dynamic
Miri/UB test. The [per-ID independent review](probes/dependency-advisories-2026-10-27/vps-unsound-final-review.json)
retains those limits.
No runtime source or lockfile changed in this follow-up. `--deny warnings` remains
in place; only the ten individually named advisory IDs are added to the service
policy, explicit audit flags and dated expiry guard. Fresh service tests and CI
must still complete, and the independent Token-2022 live gate remains open.

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
