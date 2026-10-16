export const meta = {
  name: 'logo-concepts',
  description: 'Design six ShortWin logo concepts, render a contact sheet, judge them through three lenses',
  phases: [
    { title: 'Concepts', detail: 'six designers, one direction each' },
    { title: 'Render', detail: 'one contact sheet of every concept at every size' },
    { title: 'Judge', detail: 'three judges, three lenses' },
  ],
}

const SCRATCH = '/tmp/claude-1000/-home-todor-dev-Projects-short-win/042f05f3-c79b-4ca9-be93-bfe6ebc27df2/scratchpad'
const OUT = `${SCRATCH}/logo`
const REPO = '/home/todor_dev/Projects/short.win'

const BRIEF = `You are a senior brand/identity designer working in SVG. Design a logo for **ShortWin** (the site short.win).

PRODUCT: short any Solana token by borrowing it from lenders who earn rent. A serious money product (real risk, liquidations), not a meme or a casino. The UI is a dense dark trading terminal.

CURRENT IDENTITY (to replace): the header shows a text glyph "▼" in red (#ff5c6c) beside the bold wordmark "ShortWin" (15 px, system font). The favicon (${REPO}/apps/web/public/favicon.svg) is a dark rounded tile with a red falling polyline. Both are generic. The header markup is in ${REPO}/apps/web/src/shell/AppShell.tsx (classes .brand, .brand-mark, .brand-word; CSS at ${REPO}/apps/web/src/styles/components.css lines 38-60). Look at ${REPO}/apps/web/e2e/screenshots/01-markets.png (desktop) and ${REPO}/apps/web/e2e/screenshots/phone/markets.png (phone) with the Read tool to see where the logo lives.

DESIGN TOKENS (use these exact colours): app background #0b0e11, header/panel #12161b, raised #181d23, hairline #2a323b, primary text #e8ecf1, secondary text #aab4bf, accent gold #f0b90b (primary action, active nav), loss/short red #ff5c6c, gain green #2ebd85, info blue #5aa2ff.

WHERE IT APPEARS: (1) the app header, 44 px tall on desktop and 48 px on tablet/phone, mark ~20-24 px beside the wordmark; under 400 px wide only the mark shows, so the mark must carry the identity alone; (2) the browser favicon at 16 and 32 px, on light AND dark tab strips; (3) a 180 px apple-touch-icon and a 512 px maskable icon (keep important content inside the central 80 %).

HARD CONSTRAINTS:
- SVG only. No raster. No external fonts (CSP font-src 'self'): a wordmark is either live text with font-family="Inter, system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif" (weight 650-700) or geometric letterforms you draw as paths.
- Presentation attributes only (fill=, stroke=, …): no style="" attributes (CSP style-src 'self' forbids inline styles when it becomes a React component).
- The header mark must work with fill/stroke="currentColor" for its main shape (so it can follow text colour) plus at most one brand colour (red or gold) as an accent; it must also read well fully monochrome.
- Must stay crisp at 16 px and 20 px: align strokes/edges to whole or half pixels of an even viewBox (24×24 for the mark, 32×32 for the favicon), stroke widths ≥ 2 px at 24 px, no hairlines, no gradients that vanish small (a subtle gradient ONLY in the 512 icon, with a flat fallback).
- The wordmark text stays "ShortWin" (owner decision), but you may style it (two-tone "Short"/"Win", custom tracking, a drawn detail on one letter).
- Convey "short" (a fall) and "win" (a check / an upswing / a prize feel), or a strong monogram, without being a generic down arrow. A distinctive silhouette that nobody confuses with another DeFi logo.

DELIVERABLES — write exactly these files to your directory:
- mark.svg — viewBox 0 0 24 24, the header mark. Main shape in currentColor (fill or stroke) + optional one accent colour.
- favicon.svg — viewBox 0 0 32 32, with a rounded tile background (so it reads on a light tab strip) and the mark inside.
- lockup.svg — the horizontal lockup: mark + "ShortWin" wordmark as it would sit in the header, drawn at about 4× (so ~96 px tall), on a transparent background, text in #e8ecf1.
- icon-512.svg — viewBox 0 0 512 512, app-icon version (tile + mark, maskable-safe).
- README.md — the concept in 3-5 sentences, why it says short+win, how it maps to the tokens, implementation notes (which parts are currentColor, exact stroke widths), and anything a judge should notice.

PROCESS (do not skip): after writing the SVGs, render them and LOOK at them. Make an HTML preview that shows: favicon.svg as <img> at 16, 32 and 64 px on a white tile and on a #0b0e11 tile; mark.svg at 20, 24 and 48 px on a #12161b strip beside the text "ShortWin" in bold system-ui 15 px #e8ecf1 (mimic the header); lockup.svg at 400 px wide; icon-512.svg at 128 px. Render it with the system Chromium, e.g.
  chromium --headless=new --disable-gpu --hide-scrollbars --window-size=900,700 --screenshot=<dir>/preview.png file://<dir>/preview.html
(if an <img> of an SVG with currentColor shows black, that is expected for <img>; use inline <svg> with color set on the wrapper instead so currentColor resolves). Then open preview.png with the Read tool and critique it honestly: is the mark recognisable at 16 px? Do the strokes blur? Does it look generic? Iterate at least twice; keep the final preview.png in your directory. Spend real effort: this will be the identity of a live product.

Return (as the structured output) the directory path, the one-line concept, and your own honest 1-10 self-score for legibility-at-16px and distinctiveness.`

const DIRECTIONS = [
  { slug: 'fall-to-check', text: 'A falling price line that resolves into a check mark: the drop IS the win. Work out how the descending stroke becomes the tick\'s short leg and the upward long leg, so one continuous stroke says both. Weight it so the 16 px version is still a clear tick-with-a-drop, not a squiggle.' },
  { slug: 'sw-monogram', text: 'A monogram fusing S and W into one geometric glyph with a downward emphasis (e.g. the W\'s two valleys read as two shorts; the S flows into the W). Make it a solid, logo-grade glyph, not two letters touching. Test it in mono and in two-tone.' },
  { slug: 'w-of-arrows', text: 'A W drawn from two down-pointing strokes or arrows (two shorts win = W). Explore both the stroked version and a solid chevron version; the silhouette must be a W at 16 px yet clearly a pair of downward moves.' },
  { slug: 'bear-candle', text: 'A market motif: a stylised bearish candlestick (red body, wicks) standing on a gold base/line, or a bar chart stepping down ending in a gold winning bar. Bold, geometric, big shapes; avoid thin wicks that die at 16 px (make wicks ≥ 2 px at 24 px or fold them into the body).' },
  { slug: 'cut-tile', text: 'Minimal geometric: a rounded square tile cut by one steep descending diagonal (the short), two-tone — the upper-left in dark, the lower-right wedge in red or gold — with a small gold notch or dot marking the win at the bottom. Think app-icon first; derive a currentColor header mark from the same geometry.' },
  { slug: 'wordmark-first', text: 'Typographic lockup first: "ShortWin" with a deliberate detail — e.g. the W\'s middle vertex pulled down into a red descending marker, or "Win" in gold, or a small red triangle replacing the dot in a "short.win"-style treatment while the text itself still reads ShortWin. Derive a minimal companion mark from that single detail for the favicon and the under-400-px header.' },
]

const CONCEPT_SCHEMA = {
  type: 'object',
  properties: {
    dir: { type: 'string' },
    concept: { type: 'string' },
    files: { type: 'array', items: { type: 'string' } },
    self_legibility_16px: { type: 'number' },
    self_distinctiveness: { type: 'number' },
    notes: { type: 'string' },
  },
  required: ['dir', 'concept', 'files', 'self_legibility_16px', 'self_distinctiveness'],
}

phase('Concepts')
const concepts = (await parallel(DIRECTIONS.map((d) => () =>
  agent(`${BRIEF}\n\nYOUR DIRECTION (slug "${d.slug}"): ${d.text}\n\nYour directory: ${OUT}/${d.slug}/ (create it).`, {
    label: `concept:${d.slug}`, phase: 'Concepts', schema: CONCEPT_SCHEMA,
  })
))).filter(Boolean)
log(`${concepts.length}/${DIRECTIONS.length} concepts delivered`)

phase('Render')
const RENDER_SCHEMA = {
  type: 'object',
  properties: {
    sheet_png: { type: 'string' },
    sheet_html: { type: 'string' },
    per_concept_png: { type: 'array', items: { type: 'string' } },
    problems: { type: 'string' },
  },
  required: ['sheet_png', 'sheet_html', 'per_concept_png'],
}
const sheet = await agent(`Build ONE contact sheet that lets judges compare these logo concepts side by side. Concept directories (each has mark.svg 24×24 using currentColor, favicon.svg 32×32 with its own tile, lockup.svg, icon-512.svg, README.md):\n${concepts.map((c) => `- ${c.dir} — ${c.concept}`).join('\n')}\n\nWrite ${OUT}/contact-sheet.html: one row per concept, with the slug and the one-line concept, then in cells: (a) favicon.svg as <img> at 16, 32 and 64 px on a white tile AND on a #0b0e11 tile; (b) mark.svg INLINED (read the file and paste the <svg> markup, add width/height) at 20 px and 24 px inside a strip styled like the real app header — background #12161b, height 44 px, border-bottom 1px #2a323b, the mark followed by the text "ShortWin" in 700-weight system-ui 15 px #e8ecf1 with 4 px gap — once with the strip's color #e8ecf1 (so currentColor = light text) and once mono (color #aab4bf); (c) mark.svg inlined at 48 px, color #e8ecf1, on #0b0e11; (d) lockup.svg as <img> at 420 px wide on #0b0e11; (e) icon-512.svg as <img> at 128 px on #0b0e11 and at 64 px on white. Make the page 1400 px wide, dark page background #0b0e11 with light labels, generous spacing, so the whole sheet is readable in a single screenshot. If a concept's files are missing or broken, show a red "missing" cell and say so in problems.\n\nRender with the system Chromium: chromium --headless=new --disable-gpu --hide-scrollbars --window-size=1400,<height> --screenshot=${OUT}/contact-sheet.png file://${OUT}/contact-sheet.html (compute the height so nothing is cut; verify by opening the PNG with the Read tool — if the bottom is cut, increase the height and re-render). Also render each concept's row alone to <dir>/row.png at 1400 px wide (same cells) so a judge can zoom into one concept. Return the paths.`, {
  label: 'render:contact-sheet', phase: 'Render', schema: RENDER_SCHEMA,
})
log(`contact sheet: ${sheet ? sheet.sheet_png : 'FAILED'}`)

phase('Judge')
const JUDGE_SCHEMA = {
  type: 'object',
  properties: {
    scores: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          slug: { type: 'string' },
          distinctiveness: { type: 'number' },
          legibility_16px: { type: 'number' },
          meaning_short_win: { type: 'number' },
          fit_dark_terminal: { type: 'number' },
          implementation_robustness: { type: 'number' },
          total: { type: 'number' },
          critique: { type: 'string' },
          graft: { type: 'string' },
        },
        required: ['slug', 'distinctiveness', 'legibility_16px', 'meaning_short_win', 'fit_dark_terminal', 'implementation_robustness', 'total', 'critique'],
      },
    },
    ranking: { type: 'array', items: { type: 'string' } },
    recommendation: { type: 'string' },
  },
  required: ['scores', 'ranking', 'recommendation'],
}
const LENSES = [
  { key: 'brand', text: 'You are a brand identity director. Judge memorability, originality (would it be confused with another crypto/DeFi logo?), the quality of the form (optical balance, consistent stroke logic, no awkward joins), and whether it could carry the product for years.' },
  { key: 'product-ui', text: 'You are a product designer who owns this dark trading terminal UI. Judge how the mark sits in the 44 px header beside "ShortWin" at 20-24 px, how it looks alone (the phone under 400 px), whether its colour accent competes with the gold primary buttons and the red loss numbers, and whether it feels serious enough for a product where users can lose money.' },
  { key: 'icon', text: 'You are an icon designer. Judge the favicon at 16 and 32 px on white and on dark, stroke clarity, pixel-grid alignment (blurry edges?), the silhouette at 16 px, and the 512 maskable icon composition (safe zone, balance).' },
]
const judges = (await parallel(LENSES.map((l) => () =>
  agent(`${l.text}\n\nSix logo concepts for ShortWin (short.win: short any Solana token; a dense dark trading terminal; tokens: bg #0b0e11, header #12161b, text #e8ecf1, accent gold #f0b90b, short/loss red #ff5c6c, gain green #2ebd85). The required brief: SVG, currentColor-capable header mark 24×24, favicon 32×32 with a tile, lockup with the wordmark "ShortWin", 512 icon; crisp at 16 px; conveys short + win or a strong monogram; not a generic down arrow.\n\nLOOK (Read tool) at the contact sheet ${sheet ? sheet.sheet_png : OUT + '/contact-sheet.png'} and at each concept's own row.png and preview.png in its directory; read each README.md and the SVG sources:\n${concepts.map((c) => `- ${c.dir} (${c.concept})`).join('\n')}\n\nScore every concept 1-10 on: distinctiveness, legibility_16px, meaning_short_win, fit_dark_terminal, implementation_robustness (currentColor, presentation attributes only, no fonts, pixel alignment), and give total (sum). Be harsh and specific in the critique (name the actual flaw you see in the render). In graft, say what idea from this concept should be carried into the winner even if this concept loses. Rank all six and give a recommendation: which one to ship, and the 2-4 concrete refinements it needs.`, {
    label: `judge:${l.key}`, phase: 'Judge', schema: JUDGE_SCHEMA,
  })
))).filter(Boolean)

return { concepts, sheet, judges }