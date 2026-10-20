# Promotional edits — 2026-10-21

The desktop and phone walkthroughs now have separate 56-second promotional
edits. Their opening uses the growth plan's “Rent the coin. Sell the coin. Buy
it back.”, followed by selected app actions, exit evidence and a repository
call to action. The original silent and narrated walkthroughs are preserved.

Deliverables are under `/home/todor_dev/Downloads/`:

- `ShortWin-promo-landscape-v1.0.3.mp4` — 1920×1080.
- `ShortWin-promo-phone-v1.0.3.mp4` — 1080×1920.
- Matching `-poster.png` images.
- `ShortWin-promo-editable-v1.0.3.zip` — source, prepared assets, speech cache,
  lockfile and instructions; no installed dependencies or credentials.

The editable [Remotion project](../../tools/promo/README.md) is isolated from
the web application. Studio runs locally at port 3410. Both editions use
30 fps, H.264, faststart and stereo AAC. The final MP4 measurements and hashes
are recorded in [promo-v1.0.3-export.json](promo-v1.0.3-export.json).

The edit removes waits and repeated title cards, retains the real pointer/tap
highlights, enlarges selected controls, and adds timed narration captions,
frame-driven motion, a clear brand treatment and a quiet original synth score.
ElevenLabs Bella supplies seven new voice clips at normal speed: 507 requested
characters, with the successful responses cached for offline reuse. The
uncompressed final mix measures −16.05 LUFS and −1.68 dBTP. No external music
samples or generated product screenshots are used.

## Source evidence and scope

The prepared desktop and portrait sources are crops of the previously verified
silent masters. The captured program/validator provenance remains in the
[desktop capture record](local-validator-v1.0.3-2026-10-20.md) and
[phone capture record](local-validator-phone-v1.0.3-2026-10-20.md).

| Input | SHA-256 |
| --- | --- |
| Desktop silent master | `0fa695e2c48c8bc03b40dbe17777b3c96b4a2ba34e7da69e1392b60160e27623` |
| Phone silent master | `7145dc2831c61b45bffda29a4da86fd679949c7145cdfff3a97828c29416855d` |

These are edited excerpts of historical local-validator captures, not a fresh
release test or evidence of mainnet deployment. The landscape and phone
editions use separate captures. Phone footage is Playwright browser emulation;
it does not demonstrate a physical handset or an external wallet handoff.
The two scripted exits use separate test-only mock positions. S1 narration
explicitly says that lender repayment in SOL may be worth less than the tokens
lent. Enlarged amounts reproduce actual recorded rows: 0.8859 SOL to lenders
for the S1 example and 0.0190 SOL keeper payment for the liquidation example.

## Review and validation

Independent creative and technical review agents examined both layouts and
source frames. The resulting fixes include populated opening previews,
visible signing controls, an indexed Closed result instead of the transient
stale Open display, readable S1/keeper records, a correctly rendered vector
logo, and captions separated from the progress line and outro text. The
desktop Closed frame is held at source 69.0s and phone at 73.5s. Final source
intervals avoid old chapter cards and every accelerated wait.

Verification covers TypeScript, source and final-media decoding, frame count,
dimensions, audio/video duration, loudness/true peak, faststart, poster PNGs,
caption bounds and hashes. Studio loaded successfully in Chromium via
Playwright. Review uses rendered representative frames and source-boundary
checks; it is not a claim of human listening or a new protocol audit.

No files were posted to an external service. The existing publication item in
[TODO.md](../../TODO.md) remains pending. No product, validator or release-gate
state changed.
