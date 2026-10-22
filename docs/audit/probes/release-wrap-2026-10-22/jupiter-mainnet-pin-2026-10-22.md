# Jupiter mainnet administrative pin, 2026-10-22

This closes the 1.0.3 **release-configuration** drift observed in the [October 21
live preflight](../fuzz-final-1.0.3-2026-10-22/preflight/fixture-live.log).
That historical command exited 1 because Jupiter's deploy slot had moved from
`454465850` to `459479624`; its output has not been rewritten. The subsequent
[fresh pre-update check](#checks) reproduced Jupiter as the only drifted target.
It is a scoped administrative disposition, not a review of Jupiter's private
binary or an approval to enable a Jupiter trade path.

## Mainnet observations

Read-only Solana JSON-RPC `getMultipleAccounts` and `getAccountInfo` requests to
`https://api.mainnet-beta.solana.com`, with finalized commitment, produced these
values on 2026-10-22 UTC:

| Item | Observed value |
|---|---|
| Program | `JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4`, executable, owned by `BPFLoaderUpgradeab1e11111111111111111111111`, 36-byte Program account, variant 2 |
| Program pointer | `4Ec7ZxZS6Sbdg5UGSLHbAnM7GQHp2eFd4KYWRexAipQT`, matching the source's Jupiter ProgramData address |
| ProgramData | Loader-owned, nonexecutable, variant 3, **2,892,269** bytes; finalized header context slot **460079837** |
| Deploy slot | **459479624**, `getBlockTime` = **2026-10-21 03:30:22 UTC** |
| Upgrade authority | `CvQZZ23qYDWF2RUpxYJ8y9K4skmuvYEEjH7fK58jtipQ` |
| Allocated ELF | 2,892,224 bytes; SHA-256 `0394bfa4311fac40dd3d32e431ab2f541227e6c32923715455cfac58e4004d30` |
| ELF without trailing zero padding | 2,036,377 bytes; SHA-256 `5e31d320f5d88ae93c7983ce82a1497067abd86ff540b35073abaeeb25ef397b`; full ProgramData read context slot **460079838** |
| Anchor IDL account | `C88XWfp26heEmDkmfSzeXP7Fd7GQJ2j9dDTUsyiZbUTa`, owned by Jupiter, 6,314 bytes, SHA-256 `c468169462ca9330e499b597ce8d540308dcb66616002aaefa168551e3870558`; context slot **460079981** |
| Decoded on-chain IDL | 33,312 bytes, SHA-256 `1c70d6c96e01648411ff1579a35853d966407a4dff5020a5d33e92e6923c9e67`, `metadata.name = jupiter`, `metadata.spec = 0.1.0` |

The on-chain IDL still exposes `route_v2`, `shared_accounts_route_v2`,
`exact_out_route_v2` and `shared_accounts_exact_out_route_v2`. Their four 8-byte
discriminators, fixed account names and argument names match those recorded in
[the Jupiter research](../../../research/jupiter-cpi.md#11-program-and-idl-mainnet-facts).
This is an **IDL interface comparison**; an unchanged IDL does not prove that
private executable behavior is unchanged. The [official Jupiter instruction
parser](https://github.com/jup-ag/instruction-parser) identifies the same v6
program ID, while the account data and context slots above come from Solana's
[documented account RPC](https://solana.com/docs/rpc/http/getaccountinfo).

## Scope and disposition

The 1.0.3 `VenueKind` has Pump and Raydium variants but no Jupiter variant or
Jupiter trade adapter. Its market guard masks exclude Jupiter. The keeper can
still monitor the sixth `VenueGuard` index, and `shortwin-gov init-config`
compares **all six** explicitly configured deploy slots with live ProgramData,
even when Jupiter is absent from the `venues` list. Thus a stale Jupiter slot
can block initialization without altering current trade execution.

The checker now uses the unchanged, dated `VENUE_GUARD_SLOTS_2026_10_21` program
test baseline for the other five venues and a Jupiter-only
`JUPITER_LIVE_PIN_2026_10_22` in `scripts/dump-fixtures.py`. That pin requires the
exact ProgramData address, slot, allocation size, authority, trimmed ELF
SHA-256 and on-chain IDL account hash. `services/ops/governance.example.toml` now shows all six mainnet slots
observed in the successful live check, including the new Jupiter slot. It
remains an **example**, not a production governance layout or a devnet layout;
operators must read the live cluster before any future initialization.

Neither `programs/shortwin/src/constants.rs` nor the deployed program source
or ELF was edited for this change. The retained canonical 1.0.3 ELF outside Git
still hashes to `d6f4e092dd15639c502535d68bcdb6c428c423b66d49d08f4f89e9409af538bc`,
equal to the verified devnet ProgramData in the [deployment
receipt](deployment-packet/README.md). Jupiter's 2.9 MB allocation and ELF hash
are recorded for a future v1.1 compatibility and compute review; this work
does not enable the Jupiter bit for 1.0.3 markets.

## Checks

- Pre-update, `python3 scripts/dump-fixtures.py check --live` at mainnet context
  slot **460080005** exited **1** with Jupiter as the only drift. The original
  October 21 failed aggregate check remains under `probes/fuzz-final-1.0.3-2026-10-22/preflight/`.
- After the pin change, [the retained live-check output](jupiter-live-check-after-pin-2026-10-22.log)
  at mainnet context slot **460081104** exited **0**: every pinned program and
  account matched, and Jupiter's full ELF and IDL matched the new administrative pin.
  Log SHA-256: `2878f556bc63ea176621575c662d9509b2811e0eb659953437729de57d2f3967`.
- `python3 -m py_compile scripts/dump-fixtures.py` and
  `python3 scripts/dump-fixtures.py check` passed; the offline check covered
  all **344** fixture entries. `git diff --check` passed.

The live result is a point-in-time observation. A later Jupiter upgrade, IDL
change or ProgramData extension must fail the pin until reviewed and repinned.
