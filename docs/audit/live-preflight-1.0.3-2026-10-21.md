# Draft 1.0.3 live-venue preflight (2026-10-21)

**New venue review in progress. This record does not approve deployment.**

A read-only `python3 scripts/dump-fixtures.py check --live` at mainnet slot
459,385,700 returned exit 1. The [retained log](probes/live-venue-2026-10-21/pre-update-live.log)
has SHA-256 `80d2ff2170ad061c4bcc82bfcab0035d26b12568f49c26db2a177c90cd5dcb46`.
The live pump and PumpSwap deploy slots differ from the reviewed October 13 baseline;
the source CI checks fixture integrity offline and cannot detect this deployment drift.

| Venue | Prior reviewed deploy | Observed deploy | New trimmed ELF SHA-256 |
|---|---|---|---|
| pump | 456,494,698 | 458,921,440 | `756c04ab1ac76122dcdcf2eeb0073bb139087b23d58d3f0d6c895510ab59d582` |
| PumpSwap | 454,596,406 | 458,921,517 | `4d5fc41918374b24b16d8372d14a1ac64c768c83a2971bfeef1c8660dad649e7` |

The new ProgramData reads authenticate the program addresses, loader ownership,
ProgramData mapping, actual deploy slots and unchanged upgrade authority
`7gZufwwAo17y5kg8FMyJy2phgpvv9RSdzWtdXiWHjFr8`. The trimmed hashes above were computed
from the fetched bytes, not inferred from interface docs or deploy slots.
Read-only [block-time responses](probes/live-venue-2026-10-21/deploy-times.json) date
the actual deploys to **2026-10-19 17:47:02 UTC** (pump) and **17:47:20 UTC** (PumpSwap).
The new fixture-group date denotes its October 21 local observation, not the deploy date.
The official [pump public documentation](https://github.com/pump-fun/pump-public-docs)
was at commit `639b87fb45c3adeffa6f7c4a1bf0cb85e392bf22` (2026-10-13) when read;
that documentation alone does not establish compatibility of the newer deployed bytes.

Pump-fees remains at deploy 456,132,171 with full trimmed SHA-256
`8ad2ca93c7ae97a7fb0de8a99de4d8825ccfdf89f73138cd49bda948e124b8b8`.
The retained R-PFEE withdraw interface/authority analysis therefore applies to the same
pump-fees byte identity. Changed caller ELFs still require renewed signed trade,
synthetic/migration and T-PFEE-CPI checks. Raydium v4/CPMM, Token-2022, Squads,
Memo and Squads config matched their fixtures; Jupiter matched its slot-only pin.

Before promoting the new baseline, preserve the old fixture groups, add independently
hashed new program/header fixtures, execute the supported SOL-quote paths on them,
review the changed interface/quote effects and retain the existing exact-slot latch
and refusal behavior. Pump and PumpSwap are at the loader's 10 MiB allocation and are
not among the four extensible venues with stored complete-digest extension checks;
those full-SHA checks are not an assertion that these new upstream bytes were reviewed. No new quote-asset support is authorized by this
refresh. A new fuzzer binary or program change requires fresh final-byte fuzz evidence.

## Current configuration layout check

A separate finalized `getMultipleAccounts` read at **slot 459,388,308** captured pump
Global (1,088 bytes), PumpSwap GlobalConfig (949) and both FeeConfigs (4,097 each).
All four are accepted by the unchanged production Rust decoders. The
[retained evidence bundle](probes/live-venue-2026-10-21/current-state/SHA256SUMS)
contains raw RPC data, account bytes, decoded metadata and the probe source/log.
Raw response SHA-256:
`4a1bb53a3b7f12db94d7c4dc107a3e0409d31436a27bef30eabca32eabe6f7db`.

PumpSwap GlobalConfig and both fee schedules are byte-identical to the latest retained
October 13 state. Pump Global differs only at byte 1,087: `max_curve_depth` increased
from 1 to 2. Supported short.win curve markets still require SOL quote and depth 0;
PumpSwap still requires WSOL and the ordinary SPL Token quote vault. The readonly WSOL
mint path therefore needs signed execution on the new ELFs; the newly visible upstream
Token-2022 quote-fee error string alone establishes neither compatibility nor a break.
This check covers four global/configuration accounts, not every current coin or pool,
and does not guarantee that future venue configuration will remain unchanged.
