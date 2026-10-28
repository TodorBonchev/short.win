# Current Jupiter route evidence — 2026-10-28

This directory preserves read-only mainnet RPC and Jupiter API observations,
including rejected candidates. No transaction was signed or sent, no upstream
fixture was promoted, and no paid provider was purchased. Quote availability,
local instruction compatibility, historical snapshot execution, and a funded
production service are distinct checks.

## Sources and capture contract

`index.json` pins original raw and stored hashes/lengths. Large RPC bodies use
deterministic gzip; decompression restores the exact response bytes. Request
receipts preserve public parameters, wall timestamps and RPC context slots.
Neither provider keys nor credential-bearing RPC URLs are retained. RPC account
reads span several slots; the per-account source slots make this non-atomic
snapshot explicit. `accounts.manifest.json` records full account-data hashes,
loader slots, padded and trimmed ELF hashes. Lookup table verification compares
the entire API vector with actual active on-chain table bytes.

Official current references were retained with source URL/time/hash receipts:

- [Jupiter build API](https://developers.jup.ag/docs/api-reference/swap/build)
  and [rate limits](https://developers.jup.ag/docs/portal/rate-limits).
- [Pump public docs and IDLs](https://github.com/pump-fun/pump-public-docs).
- [Token-2022 extension implementation](https://github.com/solana-program/token-2022/blob/main/interface/src/extension/mod.rs).

Keyless Jupiter access was used with at least 2.2 seconds between requests per
capture. The documented limits are 30 requests/minute keyless and 60/minute
with a Free key, sharing the main API bucket. These observations do not prove
production RPC reliability or provider capacity for concurrent loan quantities.

Current Jupiter ProgramData `4Ec7ZxZS6Sbdg5UGSLHbAnM7GQHp2eFd4KYWRexAipQT`
remained at deployment slot **459479624**, length **2,892,269**, full SHA256
`21e28c59e3b0a5536e48d31a1ca7346514a2bc61d70dc1388c920d7e3be35c1b`.
Its trimmed ELF SHA256 is
`5e31d320f5d88ae93c7983ce82a1497067abd86ff540b35073abaeeb25ef397b`.
The canonical SPL Token and Token-2022 executable/ProgramData bytes are also
captured; Token-2022 deployment slot is **460497271**. No changed Jupiter
deployment is silently accepted by this evidence.

## Request profile and observed limits

The final requested profile is a deterministic per-quote **16 → 32** account
budget fallback, excluding **Scorch**, with ordinary Route V2 headers, zero
ancillary fees, a program ceiling of 48 full instruction metas, 768 data bytes
and eight route steps. The service and SDK now use the stricter **40-meta**
ceiling in their model-bound profile. Earlier capture analyses retain their
original 48-meta diagnostic filter; a recorded pass of that older filter is
not an override of the current client/service limit. A smaller request budget
does not bound the full returned list.
The returned scalar amounts, slippage and domains must match the request.
Provider-selected shared V2 instructions remain unsupported and are retained as
rejections, rather than interpreted as ordinary V2 instructions.
The full serialized protocol transaction must also fit 1,232 bytes using actual
lookup table contents. A later mixed RAY route passed the 48-meta instruction
envelope but produced a 1,278-byte open transaction and remains a rejected
candidate. The smaller retained CLMM witness is the selected positive RAY case.

The initial eight budget-32 exploratory captures contained 35 quote attempts:
nine passed the ordinary-header/envelope filter and 26 failed it; three returned
more than 48 metas. These are bounded observations, not a market-wide success
rate. Scorch later failed actual program CPI with custom error `0x1554` despite
valid headers and retained lookup tables. The exclusion follows that concrete
failure; it does not certify every other Jupiter venue.

The final production-size read-only witnesses are:

| Capture | Exact raw q | Net sale minimum, lamports | Successful whole-q buy budget | Spread, ceiling bps |
| --- | ---: | ---: | ---: | ---: |
| `bonk-production-adaptive` | 400,000,000,000 | 124,046,329 | 127,163,071 | 252 |
| `jup-production-adaptive` | 40,000,000 | 111,131,324 | 113,923,568 | 252 |
| `ray-production-exclude-scorch16` | 10,000,000 | 178,918,323 | 183,413,759 | 252 |
| `pump-curve-adaptive` | 2,500,000,000,000 | 113,215,010 | 118,335,287 | 453 |
| `pump-graduated-adaptive-440` | 3,500,000,000,000 | 124,540,925 | 130,674,096 | 493 |
| `pump-fresh-native-adaptive` | 4,000,000,000,000 | 130,198,383 | 136,610,163 | 493 |

The first three selected routes succeeded at the primary budget 16, so no
fallback was necessary. Pump curve and PumpSwap attempts at 16 returned
`No routes found`; their selected budget-32 instructions have 30 metas.
`qualification.json` and selected `response_file` fields identify exact builds.
The capture tool uses bounded successful ExactIn budgets, not a proof of the
globally cheapest ask. A too-large successful buy is rejected by the spread
policy; exploratory doubled budgets must never be treated as qualified prices.

Further read-only targets near one SOL produced finite whole-q quote pairs for
32 million BONK, 400 JUP and 60 RAY, each with a 252-bps quoted spread. These
are quote evidence only. PumpSwap's 28-million-token target and the fresh
curve's 32-million-token target produced sale minima of 0.963258558 and
0.972177172 SOL respectively, but no whole-q acquisition within the tested
5%-spread budget. They remain unavailable under that tested bound. No five-SOL
capacity or universal mint coverage is claimed.

## Pump state and Token-2022 evidence

`current-pump-discovery-20261028` preserves independently fetched state proofs:
legacy curve mint `8FxewF272LJ7Usov6PZk2gu1JKoR5Dq1v4sDqavJpump`
was incomplete; graduated mint `Coyj3LtKn1BNSgWc9HsGK5SKoGfEoDaymig4wrN6pump`
had a completed curve and actual PumpSwap-owned pool
`H739cDUEhbW2ckYhjetYSwxTVT5vd6GrTW5GdfaTQJgB`. The current fresh mint
`5PjPp8gDLjDsoXyvUtYzWKPYaRbfNaoxW29LTFvVpump` has a 403-byte Token-2022
mint and incomplete native-SOL curve
`8vk2UuJTKKY2yXdq2qTbWdsgqJJBptantaD6dGdgoZYp` at the retained snapshot.
These classifications come from actual owned account bytes, not names or the
fact that a mint ends in `pump`.

The initial recent-token sample contained 29 Pump mints with metadata-only
Token-2022 extensions and one plain SPL Launchlab mint. Their initialized base
mint headers had no freeze authority; the 29 Pump mints had zero padding,
account type 1, and only extension types 18 (MetadataPointer) and 19
(TokenMetadata), with complete bounded TLV data. `recent-mints.tlv.json`
preserves exact fields. A later recency sample also proves both native SOL and
nested quote curves exist. Nested quote raw reserves are never labelled SOL.

The original 82-byte-only admission rule rejected metadata-bearing mints. The
fresh metadata capture was explicitly diagnostic against that old rule. The
final reviewed replacement ELF admitted the actual metadata mint and passed its
complete lifecycle in the replay below. The capture's original plain-82 rule
rejection is preserved; read-only quote success alone never closed that gate.

## Reconstruct and replay from a clean clone

The eight entries in `index.json.full_rpc_captures` contain all raw response
bytes needed for reconstruction. The derived `accounts.snapshot.json` is
intentionally omitted to avoid duplication. The materializer verifies raw
hashes, replays timestamped RPC receipts into the account map, verifies decoded
account hashes and uses actual retained lookup tables. All eight archived
snapshots reconstructed successfully; see `reconstruction-check-final.json`.

From the repository root, supplying the independently verified current SBF hash:

```sh
python3 simple/scripts/materialize-current-route-candidate.py \
  docs/audit/simple-current-route-evidence-2026-10-28/pump-fresh-native-adaptive \
  simple/target/reconstructed-current-pump \
  --own-program-sha256 VERIFIED_CURRENT_SBF_SHA256
```

Reconstruction binds a manifest to that hash; it does not itself prove program
execution. Runtime reports and complete local logs are retained under `runtime/`.
The final result is `runtime/current-route-suite-8c4a-spread500/summary.json`:
six positive cases passed and both pinned Scorch cases failed with their expected
specific error. SBF SHA256:
`8c4ad9af77afa32ae09ae48f5fdcfce868bd3759c96affac975130c63781a00d`;
native runner SHA256:
`051866471c8c59a11a2e5c9d762b78adda3d6d35759749c1039b7ea7dd05ff69`.
All positive cases used actual retained lookup tables and fit the full wire
packet, sold borrowed tokens and repaid the whole q. Actual native setup costs
on legacy Pump, graduated PumpSwap and fresh metadata Pump openings were
2,199,360 / 1,844,400 / 1,844,400 lamports respectively, paid from the borrower's
explicit allowance. Their same-loan buybacks passed with zero setup allowance
and zero setup consumption. A future different route may still need funding.

The final functional policy uses the production 500-bps spread bound. Other
functional terms and two-epoch synthetic history remain test inputs; they do not
prove real production history accumulation. All four setup budget rejection
cases, post-CPI fee-failure rollback, native/WSOL conservation, whole-q net credit
and metadata/rent checks passed. Initial Pump failures and intermediate program
reports remain dated historical evidence. No passing report is transferred to
changed bytes.

`archive-files.sha256.json` separately covers archived source, discovery and
runtime report files. `index.json` remains the exact raw capture-byte index.
Fresh quote history, funded payer operation, alert delivery, final CI and the
qualifying 24-hour frozen fuzz run remain separate release requirements.
