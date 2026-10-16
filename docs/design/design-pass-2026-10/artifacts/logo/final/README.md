# ShortWin logo — final

The W of "Win" drawn as a price chart: a full-height fall and a small bounce in the text colour (the short),
joined through a low, sharp apex to a gold check (the win). Left half `currentColor`, right half `#f0b90b`.
This is the "wordmark-first" concept with the judges' refinements applied; the header mark's geometry is
unchanged, the favicon is rebuilt on the 16 px pixel grid, the app icons are split into `any` / `maskable`,
and the lockups now mirror the app's 8 px mark-to-wordmark gap.

Rebuild everything: `python3 work/build.py && python3 work/preview.py` (chromium headless + ImageMagick).

## Files

| file | what |
| --- | --- |
| `mark.svg` | header mark, viewBox 0 0 24 24, `currentColor` + `#f0b90b` (the standalone copy of the inline React mark) |
| `mark-shelf.svg` | the "price was flat, then fell" variant for comparison — **not recommended**, see below |
| `favicon.svg` | 32-grid favicon on the rx 7 `#12161b` tile, rebuilt on whole pixels for 16 px |
| `favicon-shelf.svg` | shelf variant of the favicon, comparison only |
| `favicon-32.png`, `favicon-16.png` | reference rasters of `favicon.svg` (transparent corners) |
| `icon-512-any.svg` | app icon, rounded tile rx 114, radial gradient `#181d23 → #0b0e11` over a flat `#12161b` rect |
| `icon-512-maskable.svg` | the same W on a full-bleed square tile; every corner of the W is ≤ 185.3 px from the centre (safe circle 204.8) |
| `icon-512.png`, `icon-192.png` | `icon-512-maskable.svg` rasterised (full bleed, purpose `maskable` and `any`) |
| `apple-touch-icon.png` | 180 × 180 from the full-bleed square (iOS rounds it itself) |
| `lockup.svg` | mark at 4× + "ShortWin" as Inter 700 outlines, single-colour wordmark `#e8ecf1` |
| `lockup-two-tone.svg` | "Short" `#e8ecf1`, "Win" `#f0b90b` |
| `lockup-on-light.svg` | text `#0b0e11`, the check and "Win" in `#b58900` (3.21:1 on white; `#f0b90b` is only 1.8:1) |
| `preview.html` → `preview.png` | the review sheet (1400 px): header mocks, favicon strips with 8× pixel zooms, shelf comparison, icons, lockups |
| `work/` | `geometry.py` (the stroker), `build.py`, `preview.py`, `favicon-keyline.svg` (the rejected keyline test), `render/` (every test cell and its zoom) |

All SVGs use presentation attributes only (`fill`, `rx`, `transform`, `stroke` nowhere except the test keyline),
no `style=""`, no `<text>`, no gradients outside the two 512 icons. Colours: `#0b0e11 #12161b #181d23 #2a323b
#e8ecf1 #aab4bf #f0b90b`, plus `#b58900` in the optional on-light lockup only.

## Geometry

Every W is the same construction: a centreline polyline **(xt, y0) → (xv, y1) → (cx, ya) → mirror**, stroked
with a constant perpendicular thickness *t*, mitre joins, cut flat on the cap line *y0* and clipped at the
baseline *y1* (which gives the valleys their flat Inter-style bottoms). The apex centre sits at 37.5 % of the
height; the stem leans 3.755 : 16 and the diagonal runs 5.5 : 16, so the diagonals are 61° from horizontal.
The `currentColor` half is drawn 0.4 units past the seam **under** the gold half (the two `12.4` / `16.4` /
`261.45` points), so a fully monochrome render shows no anti-aliasing hairline at the seam (checked at 24 px,
see the preview's monochrome cells).

### mark.svg — 24 grid (unchanged from the concept)

| | |
| --- | --- |
| centreline | (2.745, 4) → (6.5, 20) → (12, 10) → (17.5, 20) → (21.255, 4) |
| stroke | 3.4 perpendicular; cap cut 3.49 wide, valley flats 3.69 wide |
| extent | x 1 – 23, cap y = 4, baseline y = 20 (whole pixels at 24 px) |
| stem / diagonal | 13.2° from vertical / 61.2° from horizontal |
| apex | tip (12, 6.47), inner notch (12, 13.53); counter bottoms (7.14, 15.3) and (16.86, 15.3) |
| path 1 (fall, currentColor) | `M1 4L4.75 20L8.44 20L12 13.53L12.4 14.25L12.4 7.2L12 6.47L7.14 15.3L4.49 4Z` |
| path 2 (check, gold) | `M12 13.53L15.56 20L19.25 20L23 4L19.51 4L16.86 15.3L12 6.47Z` |

Pixel check: at **24 px** the cap cuts, valley flats, cap and baseline all land on whole pixels; only the
diagonals anti-alias. At **28 px** (phone header) the cap is at 4.67 px and the baseline at 23.33 px, so the
top and bottom rows of each terminal are 33 % grey — visible in the 5× zoom, invisible in the 48 px bar at 1×.
If that ever matters, **30 px** puts both on whole pixels (5 / 25) with a 4.25 px stroke; 28 px is acceptable.

### favicon.svg — 32 grid, rebuilt for the 16 px pixel grid

| | |
| --- | --- |
| tile | `rect 32 × 32 rx 7 #12161b`, no keyline |
| centreline | (4.06, 6) → (8.90, 26) → (16, 13.5) → (23.10, 26) → (27.94, 6) |
| stroke | **4.0** = 2 px at 16 px, 4 px at 32 px; cap cut 4.12 (2 → 6.12 and 25.88 → 30) |
| extent | x 2 – 30, cap y = 6, baseline y = 26 — all even, i.e. whole pixels at 16 px (rows 3 and 13) |
| apex | tip (16, 9.45), notch (16, 17.55); counter bottoms (9.66, 20.62) / (22.34, 20.62); valley flats 6.85 – 11.2 |
| path 1 | `M2 6L6.85 26L11.2 26L16 17.55L16.4 18.26L16.4 10.15L16 9.45L9.66 20.62L6.12 6Z` fill `#e8ecf1` |
| path 2 | `M16 17.55L20.8 26L25.15 26L30 6L25.88 6L22.34 20.62L16 9.45Z` fill `#f0b90b` |

At 16 px (see the 8× zooms on white, `#dee1e6`, `#202124`, `#35363a`, `#0b0e11`): each stem's top row is two
solid pixels (measured coverage 88 % / 100 %, with an 18 % spill into the third), the left counter is open for five rows below the apex
(3.9 → 0.7 px wide), the apex tip lands at row 4.7 (a faint point in row 4, solid from row 5), and the valley bottoms sit
on row 12. Chrome's dark strips `#202124` / `#35363a` leave the `#12161b` tile barely distinguishable, which
is the intended "mark floating in the tab" reading; the W itself carries the contrast.

**Keyline: no.** A 1-unit `#2a323b` ring (`work/favicon-keyline.svg`) does give the tile an edge on the dark
strips at 32 px, but at 16 px it is a half-pixel ring that renders as a lighter, smudged rim around the whole
tile (preview, "keyline test" cells, left pair vs right pair). The SVG serves both sizes, so the keyline is out.

### mark-shelf.svg / favicon-shelf.svg — the "price was flat, then fell" variant

Same stroker with a flat entry at cap height: the shelf's top edge runs from the frame to the stem's inner
edge, its bottom edge to the stem's outer edge (a true mitre), so it is `t` thick everywhere.

- `mark-shelf.svg`: W at x 3 – 23 (20 wide instead of 22), shelf flush from x = 0: top edge 0 → 6.47, bottom
  edge 0 → 3.71, 3.4 thick. Apex tip (13, 6.14).
- `favicon-shelf.svg`: W at x 6 – 30, shelf from x = 2: top edge 2 → 10.08, bottom edge 2 → 6.81, 4 thick.

**Recommendation: ship without the shelf** (`mark.svg`, `favicon.svg`). Reasons, from the comparison row of the
preview (16 / 24 / 28 / 48, plus the 16 px and 24 px zooms):

1. At 16 px the shelf is a 2 × 2–4 px nub on one stem only. It does not read as a chart line; it reads as a
   serif that the other stem lacks, i.e. as a rendering error. The narrower W it forces (24 instead of 28 units)
   closes the left counter one row earlier (open for four rows, 3.6 → 0.9 px, instead of five).
2. At 24 px it still reads as a W, but as a W with a tab: the flat-then-fall story only becomes legible at
   48 px, a size the mark is never shown at in the app. Meanwhile the asymmetric top-left corner pulls the mark
   leftward beside the wordmark; the plain W sits balanced on its two cap terminals.
3. The "short" is already told by the left stem: the price enters at the cap line (its high) and falls the full
   height. The shelf restates it at the cost of the letter — and the whole concept is that the mark *is* the W
   of "Win".

Both variants are built and in the preview; the lead decides.

### icon-512-any.svg / icon-512-maskable.svg

The 24-grid mark at 13.625× (218 / 16), centred: W 299.75 × 218 at x 106.11 – 405.89, y 147 – 365, stroke
46.32. Farthest corner of the W from the centre: 185.3 px, inside the 204.8 px maskable safe circle (shown as
the dashed ring in the preview). Flat `#12161b` rect under the radial gradient in both; the `any` tile has
rx 114, the maskable tile is a full-bleed square. The PNGs were rasterised with chromium at exact window
sizes (`--window-size=N,N --screenshot`) and verified with Python (`IHDR`): 180 × 180, 192 × 192, 512 × 512,
32 × 32, 16 × 16.

### Lockups — viewBox 0 0 405 96

Mark at 4× (W 64 tall, x 4 – 92, centred on the cap height), text box at x = 128: the app's 24 px mark box +
8 px CSS gap, times four. Ink to ink that is 38.5 px at 4× = 9.6 px at 1× (the S's side bearing adds 0.6 px),
exactly what the header shows. "ShortWin" is Inter 700 outlined at 60 px (cap height 43.65, baseline 69.83,
HarfBuzz-kerned in the concept round), so the lockup renders identically on every OS. The right margin is 4,
mirroring the W's own 1-unit margin at 4×.

## Implementation

The app already carries this shape (`src/shell/Logo.tsx`, `.brand-mark-win` in `components.css`); the
path data below is identical to it. The gold path has **no `fill` attribute** in JSX — CSS sets it from the
token, so the mark follows a future accent change and the monochrome fallback is one rule away.

```tsx
export function BrandMark({ size = 24 }: { size?: number }) {
    return (
        <svg className="brand-mark" width={size} height={size} viewBox="0 0 24 24" aria-hidden="true" focusable="false">
            <path fill="currentColor" d="M1 4L4.75 20L8.44 20L12 13.53L12.4 14.25L12.4 7.2L12 6.47L7.14 15.3L4.49 4Z" />
            <path className="brand-mark-win" d="M12 13.53L15.56 20L19.25 20L23 4L19.51 4L16.86 15.3L12 6.47Z" />
        </svg>
    );
}
```

```css
.brand { display: flex; align-items: center; gap: 8px; color: var(--text-0); }
.brand-mark { flex: none; display: block; }
.brand-mark-win { fill: var(--accent); }
```

Header: `<a class="brand"><BrandMark /><span class="brand-word">ShortWin</span></a>` at 24 px in the 44 px bar;
under 400 px the existing rule makes it 28 px and hides the word. Monochrome contexts (print, a disabled
state) only need `.brand-mark-win { fill: currentColor }`.

Static files for `public/`: `favicon.svg` (replaces the old red-arrow placeholder — `<link rel="icon"
type="image/svg+xml" href="/favicon.svg">` already points at it), `favicon-32.png` as a fallback
`<link rel="icon" sizes="32x32">` if ever wanted, `apple-touch-icon.png` as
`<link rel="apple-touch-icon" href="/apple-touch-icon.png">`, and in the web manifest
`{ "src": "/icon-192.png", "sizes": "192x192", "type": "image/png", "purpose": "any maskable" }` and the 512
twin (both PNGs are the full-bleed maskable rendering, which is also fine as `any`; use `icon-512-any.svg`
where a rounded SVG icon is wanted, e.g. a `rel="icon"` for large sizes).
