# Independent build-identity review

The controlled actual-program build performed by the other agent is decisive: the exact `f4377093` source built at the CI checkout path produces the CI ELF `633046...`, while the same source at the local checkout path produces `49f11...`. Its `layout-diagnosis.json` establishes exact `.text` identity after only allocator/entry block permutation and corresponding relative call relocation normalization. This normalization is diagnostic evidence, not a reproducible-build pass.

Cargo 1.95 removes a workspace prefix when computing stable package-source hashes **only for a path inside that workspace**. A path dependency outside the workspace falls back to hashing the full URL. The standalone workspace is `simple/`; its old parser/isolation dependency was at `../crates/shortwin-jupiter-route`, outside that root. Dependency `c_metadata` hashes are mixed into the parent crate's metadata, and the compiler uses this identity for symbols. This explains path-sensitive symbol/code layout without requiring embedded absolute path strings.

Primary source evidence:

- https://github.com/rust-lang/cargo/blob/rust-1.95.0/src/cargo/core/source_id.rs#L515-L540
- https://github.com/rust-lang/cargo/blob/rust-1.95.0/src/cargo/core/package_id.rs#L160-L168
- https://github.com/rust-lang/cargo/blob/rust-1.95.0/src/cargo/core/compiler/build_runner/compilation_files.rs#L661-L674
- https://github.com/rust-lang/cargo/blob/rust-1.95.0/src/cargo/core/compiler/build_runner/compilation_files.rs#L740-L747
- https://rustc-dev-guide.rust-lang.org/backend/libs-and-metadata.html#stable-crate-id

A scratch probe of the actual program module through Anchor 1.2's `anchor-syn` produced 16 different expanded token streams. Sorting only the generated `accounts` modules' `pub use` items collapses these to one stream. The upstream macro uses randomized HashMap/HashSet iteration for these reexports. This is real token-level nondeterminism, but it is not evidence of the observed runtime difference; repeated same-path package rebuilds and the exact CI-path reproduction identify the Cargo path mechanism instead. No Anchor vendoring or patch is warranted by these observations.

The low-impact remedy authorized by the parent was applied: copy the two pure parser/isolation source files and README unchanged into `simple/crates/jupiter-route`, adjust only the copied manifest's license path/provenance comments, add the member and update the program path dependency. The legacy crate is untouched. The original source manifest SHA256 is `1cf49776317f3e4adb5f17077cd52e4dc0f73fa1fd17fc55d315f9a5a6448b75`; detailed hashes are in `jupiter-route-copy-provenance.json`. Frozen fuzz source now includes every copied crate file.

Native workspace all-target checks, format checks, and 13 parser plus 16 core plus one randomized model tests passed after the change. No SBF build or new fuzz campaign was run by this review. The next step is fresh SBF builds from different checkout paths with identical inputs and an exact byte comparison, then runtime checks and a new bounded smoke before choosing any final 24-hour baseline. Stable `/build` mounting remains an optional additional build convention, not a substitute for proving the corrected workspace's cross-path identity.
