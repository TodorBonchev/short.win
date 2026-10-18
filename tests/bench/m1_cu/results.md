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
| `disable_venue` | pump curve bit | 5712 | 5712 | 60000 | no §6.1 row; admin flag class (tests/litesvm cu.rs) |
| `flag_venue_upgraded` | pump, 109 B mock ProgramData | 6036 | 6036 | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` |
| `flag_venue_upgraded` | pump, 10 MiB ProgramData (U5) | 6036 | 6036 | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` |
| `revalidate_venue` | pump, 109 B mock ProgramData | 8506 | 8506 | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` |
| `revalidate_venue` | pump, 10 MiB ProgramData (U5) | 8506 | 8506 | 400000 | §6.1 `flag_venue_upgraded` / `revalidate_venue` |
| `create_market` | SPL Token mint, `None` venue | 54583 | 54583 | 250000 | §6.1 `create_market` |
| `create_market` | Token-2022 mint, `None` venue | 57439 | 57439 | 250000 | §6.1 `create_market` |
| `flag_unsafe_mint` | freeze authority set | 6425 | 6425 | 60000 | no §6.1 row; crank class (§6.1 `crank_price`) |
| `deposit` | first deposit, SPL Token | 36378 | 36378 | 100000 | §6.1 `deposit` / `withdraw` |
| `deposit` | first deposit, Token-2022 | 36598 | 36598 | 100000 | §6.1 `deposit` / `withdraw` |
| `deposit` | second deposit, SPL Token | 29717 | 29717 | 100000 | §6.1 `deposit` / `withdraw` |
| `withdraw` | half the shares, SPL Token | 31654 | 31654 | 100000 | §6.1 `deposit` / `withdraw` |
| `claim_sol` | pays a covered balance | 16418 | 16418 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `cover_deficit` | 1,000,000 lamports | 16765 | 16765 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `sync_lender` | lender on the current epoch | 14189 | 14189 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `deposit` | 1 rebase record behind, written | 33369 | 33369 | 100000 | §6.1 `deposit` / `withdraw` |
| `deposit` | 3 rebase records behind, written | 47742 | 47742 | 100000 | §6.1 `deposit` / `withdraw` |
| `deposit` | 8 rebase records behind, written | 68109 | 68109 | 100000 | §6.1 `deposit` / `withdraw` |
| `deposit` | 12 rebase records behind, written | 88781 | 88781 | 100000 | §6.1 `deposit` / `withdraw` |
| `withdraw` | 1 rebase record behind, written | 36820 | 36820 | 100000 | §6.1 `deposit` / `withdraw` |
| `withdraw` | 3 rebase records behind, written | 46714 | 46714 | 100000 | §6.1 `deposit` / `withdraw` |
| `withdraw` | 8 rebase records behind, written | 71581 | 71581 | 100000 | §6.1 `deposit` / `withdraw` |
| `withdraw` | 12 rebase records behind, written | 92253 | 92253 | 100000 | §6.1 `deposit` / `withdraw` |
| `claim_sol` | 1 rebase record behind, written | 21572 | 21572 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `claim_sol` | 3 rebase records behind, written | 31466 | 31466 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `claim_sol` | 8 rebase records behind, written | 56333 | 56333 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `claim_sol` | 10 rebase records behind, written | 67075 | 67075 | 1400000 | transaction limit; beyond the web's 8-record cap (§6.1's 60,000 holds to 8) |
| `claim_sol` | 12 rebase records behind, written | 77005 | 77005 | 1400000 | transaction limit; beyond the web's 8-record cap (§6.1's 60,000 holds to 8) |
| `cover_deficit` | 1 rebase record behind, written | 21919 | 21919 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `cover_deficit` | 3 rebase records behind, written | 31813 | 31813 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `cover_deficit` | 8 rebase records behind, written | 56680 | 56680 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `cover_deficit` | 10 rebase records behind, written | 67422 | 67422 | 1400000 | transaction limit; beyond the web's 8-record cap (§6.1's 60,000 holds to 8) |
| `cover_deficit` | 12 rebase records behind, written | 77352 | 77352 | 1400000 | transaction limit; beyond the web's 8-record cap (§6.1's 60,000 holds to 8) |
| `sync_lender` | 1 rebase record behind, written | 19525 | 19525 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `sync_lender` | 3 rebase records behind, written | 29419 | 29419 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `sync_lender` | 8 rebase records behind, written | 54286 | 54286 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `sync_lender` | 10 rebase records behind, written | 65028 | 65028 | 1400000 | transaction limit; beyond the web's 8-record cap (§6.1's 60,000 holds to 8) |
| `sync_lender` | 12 rebase records behind, written | 74958 | 74958 | 1400000 | transaction limit; beyond the web's 8-record cap (§6.1's 60,000 holds to 8) |
| `sync_lender` | a third party, 1 rebase record passed: the pot-only touch | 18509 | 18509 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `sync_lender` | a third party, 8 rebase records passed: the pot-only touch | 47989 | 47989 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `claim_sol` | KI-66 full: settles a forward chain, 8 records | 50929 | 50929 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `cover_deficit` | KI-66 full: settles a forward chain, 8 records | 51276 | 51276 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `sync_lender` | KI-66 full: owner settles a forward chain, 8 records | 48882 | 48882 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `claim_sol` | KI-66 full: FWD_OPEN, settles through 8 records | 52113 | 52113 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `sync_lender` | KI-66 full: a third party continues a chain, 4 records | 31655 | 31655 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `claim_sol` | KI-66 full: settles a chain a unit above it at every record (lag 1), 7 records | 59006 | 59006 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `claim_sol` | KI-66 full: settles a chain a unit above it at every record (lag 1), 8 records (the web caps a chained settlement at 7) | 65288 | 65288 | 1400000 | transaction limit; beyond the web's 8-record cap (§6.1's 60,000 holds to 8) |
| `cover_deficit` | KI-66 full: settles a chain a unit above it at every record (lag 1), 7 records | 59356 | 59356 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `cover_deficit` | KI-66 full: settles a chain a unit above it at every record (lag 1), 8 records (the web caps a chained settlement at 7) | 65638 | 65638 | 1400000 | transaction limit; beyond the web's 8-record cap (§6.1's 60,000 holds to 8) |
| `sync_lender` | KI-66 full: owner settles a chain a unit above it at every record (lag 1), 7 records | 56962 | 56962 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `sync_lender` | KI-66 full: owner settles a chain a unit above it at every record (lag 1), 8 records (the web caps a chained settlement at 7) | 63244 | 63244 | 1400000 | transaction limit; beyond the web's 8-record cap (§6.1's 60,000 holds to 8) |
| `claim_sol` | KI-67: a wiped lender's claim that closes the position | 19532 | 19532 | 60000 | §6.1 `claim_sol` / `cover_deficit` / `sync_lender` |
| `sweep_protocol_fees` | the market creation fee | 11747 | 11747 | 60000 | no §6.1 row; crank class (§6.1 `crank_price`) |
| `crank_sol_usd` | mainnet dump, empty cache | 6146 | 6146 | 60000 | no §6.1 row; crank class (§6.1 `crank_price`) |
| `crank_sol_usd` | 55 s later, step checked | 6556 | 6556 | 60000 | no §6.1 row; crank class (§6.1 `crank_price`) |
