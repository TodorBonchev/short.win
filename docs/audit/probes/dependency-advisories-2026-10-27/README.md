# Dependency advisory evidence, 2026-10-27

See [the disposition](../../dependency-advisories-2026-10-27.md). `failed-run.json`
and `failed.log.gz` retain the original failed CI result; the gzip expands to the
UTF-8 text returned by `gh run view --log-failed`, not a GitHub-signed log bundle.
The original seven and follow-up ten RustSec records are pinned by `advisory-db-revision.txt`.

`crypto-source-proof.json` and `zstd-source-trace.json` retain selected exact crate
source and archive SHA-256 identities, checked against Cargo.lock. The downloaded
crate archives remain in the local target directory rather than being promoted
into build/vendor inputs. `zstd-feature-proof.json` records all direct lockfile
callers and feature forwarding. `independent-crypto-verification.json` is a
separate agent's checksum and source/API verification. `diagnosis.json` distinguishes
actual services patches from the scoped exceptions and local build limitations.

The exceptions do not patch upstream code, validate a whole security audit or
qualify new upstream Token-2022 bytes. SHA256SUMS covers this retained evidence.

Readable downloaded text uses LF line endings. `line-ending-retention.json` records the exact original Windows-CLI CRLF bytes, each preserved in a neighbouring `.original.gz`, and the normalized text hashes. No advisory text was otherwise changed.

The `followup-40432351023-services.*` files retain the next completed service-job
failure while other workflow jobs were still running. `followup-unsound-dispositions.json`
records the ten per-ID API boundaries; policy/VPS proof JSON files retain two
independent exact-source reviews and the compatible-release check. These additions
do not patch the underlying APIs or remove `cargo audit --deny warnings`.
