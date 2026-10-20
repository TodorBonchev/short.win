# Local-validator growth demo

The full recordings also have separate [56-second promotional edits](../../../../tools/promo/README.md)
with Remotion sources, fresh narration, timed captions and landscape/phone exports.

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

## Phone edition

The separate portrait edition records the responsive app in **Chromium phone emulation**:
390×780 CSS pixels, touch input, DPR 2 and an Android user agent. It uses the injected local
test wallet; it does not demonstrate a physical handset, a native keyboard or an external
wallet handoff. Lending, borrowing and closing use the app's real mobile bottom sheets.
Gold pulses mark actual taps, including acknowledgements and transaction signing.

Start a **fresh full-profile validator and indexer database** using the commands above, then
run these commands from `apps/web`:

```sh
node e2e/demo/record.ts --phone
node e2e/demo/render.ts --phone
node e2e/validator/stop.ts
```

Phone artifacts are isolated under `.e2e/demo/phone/`; the desktop artifacts remain alongside
that directory. The MP4 is `shortwin-local-validator-v1.0.3-phone.mp4`, **1080×1920 at 30 fps**,
with the complete phone viewport, chapter progress, readable action captions and labelled
**8×** waits. The matching GIF is **540×960, 10 seconds at 10 fps**. Playwright records the
390×780 CSS viewport and the renderer enlarges it; this is not native 1080p app detail.

The [phone capture record](../../../../docs/demo/local-validator-phone-v1.0.3-2026-10-20.md)
contains its own transaction evidence, timing and media hashes. Replaying either edition
changes validator state, so reseed and archive the indexer database between recordings.

## Narrated editions

`narrate.ts` adds English narration using ElevenLabs' premade **Bella** voice and
`eleven_multilingual_v2`. Its thirteen short lines explain the actions, risks, 8× waits
and separate mock markets. Each edition places the lines using its own rendered cue
timeline. Speech stays at normal speed; an overlong line fails validation instead of
being accelerated or cut.

With the rendered MP4s already present, run from `apps/web`:

```sh
node e2e/demo/narrate.ts --all --dry-run
node e2e/demo/narrate.ts --all
```

The script reads `ELEVENLABS_API_KEY` from the environment, or a key stored in `~/.11l`.
It sends the narration text to the
[ElevenLabs speech-with-timing API](https://elevenlabs.io/docs/api-reference/text-to-speech/convert-with-timestamps).
The credential is never included in command arguments, logs or output metadata. New
speech generation uses the account's credits. Shared audio and API responses are cached
under `.e2e/demo/narration-cache/`, so both editions reuse the same thirteen voice clips.
`--all --offline` rebuilds from that cache without reading credentials or making API calls.
Omit `--all` for desktop only, or use `--phone` for phone only.

Outputs beside each silent MP4 have the suffix `-narrated.mp4`, plus a WAV voice track,
an SRT transcript and `narration.json` with exact speech timings and hashes. The MP4
contains AAC audio; the narration subtitles are supplied separately as an optional SRT.
Existing action captions remain visible. Two-pass loudness normalization targets −16 LUFS with
−1.5 dB true-peak headroom. The original video is copied without re-encoding, and its
compressed-stream hash must match before the narrated copy is published locally.

The [narration record](../../../../docs/demo/narration-v1.0.3-2026-10-20.md) lists the final
files, checks and speech timing. Silent MP4s and GIFs remain available.

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
