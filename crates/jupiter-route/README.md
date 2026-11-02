# Jupiter V2 header and isolation validation

This dependency-free `no_std` crate is the lending program's internal Jupiter scalar-header and account-isolation validator. The production integration supplies trusted expected identities, amount limits and account restrictions, then verifies executable identity and actual balance deltas around CPI.

A `ValidatedHeader` certifies only the checks in `validate_header`. It does not certify opaque route-plan semantics, a price, executable identity, token ownership or a signature. Header validation alone does not authorize a route.

## Schema

The pinned test schema identifies Jupiter `JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4` and ProgramData `4Ec7ZxZS6Sbdg5UGSLHbAnM7GQHp2eFd4KYWRexAipQT`, deploy slot `459479624`, trimmed ELF SHA256 `5e31d320f5d88ae93c7983ce82a1497067abd86ff540b35073abaeeb25ef397b`. Exact raw response and decoded IDL fixtures are in `tests/fixtures/jupiter-2026-10-27`. These establish runtime-byte/schema identity, not a source-to-ELF reproduction or future upgrade compatibility.

The parser recognizes `route_v2`, `shared_accounts_route_v2`, `exact_out_route_v2` and `shared_accounts_exact_out_route_v2`. Ordinary forms have ten fixed accounts; shared forms have twelve and an initial `id: u8`. The scalar header includes two u64 amounts, three u16 basis-point fields and a Borsh u32 route-plan length. Route bytes start at offset 34 or 35.

Each declared step requires at least five bytes, but enum variants have different payloads. The parser does not decode Swap variants, certify route indices/weights, match every declared step to its tail or reject all trailing bytes. Tests intentionally demonstrate these limits. The production application accepts its narrower reviewed ordinary ExactIn profile.

## Checks and caller responsibilities

- The instruction target, fixed program and event authority must match trusted expectations.
- Mints, token accounts and token programs must match caller-supplied bindings. Expected token programs are canonical SPL Token or Token-2022; mint eligibility is checked by the lending program.
- Shared identifiers and fixed accounts must match the caller's expectations. This crate does not derive shared PDAs or independently prove their ownership.
- The ordinary optional destination must be the Jupiter None sentinel or the expected destination with the required writable role.
- Required writability and transfer-authority signer roles are checked. Other signers are rejected; aliases and extra privileges remain subject to isolation validation.
- ExactIn input or ExactOut output must fit a positive inclusive range. Quoted output/input bounds constrain serialized fields, not independently authenticated prices or guaranteed fills.
- Slippage must fit an explicit limit below 10000 bps. Platform and positive-slippage fees are required to be zero.
- Instruction-byte, account-count and route-step bounds use checked size arithmetic without allocation.

The isolation module rejects protected-account aliases, unrelated signers and unauthorized writable accounts. Its default authority policy is read-only. The lending integration explicitly supports a writable empty System-owned loan-local authority under a separately signed native setup cap and exact post-CPI checks.

Trusted bindings must come from reviewed on-chain policy and state, not the same untrusted API response being validated. The caller must also validate complete remaining-account behavior, canonical executable identity, mints, authorities, ownership, balance deltas and absolute spend/output limits. ExactOut schema recognition does not establish provider support or an absolute post-slippage spend bound.

## Checks

From the repository root:

```sh
cargo test -p shortwin-jupiter-route --locked
```

Synthetic header/account vectors do not execute Jupiter. Actual-SBF integration and route qualification are described in the [runtime guide](../../tests/runtime/README.md).
