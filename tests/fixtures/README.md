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
| `pump_programs`, `pump_state` | `programs/pump{,_fees,_amm}.so`, `accounts/pump/*` | pump, pump-fees and PumpSwap (M5): the 2026-09 deploys (449,734,335 / 446,465,969 / 446,462,733), T-LATCH's "before" since the 2026-10 redeploy |
| `pamm_state`, `pamm_synthetic` | `accounts/pamm/*` | PumpSwap pools (M6). The synthetic group is derived offline |
| `raydium_cpmm_2026_10` (M10) | `programs/raydium_cpmm_2026_10.so`, `accounts/raydium_cpmm_2026_10/*` | CPMM **as redeployed at 452,376,460**. This group holds the ELF only. The pools and configs stay `raydium_state`'s, because the redeploy kept their layouts (AmmConfig 236 B, PoolState 637 B) and the 21 live configs still equal the fixture bytes. Every CPMM test loads this build |
| `spl_programs` (M10) | `programs/token_2022.so`, `accounts/spl/*`, `programs/spl_memo_v3.so` | the deployed Token-2022 (upgradeable, a verified build) and spl-memo v3 (BPFLoader2, where the account data *is* the ELF) |
| `pump_programs_2026_10` (M10) | `programs/pump{,_fees,_amm}_2026_10.so`, `accounts/pump_2026_10/*` | pump, pump-fees and PumpSwap **as redeployed on 2026-10-02**. ELFs only: the state stays `pump_state`'s and `pamm_state`'s, because the redeploy only appends fee buckets (`BondingCurve` @125/@133, `Pool` @271/@279) and pump `Global`, both `FeeConfig`s, `GlobalConfig` and the volume accumulators still equal the fixture bytes on mainnet (slot 452,964,363). T-LATCH's "before" since the 2026-10-13 re-pin |
| `pump_programs_2026_10_13` | `programs/pump_2026_10_08.so`, `programs/pump_amm_2026_10_08.so`, `programs/pump_fees_2026_10_12.so`, `accounts/pump_2026_10_13/*_program*.bin` | pump and PumpSwap **as redeployed on 2026-10-08** and pump-fees **as redeployed on 2026-10-12** (the 2026-10-13 re-pin; dumped at slot 456,395,404). Every curve and PumpSwap test loads this group |
| `pump_state_2026_10_13` | `accounts/pump_2026_10_13/*` | the state those builds introduced, one snapshot (slot 456,395,752): pump's 1,088-byte `Global` (`pump_state`'s 1,087 bytes, then `max_curve_depth` = 1), pump's `FeeConfig`, PumpSwap's `GlobalConfig` and `FeeConfig` (byte-identical to `pump_state`'s), and ten coin roles: 166-byte curves (`near166`, `fresh166`, `buckets166`), dormant 141- and 151-byte curves (`grow141`, `grow151`), a complete unmigrated synthetic completion of depth 1 (`synthetic_stuck`), a migrated one with its pool (`graduated_synthetic`, 98u8RJ), a cashback and a mayhem 166-byte curve, a WSOL pool carrying fee buckets (`wsol_pool_buckets`). An address both state groups hold resolves to this group |
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
| pump `6EF8rrec…` | 452,654,932 (authority `7gZufwwA…`) | `b0b777c6…0c75` | UNVERIFIED: no public source; the release of `pump-rust-client` 0.2.0's unreleased artifacts (below) |
| PumpSwap `pAMMBay6…` | 452,654,882 (authority `7gZufwwA…`) | `01557035…3cc3` | the same |
| pump-fees `pfeeUxB6…` | 452,655,002 (authority `7gZufwwA…`) | `f166c296…62fd` | the same |
| pump `6EF8rrec…` | 454,596,459 (authority `7gZufwwA…`) | `6956c2f8…bf6f` | UNVERIFIED: the interface of pump-public-docs `2293f9a6` / `pump-rust-client` 0.4.0 |
| PumpSwap `pAMMBay6…` | 454,596,406 (authority `7gZufwwA…`) | `3ffd5e3b…2ee1` | the same |
| pump-fees `pfeeUxB6…` | 456,132,171 (authority `7gZufwwA…`) | `8ad2ca93…b8b8` | UNVERIFIED: 0.4.0's `pump_fees.so` (853,528 B) is not this build; its `withdraw` is in no IDL (below); the 2026-10-08 build it replaced (454,596,501) can no longer be fetched |

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
- Every curve and PumpSwap test, bench and fuzz profile runs on `pump_programs_2026_10_13` (the
  2026-10-13 re-pin). `fork.rs` keeps loading `pump_programs` and `pump_programs_2026_10` too and
  picks a build by name (`PumpDeploy::Oct2026Synth`, the default `PumpDeploy::CURRENT`;
  `PumpDeploy::Oct2026`; `PumpDeploy::Sep2026`); an address lookup resolves to the latest
  redeploy, and `fork::FIXTURE_GUARD_SLOTS` carries its three deploy slots (=
  `VENUE_GUARD_SLOTS_2026_10_13`, the newest baseline). `Env::pump_build_running` tells the
  three builds apart by the instructions each added (pump `buy_v3`, then `multi_hop_curve_swap`;
  PumpSwap `buy_v2`, then `multi_hop_swap`; pump-fees `withdraw`). The coin roles of both state
  groups load with their own snapshot's clock. The web e2e validator deploys the same builds.

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

## pump, PumpSwap and pump-fees 2026-10-02 redeploys: what changed (M10 pump revalidation)

`check --live` on 2026-10-03 found all three pump programs redeployed on 2026-10-02 between
15:47:07Z and 15:47:39Z (ARCH V21, U30). There is no public source: `pump-public-docs` has no
commit after `cb188ce0` (2026-09-29), the on-chain Anchor IDLs were last written 2026-09-12
(pump) and 2026-05-14 (PumpSwap, pump-fees), and npm's SDKs are unchanged. The release matches
the **unreleased ELF artifacts of `pump-rust-client` 0.2.0** (crates.io, 2026-09-29), which
R-VEN Update H2 and R-PUMP Update K2–K5 already ran. Method: the Anchor discriminators of every
instruction, account and event of the PPD IDLs, the 0.2.0 IDLs and the on-chain IDLs, searched in
each ELF as `lddw` immediates and raw bytes; `.text` / `.rodata` sizes and bytes; every Anchor
error origin (file, line, code) from the disassembly.

| | 2026-09 deploys (`pump_programs`) | 2026-10-02 redeploys (`pump_programs_2026_10`) |
|---|---|---|
| pump trimmed ELF / `.text` | `dbf5541c…c42f` / 1,542,264 B | `b0b777c6…0c75` / 1,640,304 B |
| PumpSwap trimmed ELF / `.text` | `c04bea8b…6522` / 950,384 B | `01557035…3cc3` / 1,040,088 B |
| pump-fees trimmed ELF / `.text` | `1092b124…b788` / 775,384 B | `f166c296…62fd` / 779,872 B |
| Against 0.2.0's artifacts | — | the same discriminators, the same `.rodata` byte for byte, the same error sites; `.text` 312 / 352 / 1,296 B longer (pump / PumpSwap / pump-fees): source lines added in pump `buy_v2.rs` and `lib.rs`, PumpSwap `swap/buy.rs`, `swap/sell.rs` and `swap/trade_v2.rs`, pump-fees `fee_sharing/*` (one more `NotEnoughRemainingAccounts` check). Not the same build |
| New instructions | — | pump `buy_v3`, `sell_v3`, `buy_exact_quote_in_v3`, `sweep_protocol_fee`, `sweep_creator_fee`; PumpSwap `buy_v2`, `sell_v2`, `buy_exact_quote_in_v2`, `sweep_protocol_fee`, `sweep_creator_fee`; pump-fees none. Nothing removed. (`set_quote_control_mint_reserves` / `set_quote_control_reserves_admin` are new against the on-chain IDL only: the 2026-09 pump ELF already dispatches both) |
| Layouts | — | `BondingCurve` + `creator_fee` @125, `protocol_fees` @133 (141 B); `Pool` + `protocol_fees` @271, `creator_fees` @279 (287 B; `create_pool` and every trade's growth target). (`QuoteControl.reserves_admin` is new against the on-chain IDL only: the 2026-09 pump ELF already dispatches `set_quote_control_reserves_admin`.) Every other account and every event short.win or the indexer reads is unchanged |
| short.win's CPIs | — | pump `buy_v2` / `sell_v2` (+ an optional trailing `partial_fill` byte short.win does not send), `migrate_v2`, PumpSwap `buy` / `sell`, `claim_cashback`, `init_user_volume_accumulator`, pump-fees `get_fees_with_quote_mint` (called by the venues): the same discriminators, account lists, PDA seeds and arguments (IDL diff) |

What the release does (on the dumped ELFs, `pamm_it::buckets`, `pump_it::buckets`):
- v2 PumpSwap and v3 curve trades pay the buyback slice in the trade and keep the rest of the
  protocol fee and the creator fee in the pool's quote vault / the curve account, counted in the
  buckets; on PumpSwap the buckets are taken off `virtual_quote_reserves`, so the price `vault +
  VQR` excludes them; on the curve they stay outside `rQ`. A v2/v3 trade costs or pays exactly the
  v1/v2 quote. Anyone can sweep a bucket to its recipient; a sweep does not move the price.
- A PumpSwap v1 `sell` is paid from `vault − buckets` (6063 `InsufficientRealQuoteReserves`
  above it; the 2026-09 build paid up to the vault).
- Every PumpSwap trade grows a shorter pool to **287** bytes (271 before) and tops its rent up
  from `user`: at the fixture's 5,080 lamports per byte, 81,280 more than before on any pool
  shorter than 287 bytes (386,080 from 211 B; short.win measures it as `Open.venue_rent`).
- `migrate_v2` moves `rQ`, not the curve's buckets, so `B̂_str` prices the same pool.
- On mainnet on 2026-10-03 (slots 452,964,412–452,964,638): no bucket is non-zero on the
  fixture pools and curves or on any of the 1,873 PumpSwap pools of 287 bytes; a sample of 40 +
  40 recent pump and PumpSwap transactions held no v2 / v3 trade. A full census at slots
  452,994,774–452,995,262 (`getProgramAccounts` by `dataSize` with a `dataSlice`, public RPC):
  **no SOL-quoted curve and no WSOL-quoted pool carries a bucket** (5,996,499 + the SOL-quoted
  141-byte curves; 165,688 WSOL pools of 301 and 48 of 287 bytes), but the buckets are live on
  venues short.win does not list: 904 non-SOL-quote curves of 151 bytes, 627 of 141 bytes
  and 265 non-WSOL pools of 301 bytes hold non-zero buckets.

## pump 2026-10-08 and pump-fees 2026-10-12 redeploys: what the re-pin measured (gate G0)

`check --live` at slot 456,392,667 (2026-10-13) found pump, PumpSwap and pump-fees redeployed
again (454,596,459 / 454,596,406 / 456,132,171) and Jupiter at 454,465,850; after the two
groups above and `VENUE_GUARD_SLOTS_2026_10_13` it is clean (slot 456,395,860). On the dumped
ELFs, with short.win's adapter unchanged:

- **v2 (short.win's legs today), T-PUMP-MATH:** every cost, proceeds and error code as before. A
  curve shorter than **166** bytes grows to 166 on its next v2 or v3 trade, its rent reserve
  topped up by the user (76,200 lamports from 151 B; 261,600 on the 49-byte legacy curve, whose
  reserve is the old rent): out of `S` on short.win's sale, one System transfer more in that
  leg's trace (`trace_pump_it`: an open 19 entries with the creator-vault top-up). pump counts
  the reserve as `lamports − rQ −` the fee buckets: on a 141- and a 151-byte curve with
  planted buckets (20,000 + 30,000) a v2 and a v3 buy each topped up 127,000 / 76,200, the
  full shortfall (`a_bucketed_short_curve_grows_with_its_reserve_net_of_the_buckets`;
  `fork::curve_growth_top_up` is the harness's one model of it, the I13 hook's included).
- **v3 (`pump_math_it::v3_*`, T-PUMP-MATH-V3):** a buy's input is `⌊q·vQ/(vT − q)⌋ + 1` — `⌈⌉`
  off an exact division; on all 400 exact divisions constructed pump took `⌊⌋ + 1`, the v2
  charge —, a sale's `⌊⌋`, each fee `⌈⌉` per component; the buyback is paid out and the rest of
  the fees stays in the buckets outside `rQ`; `vQ − rQ` is unchanged. Events carry `ix_name`
  `buy` / `sell` / `buy_exact_quote_in` and the zero `fee_recipient`; a coin without a creator
  pays no creator fee though its event names the tier's bps.
- **Synthetic migration:** `buy_v3(rT + δ)` buys `rT` on the curve, then `δ` from the would-be
  pool `(X_s, Y_s)` = (the live `curve_base_ata − rT`, donations counted; `rQ + n₁ − fee`) at
  `⌈Y_s·δ/(X_s − δ)⌉` (`⌈⌉` on an exact division), both parts at the curve's fee tier at its
  pre-trade market cap (`⌊vQ·supply/vT⌋`, the live supply). The tier tripwire (e) shows it: a
  synthetic `FeeConfig` gives each market cap a part could be rated at (the curve before the
  trade and at completion, the would-be pool before and after the pool part, the curve before
  at the fixed 10¹⁵ supply) a tier of its own, with a bottom and a top tier holding none and
  the stable tiers, `flat_fees` and `exotic_flat_fees` bps of their own; on `near166` both
  parts named the tier of the curve's pre-trade market cap at the live supply alone (80 / 25
  bps), and the user paid exactly that charge (45,906,126,201). `B̂_v3` with that schedule on
  both legs covers it by 4 lamports (bound 7). short.win's `φ` (`fees::pump_curve`) rates the
  same pre-trade curve at the fixed 10¹⁵ supply, which on this burned coin is another tier (70
  / 22 bps), and `B̂_v3` at it is 59,057,853 lamports under the charge. So a multi-tier curve
  `FeeConfig` together with a burned supply makes `B̂_v3` undercut pump's charge, and an exit
  fails 6002: the M5 live-supply deviation
  (`tripwire_the_curve_tier_market_cap_uses_the_live_mint_supply`) reaches the synthetic leg
  too. The live curve `FeeConfig` is one flat tier, so no quote differs today; (e) pins it as a
  tripwire that flips when the math resolves the tier at the live supply.
- **Synthetic migration, events and pool:** `TradeEvent`, `CompleteEvent`,
  `PostCompleteBuyEvent`; `post_complete_base_out` = δ, `post_complete_quote_in` = the pool
  part, `rQ` without it, `vQ − rQ = ivqr`. `buy_exact_quote_in_v3` past `rT` spends the rest by
  the v2 exact-quote-in rule and takes `⌊(in − 1)·X_s/(Y_s + in − 1)⌋`. `migrate_v2` then
  creates the effective reserves `(X₁, Y₁)` = (the live `curve_base_ata`, `rQ + pcQ − fee`),
  with a donation before or after the completion. Its boost is `VQR = ⌊X₁·Y₁/10¹⁵⌋`; the
  spec's `⌊(Y − pcQ)·2069/10⁴⌋` holds only while no token was donated (a 3e12 donation after
  the completion: 17,852,426,396 against 17,584,506,040; `vault + VQR = Y₁` either way). On a
  cashback coin `buy_v3`, `buy_exact_quote_in_v3` and `sell_v3` each fail 6094; the 2026-10-02
  build reverts a buy past `rT` (6021).
- **pump-fees inside short.win's PumpSwap legs (`pamm_it::v3_era`, T-PFEE-CPI):** one
  `get_fees` / `get_fees_with_quote_mint` per sale and per buy, on `[PumpSwap's FeeConfig,
  PumpSwap]`, read-only; never `withdraw`, no short.win account.
- **pump-fees `withdraw` (R-PFEE, from the ELF: `pamm_it::v3_era::r_pfee_withdraw_probe`):**
  accounts `user` (signer, writable), `referral_vault` (a system account at PDA(pump-fees,
  `["referral-vault", user]`), writable), `quote_mint`, `vault_ata` (writable, token authority
  `referral_vault`), `user_ata` (writable, token authority `user`), `token_program`,
  `system_program`, `event_authority`, `program`; `user_ata` must be `user`'s (2015 on another
  owner's ATA). It moves the vault ATA's whole balance to `user_ata` (6036 `NothingToWithdraw`
  when empty); the vault PDA's lamports and `user`'s stay. Its writable accounts are `user`,
  the vault PDA derived from it, that PDA's ATA and an ATA `user` owns: never a `FeeConfig`, a
  pool or a curve. A short.win account could be among them only if a program short.win invokes
  forwarded the trader PDA's signature into `withdraw` as `user` (a signer stays one in nested
  CPIs). T-PFEE-CPI shows the 2026-10-08 PumpSwap does not: it calls only `get_fees` /
  `get_fees_with_quote_mint`, read-only. Were one to, `withdraw` could only credit the trader
  PDA's own ATA from its own referral vault. So R-PFEE's stop rule does not trigger on these
  builds; pinning pump-fees' 2026-10-12 build on mainnet (`initialize_config`, or Emergency's
  `revalidate_venue`) stays the owner's step.

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
