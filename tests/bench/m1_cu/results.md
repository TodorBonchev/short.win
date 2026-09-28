# M1 CU baseline

Compute units of every M1 instruction (ARCH §9 M1) on the SBF program (`anchor build --tools-version v1.57 --arch v3`), measured with Mollusk 0.15.1 on account states built by the LiteSVM harness, with the same SPL Token (p-token), Token-2022 and ATA ELFs; LiteSVM 0.16.0 runs the same instruction as a cross-check. Written by `M1_CU_WRITE=1 cargo test --manifest-path tests/bench/m1_cu/Cargo.toml` (README.md); CI fails when a Mollusk figure exceeds its budget or a case fails.

| Instruction | Case | Mollusk CU | LiteSVM CU | Budget | Budget source |
|---|---|---|---|---|---|
| `initialize_config` | defaults, 7 accounts created | 56073 | 56073 | 200000 | no §6.1 row; admin class (tests/litesvm cu.rs) |
| `propose_config` | `Params` | 12835 | 12835 | 200000 | no §6.1 row; admin class (tests/litesvm cu.rs) |
| `apply_config` | `Params` | 11377 | 11377 | 200000 | no §6.1 row; admin class (tests/litesvm cu.rs) |
| `cancel_config` | pending `Params` | 6576 | 6576 | 200000 | no §6.1 row; admin class (tests/litesvm cu.rs) |
| `set_pause` | global | 6091 | 6091 | 60000 | no §6.1 row; admin flag class (tests/litesvm cu.rs) |
| `set_pause` | one market | 8069 | 8069 | 60000 | no §6.1 row; admin flag class (tests/litesvm cu.rs) |
| `disable_venue` | pump curve bit | 5702 | 5702 | 60000 | no §6.1 row; admin flag class (tests/litesvm cu.rs) |
| `flag_venue_upgraded` | pump, 109 B mock ProgramData | 6032 | 6032 | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` |
| `flag_venue_upgraded` | pump, 10 MiB ProgramData (U5) | 6032 | 6032 | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` |
| `revalidate_venue` | pump, 109 B mock ProgramData | 8440 | 8440 | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` |
| `revalidate_venue` | pump, 10 MiB ProgramData (U5) | 8440 | 8440 | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` |
| `create_market` | SPL Token mint, `None` venue | 46362 | 46362 | 250000 | §6.1 `create_market` |
| `create_market` | Token-2022 mint, `None` venue | 44707 | 44707 | 250000 | §6.1 `create_market` |
| `flag_unsafe_mint` | freeze authority set | 6425 | 6425 | 60000 | no §6.1 row; crank class (§6.1 `crank_price`) |
| `deposit` | first deposit, SPL Token | 36273 | 36273 | 100000 | §6.1 `deposit` / `withdraw` |
| `deposit` | first deposit, Token-2022 | 37978 | 37978 | 100000 | §6.1 `deposit` / `withdraw` |
| `deposit` | second deposit, SPL Token | 29471 | 29471 | 100000 | §6.1 `deposit` / `withdraw` |
| `withdraw` | half the shares, SPL Token | 31373 | 31373 | 100000 | §6.1 `deposit` / `withdraw` |
| `claim_sol` | pays a covered balance | 15994 | 15994 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `cover_deficit` | 1,000,000 lamports | 16392 | 16392 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `sync_lender` | lender on the current epoch | 13656 | 13656 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `sweep_protocol_fees` | the market creation fee | 11749 | 11749 | 60000 | no §6.1 row; crank class (§6.1 `crank_price`) |
| `crank_sol_usd` | mainnet dump, empty cache | 6148 | 6148 | 60000 | no §6.1 row; crank class (§6.1 `crank_price`) |
| `crank_sol_usd` | 55 s later, step checked | 6558 | 6558 | 60000 | no §6.1 row; crank class (§6.1 `crank_price`) |
