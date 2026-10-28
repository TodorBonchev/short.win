> This pure parser/isolation snapshot is an internal member of the new pooled workspace. Its Rust source files are copied byte for byte from the repository's `crates/shortwin-jupiter-route` at `f4377093e1058c2aaf0c9b26865b092d7cec641c`; the license and source provenance are preserved. The internal location makes first-party Cargo identities independent of the outer checkout path. The new program's [scope and qualification](../../docs/SCOPE.md) govern its use; the earlier component notes below are retained as provenance.

# Jupiter V2 header prototype

This dependency-free, no_std crate parses four current Jupiter V2 scalar headers and checks their fixed account bindings against caller-supplied expectations. It is a core redesign prototype. Neither the short.win program nor any production service links it.

A `ValidatedHeader` certifies only the checks in `validate_header`. It is **not** a validated route, a price oracle, a deployed-program identity check, a token ownership or signature proof, an execution threshold, or authorization to invoke Jupiter. No new venue or token support is enabled by adding this crate.

## Schema identity

The finalized read-only mainnet probe on 2026-10-27 (08:08:21.602265Z–08:08:22.076133Z, context slot 461997164) matched the existing administrative pin:

| Artifact | Identity |
| --- | --- |
| Program | `JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4` |
| ProgramData | `4Ec7ZxZS6Sbdg5UGSLHbAnM7GQHp2eFd4KYWRexAipQT` |
| Deploy slot | 459479624 |
| Trimmed ELF SHA256 | `5e31d320f5d88ae93c7983ce82a1497067abd86ff540b35073abaeeb25ef397b` |
| IDL account | `C88XWfp26heEmDkmfSzeXP7Fd7GQJ2j9dDTUsyiZbUTa` |
| IDL account SHA256 | `c468169462ca9330e499b597ce8d540308dcb66616002aaefa168551e3870558` |
| Decoded IDL SHA256 | `1c70d6c96e01648411ff1579a35853d966407a4dff5020a5d33e92e6923c9e67` |

The exact raw response and extracted IDL are retained locally under `target/jupiter-route-prototype-2026-10-27/`. The checked schema establishes byte/layout identity, not source-to-ELF provenance or the safety of Jupiter's private implementation. An upgrade invalidates this schema qualification until reviewed.

The supported headers are `route_v2`, `shared_accounts_route_v2`, `exact_out_route_v2`, and `shared_accounts_exact_out_route_v2`. The ordinary form has ten fixed accounts. The shared form has twelve and an initial `id: u8` argument. The header then contains two u64 amounts, three u16 basis-point fields, and a Borsh u32 route-plan length. The route-plan bytes start at offset 34 or 35 respectively.

`RoutePlanStepV2` has a Swap enum tag, u16 bps, and two u8 indices. Five bytes per declared step is only a necessary lower bound: variants have different payloads. The parser does not decode a Swap variant, validate route indices or weights, ensure the declared step count matches the tail, or reject trailing bytes.

## Trusted input boundary

Bindings and limits must come from a future independently trusted short.win policy. Copying them from the same API JSON as the instruction defeats these checks.

- The instruction target, fixed program slot and event authority must match the pinned identities.
- Source/destination mints, token accounts and token programs must match the expected keys. Only the SPL Token and Token-2022 program IDs are accepted as expected token programs. This does not admit a mint or validate extensions.
- Shared `id`, authority and internal account keys must match caller expectations. This crate does not derive shared PDAs or check account ownership.
- The ordinary optional destination must be the Jupiter None sentinel or the same expected destination account, with the IDL writable role when present.
- Required writable roles and the transfer signer role are checked. Other signers are rejected; a repeated expected transfer authority may appear as a signer in the opaque remaining accounts. Extra writability is deliberately not certified safe. Input metas are proposed instruction roles, not runtime AccountInfo privilege unions.
- The specified input (ExactIn) or output (ExactOut) must fit a positive inclusive range. ExactIn quoted output must meet a positive lower bound; ExactOut quoted input must fit a positive upper bound. These are constraints on serialized scalar fields, not independently authenticated prices or guaranteed fills.
- Slippage must fit an explicit caller limit below 10000 bps. Platform fees and positive-slippage fees are both required to be zero in this prototype.
- Caller limits bound instruction bytes, account count and declared route steps. Counts use checked size arithmetic and no allocation.

No private Jupiter slippage-rounding formula is inferred. In particular, an ExactOut quoted-input bound is not an absolute post-slippage spend bound. Before any CPI integration, a separate policy must validate the complete route/remaining-account behavior, executable identity, mint safety, PDA/authority/ownership and account aliases, allowed writable accounts, signer scope, independent absolute input/output limits, and actual pre/post balances. Exact-out schema recognition does not establish current API availability for every route.

The tests intentionally show an invalid Swap tag and extra tail bytes passing header validation. This is a guard against confusing this incomplete type with a complete Borsh decoder or an authorized route.

## Lightweight checks

From the repository root:

```sh
rustc --edition=2024 --test crates/shortwin-jupiter-route/src/lib.rs -o /tmp/shortwin-jupiter-route-tests
/tmp/shortwin-jupiter-route-tests
rustc --edition=2024 --crate-type=lib --crate-name=shortwin_jupiter_route -D warnings crates/shortwin-jupiter-route/src/lib.rs -o /tmp/libshortwin_jupiter_route.rlib
```

The test vectors are synthetic scalar/account fixtures derived from the pinned IDL. They do not execute a Jupiter binary or establish CPI compatibility.
