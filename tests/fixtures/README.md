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
| `raydium_programs` | `programs/raydium_{amm_v4,cpmm}.so`, `accounts/raydium/*_program*.bin` | AMM v4 (deploy 445,763,204) and CPMM (445,763,504). Every AMM v4 test loads these; the CPMM build is T-LATCH's "before" |
| `raydium_state` | `accounts/raydium/*.bin` | Raydium pools, vaults, mints, observations, all 21 CPMM configs (one snapshot, slot 451,187,550) |
| `pump_programs`, `pump_state` | `programs/pump{,_fees,_amm}.so`, `accounts/pump/*` | pump, pump-fees and PumpSwap (M5) |
| `pamm_state`, `pamm_synthetic` | `accounts/pamm/*` | PumpSwap pools (M6). The synthetic group is derived offline |
| `raydium_cpmm_2026_10` (M10) | `programs/raydium_cpmm_2026_10.so`, `accounts/raydium_cpmm_2026_10/*` | CPMM **as redeployed at 452,376,460**. This group holds the ELF only. The pools and configs stay `raydium_state`'s, because the redeploy kept their layouts (AmmConfig 236 B, PoolState 637 B) and the 21 live configs still equal the fixture bytes. Every CPMM test loads this build |
| `spl_programs` (M10) | `programs/token_2022.so`, `accounts/spl/*`, `programs/spl_memo_v3.so` | the deployed Token-2022 (upgradeable, a verified build) and spl-memo v3 (BPFLoader2, where the account data *is* the ELF) |
| `squads_programs` (M10) | `programs/squads_v4.so`, `accounts/squads/*`, `idl/squads_multisig_program.json` | Squads v4 (immutable), its `ProgramConfig` and the IDL 2.1.0 vendored from `Squads-Protocol/v4@64af733` |

Program groups are stored as `header ‖ ELF`. The mainnet ProgramData account is `header ‖ ELF ‖
zeros` up to `space`; the loader in `tests/litesvm/src/fork.rs` rebuilds that account and checks
its sha256.

The M10 groups are pinned in the script:

| Program | Deploy slot | Trimmed-ELF sha256 | Source |
|---|---|---|---|
| CPMM `CPMMoo8L…` | 452,376,460 | `93cad245…a0d2` | `raydium-cp-swap` `feat/creator-fee-share` @ `bdeee23`, **reproduced byte for byte** (M10 WP-2, below); the manifest's `source.status` still says UNVERIFIED (the script's pin text) |
| Token-2022 `TokenzQd…` | 427,147,035 (authority `AeLmXCbP…`) | `7ea94a02…1947` (ELF `9bbf90b3…a5d1`) | `solana-program/token-2022` @ `9bc02757`, verified on verify.osec.io |
| spl-memo v3 `MemoSq4g…` | (BPFLoader2) | account `f520eaf0…7954` | |
| Squads v4 `SQDS4ep6…` | 302,582,236 (no authority) | `d4866083…2fac` | UNVERIFIED (probably `Squads-Protocol/v4` @ `64af733`) |

A fetch that reads anything other than these pins stops before it writes a file. The script
stages every file and writes them, `manifest.json` and `SHA256SUMS` only after every group it
fetched has passed its checks.

**Who loads the M10 groups.**
- Every CPMM test and bench runs on `raydium_cpmm_2026_10` (M10 WP-2). That group holds only the
  CPMM program and ProgramData, at the same two addresses as `raydium_programs`' CPMM entries.
  `tests/litesvm/src/fork.rs` keeps loading `raydium_programs` (it also holds AMM v4) and picks a
  CPMM build by name (`CpmmDeploy::Oct2026`, the default, or `CpmmDeploy::Sep2026`); an address
  lookup resolves to the redeploy. The harness records CPMM's guard slot as 452,376,460
  (`fork::FIXTURE_GUARD_SLOTS`, a test-side constant; the program constant is unchanged).
  The web e2e validator deploys the same build (`apps/web/e2e/validator/genesis.ts`
  `venuePrograms()`).
- Every `Env` runs on the mainnet SPL ELFs (WP-3).
- WP-7 uses `squads_programs`.

`raydium_programs` stays for T-LATCH. It is the deploy that `VENUE_GUARD_SLOTS_2026_09_27` names.
`latch_raydium_it`'s `the_2026_10_cpmm_redeploy_latches_until_revalidated` starts from it and
upgrades to the redeploy's ELF through the loader.

## CPMM 2026-10 redeploy: interface and layout diff (M10 WP-2)

The two dumped builds were compared with each other and with `raydium-io/raydium-cp-swap` at
`59fb845` (`master`, the build R-RAY matched) and `bdeee23` (`feat/creator-fee-share` head,
2026-09-15; 5 commits ahead of `59fb845`, none behind). Method: Anchor discriminators of every
instruction, account and event of both commits, searched in each ELF as `lddw` immediates and raw
bytes; `Instruction: <Name>` log strings; the new owner keys as `lddw` limbs; section sizes
(`llvm-readelf`); the source diff `59fb845..bdeee23`; the 21 `AmmConfig` fixtures decoded.

| | 445,763,504 build (`raydium_cpmm.so`) | 452,376,460 build (`raydium_cpmm_2026_10.so`) |
|---|---|---|
| Trimmed-ELF sha256 | `bdaff73e…f8cc` | `93cad245…a0d2` (= OtterSec's on-chain hash) |
| `.text` / `.rodata` | 621,928 / 19,416 B | 653,672 (+31,744) / 19,744 (+328) B |
| SBPF `e_flags` | 0 | 0 |
| Instructions | the 18 of `59fb845` | the same 18 plus `create_creator_fee_share` (`7f1fbc77…`) and `close_creator_fee_share` (`4aa6523a…`): exactly `bdeee23`'s 20 |
| Accounts | `AmmConfig`, `PoolState`, `ObservationState`, `Permission`, `SupportMintAssociated` | the same plus `CreatorFeeShare` (`1eeb62fc…`, 145 B) |
| Events | `SwapEvent`, `LpChangeEvent` | unchanged |
| Build flavour | mainnet keys | mainnet keys: `create_creator_fee_share_owner` `RayFfEhJ…` present (2 uses), devnet `DRayDe7A…` absent; `admin` uses 13 → 15 |

What changed between `59fb845` and `bdeee23` (program sources only):
- **Swap path: nothing.** `swap_base_input.rs`, `swap_base_output.rs`, `curve/*` (except one new
  helper in `fees.rs`), `error.rs`, the events and the oracle are byte-identical. The swap
  accounts, the fee math at swap time and `vault_amount_without_fee` (vault minus protocol, fund
  and creator counters: the pricing reserves short.win reads) are the same.
- **`AmmConfig`** gains `creator_fee_share_rate: u64` at offset **116**, carved out of the
  padding (`padding [u64; 15]` → `[u64; 14]`); the length stays **236**. All 21 fixtures (equal
  to the live configs at slot 452,488,420) hold 0 there and zero padding. short.win reads
  `AmmConfig` up to `creator_fee_rate` @108 only.
- **`PoolState`**: no field change, length **637**. New method `settle_creator_fee` moves the
  protocol's share of the accrued creator fee from `creator_fees_token_*` to
  `protocol_fees_token_*` when the creator fee is collected; both counters are excluded from the
  pricing reserves, so a collection does not move short.win's price.
- **`collect_creator_fee` / `collect_creator_fee_permissionless`** take a new trailing
  `creator_fee_share` account (and the permissionless one an `amm_config`), and pay the creator
  `fee − ⌊fee · share_rate / 10⁶⌋`. `update_amm_config` param 8 sets the share rate. short.win,
  its keeper and services never call either instruction.
- `collect_excess_lamports` runs its token CPIs before its direct lamport moves (a fix).
- `638d560`'s `collect_shared_creator_fee` is absent from the ELF, so the build is at least
  `0dde43d`; `0dde43d` and `bdeee23` differ only in the account order of the two collect
  instructions. `ef96045` adds `[workspace.metadata.cli] solana = "3.1.10"`, which selects
  solana-verify's image.

**Reproduction (M10 WP-2, 2026-10-02).** `raydium-cp-swap@bdeee23` built with solana-verify 0.5.2
(`solana-verify build --library-name raydium_cp_swap`, default `--arch v0`) in
`solanafoundation/solana-verifiable-build@sha256:f71be5ca…80d3` (its IMAGE_MAP entry for
3.1.10), run without Docker through WP-4's `scripts/svb-shim` (the image fetched by digest,
every blob checked, run under bubblewrap). The output `raydium_cp_swap.so` (722,632 B, sha256
`2a90c330…8b3d`) is **byte-identical** to `programs/raydium_cpmm_2026_10.so`; its executable
hash is `93cad245…a0d2`, the on-chain one. The deployed CPMM is therefore `bdeee23`.

Conclusion: **no adapter change.** The swap interface and both layouts short.win reads are
unchanged (`shortwin-venue-state`'s unit test spells `CONFIG_LEN` as `… + 15 * 8` of padding; the
new field is one of those 15 words, the total still 236 B). Every LiteSVM suite, the services
workspace and the m3/m5/m6 benches pass on the redeploy; each CPMM swap costs 5 CU more inside
the venue (`tests/bench/m3_cu`).

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
