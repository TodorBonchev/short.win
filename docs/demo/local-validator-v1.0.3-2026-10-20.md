# Local-validator demo capture — 2026-10-20

The growth-plan demo was recorded against the `full` E2E local-validator profile and its real
indexer. The program source at capture was `778f54c12f58ce098e419ca33b63cc64a1d781d7`;
the test-venue `shortwin.so` SHA-256 was
`994ad12c468f465937af21080ec3c48678bb1f197ab143ab44f91f98ca56132b` and the mock
CPMM SHA-256 was `4cf1700761782aece1a4e3463ce0b2b9f2afaed2ecf3d0a6362f115faf9c7ab9`.
The burned caption uses the current program version, **v1.0.3**, instead of the plan's older
v1.0.1 text. This is a local test-venue run, not a deployment or external audit report.

The first four steps use the dumped Pump.fun curve on the validator: market creation, lender
deposit, curve short, then one-transaction close through the synthetic migration path. The
scripted liquidation and S1 settlement use two **separate, pre-seeded mock markets**. This is
explicit in the shareable video. All six actions landed and appeared in the real indexer's
event or exit history:

| Step | Local transaction signature |
| --- | --- |
| MarketCreated | `4dhq2n2m8Jm3ZfgKUJJpWcXb4jnKeompFbGVZbyqzsUMYfaoNxS1aXM1xpTmzVPjGDRhW5HiCCzZz1kcQMhQR2vz` |
| Deposit | `2ZL3bHxoMkPBkTz8zLwR16FUma2wEGEKNhW653sdnmWJYkuAVQwDfABBMt6ypLqwU11EQnUcqK3ojqd9PdHNYYa4` |
| Open | `5UQzrUbkPJ9i2QvCfqhZYrirCgCRaAjiEcH6mNjRABAMFennW4LsmZnbBaRTjvp3eLx3noff93fPqkfjbP6AyHuJ` |
| Close | `5WyuY6UexVqcMqRMY4xsKFNJhmPE8YTVXXCCoDQUkU5LwsUQBoDR4FTZ7ZdJkCj6v8CYAeua2EkEt69BEDvUXLFx` |
| Liquidate | `5GwgCCuxHS9v38TTBaixb3aVqkVVxfe9af73nNJkGuYMBwLqe5Zdd2yNamRTkNdqZ8r7qjdANy2U2vosm89xXwRd` |
| SolSettle S1 | `3eD1JSvxYqABztSUjccosFHnLUzWKVXsThVHjmHGeMGVHnKhFjaCc8UWgTCWphuRDvWJZNwaeBvZ4w8wMFKrW1SC` |

Generated media is gitignored under `apps/web/.e2e/demo/`. The continuous Playwright WebM
is 3:30 (`3e4f0ca67e3459e5c466af23f7bc8d1ad6a9917dd5ce1b32ec2bd96cf2bec747` SHA-256).
The shareable H.264 MP4 is **2:43.83**, 1440×900 at 30 fps, 12,470,258 bytes,
SHA-256 `2fd7e9179b90bcc6c0c28ceaed89673391f82bb83148692eacdc255b07436045`.
The 960×600 GIF is **10.00 seconds**, 100 frames, 3,494,285 bytes,
SHA-256 `3b23aafb49d1c8cdc1819cbe550f015dee2ba7c25fe21c2f647a896cf802aa33`.
Only the market warm-up and keeper settlement waits are shown at 2× in the MP4, with labels.

The capture manifest reports `failure: null` and no browser page errors. `ffmpeg` decoded the
final MP4 without errors; the rendered duration, dimensions and GIF frame count were checked
with `ffprobe`. Web lint, production build, TypeScript typecheck and 1,066 web unit tests passed.
The [demo README](../../apps/web/e2e/demo/README.md) has the capture commands and suggested
video description. An unlisted URL, site/pinned-post link and GIF post still need to be made by
the account owner; the media has not been uploaded.
