# M1 CU baseline

Compute units of every M1 instruction (ARCH §9 M1) on the SBF program (`anchor build --tools-version v1.57 --arch v3`), measured with Mollusk 0.15.1 on account states built by the LiteSVM harness, with the same SPL Token (p-token), Token-2022 and ATA ELFs; LiteSVM 0.16.0 runs the same instruction as a cross-check. Written by `M1_CU_WRITE=1 cargo test --manifest-path tests/bench/m1_cu/Cargo.toml` (README.md); CI fails when a Mollusk figure exceeds its budget or a case fails.

| Instruction | Case | Mollusk CU | LiteSVM CU | Budget | Budget source |
|---|---|---|---|---|---|
| `initialize_config` | defaults, 7 accounts created | 56145 | 56145 | 200000 | no §6.1 row; admin class (tests/litesvm cu.rs) |
| `propose_config` | `Params` | 12833 | 12833 | 200000 | no §6.1 row; admin class (tests/litesvm cu.rs) |
| `apply_config` | `Params` | 11375 | 11375 | 200000 | no §6.1 row; admin class (tests/litesvm cu.rs) |
| `cancel_config` | pending `Params` | 6576 | 6576 | 200000 | no §6.1 row; admin class (tests/litesvm cu.rs) |
| `set_pause` | global | 6089 | 6089 | 60000 | no §6.1 row; admin flag class (tests/litesvm cu.rs) |
| `set_pause` | one market | 8067 | 8067 | 60000 | no §6.1 row; admin flag class (tests/litesvm cu.rs) |
| `set_coverage_pause` | one market, role installed by the timelock (M7) | 8091 | 8091 | 60000 | no §6.1 row; admin flag class (tests/litesvm cu.rs) |
| `disable_venue` | pump curve bit | 5700 | 5700 | 60000 | no §6.1 row; admin flag class (tests/litesvm cu.rs) |
| `flag_venue_upgraded` | pump, 109 B mock ProgramData | 6028 | 6028 | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` |
| `flag_venue_upgraded` | pump, 10 MiB ProgramData (U5) | 6028 | 6028 | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` |
| `revalidate_venue` | pump, 109 B mock ProgramData | 8440 | 8440 | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` |
| `revalidate_venue` | pump, 10 MiB ProgramData (U5) | 8440 | 8440 | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` |
| `create_market` | SPL Token mint, `None` venue | 50045 | 50045 | 250000 | §6.1 `create_market` |
| `create_market` | Token-2022 mint, `None` venue | 57401 | 57401 | 250000 | §6.1 `create_market` |
| `flag_unsafe_mint` | freeze authority set | 6425 | 6425 | 60000 | no §6.1 row; crank class (§6.1 `crank_price`) |
| `deposit` | first deposit, SPL Token | 36200 | 36200 | 100000 | §6.1 `deposit` / `withdraw` |
| `deposit` | first deposit, Token-2022 | 36420 | 36420 | 100000 | §6.1 `deposit` / `withdraw` |
| `deposit` | second deposit, SPL Token | 28029 | 28029 | 100000 | §6.1 `deposit` / `withdraw` |
| `withdraw` | half the shares, SPL Token | 31483 | 31483 | 100000 | §6.1 `deposit` / `withdraw` |
| `claim_sol` | pays a covered balance | 16099 | 16099 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `cover_deficit` | 1,000,000 lamports | 16454 | 16454 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `sync_lender` | lender on the current epoch | 13786 | 13786 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `deposit` | 1 rebase record behind, written | 32912 | 32912 | 100000 | §6.1 `deposit` / `withdraw` |
| `deposit` | 3 rebase records behind, written | 42358 | 42358 | 100000 | §6.1 `deposit` / `withdraw` |
| `deposit` | 8 rebase records behind, written | 66101 | 66101 | 100000 | §6.1 `deposit` / `withdraw` |
| `deposit` | 12 rebase records behind, written | 85869 | 85869 | 100000 | §6.1 `deposit` / `withdraw` |
| `withdraw` | 1 rebase record behind, written | 36401 | 36401 | 100000 | §6.1 `deposit` / `withdraw` |
| `withdraw` | 3 rebase records behind, written | 45847 | 45847 | 100000 | §6.1 `deposit` / `withdraw` |
| `withdraw` | 8 rebase records behind, written | 69590 | 69590 | 100000 | §6.1 `deposit` / `withdraw` |
| `withdraw` | 12 rebase records behind, written | 89358 | 89358 | 100000 | §6.1 `deposit` / `withdraw` |
| `claim_sol` | 1 rebase record behind, written | 21005 | 21005 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `claim_sol` | 3 rebase records behind, written | 30451 | 30451 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `claim_sol` | 8 rebase records behind, written | 54194 | 54194 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `claim_sol` | 10 rebase records behind, written | 64484 | 64484 | 1400000 | transaction limit; beyond the web's 8-record cap (§6.1's 60,000 holds to 8) |
| `claim_sol` | 12 rebase records behind, written | 73962 | 73962 | 1400000 | transaction limit; beyond the web's 8-record cap (§6.1's 60,000 holds to 8) |
| `cover_deficit` | 1 rebase record behind, written | 21360 | 21360 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `cover_deficit` | 3 rebase records behind, written | 30806 | 30806 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `cover_deficit` | 8 rebase records behind, written | 54549 | 54549 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `cover_deficit` | 10 rebase records behind, written | 64839 | 64839 | 1400000 | transaction limit; beyond the web's 8-record cap (§6.1's 60,000 holds to 8) |
| `cover_deficit` | 12 rebase records behind, written | 74317 | 74317 | 1400000 | transaction limit; beyond the web's 8-record cap (§6.1's 60,000 holds to 8) |
| `sync_lender` | 1 rebase record behind, written | 18691 | 18691 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `sync_lender` | 3 rebase records behind, written | 28137 | 28137 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `sync_lender` | 8 rebase records behind, written | 51880 | 51880 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `sync_lender` | 10 rebase records behind, written | 62170 | 62170 | 1400000 | transaction limit; beyond the web's 8-record cap (§6.1's 60,000 holds to 8) |
| `sync_lender` | 12 rebase records behind, written | 71648 | 71648 | 1400000 | transaction limit; beyond the web's 8-record cap (§6.1's 60,000 holds to 8) |
| `sweep_protocol_fees` | the market creation fee | 11747 | 11747 | 60000 | no §6.1 row; crank class (§6.1 `crank_price`) |
| `crank_sol_usd` | mainnet dump, empty cache | 6146 | 6146 | 60000 | no §6.1 row; crank class (§6.1 `crank_price`) |
| `crank_sol_usd` | 55 s later, step checked | 6556 | 6556 | 60000 | no §6.1 row; crank class (§6.1 `crank_price`) |
