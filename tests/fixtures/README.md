# Mainnet fixtures

Accounts, programs and one IDL dumped from mainnet for the program tests (ARCHITECTURE §1.2,
§8.2). `scripts/dump-fixtures.py` writes and checks every file here except the Pyth dump
(`accounts/pyth_sol_usd_shard0.*`, recorded by hand in `manifest.json`); its docstring is the
reference, and docs/DEVELOPMENT.md ("The Raydium mainnet fork (M3) and its fixtures") describes
how the harness loads them.

- `manifest.json`: one entry per account (address, owner, lamports, executable, rent epoch, space,
  slot, sha256, purpose, decoded fields) and one `snapshots` record per group (slot, RPC, method;
  `vendored`: files taken from a pinned source).
- `SHA256SUMS`: every fixture file (CI: `cd tests/fixtures && sha256sum -c SHA256SUMS`).

## Groups

| Group | Files | What |
|---|---|---|
| `raydium_programs` | `programs/raydium_{amm_v4,cpmm}.so`, `accounts/raydium/*_program*.bin` | AMM v4 (deploy 445,763,204) and CPMM (445,763,504). The CPMM tests and T-LATCH load these |
| `raydium_state` | `accounts/raydium/*.bin` | Raydium pools, vaults, mints, observations, all 21 CPMM configs (one snapshot, slot 451,187,550) |
| `pump_programs`, `pump_state` | `programs/pump{,_fees,_amm}.so`, `accounts/pump/*` | pump, pump-fees and PumpSwap (M5) |
| `pamm_state`, `pamm_synthetic` | `accounts/pamm/*` | PumpSwap pools (M6). The synthetic group is derived offline |
| `raydium_cpmm_2026_10` (M10) | `programs/raydium_cpmm_2026_10.so`, `accounts/raydium_cpmm_2026_10/*` | CPMM **as redeployed at 452,376,460**. This group holds the ELF only. The pools and configs stay `raydium_state`'s, because the redeploy kept their layouts (AmmConfig 236 B, PoolState 637 B) and the 21 live configs still equal the fixture bytes |
| `spl_programs` (M10) | `programs/token_2022.so`, `accounts/spl/*`, `programs/spl_memo_v3.so` | the deployed Token-2022 (upgradeable, a verified build) and spl-memo v3 (BPFLoader2, where the account data *is* the ELF) |
| `squads_programs` (M10) | `programs/squads_v4.so`, `accounts/squads/*`, `idl/squads_multisig_program.json` | Squads v4 (immutable), its `ProgramConfig` and the IDL 2.1.0 vendored from `Squads-Protocol/v4@64af733` |

Program groups are stored as `header ‖ ELF`. The mainnet ProgramData account is `header ‖ ELF ‖
zeros` up to `space`; the loader in `tests/litesvm/src/fork.rs` rebuilds that account and checks
its sha256.

The M10 groups are pinned in the script:

| Program | Deploy slot | Trimmed-ELF sha256 | Source |
|---|---|---|---|
| CPMM `CPMMoo8L…` | 452,376,460 | `93cad245…a0d2` | UNVERIFIED (probably `raydium-cp-swap` `feat/creator-fee-share` @ `bdeee23`) |
| Token-2022 `TokenzQd…` | 427,147,035 (authority `AeLmXCbP…`) | `7ea94a02…1947` (ELF `9bbf90b3…a5d1`) | `solana-program/token-2022` @ `9bc02757`, verified on verify.osec.io |
| spl-memo v3 `MemoSq4g…` | (BPFLoader2) | account `f520eaf0…7954` | |
| Squads v4 `SQDS4ep6…` | 302,582,236 (no authority) | `d4866083…2fac` | UNVERIFIED (probably `Squads-Protocol/v4` @ `64af733`) |

A fetch that reads anything other than these pins stops before it writes a file. The script
stages every file and writes them, `manifest.json` and `SHA256SUMS` only after every group it
fetched has passed its checks.

**No test loads the M10 groups yet.**
- WP-2 moves the CPMM suites onto `raydium_cpmm_2026_10`. That group holds only the CPMM program
  and ProgramData, at the same two addresses as `raydium_programs`' CPMM entries. A loader keeps
  `raydium_programs` (it also holds AMM v4) and picks the CPMM accounts by their
  `raydium_cpmm_2026_10_*` names, never by address.
- WP-3 moves the suites onto the mainnet SPL ELFs.
- WP-7 uses `squads_programs`.

`raydium_programs` stays for T-LATCH. It is the deploy that `VENUE_GUARD_SLOTS_2026_09_27` names.

## Commands

```sh
python3 scripts/dump-fixtures.py                       # offline check (= `check`)
(cd tests/fixtures && sha256sum -c SHA256SUMS)
python3 scripts/dump-fixtures.py fetch --group spl_programs   # fetch a missing group (pins enforced)
python3 scripts/dump-fixtures.py check --live          # mainnet drift, read-only; exit 1 names it
```

`check --live` makes about ten paced read-only calls:
- one `getMultipleAccounts` call for the program accounts;
- one `getMultipleAccounts` call for the 45-byte ProgramData headers;
- one `getAccountInfo` call per ELF whose deploy slot still matches its fixture.

It compares each program against two things:
- **Its newest fixture.** It checks the deploy slot, the upgrade authority and the trimmed ELF. Older groups of the same program are listed as "kept".
- **The newest `VENUE_GUARD_SLOTS_YYYY_MM_DD`** in `programs/shortwin/src/constants.rs`. This applies to the six VenueGuard programs only. Jupiter has no fixture, so only this check applies to it.

When a program is redeployed:
1. Dump it as a **new** group; never refresh an old one, because the tests that need the old deploy (T-LATCH) still load it.
2. Pin the new group in the script.
3. Move the test baseline deliberately.
