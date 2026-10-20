# Narrated validator demos — 2026-10-20

Both polished videos now have English narration generated with ElevenLabs' premade
**Bella — Professional, Bright, Warm** voice (`hpp4J3VqNfWAUOO0d1Us`) and
`eleven_multilingual_v2`. Speech uses normal speed, stability 0.6 and similarity 0.75.
Thirteen shared clips use **980 text characters** across thirteen synthesis requests;
they are generated once and placed on each edition's own rendered action timeline.

The narration explains the market, token deposit, curve short, close, scripted liquidation
and S1 settlement. It identifies the local validator, version 1.0.3, accelerated waits and
separate TEST-ONLY mock markets. It makes no deployment, audit or profit claim. “S one”
and “sol” are spelled for speech; the existing visual labels remain unchanged.

## Files and checks

The silent MP4s and GIFs are preserved. The narrated copies contain the original H.264
video stream and a new mono AAC voice track at 48 kHz. Each output's compressed video
stream SHA-256 matches its source, and both complete audiovisual files decode without
errors. There is no video re-encoding, retiming or speech acceleration.

| Edition | Duration | Dimensions | Bytes | SHA-256 |
| --- | --- | --- | ---: | --- |
| desktop | 105.40 s | 1440×1040 | 10,887,405 | `f683731875743a8dbd9488315ec4c35000e74380ee873d7e62db7a1b097a4bfe` |
| phone | 115.07 s | 1080×1920 | 13,798,768 | `70cbf1f04439a26b261c95852d32258ecb684faf6e6b7e9954c5a3a3099264f1` |

Shareable copies:

- `/home/todor_dev/Downloads/ShortWin-local-validator-v1.0.3-narrated.mp4`
- `/home/todor_dev/Downloads/ShortWin-local-validator-v1.0.3-phone-narrated.mp4`

| Edition | Integrated loudness | True peak | Speech clips | Speech duration |
| --- | ---: | ---: | ---: | ---: |
| desktop | -16.21 LUFS | -1.32 dBTP | 13 | 60.929 s |
| phone | -16.18 LUFS | -1.47 dBTP | 13 | 60.929 s |

Two-pass normalization targets −16 LUFS with −1.5 dB true-peak headroom. Measurements
above were made from the final AAC streams. Every clip fits its assigned window, with
natural gaps and no overlap. Liquidation and S1 result statements begin after the matching
visual result cue. The minimum remaining window margin is over 0.69 seconds.

An independent ElevenLabs Scribe v2 transcription of the concatenated generated clips
confirmed all spoken content, including the version, sixty-million-token deposit,
one-point-five-million-token short, mock-market disclosure and S1 allocation. Differences
were punctuation, number formatting and domain-name formatting. This is an automated
intelligibility check, not a claim of a human listening review.

Optional narration subtitles are saved as separate SRT files beside the repository's
narrated outputs. They follow speech alignment, use at most two lines of 42 characters,
and avoid brief one-word leftovers. The MP4s carry no additional subtitle track, so the
existing action captions remain the only text burned into the video.

## Speech timing

| Clip | Desktop start–end | Phone start–end | Spoken text |
| --- | --- | --- | --- |
| intro | 0.200–5.912 s | 0.200–5.912 s | Short dot win. Version one point zero point three. Local validator; not a deployment. |
| create | 7.328–12.482 s | 7.009–12.164 s | Create a market for this Pump dot fun token. Review the simulation, then sign. |
| deposit | 15.021–20.036 s | 14.376–19.392 s | The lender supplies sixty million tokens, making inventory available for borrowers. |
| lender-risk | 22.195–26.746 s | 24.845–29.396 s | Review the shares and confirm the disclosure: repayment may be in sol. |
| waits | 33.218–35.261 s | 35.035–37.079 s | Waits play at eight times speed. |
| open | 37.717–43.987 s | 39.273–45.543 s | Borrow one point five million tokens and sell them on the curve. Sale proceeds stay locked as collateral. |
| borrower-risk | 46.426–49.723 s | 51.165–54.462 s | Review the collateral and liquidation risk before signing. |
| close | 57.357–63.209 s | 61.836–67.688 s | After a price move, close the short. The preview shows the buyback cost and what returns to the borrower. |
| liquidation | 70.841–77.018 s | 76.702–82.879 s | Now, a separate, test-only mock market. A scripted price rise triggers a keeper liquidation. |
| liquidation-result | 79.737–82.755 s | 86.769–89.788 s | The history records the liquidation and keeper payment. |
| s1 | 84.768–92.663 s | 91.785–99.680 s | In another test market, a larger price gap prevents token buyback. S one settlement allocates the remaining sol to lenders. |
| s1-result | 95.307–97.907 s | 104.844–107.445 s | The record shows S one and the lenders’ sol. |
| outro | 100.312–103.655 s | 109.758–113.102 s | Six confirmed actions, all on the local validator. |

## Reproduction and handoff

See the [narration commands](../../apps/web/e2e/demo/README.md#narrated-editions).
`narrate.ts --all` synthesizes missing clips and builds both outputs; `--all --offline`
rebuilds from the shared local cache with no API calls or credential reads. A dry run
prints the narration and available windows without generating speech. Successful API
responses are cached before local audio processing, so an encoding failure does not
require another charged synthesis. Overlong speech fails before further synthesis.

The key is read from `ELEVENLABS_API_KEY` or `~/.11l` only for a new API request. It is
not stored in the repository, audio metadata, command arguments or logs. Generated
media, alignment data, verification results and request caches remain gitignored under
`apps/web/.e2e/demo/`. `narration.json` in each edition records its hashes and exact timing.

Web lint, TypeScript typecheck, offline rebuilds, full media decoding, stream-hash
comparison, loudness measurement and subtitle timing checks pass. The separate review
agent checked script/key handling and narration timing. No product or validator state
changed. Uploading the finished videos remains pending in [TODO.md](../../TODO.md).
