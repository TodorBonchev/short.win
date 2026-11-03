# Fresh Jupiter route inputs, 2026-11-03

These are new, non-atomic mainnet account and Jupiter Swap V2 build reads for
the eight public route recipes selected in
`tests/runtime/current-routes-2026-11-03.json`. The historical fixtures under
`tests/fixtures/current-routes/` remain unchanged. Five positive recipes and
both Scorch recipes reuse their public mint, quantity, program identity and
DEX selection. The metadata-only Token-2022 case uses a newly discovered
active mature mint as explained below.

The free public endpoints were `https://api.jup.ag/swap/v2/build` and
`https://api.mainnet-beta.solana.com`. `JUPITER_API_KEY` was absent; the API
accepted these requests without authentication. No credential file, wallet,
signer, simulation or transaction-submission endpoint was used. Addresses
derived by the collector are public PDAs for the production program identity
and the runtime's fixed public borrower address.

Each selected capture retains the exact raw HTTP/RPC response bodies, their
SHA-256 receipts, the decoded account manifest, public binding, quote analysis
and qualification manifest. Large JSON files are gzip-compressed without
changing their decompressed bytes. The route materializer verifies their
original response hashes after decompression. `SHA256SUMS` covers the retained
files, including their compressed storage bytes.

`captures/capture-summary.json` records the original orchestration attempt.
Its metadata case stopped at the collector's legacy plain-82-byte mint gate;
the attempted reads and failure remain in
`captures/pump-metadata-current-8GV/`. The 8GV retries were collected with the
historical recipe's explicit `--capture-ineligible` flag. That flag bypasses only the
collector's legacy gate; actual metadata-only mint admission is decided by
the runtime replay. The orchestration wrapper now carries the same flag
automatically for that historical recipe. `capture-selection.json` records
the final selection and rejected attempts. The initial quote-only selection
and initial dated matrix are preserved as `capture-selection-initial-quotes.json`
and `initial-route-matrix.json`.

Five positive captures excluded Scorch. The metadata-only positive explicitly
selected `Pump.fun Amm` in both directions, so it also contains no Scorch route.
Both diagnostic captures explicitly selected Scorch. All eight selected captures obtained an ordinary
Jupiter route with a whole-quantity buyback quote within the configured 500bps
spread bound. Each selected account snapshot contains Jupiter deployment slot
462802085. Their current account context slots are later and are retained
individually. These observations establish exact input availability at capture
time. They do not establish actual CPI execution, a synchronized mainnet fork,
continuous availability, history provenance, duration qualification or live
transaction success. Offline actual-SBF replay results are maintained
separately by the reviewed-route suite.

Subsequent inventory checks found that the current 8GV curve holds the entire
mint supply and only 15 lamports of real SOL. Its `retry2` recapture remains
diagnostic evidence; a virtual curve quote does not establish actual lender
inventory or sell liquidity. The bounded search recorded in
`discovery-active-metadata/` checked three recent Token-2022 mints. Its native
SOL candidate, 8mC1, had real SOL and circulating tokens during discovery but
had returned to 9 lamports and zero circulating tokens in the route capture.
That attempted capture also remains diagnostic evidence.

`discovery-mature-metadata/` records three additional mature Pump AMM candidates
selected from the retained top-traded response. C5x and nxPq use non-WSOL quote
assets. Ai66 uses WSOL and passed full metadata-only mint layout checks, exact
mint/market/vault ownership and identity checks, and actual inventory checks
in one bank before capture, within the exact route capture, and in a separate
bank afterward. Its fresh route capture is
`captures/pump-metadata-active-Ai66/`, with quantity 132636970 raw tokens, sized
for roughly 0.05 SOL. That first route used Meteora DLMM to sell and Pump.fun
AMM to buy. Offline replay observed a successful Meteora CPI followed by
Jupiter's 6001 slippage refusal and safe rollback; this attempt is retained
as diagnostic evidence.

The selected sixth positive is
`captures/pump-metadata-active-Ai66-pumpamm/`, with the same mint and quantity,
explicit `dexes=Pump.fun Amm` for both directions, and unchanged 50bps quote
slippage and 500bps spread bounds. Actual-SBF probes on both financial ELFs
passed its full lifecycle and their old-pin refusal controls before the dated
matrix was updated. Its same-bank capture contains approximately 10891 SOL
of actual WSOL reserves and 934861250918008 raw tokens circulating outside
the pool. Separate before/after inventory reads remain in
`discovery-mature-metadata/`; the following full matrix replay is recorded
separately. The official public Pump AMM IDL used to decode the pool layout
is retained with its source URL and hash.

To start another fresh capture in a new directory:

```sh
python3 scripts/capture-current-route-matrix.py \
  --matrix tests/runtime/current-routes-2026-11-03.json \
  --output tests/fixtures/NEW-DATED-CAPTURE
```

The original collector remains `scripts/qualify-current-routes.py`. Its hash
is recorded per qualification manifest. The wrapper never overwrites an
existing case, including when `--resume` preserves a partially completed run.
