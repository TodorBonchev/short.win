# Local-validator growth demo

The growth plan calls for a 2–3 minute,
one-take local-validator demo and a 10-second GIF. The current program is **v1.0.3**, so the
burned-in caption says **“local validator, v1.0.3, not a deployment.”** The plan's v1.0.1 caption
was written for an earlier release. The recording is not a devnet or mainnet deployment and is
not an external audit report.

The revised 1:45 shareable cut shows, in order: create a market; deposit tokens; open a Pump curve short;
close it; scripted CR liquidation; and S1 SOL settlement. The first four actions use a dumped
Pump.fun curve on the local validator. The two scripted exits use **separate, pre-seeded,
TEST-ONLY mock markets** on that same validator; they are two other positions, not alternate
endings of the curve position. All actions are real local-validator transactions. The raw WebM
is one continuous Playwright capture. The revised recording adds a gold cursor halo, smooth
mouse travel and click rings driven by the actual browser pointer events, including inside
transaction dialogs. A separate guide strip below the full app viewport shows the active step,
the action being taken and the local-validator/version label.

Only the recorded market warm-up, scripted liquidation wait and keeper settlement wait run
at **8×** in the MP4, each with a visible speed label. Mouse interactions, transaction reviews,
and confirmed results remain at normal speed. The exact wait boundaries and action cues are
recorded in the manifest rather than inferred from fixed offsets. The GIF takes 10 seconds
from the curve-open sequence.
The renderer aligns the manifest's clock to the visible closing title card to account for
Playwright's video startup delay, and records that calibration in `render.json`.

The scripted liquidation raises the mock price in 4% steps and holds it when the reference
needs to catch up, preserving the buyback ceiling and S1 refusal. Exit-result navigation waits
until the real indexer reports the same transaction signature. These waits can vary between
seeds; the output timing follows the captured manifest.

From the repository root, build the program and test venue after source changes:

```sh
anchor build --tools-version v1.57 --arch v3
scripts/build-test-venue.sh
pnpm --filter @shortwin/math-wasm build
```

Then, from `apps/web`, use the local-validator bootstrap documented in
[`docs/DEVELOPMENT.md`](../../../../docs/DEVELOPMENT.md#run-the-app-against-a-local-validator):

```sh
SHORTWIN_E2E_PROFILE=full SHORTWIN_INDEXER_BIN=../../services/target/release/shortwin-indexer \
  node e2e/validator/start.ts --app --env
node e2e/demo/record.ts
node e2e/demo/render.ts
node e2e/validator/stop.ts
```

`--app` builds and serves the production-style E2E app at `http://127.0.0.1:14173`, which is
what the recorder captures. `--env` also writes the Vite environment, so the plan's manual
`pnpm dev:e2e` command works at `http://127.0.0.1:5173` against the same validator. The `full`
profile runs the real local indexer, which makes the liquidation and S1 rows visible in the app.
Build the indexer with `cargo build --manifest-path services/Cargo.toml -p shortwin-indexer
--release` if its binary is missing. The bootstrap resets the local validator, and the recorder
changes its state; reseed before running E2E specs that expect the original positions.
When recording another take, stop the stack and archive `.e2e/pg` before the fresh bootstrap:
the indexer's database must describe the new validator, without events from an older take.
For example, after `node e2e/validator/stop.ts`:

```sh
node --input-type=module -e 'import {existsSync,renameSync} from "node:fs"; if(existsSync(".e2e/pg")) renameSync(".e2e/pg", ".e2e/pg-archive-" + Date.now())'
```

Outputs are gitignored under `apps/web/.e2e/demo/`: `raw.webm`,
`shortwin-local-validator-v1.0.3.mp4`, `shortwin-local-validator-v1.0.3.gif`, and
`recording.json` (timeline, cues, exact waits, transaction signatures, browser errors), and
`render.json` (the mapped output timeline). The shareable cut is
1440×1040 H.264 at 30 fps, including a 140-pixel guide strip; the GIF is 960 pixels wide
at 10 fps. The scripts verify the on-chain market,
curve position, PumpMigrating close, Liquidate event, S1 event, and the real indexer's exit rows.
The [capture record](../../../../docs/demo/local-validator-v1.0.3-2026-10-20.md) lists the
transaction signatures and media hashes for this take.

Suggested unlisted-video description (add the URL after upload):

> short.win local-validator walkthrough — v1.0.3 candidate, not deployed; no external audit
> report. Create market → deposit → Pump curve short → close → scripted liquidation → S1 SOL
> settlement. The two scripted exits use separate TEST-ONLY mock markets. This is one continuous
> capture with a visible cursor and click highlights; only validator warm-up and scripted keeper
> waits are shown at 8× speed. All clicks, transaction reviews and result screens play at normal
> speed. Reproduce using
> [the local-validator commands in docs/DEVELOPMENT.md](https://github.com/TodorBonchev/short.win/blob/audit-1.0.3-gates/docs/DEVELOPMENT.md#run-the-app-against-a-local-validator).

Publishing the MP4 as an unlisted video and pinning its URL is still pending. No upload is
performed by these scripts.
