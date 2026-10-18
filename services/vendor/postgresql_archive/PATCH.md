# Temporary ZIP dependency update

This is `postgresql_archive` 0.21.0 from crates.io, corresponding to upstream
commit `a995b73d5b0b220f96b03634ec8cdfe117996c26` in
<https://github.com/theseus-rs/postgresql-embedded>. The original normalized
manifest, source, tests, benchmarks and README are retained; `Cargo.toml.orig`
records the upstream workspace manifest. The upstream Apache and MIT license
files are included from that commit. The PostgreSQL license is included in
`LICENSES/PostgreSQL.txt` at the repository root. The original package license remains
`(Apache-2.0 OR MIT) AND PostgreSQL`.

The dependency change is the normalized `Cargo.toml` requirement
from `zip = "8.1.0"` to `zip = "9.0.0"`, with its existing `deflate` feature and
disabled default features unchanged. The services lockfile selects a patched
9.x release. Two filename reads (`src/extractor/zip_extractor.rs` and
`src/configuration/zonky/extractor.rs`) propagate ZIP 9's fallible `name()` result
and convert its `Cow<str>` into the existing owned path/name. Any error returned
by ZIP's filename API is propagated. ZIP 9.0.5 falls back to CP437 for non-UTF-8
names; this patch does not impose stricter encoding validation. No other
production Rust source is changed.

`tests/zip9_compat.rs` adds two offline extraction regressions: the generic ZIP
and Zonky JAR paths both extract ASCII (borrowed) and CP437 (owned) decoded
filenames and reject a truncated archive. The tiny synthetic ZIP/JAR fixtures
contain only `zip9 filename compatibility fixture\n`, with fixed ZIP timestamps
(1980-01-01), and, inside the JAR, a USTAR/XZ `bin/postgres` entry with mtime 0.
They contain no PostgreSQL binary. The manifest registers this test target and
isolates this vendored package as a workspace so it can run independently:

```sh
cargo test --manifest-path services/vendor/postgresql_archive/Cargo.toml \
  --no-default-features --features zonky --test zip9_compat
```

The tests passed against the services lockfile's ZIP 9.0.5 using an offline
host test harness; the independent command above may need to fetch upstream's
optional development dependencies before running.

The latest published `postgresql_embedded` and `postgresql_archive` versions are
0.21.0 as of 2026-10-18. Their `zonky` feature requires ZIP support even on Linux:
the PostgreSQL tar.xz archive is read from a Maven JAR/ZIP. Disabling ZIP would
break the embedded PostgreSQL fixture. ZIP 9 fixes
[RUSTSEC-2026-0367](https://rustsec.org/advisories/RUSTSEC-2026-0367),
[RUSTSEC-2026-0368](https://rustsec.org/advisories/RUSTSEC-2026-0368), and
[RUSTSEC-2026-0369](https://rustsec.org/advisories/RUSTSEC-2026-0369).

This patch is used only through the indexer's optional `embedded-pg` test
fixture. Remove the services `[patch.crates-io]` entry and this directory when a
compatible upstream release requires ZIP 9 or newer. Validate the replacement
with the services cargo-deny/cargo-audit checks and the indexer's `pg_fixture`
integration test, including a fresh archive download and extraction.
