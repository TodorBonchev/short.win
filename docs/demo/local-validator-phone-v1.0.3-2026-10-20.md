# Phone local-validator demo — 2026-10-20

This separate portrait edition records the responsive app in Chromium phone emulation:
390×780 CSS pixels, DPR 2, touch enabled and a Pixel 7 Android user agent. It uses the
injected E2E wallet against the real local validator and indexer. It is not a recording
of a physical phone, native keyboard or external wallet handoff.

The mobile flow opens Lend and Short from the market action bar, opens Close from the
position action bar, acknowledges the disclosures and signs from the transaction review
sheets. Gold rings come from actual browser touch events. The Liquidations tab is tapped,
and its mobile cards show the confirmed liquidation and S1 lender payout.

The first four actions create a Pump.fun curve market, deposit tokens, open a curve short
and close it through the synthetic migration path. Liquidation and S1 use separate,
pre-seeded TEST-ONLY mock markets on the same validator. Persistent captions identify the
local validator and v1.0.3; the scripted exits also disclose the mock markets.

The 1080×1920 MP4 includes the full app viewport, chapter progress, action captions and
visible 8× labels on the three recorded waiting intervals. Taps, reviews and results play
at normal speed. The app image is enlarged from Playwright's 390×780 recording to
780×1560 within the portrait frame; it is not native 1080p app detail. The companion GIF
is 540×960, 10 seconds at 10 fps.

The existing desktop MP4 and GIF remain unchanged. Outputs are gitignored under
`apps/web/.e2e/demo/phone/`, with shareable copies in the user's Downloads directory.
The [demo README](../../apps/web/e2e/demo/README.md#phone-edition) documents reproduction
using `record.ts --phone` and `render.ts --phone` on a fresh validator/indexer seed.

## Program identity

The program binaries are unchanged from the desktop capture. SHA-256:

- Test-venue `shortwin.so`: `994ad12c468f465937af21080ec3c48678bb1f197ab143ab44f91f98ca56132b`.
- Mock CPMM: `4cf1700761782aece1a4e3463ce0b2b9f2afaed2ecf3d0a6362f115faf9c7ab9`.

## Capture and transaction evidence

The continuous capture began at **2026-10-20T07:57:00.040Z**. All six signatures were read back
from the local RPC as **finalized, with no transaction errors**. The real indexer's exit
records match the liquidation and S1 signatures. S1 records **0.885920872 SOL** for lenders.

| Step | Local transaction signature |
| --- | --- |
| create | `4BfSFDdpwMtT3xmHVfJnjnpv6zZkUTtrHyEEXKF9zW5XtWeXoyGQnQNNxLFUezyZ8SCjDagSo9Zhxc28AWUmV7LW` |
| deposit | `4Ky7r8keJwCHGhQvZUeSWCrmb4HLK2myy3GqYj3xBMERMVmS1vkZ4Tgd4hE9GDysSy6JEZf7fY5VC3Uhtk2tUo6Y` |
| open | `5uoYKcCokQxPU3ngh4YEfn4hSWRgFiTQac7eoZb3vCWndtQJbfCVCcvUqzT4hyd8yS7uP7E291q1WuPAj4yfvXBj` |
| close | `24SGdsUyd8ktfZhqUQsCKNxpxtTd46zxQmTSN47axGmxygrNk7xi3AnLyT2Y6cnnWuDdD8NvpDnpWFvsBTNei5Es` |
| liquidation | `2ZpuKP9N1d6PnEskSy2moSn3sffTvntXuab4sbFPi7ntxX4xP6pcARqX1Nf6bd1KMuRVyJoaQrYXESHVZf4aeX85` |
| s1 | `5tiWiJCLCoE97SSkKSw3X8ujSXNQSCLfwn4Pg13Z4c8FHSA4YNTmKRgu8W9kP54Z7ut9xqZLwjUQzk2EQ26At9RQ` |

## Media and timing

| File | Duration | Dimensions | Bytes | SHA-256 |
| --- | --- | --- | ---: | --- |
| `raw.webm` | 240.56 s | 390×780 | 7,410,909 | `0911798158a128b33e44cf8f81567b8fe42eae6e484e035b8f6b756b7ad57a39` |
| `shortwin-local-validator-v1.0.3-phone.mp4` | 115.07 s | 1080×1920 | 12,316,635 | `7145dc2831c61b45bffda29a4da86fd679949c7145cdfff3a97828c29416855d` |
| `shortwin-local-validator-v1.0.3-phone.gif` | 10.00 s | 540×960 | 4,044,265 | `bd10f6dcf20234c48bf0b6e0e46d3896ac97edd0bed644f65f7d1781d0fc5998` |

| Wait | Recorded duration | Playback duration | MP4 interval | Speed |
| --- | ---: | ---: | --- | --- |
| warmup | 32.303 s | 4.038 s | 34.935–38.973 s | 8× |
| liquidation | 47.806 s | 5.976 s | 78.751–84.727 s | 8× |
| settlement | 63.360 s | 7.920 s | 93.845–101.765 s | 8× |

The final MP4 is **115.07 seconds** at 30 fps. Timing calibration measures a
**1.825667-second** screencast startup offset from the final title-card
transition, aligning captions, chapter boundaries and accelerated waits. The GIF contains
100 frames from the curve-open chapter. `recording.json`, `render.json` and
`verification.json` retain the capture timeline, mapped output timeline and RPC/indexer
readback respectively.

## Validation and handoff

- Mobile rehearsals passed for the lending, short and close sheets, including lending
  and close simulations. Setup refreshed mock price observations; no lending, opening
  or closing trades were signed during rehearsal.
- The full six-action phone capture passed on its first take with `failure: null` and
  `pageErrors: []`. The curve position closed, and liquidation/S1 history assertions passed.
- ffprobe verified both output dimensions and durations; the MP4 fully decodes without
  errors and the GIF contains exactly 100 frames.
- Visual review checked opening/closing cards, mobile forms, transaction reviews, touch
  pulses, caption fit, 8× badges and the confirmed exit cards. A separate agent's review
  found no material visual issues.
- Web lint, TypeScript typecheck and `git diff --check` pass. No product behavior or
  program binary changed. Desktop rendering logic and existing media hashes are preserved.

The exit-history refresh issue already listed in [TODO.md](../../TODO.md) remains pending.
This recording waits for ingestion before tapping Liquidations, and reloads the indexed
close result. It does not claim to fix that app issue. Uploading/publishing remains pending;
these files have only been saved locally.
