# M1 CU baseline

Compute units of every M1 instruction (ARCH §9 M1) on the SBF program (`anchor build --tools-version v1.57 --arch v3`), measured with Mollusk 0.15.1 on account states built by the LiteSVM harness, with the same SPL Token (p-token), Token-2022 and ATA ELFs; LiteSVM 0.16.0 runs the same instruction as a cross-check. Written by `M1_CU_WRITE=1 cargo test --manifest-path tests/bench/m1_cu/Cargo.toml` (README.md); CI fails when a Mollusk figure exceeds its budget or a case fails.

| Instruction | Case | Mollusk CU | LiteSVM CU | Budget | Budget source |
|---|---|---|---|---|---|
| `initialize_config` | defaults, 7 accounts created | 56073 | 56073 | 200000 | no §6.1 row; admin class (tests/litesvm cu.rs) |
| `propose_config` | `Params` | 12823 | 12823 | 200000 | no §6.1 row; admin class (tests/litesvm cu.rs) |
| `apply_config` | `Params` | 11375 | 11375 | 200000 | no §6.1 row; admin class (tests/litesvm cu.rs) |
| `cancel_config` | pending `Params` | 6566 | 6566 | 200000 | no §6.1 row; admin class (tests/litesvm cu.rs) |
| `set_pause` | global | 6089 | 6089 | 60000 | no §6.1 row; admin flag class (tests/litesvm cu.rs) |
| `set_pause` | one market | 8067 | 8067 | 60000 | no §6.1 row; admin flag class (tests/litesvm cu.rs) |
| `set_coverage_pause` | one market, role installed by the timelock (M7) | 8085 | 8085 | 60000 | no §6.1 row; admin flag class (tests/litesvm cu.rs) |
| `disable_venue` | pump curve bit | 5691 | 5691 | 60000 | no §6.1 row; admin flag class (tests/litesvm cu.rs) |
| `flag_venue_upgraded` | pump, 109 B mock ProgramData | 6028 | 6028 | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` |
| `flag_venue_upgraded` | pump, 10 MiB ProgramData (U5) | 6028 | 6028 | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` |
| `revalidate_venue` | pump, 109 B mock ProgramData | 8434 | 8434 | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` |
| `revalidate_venue` | pump, 10 MiB ProgramData (U5) | 8434 | 8434 | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` |
| `create_market` | SPL Token mint, `None` venue | 59039 | 59039 | 250000 | §6.1 `create_market` |
| `create_market` | Token-2022 mint, `None` venue | 49895 | 49895 | 250000 | §6.1 `create_market` |
| `flag_unsafe_mint` | freeze authority set | 6425 | 6425 | 60000 | no §6.1 row; crank class (§6.1 `crank_price`) |
| `deposit` | first deposit, SPL Token | 34760 | 34760 | 100000 | §6.1 `deposit` / `withdraw` |
| `deposit` | first deposit, Token-2022 | 36480 | 36480 | 100000 | §6.1 `deposit` / `withdraw` |
| `deposit` | second deposit, SPL Token | 27979 | 27979 | 100000 | §6.1 `deposit` / `withdraw` |
| `withdraw` | half the shares, SPL Token | 31375 | 31375 | 100000 | §6.1 `deposit` / `withdraw` |
| `claim_sol` | pays a covered balance | 15980 | 15980 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `cover_deficit` | 1,000,000 lamports | 16379 | 16379 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `sync_lender` | lender on the current epoch | 13654 | 13654 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `sweep_protocol_fees` | the market creation fee | 11747 | 11747 | 60000 | no §6.1 row; crank class (§6.1 `crank_price`) |
| `crank_sol_usd` | mainnet dump, empty cache | 6146 | 6146 | 60000 | no §6.1 row; crank class (§6.1 `crank_price`) |
| `crank_sol_usd` | 55 s later, step checked | 6556 | 6556 | 60000 | no §6.1 row; crank class (§6.1 `crank_price`) |
