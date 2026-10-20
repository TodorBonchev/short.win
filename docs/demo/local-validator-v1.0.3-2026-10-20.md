# Local-validator demo capture — 2026-10-20

The revised guided capture began at **2026-10-20T07:36:46.965Z** against the `full` E2E local-validator
profile and its real indexer. The shareable video is **1:45.40** and the companion GIF is
**10 seconds**. A visible cursor, click rings, active chapter strip and action captions explain
the interactions. Only the three explicitly recorded waiting intervals run at **8×**;
clicks, reviews and result holds remain at normal speed.

## Scenario and binary identity

The first four steps use the dumped Pump.fun curve: market creation, lender deposit, curve
short, then one-transaction close through the synthetic migration path. Scripted liquidation
and S1 settlement use **separate, pre-seeded TEST-ONLY mock markets**. This is stated in the
video. The footer identifies **local validator, v1.0.3, not a deployment**; it is not an external
audit report.

The program binaries are unchanged from the initial capture (program source recorded then as
`778f54c12f58ce098e419ca33b63cc64a1d781d7`). SHA-256:

- Test-venue `shortwin.so`: `994ad12c468f465937af21080ec3c48678bb1f197ab143ab44f91f98ca56132b`.
- Mock CPMM: `4cf1700761782aece1a4e3463ce0b2b9f2afaed2ecf3d0a6362f115faf9c7ab9`.

## Confirmed transactions

All six signatures were independently read back from the local RPC as **finalized, with no
transaction errors**. Liquidation and S1 also appeared in the indexer's exit history; S1 reported
a positive lender amount. The continuous browser capture verifies the corresponding UI results.

| Step | Local transaction signature |
| --- | --- |
| MarketCreated | `587LU5R5GqUHzMPXUwauETYvYNLNUwaP2DhMkipskyHqDgfckLKy81yKw4x54GguodSDPNVcJQHm7F4WcdPzv6rT` |
| Deposit | `2yK1exZLbLykQfMYX8edJQJpJNMAMTLb2kvVyeja5RRhqtUVW2qBmgfEfZAS31siqJM7MYZroQvBnFwLEAptUgY5` |
| Open | `2vqdntt2U8rh24Pr21kHJcdJUXurFPXnABuq75QSbXQyiHUbqq7ZHKoM1p5CTFHg5jTX4vtPFRP1BkWD9VWCThRM` |
| Close | `JDdB5yuS2JqGNFjLxzuHa9XS9ooR7zmTKe7yr5uk4qRNbvnkRTgWpPi9VkQkEuMNGusCQX5sezham4YRfuaLutc` |
| Liquidate | `5TTP9JQ5QV1zGFqeE6wF1si4zLP4KGetTUTaBbuT9UqM2hVYWbWS8rWC6m6fELmfb5bQ2kAXe9CG5jVe96YRzJos` |
| SolSettle S1 | `3947nbfmhXionAEYiXuE5ipdwNr5tCfuh7Q6Tmt23ZdzdBdnFddvCF26f7xJn6zkXsFi8ZV5CiVyVM9WZUsKDCx7` |

## Media and timing

Generated files are gitignored under `apps/web/.e2e/demo/`. The MP4 adds a 140-pixel guide strip
below the complete 1440×900 app viewport; it does not cover controls or transaction dialogs.

| File | Duration | Dimensions | Bytes | SHA-256 |
| --- | --- | --- | ---: | --- |
| `raw.webm` | 234.44 s | 1440×900 | 20,763,704 | `c81297455ca428032f2efed28cfc41d2b96dfdc32767137c2c0235e31dad3369` |
| `shortwin-local-validator-v1.0.3.mp4` | 105.40 s | 1440×1040, 30 fps | 9,412,356 | `0fa695e2c48c8bc03b40dbe17777b3c96b4a2ba34e7da69e1392b60160e27623` |
| `shortwin-local-validator-v1.0.3.gif` | 10.00 s | 960×694, 10 fps | 4,819,827 | `f988d375853e39f7eab5a6953029f5cf5fa39e041d03c30b28651e90c5698157` |

| Wait | Recorded duration | Playback duration | MP4 interval | Speed |
| --- | ---: | ---: | --- | --- |
| warmup | 34.396 s | 4.300 s | 33.118–37.417 s | 8× |
| liquidation | 49.797 s | 6.225 s | 72.922–79.147 s | 8× |
| settlement | 63.344 s | 7.918 s | 86.853–94.771 s | 8× |

The opening warm-up is now **4.30 seconds** in the MP4. `recording.json` preserves the original
wall-clock marks, action cues, wait intervals, signatures and browser errors. `render.json`
records the output timeline and the measured **1.843333-second** screencast startup offset,
aligned against the visible closing title card. This calibration applies equally to captions,
chapter boundaries and speed changes. The GIF contains 100 frames from the guided curve-open step.

## Validation and handoff

The completed capture has `failure: null` and `pageErrors: []`. All six actions, the closed
position, liquidation row and S1 condition passed their live assertions. The final MP4 decodes
without errors, and ffprobe checks duration and dimensions for both outputs. Web lint and
TypeScript typecheck pass for the revised scripts. Cursor movement, click pulses over native
transaction dialogs, footer placement, speed labels and result screens were visually checked.

The demo-only liquidation driver now holds a safe spot price while the reference catches up,
preserving its buyback ceiling, S1 refusal and bounded retry time. The recorder waits for exit
indexing before opening history and reloads the indexed close result. A rehearsal exposed a
history-refresh race in the app; that remains pending in [TODO.md](../../TODO.md), rather than
being treated as fixed by the recording changes.

The original 2:43.83 MP4 and its original raw/GIF files are retained locally in
`apps/web/.e2e/demo/original/`. Their SHA-256 values were:

- Original raw: `3e4f0ca67e3459e5c466af23f7bc8d1ad6a9917dd5ce1b32ec2bd96cf2bec747`.
- Original MP4: `2fd7e9179b90bcc6c0c28ceaed89673391f82bb83148692eacdc255b07436045`.
- Original GIF: `3b23aafb49d1c8cdc1819cbe550f015dee2ba7c25fe21c2f647a896cf802aa33`.

The [demo README](../../apps/web/e2e/demo/README.md) contains reproduction commands and a
suggested video description. Uploading an unlisted video, adding its URL and posting the GIF
remain pending; the media has not been uploaded.
