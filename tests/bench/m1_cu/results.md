# M1 CU baseline

Compute units of every M1 instruction (ARCH §9 M1) on the SBF program (`anchor build --tools-version v1.57 --arch v3`), measured with Mollusk 0.15.1 on account states built by the LiteSVM harness, with the same SPL Token (p-token), Token-2022 and ATA ELFs; LiteSVM 0.16.0 runs the same instruction as a cross-check. Written by `M1_CU_WRITE=1 cargo test --manifest-path tests/bench/m1_cu/Cargo.toml` (README.md); CI fails when a Mollusk figure exceeds its budget or a case fails.

| Instruction | Case | Mollusk CU | LiteSVM CU | Budget | Budget source |
|---|---|---|---|---|---|
| `initialize_config` | defaults, 7 accounts created | 56069 | 56069 | 200000 | no §6.1 row; admin class (tests/litesvm cu.rs) |
| `propose_config` | `Params` | 12829 | 12829 | 200000 | no §6.1 row; admin class (tests/litesvm cu.rs) |
| `apply_config` | `Params` | 11369 | 11369 | 200000 | no §6.1 row; admin class (tests/litesvm cu.rs) |
| `cancel_config` | pending `Params` | 6562 | 6562 | 200000 | no §6.1 row; admin class (tests/litesvm cu.rs) |
| `set_pause` | global | 6085 | 6085 | 60000 | no §6.1 row; admin flag class (tests/litesvm cu.rs) |
| `set_pause` | one market | 8063 | 8063 | 60000 | no §6.1 row; admin flag class (tests/litesvm cu.rs) |
| `disable_venue` | pump curve bit | 5698 | 5698 | 60000 | no §6.1 row; admin flag class (tests/litesvm cu.rs) |
| `flag_venue_upgraded` | pump, 109 B mock ProgramData | 6026 | 6026 | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` |
| `flag_venue_upgraded` | pump, 10 MiB ProgramData (U5) | 6026 | 6026 | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` |
| `revalidate_venue` | pump, 109 B mock ProgramData | 8434 | 8434 | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` |
| `revalidate_venue` | pump, 10 MiB ProgramData (U5) | 8434 | 8434 | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` |
| `create_market` | SPL Token mint, `None` venue | 38859 | 38859 | 250000 | §6.1 `create_market` |
| `create_market` | Token-2022 mint, `None` venue | 53704 | 53704 | 250000 | §6.1 `create_market` |
| `flag_unsafe_mint` | freeze authority set | 6419 | 6419 | 60000 | no §6.1 row; crank class (§6.1 `crank_price`) |
| `deposit` | first deposit, SPL Token | 34752 | 34752 | 100000 | §6.1 `deposit` / `withdraw` |
| `deposit` | first deposit, Token-2022 | 39457 | 39457 | 100000 | §6.1 `deposit` / `withdraw` |
| `deposit` | second deposit, SPL Token | 29447 | 29447 | 100000 | §6.1 `deposit` / `withdraw` |
| `withdraw` | half the shares, SPL Token | 31351 | 31351 | 100000 | §6.1 `deposit` / `withdraw` |
| `claim_sol` | pays a covered balance | 15988 | 15988 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `cover_deficit` | 1,000,000 lamports | 16386 | 16386 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `sync_lender` | lender on the current epoch | 13650 | 13650 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `sweep_protocol_fees` | the market creation fee | 11743 | 11743 | 60000 | no §6.1 row; crank class (§6.1 `crank_price`) |
| `crank_sol_usd` | mainnet dump, empty cache | 6148 | 6148 | 60000 | no §6.1 row; crank class (§6.1 `crank_price`) |
| `crank_sol_usd` | 55 s later, step checked | 6558 | 6558 | 60000 | no §6.1 row; crank class (§6.1 `crank_price`) |
