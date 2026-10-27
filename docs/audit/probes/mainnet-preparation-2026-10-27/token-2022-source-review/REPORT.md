# Scoped Token-2022 11.0.0 → 11.1.0 compatibility review

Read-only engineering review, 2026-10-27. No program, root lockfile, fixture,
configuration pin or production artifact was changed. No signed transaction or
new-ELF execution test was run. This reviews the newly observed upstream upgrade;
it does not resume either stopped historical whole-repository audit/fuzz task.

## Identity: established attribution, not an embedded-version guess

| Item | Tested fixture | Newly observed mainnet |
| --- | --- | --- |
| Official tag | `program@v11.0.0` | `program@v11.1.0` |
| Peeled tagged commit | `9bc02757f600ffe754746708a8a072bcd49d1260` | `8867f751c0f69367ba03af4f85510b5611989491` |
| Deploy slot | 427,147,035 | 460,497,271 |
| ELF bytes | 632,560 | 611,056 |
| Full SHA-256 | `9bbf90b30e06778ca0feca100b29f0eeb9be576ae024f6323cc207308f51a5d1` | `3dde54d202caa2d10514fd58a154df5f26f07643f9f5fd0402ff08084f0cb954` |
| Trimmed SHA-256 | `7ea94a027005b39196fa08e6d0ddcd55ec32eb43266179921a834a1ba2eb1947` | `5e888b294bd1941bfc47107a73d5e40bb24fde0d639f69099e7920ad8b6413d9` |

Both downloaded official release assets are **byte-for-byte equal** to their
respective retained fixture/mainnet observations, including the complete ELF
rather than just a version string. The official release publishes the new
trimmed hash and source commit. The separately retrieved OtterSec public verifier
reports `is_verified=true`, both hashes equal to `5e888b29…413d9`, commit
`8867f751…9491`, last verified **2026-10-23T15:43:22.904525**. This supplies retained
publisher and third-party source/build attribution. It is not a fresh independent
local reproducible build performed in this task, and the upgrade authority remains
able to replace the program again.

Sources:
- https://github.com/solana-program/token-2022/releases/tag/program%40v11.0.0
- https://github.com/solana-program/token-2022/releases/tag/program%40v11.1.0
- https://github.com/solana-program/token-2022/compare/program%40v11.0.0...program%40v11.1.0
- https://verify.osec.io/status/TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb

The new upstream release links a Token-2022 audit; that is not an external audit
of short.win. No conclusion here relies on treating that upstream report as a
review of short.win's integration.

## Scope and source results

The complete tagged trees change **302 paths**. GitHub's compare API exposes only
300 files, so a blob-filtered bare upstream repository was used to enumerate the
full tree diff. No checkout or upstream code execution was needed. Most of the
large processor diff concerns the conversion of upstream tests to Mollusk; the
production changes and unchanged files were checked separately.

| short.win dependency | Source comparison and implication |
| --- | --- |
| Mint/account base layouts and instruction/error ABI | Upstream `interface/src/state.rs`, `pod.rs`, `instruction.rs`, and `error.rs` are byte-identical between tags. The complete `ExtensionType` and `AccountType` definitions are identical. The base 82-byte mint, 165-byte account, account-type byte and TLV interpretation used by short.win do not acquire new field offsets/discriminants in this diff. This is a source comparison, not a substitute for executing the new binary. |
| `TransferChecked` | The complete 11,295-byte `program/src/processor.rs::process_transfer` source is identical between tags. This includes its mint/account/decimal/authority checks. Its memo-check callee gains v4 acceptance, and dependency builds changed; whole-program executable identity is different and no equal-CU/log claim follows from unchanged function text. |
| `InitializeAccount3` / ATA sizing | The public wrapper is unchanged, but `_process_initialize_account` and `process_get_account_data_size` now use allocation-free TLV walkers and a deduplicating length accumulator. Required mint→account extension mappings are unchanged. The six mint extensions allowed by short.win—metadata pointer, token metadata, group pointer/group, member pointer/member—map to no required account extension in both versions. Therefore the plain 165-byte pool vault remains the expected size; ATA's requested ImmutableOwner remains an additional account extension. This is the most directly relevant changed implementation and requires actual initialization/ATA tests. |
| Memo-required withdrawals | `program/src/extension/memo_transfer/mod.rs` now accepts memo v4 in addition to existing memo v3/v1. short.win still CPIs memo v3 immediately before the Token-2022 transfer; that previous-sibling route remains accepted in source. Incoming memo account layouts are unchanged. Existing positive and negative sibling tests must run on 11.1.0. |
| CPI guard | `process_withdraw_excess_lamports` adds a CPI-guard refusal on token accounts. short.win does not call WithdrawExcessLamports/UnwrapLamports. Its token-account transfer CPI guard behavior is in the unchanged transfer body. Deposit from a CPI-locked user token account remains a documented refusal; emergency open pauses do not change this. |
| Mint safety / transfer hooks / fees | short.win rejects transfer hooks, transfer fees, confidential/permissioned-burn, nontransferable, pausable, interest-bearing, scaled-UI and all unknown mint extensions. New TLV sizing, transfer-hook dependency optimizations and UI-float implementation changes do not justify broadening that allowlist. The upstream extension enum is unchanged. No mint-safety or trading-adapter code change is identified as necessary by this scoped source review. |
| Close/sync/immutable owner | `process_close_account`, `process_sync_native` and `process_initialize_immutable_owner` source bodies are unchanged. The native WSOL leg remains the legacy token program; no new Token-2022 quote asset is admitted. |

Key local paths are `programs/shortwin/src/mint_filter.rs:49–55`,
`crates/shortwin-venue-state/src/mint.rs:13–27,89–121`,
`programs/shortwin/src/instructions/create_market.rs:152–153,342–350`,
`deposit.rs:170–180`, `withdraw.rs:38–45,146–160,254`, and
`programs/shortwin/src/engine/trader.rs`. Selected scoped production diffs, full tree path/stat listings, file/function hashes,
tag/release/verifier metadata and local test searches are retained beside this report.
The complete bare upstream repository and full tagged source snapshots remain in the
local `target/token2022-compat-review-2026-10-27` working record; they are not all
duplicated in this compact retained packet.
The new tagged interface package is 3.1.2; short.win's older generated interface
is not silently upgraded by this review.

## Qualification still required before promotion

The current test loader is explicit: `tests/litesvm/src/fork.rs:3786–3787,3810–3823`
pins the **old** trimmed Token-2022 hash and reconstructs the old ProgramData.
`security.rs::assert_mainnet_spl` independently checks that identity. Consequently,
a green current CI run still executes the old Token-2022 fixture. Setting
`SHORTWIN_BUNDLED_SPL=1` would test LiteSVM's bundled program and would not qualify
the new observed 11.1.0 binary.

The legitimate next preparation batch is:

1. Independently review/accept this upstream source and identity disposition.
   Retain the new ELF/header as a **new dated fixture group**, preserving the old
   group and its checksums; update explicit active test-loader pins to the new
   identity in the reviewed batch. Re-run static and live fixture checks. Do not
   merely relabel the old fixture or suppress the current drift.
2. Run old/new differential and direct new-ELF tests for market creation,
   InitializeAccount3/ATA (plain mint and all six admitted metadata/group types),
   token-program confusion, malformed/unknown/unsafe TLVs and 165-byte vaults.
   Existing suites include `market_create`, `mint_filter_extensions` and
   `mint_filter::tests`. Retain rejected-extension controls as well as successes.
3. Run the lender and position lifecycle suites on both token programs, including
   `lender_it::flows::deposit_and_withdraw_round_trip_on_both_token_programs`,
   `engine_it::flows::open_and_close_token_2022_base`, liquidate/expire/settlement,
   trader sweep/cashback, Token-2022 CPMM and Pump/graduation forks. Run
   `security::sec_7_memo_required_destination_is_served` and
   `security::t_memo_control_top_level_sibling_check`; preserve NoMemo/gapped-sibling
   negative controls, and actual CPI-guard deposit behavior.
4. Re-run affected CU/trace/depth/size benches (Token-2022 cases in m1/m2/m3/m5,
   PumpSwap/m6 and keeper/m7 where their fixtures use it), recording real budgets
   instead of assuming a smaller ELF is cheaper. Re-run keeper Token-2022 Raydium
   and wipe scenarios and indexer replay/session checks on generated new-ELF flows.
5. Run fresh affected fuzz profiles on the new frozen fixture baseline. `mock`
   and `mixed` include Token-2022; `curve` includes t22_115 and graduation;
   `pumpswap` includes Token-2022 mints; `cpmm` explicitly includes its Token-2022
   pool. A fresh all-six-profile campaign is the clear final-baseline route.
   Keep `amm-v4` as an unchanged-venue control rather than inventing a separate
   Token-2022 coverage count. The previous 24-hour run remains valid evidence for
   its old baseline and unchanged short.win bytes; it did not execute this new
   external binary. Do not borrow that duration for the new baseline. AUDIT §12's
   upstream-upgrade clause explicitly requires affected forks, benches and fuzz;
   changing the adapter would additionally invoke the full fix-batch policy.
6. Obtain fresh exact-revision CI/source/artifact receipts and repeat current
   live identities immediately before mainnet commissioning. The selected initial
   venues, funded keeper/RPC/alerts and transparent unaudited-launch disclosures
   remain separate operating requirements.

**Result:** source-level compatibility is plausible within the existing mint
allowlist, and source/build attribution is established through exact publisher
bytes plus the retained public verifier. There is no identified requirement to
change short.win's on-chain adapter in this scoped review. **New-ELF execution,
bench/fuzz qualification and fixture promotion remain pending**, so this review
does not close the current live-preflight gate or authorize the final mainnet tag,
deployment or unpause.

## Independent source/provenance review

`independent-review.json` records a separate AI review of publisher assets/tags,
source/function/layout comparisons and compatibility conditions. It approves
preparing fresh fixture qualification; actual new-ELF execution remains open.
The full extension-module diff and new account-length helper are retained as
`interface__src__extension__mod.rs.diff.gz` and
`interface__src__extension__account_len.rs.new.rs.gz`, covering production helpers
after the first cfg(test) marker as well as the shorter selected production diffs.
