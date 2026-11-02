# short.win — social workflow films

Editable standalone Remotion project: `/home/todor_dev/Projects/short.win/tools/social-video`.

The films use real app screenshots captured against the local historical fork. Screenshots are assembled from separate successful local workflow takes; balances and position IDs can differ between scenes. They do not represent public-network transactions, live prices, or a production launch. All transactions shown use test funds. The demonstrated history service uses synthetic price history. Historical accounts were captured at multiple slots and are not a synchronized mainnet snapshot.

## Deliverables

| File | Format | Duration |
| --- | --- | --- |
| `01-lend.mp4` | 1080 × 1920, 30 fps, H.264 + AAC | 24 seconds |
| `02-short.mp4` | 1080 × 1920, 30 fps, H.264 + AAC | 28 seconds |
| `03-manage-close.mp4` | 1080 × 1920, 30 fps, H.264 + AAC | 28 seconds |
| `04-overview.mp4` | 1920 × 1080, 30 fps, H.264 + AAC | 45 seconds |

Matching `*-poster.jpg` files are still-image covers. `captions.txt` contains suggested copy for all four films. `shortwin-editable-project.zip` contains portable source, real captures, fonts, original audio, and dependency lockfile; run `npm install` after extracting it. Exports are saved to `/home/todor_dev/Videos/shortwin-2026-11-02` by default.

## Preview and export

```sh
cd /home/todor_dev/Projects/short.win/tools/social-video
npm install
npm run dev -- --port=3001
```

Open the exact Studio URL printed by the command. Each film is a separate composition with individually authored scenes, timing, text, and screenshot references.

```sh
node scripts/export.mjs --previews
node scripts/export.mjs
node scripts/verify.mjs
```

The export script blocks if a required capture is missing. The verification command checks dimensions, frame counts, H.264/AAC streams, full video/audio decoding and duration, poster sizes, and editable archive integrity with the original W branding, audio, and captions; it writes `verification.json`. Set `SHORTWIN_VIDEO_OUTPUT` to change the output directory. For an individual film:

```sh
npx remotion render Lend /home/todor_dev/Videos/shortwin-2026-11-02/01-lend.mp4 --codec=h264 --crf=18 --pixel-format=yuv420p --audio-codec=aac --audio-bitrate=192k
```

## Capture manifest

The phone captures in `public/captures/` use a consistent 375 × 812 captured image from the logical 390 × 844 viewport. They preserve the real app UI, balances, spending limits, confirmations, and transaction results.

- `workspace.jpg`
- `lend-form.jpg`
- `lend-review.jpg`
- `lend-success.jpg`
- `short-form.jpg`
- `short-review.jpg`
- `short-success.jpg`
- `position.jpg`
- `margin.jpg`
- `close-review.jpg`
- `close-success.jpg`

Optional `desktop-workspace.jpg` uses a 1440 × 900 viewport. A missing asset has an explicit capture-pending placeholder in Studio; the placeholder is never an acceptable final export.

## Suggested social captions

**Lend**

Supply tokens, review your deposit, and receive pool shares. Lenders share in SOL fees actually paid by borrowers; withdrawals depend on available liquidity. A walkthrough of short.win on a local historical fork with test funds and synthetic price history. Experimental software.

**Short**

Set your token quantity and spending limits, then review the fixed upfront fee and SOL collateral. This local-fork walkthrough demonstrates borrowing and selling through the genuine Jupiter program, using test funds. Synthetic price history. No public-network trade is shown. Experimental software.

**Manage & close**

See your position, add SOL collateral, and review a full buyback. Closing requires repayment of every borrowed token; early closing does not refund the term fee. Local historical fork, test funds, and synthetic price history. Experimental software.

**Overview**

Choose your side. Know your terms. A short.win walkthrough: lend tokens, open a short through Jupiter, manage collateral, and close with full repayment. Captured on a local historical fork with test funds and synthetic price history. Experimental software; no mainnet availability or financial return is promised.

## Audio and type

The instrumental bed is an original procedural composition made for these films. It uses synthesized oscillators and deterministic noise, with no samples, archived narration, or third-party recordings. Source is `scripts/synthesize-audio.mjs`; the distributable score is `public/audio/original-score.m4a`. To regenerate: run `node scripts/synthesize-audio.mjs`, then encode the generated WAV with FFmpeg using `-af volume=8dB -c:a aac -b:a 192k`.

Noto Sans and Noto Serif font files are bundled in `public/fonts/`; their Apache 2.0 license is included alongside them.

## Editing notes

The original W mark uses white `#e8ecf1` and orange `#f0b90b`. Brand surfaces use `#0b0e11`, `#12161b`, and `#181d23`, with muted text `#aab4bf`. Text and key content remain inside social-safe margins. All motion is deterministic and driven by the Remotion frame clock. The persistent local-fork/test-funds label and disclosure end cards should remain when cutting or republishing.
