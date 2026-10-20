# Local-validator growth demo

The growth plan calls for a 2–3 minute,
one-take local-validator demo and a 10-second GIF. The current program is **v1.0.3**, so the
burned-in caption says **“local validator, v1.0.3, not a deployment.”** The plan's v1.0.1 caption
was written for an earlier release. The recording is not a devnet or mainnet deployment and is
not an external audit report.

The 2:44 shareable cut shows, in order: create a market; deposit tokens; open a Pump curve short;
close it; scripted CR liquidation; and S1 SOL settlement. The first four actions use a dumped
Pump.fun curve on the local validator. The two scripted exits use **separate, pre-seeded,
TEST-ONLY mock markets** on that same validator; they are two other positions, not alternate
endings of the curve position. All actions are real local-validator transactions. The raw WebM
is one continuous Playwright capture (3:30); ffmpeg shows only the market warm-up and keeper
settlement waits at 2× speed in the shareable MP4, with on-screen labels. The GIF takes 10 seconds
from the curve-open sequence.

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

Outputs are gitignored under `apps/web/.e2e/demo/`: `raw.webm`,
`shortwin-local-validator-v1.0.3.mp4`, `shortwin-local-validator-v1.0.3.gif`, and
`recording.json` (timeline, transaction signatures, browser errors). The shareable cut is
1440×900 H.264 at 30 fps; the GIF is 960×600 at 10 fps. The scripts verify the on-chain market,
curve position, PumpMigrating close, Liquidate event, S1 event, and the real indexer's exit rows.
The [capture record](../../../../docs/demo/local-validator-v1.0.3-2026-10-20.md) lists the
transaction signatures and media hashes for this take.

Suggested unlisted-video description (add the URL after upload):

> short.win local-validator walkthrough — v1.0.3 candidate, not deployed; no external audit
> report. Create market → deposit → Pump curve short → close → scripted liquidation → S1 SOL
> settlement. The two scripted exits use separate TEST-ONLY mock markets. This is one continuous
> capture; only validator warm-up and keeper waiting are shown at 2× speed. Reproduce using
> [the local-validator commands in docs/DEVELOPMENT.md](https://github.com/TodorBonchev/short.win/blob/audit-1.0.3-gates/docs/DEVELOPMENT.md#run-the-app-against-a-local-validator).

Publishing the MP4 as an unlisted video and pinning its URL is still pending. No upload is
performed by these scripts.
