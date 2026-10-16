# ShortWin mark — "fall-to-check"

## Concept

One continuous stroke tells the whole trade. A price line enters flat from the left edge (the token is holding its level), breaks and falls steeply off a cliff — the **short** — and that falling stroke *is* the short leg of a check mark: it bottoms out at a sharp spike and the long leg climbs away to the top right as the tick — the **win**. The fall is red (the loss/short colour, continuity with the old red ▼) and the tick is the text colour, so the stroke changes meaning exactly at the vertex, where the two pieces meet on a mitred cut like a picture frame. Read fully monochrome it stays a check with a flat shelf and a steep first leg — a silhouette no stock ✓ icon has, and nothing like a generic down arrow.

## Why it says short + win

- **Short**: the flat shoulder → steep fall (68.7° from horizontal, steeper than any standard check's 45–55° short leg) reads as a chart line falling off a cliff, and it is cut flush with the left edge like a sparkline cropped by its frame.
- **Win**: the long leg (49°) is the dominant gesture, ending higher than the line started — the resolution. The vertex is a sharp mitre spike: the fall ends in a point, and the tick springs out of that same point.

## Token mapping

| Element | Colour |
|---|---|
| Rise (the tick) in the header mark | `currentColor` — follows `--text-0` #e8ecf1 |
| Fall (shoulder + drop) | `--loss` #ff5c6c (the one accent; gold stays reserved for actions so the logo never competes with "Connect wallet" or the active nav) |
| Favicon / app-icon tile | `--bg-1` #12161b, mark in #e8ecf1 + #ff5c6c |
| icon-512 only | tile gradient #181d23 → #0b0e11 over a flat #12161b fallback rect |
| Wordmark | #e8ecf1, "ShortWin" in the UI font stack, weight 650, letter-spacing −0.01em |

## Geometry and implementation notes

- All shapes are **exact filled outlines** (no stroke attributes), computed from a polyline with mitre joins and butt caps, so every renderer draws the same spike and the colour split lands exactly on the vertex bisector. No `style=`, no gradients outside icon-512, no external fonts.
- **mark.svg** (viewBox 24×24): centreline `(0,10.5) → (6,10.5) → (9.5,19.5) → (22.5,4.5)`, stroke width **3** (2 px at 16 px, 2.5 px at 20 px, 3 px at 24 px). Shoulder edges at y=9 and y=12 → whole pixels at 16 px (6/8) and 24 px (9/12). Two paths: the full outline in `fill="currentColor"` underneath, the fall overlaid in `#ff5c6c`. Delete the second path for the monochrome mark. Lowest point is the mitre spike at y=22.36.
- **favicon.svg** (32×32): tile `rx=7` in #12161b; centreline `(0,12) → (8.5,12) → (13,24) → (28,6)`, stroke width **4** (2 px at 16 px, 4 px at 32 px), shoulder edges y=10/14 → 5/7 px at 16 px. The shoulder bleeds off the tile's flat left edge on purpose.
- **lockup.svg** (420×96): the 24-grid mark at 4× (stroke 12) + live text at 60 px with 20 px between the tick's tip and the "S". 
- **icon-512.svg**: the 32-grid mark at 12.5× (stroke 50); the farthest tip/spike corner is 177 px from centre, inside the 204.8 px maskable safe circle. Rasterise the same file for the 180 px apple-touch-icon and the 512 maskable icon; only the left bleed is ever cropped, which is the intended "cropped chart" reading.
- **Header**: render the mark at **24 px** on both desktop and phone (crisp edges; at 20 px the shoulder straddles half-pixels and goes slightly soft). Keep the `<svg>` inline as a React component so `currentColor` follows `.brand`; `.brand-mark { color: var(--loss) }` is no longer needed — give the fall path a class and set `fill: var(--loss)` in CSS if you want it token-driven instead of the hex. Suggested gap to the wordmark: 6–8 px (the tick's tip ends at the box edge).

## What to notice

- The stroke is cut flush at x=0: the line continues off-frame like a chart, not a floating glyph.
- The vertex is a true mitre: a sharp spike down, and the red/light boundary is the 62° corner's bisector — the two parts are "joined" like frame moulding, not simply painted in two colours.
- The pixel grid was designed in: the only horizontal edges (the shoulder) sit on whole pixels at 16, 24 and 32 px; diagonals anti-alias cleanly because the strokes are ≥ 2 px everywhere.
- preview.png shows the favicon on white, #0b0e11 and mock light/dark Chrome tab strips, the header at 20/24/48 px with the real neighbouring elements (cluster badge, gold nav, gold button), the phone header where the mark stands alone, the monochrome mark on dark and on white, the lockup, and the 512 icon at 128/64/32.
