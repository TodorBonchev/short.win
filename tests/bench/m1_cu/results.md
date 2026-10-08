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
| `create_market` | SPL Token mint, `None` venue | 45545 | 45545 | 250000 | §6.1 `create_market` |
| `create_market` | Token-2022 mint, `None` venue | 78401 | 78401 | 250000 | §6.1 `create_market` |
| `flag_unsafe_mint` | freeze authority set | 6425 | 6425 | 60000 | no §6.1 row; crank class (§6.1 `crank_price`) |
| `deposit` | first deposit, SPL Token | 34732 | 34732 | 100000 | §6.1 `deposit` / `withdraw` |
| `deposit` | first deposit, Token-2022 | 36452 | 36452 | 100000 | §6.1 `deposit` / `withdraw` |
| `deposit` | second deposit, SPL Token | 29561 | 29561 | 100000 | §6.1 `deposit` / `withdraw` |
| `withdraw` | half the shares, SPL Token | 31547 | 31547 | 100000 | §6.1 `deposit` / `withdraw` |
| `claim_sol` | pays a covered balance | 16163 | 16163 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `cover_deficit` | 1,000,000 lamports | 16486 | 16486 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `sync_lender` | lender on the current epoch | 13950 | 13950 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `deposit` | 1 rebase record behind, written | 32944 | 32944 | 100000 | §6.1 `deposit` / `withdraw` |
| `deposit` | 3 rebase records behind, written | 45390 | 45390 | 100000 | §6.1 `deposit` / `withdraw` |
| `deposit` | 8 rebase records behind, written | 66133 | 66133 | 100000 | §6.1 `deposit` / `withdraw` |
| `deposit` | 12 rebase records behind, written | 85901 | 85901 | 100000 | §6.1 `deposit` / `withdraw` |
| `withdraw` | 1 rebase record behind, written | 36465 | 36465 | 100000 | §6.1 `deposit` / `withdraw` |
| `withdraw` | 3 rebase records behind, written | 45911 | 45911 | 100000 | §6.1 `deposit` / `withdraw` |
| `withdraw` | 8 rebase records behind, written | 69673 | 69673 | 100000 | §6.1 `deposit` / `withdraw` |
| `withdraw` | 12 rebase records behind, written | 89441 | 89441 | 100000 | §6.1 `deposit` / `withdraw` |
| `claim_sol` | 1 rebase record behind, written | 21069 | 21069 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `claim_sol` | 3 rebase records behind, written | 30515 | 30515 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `claim_sol` | 8 rebase records behind, written | 54258 | 54258 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `claim_sol` | 10 rebase records behind, written | 64548 | 64548 | 1400000 | transaction limit; beyond the web's 8-record cap (§6.1's 60,000 holds to 8) |
| `claim_sol` | 12 rebase records behind, written | 74026 | 74026 | 1400000 | transaction limit; beyond the web's 8-record cap (§6.1's 60,000 holds to 8) |
| `cover_deficit` | 1 rebase record behind, written | 21392 | 21392 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `cover_deficit` | 3 rebase records behind, written | 30838 | 30838 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `cover_deficit` | 8 rebase records behind, written | 54581 | 54581 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `cover_deficit` | 10 rebase records behind, written | 64871 | 64871 | 1400000 | transaction limit; beyond the web's 8-record cap (§6.1's 60,000 holds to 8) |
| `cover_deficit` | 12 rebase records behind, written | 74349 | 74349 | 1400000 | transaction limit; beyond the web's 8-record cap (§6.1's 60,000 holds to 8) |
| `sync_lender` | 1 rebase record behind, written | 19051 | 19051 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `sync_lender` | 3 rebase records behind, written | 28497 | 28497 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `sync_lender` | 8 rebase records behind, written | 52240 | 52240 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `sync_lender` | 10 rebase records behind, written | 62530 | 62530 | 1400000 | transaction limit; beyond the web's 8-record cap (§6.1's 60,000 holds to 8) |
| `sync_lender` | 12 rebase records behind, written | 72008 | 72008 | 1400000 | transaction limit; beyond the web's 8-record cap (§6.1's 60,000 holds to 8) |
| `sync_lender` | a third party, 1 rebase record passed: the pot-only touch | 16673 | 16673 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `sync_lender` | a third party, 8 rebase records passed: the pot-only touch | 31575 | 31575 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `sweep_protocol_fees` | the market creation fee | 11747 | 11747 | 60000 | no §6.1 row; crank class (§6.1 `crank_price`) |
| `crank_sol_usd` | mainnet dump, empty cache | 6146 | 6146 | 60000 | no §6.1 row; crank class (§6.1 `crank_price`) |
| `crank_sol_usd` | 55 s later, step checked | 6556 | 6556 | 60000 | no §6.1 row; crank class (§6.1 `crank_price`) |
