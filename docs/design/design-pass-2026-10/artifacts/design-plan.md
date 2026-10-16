# short.win design polish — implementation plan

Branch `design-polish` (from `main` at c0e0900). The working tree already carries the logo task's uncommitted work — `apps/web/src/shell/Logo.tsx` (new), `apps/web/src/shell/AppShell.tsx` and the shell section of `apps/web/src/styles/components.css`. Package P2 builds on that commit; if the logo task is still in flight when P2 starts, P2 waits for it.

All paths below are relative to `apps/web/` unless they start with `docs/` or `e2e/`.

## 0. How to read and run this plan

- Nine packages, implemented in order P1 → P9, then one closing docs pass (P10). A later package only consumes tokens, classes, props and icons that an earlier package defined; no two packages edit the same file or the same components.css section. Two verified items pointed the wrong way in the order (the `Stat` `hero` prop was defined in P8 but consumed in P7; P7-3 deletes rules that P8-3 replaces); this plan has already moved them and flags the moves in the items.
- Every item is one instruction: the reviewers' amendments are folded into the change text. The "Watch out" line keeps the reasons that still matter after the fold (cascade traps, gate hazards, dependencies). Where two reviewers amended an item differently, the plan picks one and says so.
- Before each package, grep `src/styles/tokens.css` for every token the package consumes and `src/shell/icons.tsx` for every icon name it renders. An undefined custom property makes the declaration invalid at computed-value time (`background: var(--undefined)` → transparent, `border-color` → `currentColor`, a `grid-template-columns` → one column, a `calc()` → the property's initial value); an unknown `NavIconName` or component prop is a TypeScript error. Where an item writes `var(--token, <fallback>)` the fallback is today's value, so a slipped package degrades to the current look, never to nothing.
- After each package: `npx vitest run src/styles src/components` (breakpoints.test.ts, stacking.test.ts, the jsdom suites) plus the unit tests the package names; the responsive gate (`SHORTWIN_RESPONSIVE_GATE=1`, `e2e/specs/responsive.spec.ts`) at 320, 360, 390, 768, 844×390, 1024, 1280 and 1440 px; `screens.spec.ts`; fresh screenshots of every file the package's Verification names, compared against the committed ones. Screenshots that change by design are listed per package.
- Values that recur (tokens.css today): `--sp-1` 2 px, `--sp-2` 4 px, `--sp-3` 8 px, `--sp-4` 12 px, `--sp-5` 16 px, `--sp-7` 32 px; `--row-h` 28 px (44 under `(pointer: coarse)`); `--control-h` 30 px (44 coarse); `--header-h` 44 px (48 under 1024 px; 52 under coarse after P1); `--statusbar-h` 24 px (28 on phones); `--gutter` 8 px desktop, 12 px under 1024, `clamp(12px, 3.5vw, 16px)` under 640; `--tap-min` 24 / 44; `--tap-sm` 24 / 36; `--fs-xs / sm / md / lg / xl / 2xl` = 12 / 12 / 13 / 15 / 18 / 22 px on a desktop and 12 / 13 / 14 / 16 / 20 / 24 px under 640 px; `--lh-tight` 1.25, `--lh` 1.45. tokens.css has these blocks in this order: `:root` (l.24), `(max-width: 1023.98px)` (l.144), `(max-width: 639.98px)` (l.153), `(pointer: coarse)` (l.168), `(prefers-reduced-motion: reduce)` (l.179). It has no `(max-width: 1179.98px)` and no `(min-width: 1280px)` block today.
- components.css section map (line numbers as of this branch; they drift, the comment banners do not): shell l.6, shell: tablet l.177, shell: phone l.198, layouts l.404, panel l.482, key / value grid l.521, buttons l.628, inputs l.722, tabs l.843, table l.887, badges l.1126, numbers l.1211, disclosure l.1232, modal l.1305, review: the plan's instructions l.1421, toast l.1450, search l.1592, menus l.1729, states l.1776, untrusted text l.1862, chain-read age l.1890, responsive building blocks l.1916 (form foot ~l.2035, action bar ~l.2092, chips ~l.2125, reveal ~l.2156), the More sheet l.2218, wallet l.2245, review: deltas l.2417, glossary l.2519.

## 1. Visual direction (the shared principles)

- The terminal stays dark, dense and data-first; the polish is a system of beats, not new decoration.
- Three type tiers do all the labelling work and nothing else does: a panel title (13 px bold white uppercase) outranks a section heading (12 px bold light-grey uppercase) which outranks a label (12 px medium muted uppercase, one 0.04em tracking), and the page title steps up to 20 px so a hero number, a headline price and a title are never the same size as a stat value.
- Surfaces carry one meaning each: bg-0 is the page and the sunken well for read-only figures (previews, tx steps, lend position), bg-1 is a panel edged with `--line-panel` and a 1 px top highlight, bg-2 is a note (disclosure, banner, menu, creator box), bg-3 is an input or the selected row, and a new `--bg-4` is what a bg-2 surface becomes under the pointer, so every hover, pressed and selected state is a visible step and never a reuse of a neighbouring background.
- Gold means exactly one thing, the primary action or the selected state as a fill or a line; caution moves to amber text and left bars, links keep blue, and informational tags turn grey so nothing static looks clickable.
- Space groups: 12 px between boxes, 16 px under the page head, 28 px rows on the 4 px grid, while the density inside a box (12 px cell padding, 12 px rows) is untouched.
- Every icon is drawn once in icons.tsx as a 16 px, 1.6-stroke SVG (close, chevron, sort, copy, check, external, shield, plus) and font glyphs disappear from controls, so the same gesture looks the same on every surface and OS.
- Packages are implemented in order 1 to 9: later packages only consume tokens, props and icons that earlier packages define, and no two packages edit the same file or the same components.css section.

Refinements the verification round made to the direction (each is also folded into the item it touches):

- The brand mark lives in `src/shell/Logo.tsx` (the logo task's W drawn as a price chart: 24-viewBox, `className="brand-mark"` on the `<svg>`, the fall half in `currentColor`, the win half in the accent through `.brand-mark-win`), and `public/favicon.svg` is its pair. icons.tsx holds only the glyph registry; no brand tokens are needed.
- A hero number always outranks the title: `--fs-hero` (= `--fs-2xl`, 22 px desktop / 24 px phone) is consumed by the market head's EMA price (P5-3) and the position page's CR (P6-2); the page title is 20 px (`--fs-title`).
- Read-only key/value rows do not answer the pointer (nothing static looks clickable): the existing invisible kv hovers are deleted, not strengthened (P4-2, P6-3). Interactive table rows hover with `--bg-hover`; bg-2 surfaces (menus, search options, chips, default buttons) hover with `--bg-4`.
- `.icon` / `.icon-sm` are defined once, in P2-1 (shell section), sized from `--icon` / `--icon-sm`. The `Stat` `hero` prop is defined in P4 (P4-7), beside the CSS that styles it. P7-3 lands as P7's last step, after P8-3.
- FRONTEND.md records, in each package, only what that package built; rows that describe other packages' behaviour are written once, in the closing docs pass (P10), each citing the package that built it.

## 2. Hard constraints (verbatim)

HARD CONSTRAINTS every proposal must respect (tests enforce several; a proposal that breaks one is worthless):
1. Keep the dark trading terminal (owner decision): dense, dark, data-first. Polish it, do not turn it into a marketing site or a light theme.
2. One DOM at every width: every width switch is CSS only; no duplicate markup per breakpoint.
3. Closed breakpoint set (src/styles/breakpoints.test.ts rejects anything else): only @media (max-width: 399.98px | 639.98px | 1023.98px | 1179.98px | 1279.98px), (min-width: 640px | 1024px | 1280px | 1680px), (pointer: coarse), (hover: hover), (prefers-reduced-motion: reduce); (orientation: landscape) only together with coarse in the tablet tier; the only height query is (max-width: 399.98px) and (max-height: 699.98px). No range syntax. Container queries only on .tbl-wrap (tbl) and .modal-body (dialog).
4. CSP: style-src 'self' — no inline style attributes, no <style> tags in TSX; SVG presentation attributes are fine. font-src 'self': no web fonts, system stacks only (--font-ui, --font-num). No remote images; no token logos.
5. WCAG 2.2 AA: text ≥ 4.5:1, borders/focus rings ≥ 3:1; colour never the only cue; no text under 12 px at any width; targets ≥ 24 px, primary controls ≥ 44 px under (pointer: coarse); hover styles only under (hover: hover); transitions off under prefers-reduced-motion; keep :focus-visible rings.
6. Reading order = DOM order: no CSS order, no row-reverse.
7. Tokens first: new values go into tokens.css as variables; feature CSS places only its own pieces.
8. Copy is owner-approved: do not reword disclosures, glossary or page copy (trivial casing excepted); the header wordmark text stays "ShortWin".
9. Never hide information (risk, staleness, numbers); e2e specs select by data-testid — keep every data-testid and the class names the CSS already keys on (adding classes is fine).
10. The responsive gate (e2e/specs/responsive.spec.ts) fails on sideways overflow, an element off screen, an axe violation, a target < 24 px, a primary control < 44 px under coarse, text < 12 px, or a control under another control's tap area — at 320, 360, 390, 768, 844×390, 1024, 1280 and 1440 px.

## 3. Packages

### P1 — Tokens and base: type tiers, surfaces, state colours, structural measures

**Goal.** Define every new token the other eight packages consume (the values below are final and contrast-verified) and the base element rules; record in FRONTEND.md exactly what P1 lands. Nothing else can start before it. Expect these visible changes from P1 alone: 20 px desktop page titles, h3 sections one pixel smaller and bolder, press states, every muted label slightly lighter, every warning amber instead of gold, a 52 px header and 56 px sheet head under a finger, the landscape phone's chrome 128 → 136 px, 12 px gutters from 1,280 px (if the measurement in P1-4 allows it) and, on classic-scrollbar platforms, a reserved scrollbar gutter from 1,280 px.

**Owned files.**
- `src/styles/tokens.css`
- `src/styles/base.css`
- `docs/design/FRONTEND.md` (only the rows P1-6 names)

**Items.**

#### P1-1 — Type tiers, title and hero sizes, prose tokens (high)

**Change.** tokens.css, in `:root`:

```css
/* The three label tiers (FRONTEND.md §7.2): title > section > label. */
--title-fs: var(--fs-md);   --title-fw: var(--fw-bold);   --title-ls: 0.06em; --title-color: var(--text-0);
--section-fs: var(--fs-sm); --section-fw: var(--fw-bold); --section-ls: 0.05em; --section-color: var(--text-1);
--label-fs: var(--fs-xs);   --label-fw: var(--fw-medium); --label-ls: 0.04em; --label-color: var(--text-2);
--fs-title: 20px;           /* the page h1; phones keep 20 px (set it again in the (max-width: 639.98px) block) */
--fs-hero: var(--fs-2xl);   /* 22 px desktop / 24 px phone: the market head's EMA price (P5-3) and the position
                               page's CR hero (P6-2) consume it, so a hero number always outranks the 20 px title */
--fs-prose: 14px;           /* reading pages (How it works, Security); 15px in the (max-width: 639.98px) block */
--lh-prose: 1.55;           /* line-height of that prose */
```

Add `--fs-title: 20px;` and `--fs-prose: 15px;` to the existing `(max-width: 639.98px)` block.

base.css:

```css
h1 { font-size: var(--fs-title); letter-spacing: -0.01em; }
h3 { font-size: var(--section-fs); font-weight: var(--section-fw); letter-spacing: var(--section-ls); color: var(--section-color); text-transform: uppercase; }
```

(h3 is `--fs-md` / 0.04em / `--text-1` today.) Result: title (13 px bold white) > section (12 px bold light grey) > label (12 px medium muted); the page title is 20 px; the hero numbers are 22 px once P5-3 and P6-2 point at `--fs-hero`.

**Files.** `src/styles/tokens.css`, `src/styles/base.css`.

**Why.** One 12 px uppercase micro-label plays nine roles today (panel titles are smaller than the h3 sections inside them; stat labels, column headers, field labels and section titles alternate 0.04/0.05em and text-1/text-2 with no rule), and the desktop scale tops out at 18 px, so 'Markets' sits beside a 15 px bold wordmark; the CR hero and the EMA price are 18 px today, the same as a title would be. Tokens make the tiers a contract every feature stylesheet points at.

**Watch out.** `--title-fs` (the 13 px panel title) and `--fs-title` (the 20 px page title) are mirror names for different things — never swap them. The h1 at 20 px is only right because P5-3 and P6-2 lift the EMA price and the CR to `--fs-hero`; if either package is cut, revert `h1` to `var(--fs-xl)` so a title never outranks the number a trader reads first. h3 lands at exactly the 12 px floor on a desktop (the gate fails only below 12). Every long h1 already wraps (`overflow-wrap: anywhere` on `.mk-head-title h1` and `.position-head h1`).

#### P1-2 — Surfaces, elevation and the press states (high)

**Change.** tokens.css surfaces and elevation (in `:root`, beside the existing `--bg-*` and `--line*` tokens):

```css
--bg-hover: #1f252c;   /* hover on an interactive bg-1 row, and the press colour of tabs, nav and rows. The same value
                          as --bg-3 by design; a selected row, if one is ever introduced, takes --bg-4 */
--bg-4: #242c35;       /* hover / pressed / selected on a bg-2 surface: menus, search options, chips, default buttons.
                          text-0 11.9, text-1 6.7, text-2 5.1, info 5.4, loss 4.7, gain 5.9, accent 7.8 : 1.
                          Never behind an input border: line-strong is 2.88 on it */
--line-panel: #363f4b; /* outer edge of panels, cards, wells, popovers (1.70:1 on bg-1); --line stays for in-panel hairlines */
--surface-well: var(--bg-0);   /* read-only figures: previews, tx steps, lend position */
--surface-note: var(--bg-2);   /* disclosure, banner, menu, creator box */
--surface-nested: var(--bg-3); /* a box inside a note */
--edge-highlight: inset 0 1px 0 rgb(255 255 255 / 3%);
--shadow-header: 0 1px 0 var(--line-panel), 0 6px 16px rgb(0 0 0 / 35%); /* the 1 px line first: the header keeps a hairline edge */
--shadow-up: 0 -12px 20px -12px rgb(0 0 0 / 65%);  /* action bar, sticky form foot */
--shadow-pop: 0 12px 32px rgb(0 0 0 / 55%);         /* menus, popovers, search results */
--shadow-modal: 0 16px 48px rgb(0 0 0 / 60%);
--shadow-sheet: 0 -12px 32px rgb(0 0 0 / 60%);
--shadow-toast: 0 8px 24px rgb(0 0 0 / 50%);
--scrollbar-thumb: var(--line-strong);
```

base.css, the press rules at l.123–130 — split them in two:

```css
.btn:active:not(:disabled):not([data-variant="primary"]):not([data-variant="short"]),
.menu-item:active,
.search-option:active {
    background: var(--bg-4);
}

.tab:active,
.nav a:active,
.nav-more:active,
tr[data-interactive]:active {
    background: var(--bg-hover);
}
```

(`.nav-more:active` is new — the phone's More button has no press feedback today. The filled primary and short variants are excluded on purpose: P3-3 gives them `--accent-active` and `--loss-hover` presses.)

**Files.** `src/styles/tokens.css`, `src/styles/base.css`.

**Why.** Hover, selected and pressed states today reuse the neighbouring background (bg-2 on bg-1 is a 1.07:1 step; bg-3 on bg-2 1.10:1), so rows do not answer the pointer, the keyboard-highlighted search result is invisible and More has no press feedback. Six different shadow literals and a 1.4:1 panel edge make the shell, panels and floating layers read as one flat charcoal sheet.

**Watch out.** All the bg-4 ratios reproduce (text-0 11.91, text-1 6.72, the new text-2 5.08, info 5.41, loss 4.71, gain 5.87, accent 7.84, line-strong 2.88). `.btn:active:not(:disabled):not(…):not(…)` is (0,5,0) and the filled variants' own `:active` rules in P3-3 are written to outrank it. `--bg-hover` equals `--bg-3`, which the dead `.tbl tbody tr[aria-selected="true"]` rule also uses — that is fine only because DataTable never sets `aria-selected` (P4-2 leaves that rule alone). P4's reviewers preferred a lighter row hover; if the 1.18:1 lift of `--bg-hover` proves too faint in the 1440 shot, retune `--bg-hover` here, in one place, no lighter than #262d36 (so `--text-2` keeps 4.5:1 on muted zeros in a hovered row) — do not point rows at `--bg-4`.

#### P1-3 — Colour meaning: muted text, amber caution, grey tags, icon tone, button tints (high)

**Change.** tokens.css:

```css
--text-2: #919cab;   /* muted: 6.96 / 6.53 / 6.09 / 5.55 / 5.08 : 1 on bg-0 / bg-1 / bg-2 / bg-3 / bg-4 (was #8a95a1) */
--warn: #ffa726;     /* caution: amber. 9.35 on bg-1, 7.39 on --warn-bg (which stays #33290a). --risk-watch, --tier-c and
                        --accent keep #f0b90b: gold is the primary action and the selected state, amber is caution */
--tag: #b9c6d6;      /* informational badges (9.03 on --tag-bg), so link blue is reserved for links */
--tag-bg: #1c242e;
--icon-muted: #737e8a; /* resting glossary ⓘ and other icon-only marks: 4.68 / 4.39 / 4.10 / 3.74 on bg-0..bg-3 (≥ 3:1 non-text).
                          SVG icons stroked in currentColor only (the glossary ⓘ is NavIcon "info", an SVG in a button with an
                          aria-label); never a text glyph: 4.39:1 on bg-2 and 4.10:1 on bg-3 fail AA for text, so the sort ↕
                          (.sort-hint), the ▸/⌄ chevrons and any other font glyph keep --text-1 / --text-2 until icons.tsx
                          replaces them */
--meter-track: #3d4753;   /* the utilisation track: 1.92 on bg-1 */
--loss-hover: #ff7281;    /* the Short button's hover / press: text-inverse 7.35 on it */
--accent-active: #d9a60a; /* the primary button's press: text-inverse 8.67 on it */
```

Update the header comment on `--text-2`. Do **not** add `--brand-mark` / `--brand-tile`: the logo task's `Logo.tsx` draws the mark in `currentColor` and `--accent`, so no brand tokens are consumed anywhere.

**Files.** `src/styles/tokens.css`.

**Why.** One gold is brand accent, warning, watch-risk and Tier C at once (the gold 'Coverage paused' sits beside the gold 'Tier C' and under the gold 'Connect wallet'); informational badges borrow link blue so every 'Mock (test)' pill looks clickable; the muted label tone at 5.5:1 reads muddy in a dense terminal. Separate tokens let each meaning be tuned once.

**Watch out.** Every `--warn` consumer today is a caution surface (13 declarations: lend.css:109, short.css:126, markets.css:41/419/768, components.css:164, 342, 361–362, 1141–1142, 1232, 1838) and none puts `--text-inverse` on it (9.96:1 if one did), so the swap touches only caution. `--icon-muted` is below AA for text on bg-2/bg-3: one misapplied `color: var(--icon-muted)` on a font glyph is an axe colour-contrast violation and a gate failure. No test pins a literal (#8a95a1 / #f0b90b appear in no test or spec).

#### P1-4 — Structural measures (medium)

**Change.** tokens.css, in `:root`:

```css
--page-gap: var(--sp-4);        /* 12 px between boxes */
--page-head-gap: var(--sp-5);   /* 16 px under the page head */
--kv-row-h: 28px;               /* key / value rows (not --row-h: kv rows are not controls and must not become 44 px under coarse) */
--side-col: 360px;              /* the trade / Manage / create-market side column */
--page-max: 1680px;
--sheet-head-h: calc(var(--header-h) + var(--sp-2));
--tabbar-indicator-w: 32px;
--field-h: var(--control-h);    /* the .input-wrap box; 46 px under coarse so the inner 44 px input fits its 1 px border */
--select-arrow-w: 28px;
--check: 18px;                  /* checkbox / radio box; 20 px under coarse */
--icon: 16px;
--icon-sm: 14px;
--icon-btn: 32px;               /* square icon buttons (close ✕); 44 px under coarse */
--form-measure: 64ch;
--field-narrow-w: 120px;        /* slippage / tolerance boxes */
```

New block, between the `:root` block and the `(max-width: 1023.98px)` block (tokens.css has no 1179.98 block today; the query is in the closed set):

```css
@media (max-width: 1179.98px) {
    :root {
        --side-col: 320px;
    }
}
```

Into the **existing** `(pointer: coarse)` block (l.168, already after every width block, so it wins): `--header-h: 52px; --field-h: calc(var(--control-h) + 2px); --check: 20px; --icon-btn: 44px;` (44 px controls get 4 px of air in the header; everything keyed on `--header-h` — nav height, scroll-padding, the sheet head — follows).

Landscape phone (after the width blocks; the prelude is the one components.css:2042 and markets.css:95 already use):

```css
@media (min-width: 640px) and (max-width: 1023.98px) and (pointer: coarse) and (orientation: landscape) {
    :root {
        --statusbar-h: 28px; /* the phone strip P2-6 gives this tier */
    }
}
```

Gutter — **land only after measuring**: add, after the `(max-width: 1023.98px)` block, `@media (min-width: 1280px) { :root { --gutter: 12px; } }` (the desktop is never tighter than the tablet; 1024–1279 keeps 8 px because its tables are measured tight). Before committing it, run the responsive gate at 1280 and check that no `.tbl-wrap` gains a sideways scroll — the lender dashboard with its three action buttons, the market's Positions and Liquidations tabs beside the trade panel, the markets list — and record the numbers in the PR; if any does, place the block under `(min-width: 1680px)` instead. Either way, note for P2-4 that `.statusbar`'s inset is `--sp-4` while the header and page use `--gutter`; keying the status bar on `--gutter` is what makes the three insets agree.

**Files.** `src/styles/tokens.css`.

**Why.** Every vertical gap is 8 px so a panel's inside is roomier than the space between panels; kv rows are 26.4 px against 28 px table rows; the side column is typed three times with two values (360/320 vs the create page's 380/340); the header, page and status bar use three insets and the widest screens get the tightest margins; under a finger 44 px controls fill a 48 px header with 2 px of air. Measures as tokens keep every package on one grid.

**Watch out.** `--icon-btn` must be 44 under coarse because `.modal-close` and `.sheet-close` are in the gate's PRIMARY list (`e2e/responsive/metrics.ts` ~l.464). The gate measures a checkbox by its wrapping label, so `--check` 18/20 shrinks no target. The 52 px header grows the 844×390 landscape chrome from 128 to 136 px (inside the tier §7.7 already accepts). After P1, re-run the gate at 1280/1440 and re-record the h-scroll counts FRONTEND.md §7.2 cites (P10 writes them).

#### P1-5 — The document scrollbar gutter (medium)

**Change.** base.css:

```css
/* From 1,280 px the document keeps its scrollbar gutter while a dialog or sheet locks scrolling
   (html:has(dialog[open]), html[data-sheet-open] { overflow: hidden }), so opening the review no longer
   shifts the header, nav and trade panel by the scrollbar width on classic-scrollbar platforms.
   Overlay-scrollbar platforms (macOS default, phones) are unchanged. 1,024–1,279 px keeps today's
   behaviour: its tables are measured to the pixel and screens.spec asserts the markets table fits at 1024 × 720. */
@media (min-width: 1280px) {
    html {
        scrollbar-gutter: stable;
    }
}
```

Never unconditional, never `both-edges`. If the owner wants 1,024–1,279 covered too, the only safe form there is on the lock rule itself — `html:has(dialog[open]), html[data-sheet-open] { overflow: hidden; scrollbar-gutter: stable; }` — which costs nothing at rest but moves the jump to short pages at open time.

**Files.** `src/styles/base.css`.

**Why.** On a page taller than the viewport (the market page at 1440 is 1,291 px) the document scrollbar disappears the moment a dialog opens, so the whole shell jumps 15–17 px right and back, moving Cancel/Sign under a pointer heading for them. The cost — an empty bg-0 strip on short pages with classic scrollbars — is the standard trade.

**Watch out.** Unconditional, it breaks screens.spec's 'the laptop minimum: 1024 px' assertion (Playwright's desktop contexts have classic 15 px scrollbars; 16-markets-1024.png is exactly 1024 × 720 with the headers already wrapped, so 15 px less becomes sideways scroll). Desktop screens.spec PNGs of short pages gain a 15 px strip at the right; they are written, never compared.

#### P1-6 — FRONTEND.md: record what P1 lands (low)

**Change.** Write only P1's own rows. §7.1 token table: `--bg-4` (#242c35 with its pairs), `--bg-hover` (= `--bg-3` by design), `--line-panel` #363f4b, `--tag` / `--tag-bg` (9.03), `--icon-muted` #737e8a (4.68 / 4.39 / 4.10 / 3.74, marked 'SVG icons only'), `--meter-track` #3d4753, `--warn` #ffa726 (9.35 on bg-1, 7.39 on `--warn-bg`), `--text-2` #919cab (6.96 / 6.53 / 6.09 / 5.55 / 5.08), `--loss-hover` / `--accent-active` (text-inverse 7.35 / 8.67), the three surface roles (well / note / nested), the three label tiers, `--fs-title`, `--fs-hero`, `--fs-prose` / `--lh-prose`, the shadow set. §7.3: header 52 px under a coarse pointer, page gap 12 px and 16 px under the page head, kv rows 28 px, `--side-col` 360 / 320, `--icon-btn` 32 / 44 px, the gutter rule as finally placed (1,280 or 1,680), the landscape phone's 28 px strip. §7.6: the scrollbar gutter reserved from 1,280 px. §3.1: the shell diagram's 48 becomes '48 (52 under a coarse pointer)'. Everything that describes another package's behaviour (the §7.4 Button and DataTable rows, §5.3's chip pair, §3.3 / §7.6 for P5-1, §3.1 / §7.7 for the wordmark and the landscape strip) is written by P10 after that package's gate run.

**Files.** `docs/design/FRONTEND.md`.

**Why.** The design contract is what the e2e specs and future rounds are checked against; a row recorded before its package lands describes an app that does not exist and survives as a false contract if that package is amended or dropped.

**Watch out.** The DataTable row, when P10 writes it, must say that rows carry `scroll-margin-top: calc(var(--header-h) + 3 * var(--row-h))` and that focus.spec's markets walks pass — a stuck thead with sort buttons is judged by `e2e/focus-obscured.ts`, not exempt.

**Verification (P1).** `npx vitest run src/styles` passes (the new `(max-width: 1179.98px)`, `(min-width: 1280px)`, `(pointer: coarse)` additions and the landscape block use only allowed features). Desktop 1440 full-page shots of 01-markets, 10-market-overview and 31-position-detail: page titles read 20 px (visibly larger than 'ShortWin'); h3 sections ('MARKET', 'EXITS') are 12 px bold light grey; every muted label is #919cab; the binding cap, 'Coverage paused' and the lagging badge are amber. A 1440 shot with the review open (21-short-review) shows the header's nav and wallet button at the same x as before opening. Phone 390 shots are typographically unchanged except the header (52 px with 4 px air around Connect). Contrast: the computed table above (text-2 ≥ 5.08 on every surface, amber ≥ 7.39, icon-muted ≥ 3.74). Gate at 1280/1440 re-run with the h-scroll counts recorded; screens.spec at 1024 unchanged.

---

### P2 — Shell: brand lockup, header, status strip, tab bar, action bar, menus, wallet sheet, More sheet, icons registry

**Goal.** The first screen's chrome reads as a layered terminal with a real logo: brand > cluster > actions in the header, a status strip that shouts when something is wrong, a tab bar whose active state is unmistakable, floating layers that visibly float. P2 also owns icons.tsx, so every SVG glyph other packages render is registered here first. P2 starts from the logo task's commit (Logo.tsx, AppShell.tsx rendering `<BrandMark />`, `.brand` / `.brand-mark` / `.brand-mark-win` in the shell section) and leaves the brand drawing and favicon.svg to it.

**Owned files.**
- `src/styles/components.css` — sections 'shell' (l.6–176), 'shell: tablet', 'shell: phone' up to the `.dot` rules (i.e. l.6–329), plus 'layouts', 'search', 'menus', 'the More sheet (phone)', 'wallet', and the 'action bar' rules inside 'responsive building blocks'
- `src/shell/AppShell.tsx`, `src/shell/StatusBar.tsx`, `src/shell/MoreSheet.tsx`, `src/shell/SearchBox.tsx`, `src/shell/SlotAge.tsx`, `src/shell/icons.tsx`, `src/shell/NotFound.tsx`
- `src/wallet/WalletButton.tsx`, `src/wallet/WalletChooser.tsx`, `src/wallet/ConnectCta.tsx`
- `src/components/ActionBar.tsx`

Order inside the package: P2-1 (registry and `.icon` sizing) before P2-3 / P2-8 / P2-9; P2-3's status rules are written straight into P2-6's combined block so the block is moved once; P2-3 (strip at bg-2) before P2-5 (the action bar's shadow relies on it); P2-2's nav-margin scoping lands with P2-5 in the phone block.

Tokens consumed (all from P1): `--bg-4`, `--line-panel`, `--line-strong`, `--shadow-header`, `--shadow-pop`, `--shadow-sheet`, `--shadow-up`, `--page-max`, `--page-gap`, `--page-head-gap`, `--side-col` (360 / 320 under 1180), `--tabbar-indicator-w`, `--icon`, `--icon-sm`, `--scrollbar-thumb`, `--warn`, `--warn-bg`, `--danger-bg`.

**Items.**

#### P2-1 — The icon registry and the two size classes (high)

**Change.** icons.tsx: extend `PATHS` (20 px viewBox, `currentColor` stroke 1.6, presentation attributes only, every attribute camelCase in TSX) with

```tsx
close:    <path d="M5 5l10 10M15 5L5 15" />,
chevron:  <path d="M5 7.5l5 5 5-5" />,                     // points down; rotate for up / right
sort:     <path d="M6 8l4-4 4 4M6 12l4 4 4-4" />,
copy:     <><rect x="7" y="7" width="9" height="9" rx="1.5" /><path d="M4 13V5.5A1.5 1.5 0 0 1 5.5 4H13" /></>,
check:    <path d="M4 10.5l4 4 8-8" />,
external: <path d="M8 4h8v8M16 4l-9 9M6 7H4v9h9v-2" />,
shield:   <path d="M10 2.5l6 2.5v5c0 3.5-2.6 6.2-6 7.5-3.4-1.3-6-4-6-7.5V5z" />,
plus:     <path d="M10 4v12M4 10h12" />,
```

and widen `NavIconName` to `'markets' | 'positions' | 'lend' | 'status' | 'more' | 'search' | 'info' | 'close' | 'chevron' | 'sort' | 'copy' | 'check' | 'external' | 'shield' | 'plus'`. `NavIcon` keeps `fill="none"`, `strokeWidth="1.6"`, `strokeLinecap="round"`, `strokeLinejoin="round"`, `aria-hidden`, `focusable="false"`. No `BrandMark` here and no favicon regeneration: the brand is `src/shell/Logo.tsx` and `public/favicon.svg` is the logo task's pair.

components.css, shell section — the two size classes every later package uses (nothing defines them today):

```css
/* Registry icons outside the tab bar: NavIcon's default class .nav-icon is display: none from 640 px up,
   so every other use passes one of these. */
.icon { width: var(--icon); height: var(--icon); flex: none; }
.icon-sm { width: var(--icon-sm); height: var(--icon-sm); flex: none; }
```

**Files.** `src/shell/icons.tsx`, `src/styles/components.css`.

**Why.** Close is a text ✕ in five box sizes, expand/collapse is drawn four ways (▾, ⌃, a CSS chevron, the OS select arrow), sort hints are 60 %-opacity ↕ specks and the active sort reuses the P&L ▼, copy/explorer are tiny font glyphs. One registry, one stroke weight.

**Watch out.** A second `BrandMark` would give the product two brand drawings (Logo.tsx already exports one and AppShell renders it). `stroke-width` / `stroke-linecap` in TSX make React warn — camelCase only. P3 does not redefine `.icon` / `.icon-sm`.

#### P2-2 — Brand lockup: the wordmark at every width, the group before the nav (high)

**Change.** AppShell.tsx stays as the logo task left it: `<BrandMark />` from `./Logo.tsx` followed by `<span className="brand-word">ShortWin</span>` inside the link with `aria-label="ShortWin home"`; do not add a span wrapper, `.brand-mark svg`, `.brand-mark-tile` or brand tokens, and leave the logo task's `.brand { gap: var(--sp-3) }`, `.brand-mark { flex: none; display: block }`, `.brand-mark-win` and the xs `.brand-mark { width: 28px; height: 28px }` rules untouched.

components.css, in the existing `(max-width: 399.98px)` block (shell: phone, ~l.324–335): replace the unconditional `.brand-word` hiding rule with

```css
/* The word yields only to the cluster badge: on mainnet 'ShortWin' shows at every width; with LOCALNET / DEVNET
   in the bar a connected wallet at 320 px would overflow (mark 28 + word + badge + ⌕ 44 + wallet ≈ 326 px > 296). */
.app-header:has(.cluster-badge) .brand-word {
    position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); white-space: nowrap;
}
.app-header { gap: var(--sp-2); }
.cluster-badge { padding-inline: var(--sp-1); }
```

In the `(max-width: 639.98px)` shell block: `.brand { justify-content: flex-start; }` (keep its `min-width` / `min-height: var(--tap-min)`) so the mark lines up with the page's h1 at the gutter. Group brand and badge only where the nav is in the header's flow:

```css
@media (min-width: 640px) {
    .cluster-badge { margin-left: calc(-1 * var(--sp-2)); }
    .nav { margin-left: var(--sp-3); }   /* brand + badge read as one group with 24 px before the nav */
}
```

Budget re-checked with the logo task's sizes: under 400 px with the badge the word is hidden, brand = 44 px → ≈ 257 px at 320; mainnet connected ≈ 24 + 122 + 4 + 44 + 4 + 100 = 298 px — both fit at 320. The §3.1 / §7.7 wording ('ShortWin visually hidden < 400' → 'hidden under 400 px only while the cluster badge shows') is P10's.

**Files.** `src/shell/AppShell.tsx` (no change expected), `src/styles/components.css`.

**Why.** Every phone screenshot shows '▼ LOCALNET ⌕ … Connect': the product has no name at 390 px. On mainnet nothing stops the wordmark fitting at 320 px; the cluster badge is information for testers, so only it may push the word out.

**Watch out.** Under 640 px `.nav` is `position: fixed; inset: auto 0 0`; a `margin-left` on it shifts the fixed tab bar 8 px right and narrows it — hence the `(min-width: 640px)` scope. On localnet at 390 px the word stays hidden by design (badge present); the gain is mainnet at 320–399 px, the production case.

#### P2-3 — The phone status strip: a real chevron, a tint that shouts, a scrim, the formatted slot (high)

**Change.** StatusBar.tsx: replace the '⌃' span with `<NavIcon name="chevron" className="status-chevron" />` (aria-hidden by NavIcon); while `expanded`, render `<div className="status-scrim" aria-hidden="true" onPointerDown={() => setExpanded(false)} />` as the footer's **first** child, before the items (`data-sheet-keep` is inert here — the footer inherits it — so leave it off); format the RPC item like the summary: `Number(slot.slot).toLocaleString('en-US')` inside the existing `.num` (`slot.slot` is `bigint | null`).

components.css — every rule below goes in the **phone status block** (the block P2-6 moves under the combined phone + landscape prelude), never in the shell section (`.status-summary` is in the DOM at every width and `:has()` matches DOM presence, so a shell-section placement would tint the desktop bar):

```css
.status-chevron { width: var(--icon); height: var(--icon); color: var(--text-2); transform: rotate(180deg); }
.statusbar[data-expanded] .status-chevron { transform: rotate(0); }       /* replaces the existing rotate rule */

.statusbar { background: var(--bg-2); border-top: 0; box-shadow: 0 -1px 0 var(--line-panel); } /* matches its own expanded list; the shadow-not-border technique stays */
.statusbar:has(.status-summary[data-state="warn"]) { background: var(--warn-bg); box-shadow: 0 -1px 0 var(--warn); }
.statusbar:has(.status-summary[data-state="bad"]) { background: var(--danger-bg); box-shadow: 0 -1px 0 var(--danger); }
.status-summary[data-state="warn"],
.status-summary[data-state="bad"] { color: var(--text-0); font-weight: var(--fw-medium); }  /* tint, weight and the word: three cues */

/* The fixed footer is its own stacking context: the scrim sits at 0 and the list above it, both inside the footer,
   which already paints over the header (z 20) and the action bar (z 30, earlier in the DOM). The scrim stops at the
   strip's top edge so its toggle stays usable. */
.status-scrim { position: fixed; inset: 0 0 calc(var(--tabbar-h) + var(--statusbar-h) + var(--safe-b)); z-index: 0; background: var(--bg-overlay); }
.statusbar[data-expanded] .statusbar-items { z-index: 1; box-shadow: var(--shadow-sheet); display: grid; grid-template-columns: 1fr 1fr; } /* write it AFTER the existing display: flex declaration so grid wins */
.statusbar[data-expanded] span.statusbar-item { grid-column: 1 / -1; background: var(--bg-2); }
.statusbar[data-expanded] .statusbar-item.muted { color: var(--text-1); }
.statusbar[data-expanded] a.statusbar-item { justify-content: center; margin: var(--sp-3) var(--sp-2) 0; border: 1px solid var(--line-strong); border-radius: var(--radius); }
```

Replace the existing `.statusbar[data-expanded] .statusbar-item:last-child { border-bottom: 0; }` with `.statusbar[data-expanded] span.statusbar-item:last-of-type { border-bottom: 0; }` (the old rule, at (0,3,0), outranks the link rule's border and would strip the Security button's bottom edge).

Desktop bar (shell section, scoped to `.statusbar` so the market header's FreshnessBadge is untouched):

```css
.statusbar .stale-badge[data-state="live"] { color: var(--text-1); }
.statusbar .statusbar-item:has(> .dot[data-state="warn"]),
.statusbar .stale-badge[data-state="lagging"] { color: var(--warn); }
.statusbar .statusbar-item:has(> .dot[data-state="bad"]),
.statusbar .stale-badge[data-state="stale"],
.statusbar .stale-badge[data-state="down"] { color: var(--danger); }
```

**Files.** `src/shell/StatusBar.tsx`, `src/styles/components.css`.

**Why.** 'Stale data is never silent' (§3.1), yet on a phone a bad state is a 12 px red ✕ among grey text in a strip whose only expand cue is a 6 px caret; a dismissing tap outside the list lands on the page underneath; 'RPC 2064' sits 30 px under 'RPC 2,064'; the desktop bar paints the one healthy item green while a degraded 'Live reconnecting' stays grey.

**Watch out.** Contrast: text-0 on `--warn-bg` 12.0:1, on `--danger-bg` 13.1:1; `--warn` on bg-1 9.35, `--danger` 6.1. The expanded items keep 44 px; every testid stays; smoke.spec checks `status-rpc`'s `data-state` only. The scrim's `onPointerDown` is inside the footer, so the document handler does not double-fire and mobile-shell.spec's tap → Escape flow survives.

#### P2-4 — Header geometry, shell alignment, floating layers (medium)

**Change.** components.css:

- shell: `.app-header { box-shadow: var(--shadow-header); border-bottom: 0; }` (`--shadow-header` begins with `0 1px 0 var(--line-panel)`, so the edge stays a hairline with a faint drop under it); `.statusbar { padding: 0 max(var(--gutter), var(--safe-r)) var(--safe-b) max(var(--gutter), var(--safe-l)); }` (it pads `--sp-4` today where the header and page use `--gutter`; the phone block's `.statusbar { padding: 0 }` still wins there).
- `(max-width: 1023.98px)` shell block: `.search-toggle { margin-left: auto; } .app-header > .header-spacer { display: none; }` — DOM order unchanged; the free space moves before the search so ⌕ and Connect form the trailing group of the §3.1 diagram.
- search section (it follows the buttons section, so this outranks `.btn[data-variant="ghost"] { color: var(--text-1) }`): `.app-header .search-toggle { color: var(--text-0); }`.
- layouts: `.page { max-width: var(--page-max); margin-inline: auto; }`; under the existing `@media (min-width: 1680px)`: `.app-header, .statusbar { padding-inline: max(var(--gutter), var(--safe-l), calc((100% - var(--page-max)) / 2)) max(var(--gutter), var(--safe-r), calc((100% - var(--page-max)) / 2)); } .app-main > .banner { max-width: var(--page-max); margin-inline: auto; }`.
- menus and search results: `.menu, .search-results { border-color: var(--line-strong); box-shadow: var(--shadow-pop); }`; `.menu-item:focus-visible, .search-option[aria-selected="true"] { background: var(--bg-4); }`; `.search-option[aria-selected="true"] { box-shadow: inset 2px 0 0 var(--accent); }`; inside `@media (hover: hover)`: `.menu-item:hover:not(:disabled), .search-option:hover { background: var(--bg-4); }` (keep the `:not(:disabled)` — a wallet entry disabled while `connect.isRunning` must not light up); in the `(max-width: 1023.98px)` search block add `box-shadow: none;` to the existing `.search[data-open] .search-results` rule (the full-screen overlay is static and borderless; `--shadow-pop` would paint a band over the input row).
- `.muted` names on a highlighted option: with P1's values (`--text-2` #919cab on `--bg-4` #242c35 = 5.08:1) no override is needed; confirm with axe on `header-search-results` in the gate.

**Files.** `src/styles/components.css`.

**Why.** The magnifier hugs the cluster badge while Connect sits alone 107 px away; the title, panel edge, status-bar dot and brand start at three different x; at ≥ 1680 the page centres but the header and status bar stay full-bleed; dropdowns cast no shadow and the keyboard-highlighted result is a 1.10:1 step from its neighbours.

**Watch out.** `.search-toggle { color }` at (0,1,0) is a no-op against the ghost variant's (0,2,0) — hence the `.app-header .search-toggle` form in the search section. A blur-only header edge softens a dense terminal header into a marketing bar; the 1 px line in the shadow is not optional.

#### P2-5 — Phone bottom chrome: the active tab, the action bar (medium)

**Change.** components.css, shell: phone block — tab bar (replace the current `border-top-color: var(--accent)` active rule):

```css
.nav { border-top-color: var(--line-panel); }
.nav a, .nav-more { position: relative; border-top: 0; }
.nav a::before, .nav-more::before {
    content: ""; position: absolute; top: 0; left: 50%; width: var(--tabbar-indicator-w); height: 2px;
    translate: -50%; border-radius: 0 0 2px 2px; background: transparent; pointer-events: none;
}
.nav a[aria-current="page"], .nav a.active, .nav-more[data-current] { color: var(--accent); }
.nav a[aria-current="page"]::before, .nav a.active::before, .nav-more[data-current]::before { background: var(--accent); }
.nav a[aria-current="page"] .nav-icon, .nav a.active .nav-icon, .nav-more[data-current] .nav-icon { stroke-width: 2; }
```

Action bar (its rules in the responsive-blocks section under `(max-width: 1023.98px)`): `.action-bar { border-top: 0; box-shadow: var(--shadow-up); }` — it keeps bg-1; with the strip at bg-2 (P2-3) the three bars read as panel / status band / tab bar. P2-2's `.nav` margin scoping lands in this same block.

**Files.** `src/styles/components.css`.

**Why.** The active tab is a 2 px line under a hairline plus a slight text shift and the icon never changes; action bar, strip and tab bar are one 140 px slab of bg-1 separated by hairlines, so the page's Short/Lend buttons look like shell chrome and a low thumb expands the strip instead.

**Watch out.** The `::before` indicator is out of the column-flex flow with `pointer-events: none`, so the tap-cover check and the 44 px tab height do not change; the action-bar button's inner height rises from 45 to 46 px once the border goes. CSS `stroke-width: 2` wins over the svg's `strokeWidth="1.6"` presentation attribute. mobile-shell.spec asserts `aria-current` only.

#### P2-6 — Landscape phones get the phone's status strip (medium)

**Change.** components.css: move the whole phone status block — the fixed `.statusbar`, `.status-summary`, `.status-chevron`, the hidden / expanded `.statusbar-items`, the expanded item rules, `.statusbar[data-expanded] .header-spacer`, `.statusbar-cluster` display, P2-3's tint, scrim and list rules, and the `.app-main` padding-bottom rule — under the combined prelude, placed after the tablet block as the phone block is now:

```css
@media (max-width: 639.98px), (min-width: 640px) and (max-width: 1023.98px) and (pointer: coarse) and (orientation: landscape) {
    /* …the phone status block… */
}
```

(Both halves are in the closed set; the gate's `phone-land` viewport, 844 × 390 with touch, matches it, and `--statusbar-h` is 28 px there from P1-4.) In the tablet block keep only `.statusbar-items { gap: var(--sp-3); }` as the mouse-window concession (it recovers 24–28 px so the bar fits from ≈ 666 px); do not add `a.statusbar-item { flex: none; }` (every `.statusbar-item` is `white-space: nowrap`, so no flex item shrinks and `overflow: hidden` clips the end of the row whatever the flex values). Do not add ellipsis, wrapping or hidden items: the bar's height is a token other offsets depend on, and staleness is never hidden. Record in the PR that a 640–665 px mouse window still clips the end of 'Security' (no gate viewport, no device class); §7.7's tablet row ('a landscape phone gets the strip') is P10's.

**Files.** `src/styles/components.css`.

**Why.** From 640 px the shell keeps the eight-item desktop bar with `overflow: hidden`; at 12 px it needs ≈ 690 px (≈ 740 with a nine-digit mainnet slot), so a 667 px landscape iPhone SE/8 clips half of 'Security' — navigation and network facts silently cut on a real device class the 844 × 390 gate never sees.

**Watch out.** `.status-summary` is not in the gate's PRIMARY list, so a 28 px strip under a coarse pointer passes (the floor is 24); the expanded items keep 44 px. breakpoints.test.ts passes: the parser sees 639.98 / 640 / 1023.98, no range syntax, the orientation only with coarse in the tablet tier.

#### P2-7 — Layouts: the page rhythm, the shared side column and sticky panel (medium)

**Change.** components.css, layouts section:

```css
.page { gap: var(--page-gap); }
.page-head { min-height: 36px; margin-bottom: calc(var(--page-head-gap) - var(--page-gap)); }
.page > .stack, .layout-trade > .stack, .layout-side > .stack { gap: var(--page-gap); } /* top-level stacks only; .stack itself keeps var(--sp-3): it is also the lend / Manage forms, SettlementRisk, nested create-market stacks and the toast region */
.app-main { padding-top: var(--sp-4); }

.layout-side { display: grid; grid-template-columns: minmax(0, 1fr) var(--side-col); gap: var(--page-gap); align-items: start; }
```

Add `.layout-side` to the existing `(max-width: 1023.98px)` one-column rule beside `.layout-trade`. `--side-col` is 360 px and 320 px under 1,180 px from P1-4, so the local `(max-width: 1179.98px)` 320 px override is redundant — confirm tokens.css has that block, then delete the local override for `.layout-trade` (P5 / P6 delete their copies). The sticky recipe, shared:

```css
@media (min-width: 1024px) {
    .side-sticky {
        position: sticky; top: calc(var(--header-h) + var(--sp-3));
        max-height: calc(100dvh - var(--header-h) - var(--statusbar-h) - 2 * var(--sp-3));
        overflow-x: hidden; overflow-y: auto; overscroll-behavior: contain;
        scrollbar-gutter: stable both-edges; scrollbar-width: thin; scrollbar-color: var(--scrollbar-thumb) transparent;
        scroll-padding-top: var(--sp-3); scroll-padding-bottom: calc(120px + var(--sp-3));
    }
    .side-sticky .form-foot { box-shadow: var(--shadow-up); }
}
```

Keep `.layout-trade` / `.layout-trade > .trade-panel` as aliases of the same declarations (point them at `--side-col`; add the scrollbar-color, scroll-padding and foot shadow) until P5 adds the shared classes to MarketPage and drops the alias. The sticky rule must stay inside `(min-width: 1024px)` so the `.sheet-host[data-sheet="open"]` fixed-sheet rules keep winning below it.

**Files.** `src/styles/components.css`.

**Why.** Uniform 8 px gaps invert the proximity cue (a box is roomier inside than the space between boxes); the sticky trade panel scrolls invisibly — its text is cut mid-sentence above the foot with no scrollbar discernible and the acknowledgement 1,100 px below the fold — and the same sticky recipe is typed in three places with two widths.

**Watch out.** `.stack { gap: var(--page-gap) }` unscoped would loosen the density inside boxes, which the direction keeps untouched. At 1024 px the market column must stay 680 px (§7.2 / §7.6 fit the tab tables there with a 320 px panel) — that is what the 320 px value of `--side-col` under 1,180 px guarantees. The `.trade-panel` locators of mobile-trade / mobile-markets / mobile-guide keep working through the alias.

#### P2-8 — Wallet sheet, chooser and identity block (low)

**Change.** components.css, wallet section, inside its `(max-width: 639.98px)` block:

```css
.wallet-anchor > .menu { border-width: 1px 0 0; box-shadow: var(--shadow-sheet); }
.wallet-anchor > .menu > .menu-head { margin-bottom: var(--sp-3); }
.wallet-anchor > .menu > .menu-head + .menu-label,
.wallet-anchor > .menu .wallet-chooser { padding-top: var(--sp-3); }
```

Everywhere: `.menu-label:has(> .num) { font-size: var(--fs-sm); color: var(--text-1); } .menu-label > .num { color: var(--text-0); font-size: var(--fs-md); }` (the balance becomes the block's one focal number); `.menu .banner` keeps its margins. Chooser:

```css
.wallet-chooser { gap: var(--sp-4); padding: var(--sp-4) var(--sp-3); }
.wallet-links { gap: 0; border: 1px solid var(--line-strong); border-radius: var(--radius); } /* no overflow: hidden — it would clip the rows' focus rings */
.wallet-links .btn { border: 0; border-radius: 0; min-height: 48px; justify-content: space-between; }
.wallet-links .btn:first-child { border-radius: var(--radius) var(--radius) 0 0; }
.wallet-links .btn:last-child { border-radius: 0 0 var(--radius) var(--radius); }
.wallet-links .btn:focus-visible { outline-offset: -3px; }   /* the ring sits inside the row */
.wallet-links .btn + .btn { border-top: 1px solid var(--line); }
.wallet-note { border-left: 3px solid var(--info); padding-left: var(--sp-3); }
.wallet-install { font-size: var(--fs-sm); color: var(--text-1); }
```

WalletChooser.tsx: add `className="wallet-note"` to the cluster-hint paragraph and render `<NavIcon name="external" className="icon-sm" />` in place of the '↗' text in the open-in links; WalletButton.tsx: the same for 'View on explorer ↗' and `<NavIcon name="close" className="icon" />` in the sheet's ✕ button (classes, aria-labels and testids unchanged). Copy and order unchanged. Lands after P2-1.

**Files.** `src/styles/components.css`, `src/wallet/WalletChooser.tsx`, `src/wallet/WalletButton.tsx`.

**Why.** The wallet sheet keeps a 1 px frame down both screen edges and crams 'ShortWin E2E' 4 px under its head with the balance as 12 px muted text; the no-wallet chooser is eight equally weighted items (three identical buttons, a cluster hint, 'Don't have one?', three links, Reload) at the moment a newcomer is sent to another app.

**Watch out.** `.wallet-links .btn` (0,2,0) in the wallet section outranks `.btn` in the buttons section; the 48 px rows satisfy the gate's `.btn:not([data-size="sm"])` ≥ 44 rule (Reload is `data-size="sm"` and exempt). wallet.test.tsx asserts hrefs, testids and `not.toMatch(/·/)`; mobile-shell.spec asserts hrefs and the ✕'s `aria-label="Close"` — all kept.

#### P2-9 — More sheet rows: a leading icon and a trailing chevron (low)

**Change.** MoreSheet.tsx: give each `.more-row` Link a leading `<NavIcon name=… className="more-icon" />` (How it works → `info`, Status → `status`, Security policy → `shield`, Create market → `plus`) and, as the Link's last child, `<NavIcon name="chevron" className="more-chevron" />` (both aria-hidden by NavIcon). components.css, More section:

```css
.more-row { display: grid; grid-template-columns: 24px minmax(0, 1fr) 16px; column-gap: var(--sp-3); align-items: center; border-bottom-color: var(--line-panel); }
.more-row > .more-icon { grid-row: 1 / span 2; width: 20px; height: 20px; color: var(--text-1); }
.more-row > .muted { grid-column: 2; }
.more-row > .more-chevron { grid-column: 3; grid-row: 1 / span 2; width: 16px; height: 16px; color: var(--text-2); rotate: -90deg; justify-self: center; } /* points right */
```

No `::after` chevron (the registry's one glyph serves). The bare title text is an anonymous grid item: with the icon locked to column 1 rows 1–2, `.muted` to column 2 and the chevron to column 3, auto-placement puts the title in row 1 column 2 — verify in the phone More shot. `.more-network h3` inherits the section tier from base.

**Files.** `src/shell/MoreSheet.tsx`, `src/styles/components.css`.

**Why.** The More sheet is four flat two-line rows with no icon or navigation affordance while a Status icon exists and is never shown on a phone; the tab bar's icon language stops at the fourth tab. Rows stay 48 px Links with their testids (mobile-shell.spec taps `more-status` / `more-security`); the hairlines need `--line-panel` once the sheet surface is bg-2 (P9-2).

**Watch out.** A CSS-border `::after` chevron would be a fifth chevron drawing beside the registered one.

**Verification (P2).** Phone 390 shots of markets, market, positions, status and status-expanded: the header shows the W mark (24 px; 28 px under 400) and, on mainnet, 'ShortWin'; ⌕ sits next to Connect; the active tab is gold with a 32 px indicator and a 2-stroke icon; the strip is a bg-2 band with a 16 px chevron; a forced `data-state=bad` (stop the indexer, as 50-fallback does) tints the strip red with white medium text; the expanded list shows 'RPC 2,064' and two bordered links on one row over a dimmed page, every item still tappable (the scrim is below the list). 320 px gate run with the LOCALNET badge and a connected wallet: no sideways overflow. Desktop 1440: header shadow with a hairline edge, no border; title x = panel x = status-bar dot x = gutter; 'Indexer live' is grey, 'Live reconnecting' amber in the fallback state; the open search results cast a shadow and the highlighted option is bg-4 with a gold inset bar. 1920 px window: header and status bar padding start at the page column. A 667 × 375 coarse landscape emulation shows the strip, not a clipped bar; the phone search overlay shows no stray shadow band. More sheet: icon, title, muted line, chevron per row. e2e: mobile-shell.spec, smoke.spec (status-rpc data-state), wallet.test.tsx, StatusBar.test.ts and the responsive gate at all eight viewports pass.

---

### P3 — Core components: panels, buttons, inputs, tabs, badges, disclosures, banners, glossary marks, chips, reveal, form foot

**Goal.** One control system that answers the pointer the same way everywhere, inputs drawn by the app rather than the OS, nested tabs that read as a sub-level, an acknowledgement that looks like the gate it is, and a disabled submit that keeps its shape but never competes with a Connect CTA.

**Owned files.**
- `src/styles/components.css` — sections 'panel', 'buttons', 'inputs', 'tabs', 'badges', 'disclosure', 'untrusted text', the `.dot` and `.banner` rules (~l.330–392), the 'glossary' section, and inside 'responsive building blocks' the 'form foot', 'chips' and 'reveal' rules
- `src/components/Button.tsx`, `Panel.tsx`, `Field.tsx`, `Tabs.tsx`, `Badge.tsx`, `Disclosure.tsx`, `Glossary.tsx`, `Reveal.tsx`, `Address.tsx`, `TokenId.tsx`, `Countdown.tsx`, `FormFoot.tsx`, `PageBanners.tsx`

Tokens consumed (P1): `--line-panel`, `--edge-highlight`, `--surface-note`, `--bg-4`, `--loss-hover`, `--accent-active`, `--title-fs/-fw/-ls/-color`, `--label-fs/-fw/-ls/-color`, `--field-h`, `--select-arrow-w`, `--check`, `--icon-btn`, `--icon-muted`, `--tag`, `--tag-bg`, `--shadow-pop`, `--warn`. Icons consumed (P2-1): `close`, `copy`, `check`, `external`; size classes `.icon` / `.icon-sm`.

Cascade order matters in three places because specificities tie: the shared close rule goes after `.btn[data-size="sm"]`; `.btn[aria-busy]` after `.btn:disabled`; the new `.form-foot > .btn` rule replaces the existing `width: 100%` rule in place so the coarse 48 px floor still wins.

**Items.**

#### P3-1 — Panels, titles, field labels and the disclosure surface on the tiers (high)

**Change.** components.css panel section:

```css
.panel { border-color: var(--line-panel); box-shadow: var(--edge-highlight); }
.panel.sheet-host[data-sheet="open"] { box-shadow: none; }   /* the open sheet resets the panel's border, not its shadow */
.panel-title { font-size: var(--title-fs); font-weight: var(--title-fw); letter-spacing: var(--title-ls); color: var(--title-color); }  /* .panel-head keeps 34 px */
```

inputs section: `.field-label { font-size: var(--label-fs); font-weight: var(--label-fw); letter-spacing: var(--label-ls); color: var(--label-color); }`. disclosure section: `.disclosure { border-color: var(--line-panel); background: var(--surface-note); }` (the severity left-bar colours are set by `.disclosure[data-severity]` at (0,2,0) and lend.css's blocked rule, which still outrank this). Result: a panel's title (13 px bold white) outranks the h3 sections in its body, which outrank field labels.

**Files.** `src/styles/components.css`.

**Why.** Panel titles ('POSITION', 'MANAGE') are 12 px light grey — smaller than the 13 px h3 sections inside them — and field labels share the sections' colour, so 'ADD MARGIN' over 'AMOUNT (SOL)' is four grey uppercase lines. The 1.4:1 panel edge vanishes on a dim laptop panel.

**Watch out.** `--title-fs` must be 13 px and `--label-fs` 12 px at every width (the gate fails text < 12 px); `--label-color` (`--text-2`, 6.53:1 on bg-1) passes.

#### P3-2 — Disabled, busy and long submits; the form foot (high)

**Change.** components.css buttons section — the disabled rule (shape restored, still neutral, and a weight cue against an enabled default button), then the busy spinner right after it:

```css
.btn:disabled, .btn[data-variant]:disabled { background: var(--bg-2); border-color: var(--line-strong); color: var(--text-2); font-weight: var(--fw-regular); }
.btn[aria-busy="true"] { cursor: progress; }
.btn[aria-busy="true"]::before { content: ""; flex: none; width: 1em; height: 1em; border: 2px solid currentColor; border-right-color: transparent; border-radius: 50%; animation: btn-spin 0.8s linear infinite; }
@keyframes btn-spin { to { transform: rotate(1turn); } }
@media (prefers-reduced-motion: reduce) { .btn[aria-busy="true"]::before { animation: none; border-right-color: currentColor; opacity: 0.5; } }
```

Form foot (responsive building blocks) — replace the existing `.form-foot > .btn { width: 100%; }` rule in place, before the coarse 48 px rule:

```css
.form-foot > .btn { width: 100%; white-space: normal; height: auto; min-height: var(--control-h); padding-block: var(--sp-1); line-height: var(--lh-tight); }
.form-foot::before { content: ""; position: absolute; left: 0; right: 0; bottom: 100%; height: 12px; background: linear-gradient(to bottom, transparent, var(--bg-1)); pointer-events: none; }
.form-foot:has(.wallet-inline)::before { display: none; }   /* short.css puts this foot back to static while the inline chooser is open */
```

Beside the existing landscape-phone rule: `.sheet-host[data-sheet="open"] .form-foot::before { display: none; }`, and `@media (max-width: 639.98px) { .mk-create-foot .form-foot::before { display: none; } }` (that foot is `position: fixed` over the page's bg-0, so a fade to bg-1 would glow over nothing). While a Connect CTA is in the foot:

```css
.form-foot:has(.connect-cta) { gap: var(--sp-1); }
.form-foot:has(.connect-cta) > .btn:disabled { background: transparent; border-color: transparent; color: var(--text-2); font-weight: var(--fw-regular); min-height: var(--tap-min); }
button.form-reason { color: var(--info); }   /* unscoped: it must also reach the modal's button.form-reason.tx-sign-hint; p.form-reason stays text-1 */
```

The demoted submit is 24 px with a mouse and 44 px under coarse — this (0,4,0) rule outranks the coarse `.form-foot > .btn { min-height: 48px }` (0,2,0), and the gate's `.form-foot .btn` ≥ 44 floor holds; if 48 is wanted, add `@media (pointer: coarse) { .form-foot:has(.connect-cta) > .btn:disabled { min-height: 48px; } }`. The reason line, the CTA and the submit stay in the DOM with their testids and texts.

**Files.** `src/styles/components.css`.

**Why.** A disabled submit is a flat bg-2 slab with a 1.4:1 border — the recipe of a panel head — so under a Connect CTA two full-width buttons stack and say the same thing three times (~130 px of a phone sheet); a working submit looks identical to a disabled one; the foot slices rows with a hard edge; 'Review deposit (with CPI Guard bracket)' overflows a 320 px foot; the clickable 'Tick "I understand…" above' is grey-underlined where every other in-page action is blue.

**Watch out.** `button.form-reason` blue is 6.96:1 and keeps its underline. mobile-trade.spec:221–222 and mobile-lend.spec:254 assert the reason's visibility and text — unchanged. Button.tsx sets `aria-busy` only while busy, so the spinner is honest.

#### P3-3 — Button hover / active system, chips, the shared close button (high)

**Change.** components.css buttons section, inside the existing `@media (hover: hover)` block:

```css
.btn:hover:not(:disabled) { background: var(--bg-4); }
.btn[data-variant="short"]:hover:not(:disabled) { background: var(--loss-hover); border-color: var(--loss-hover); filter: none; }
.btn[data-variant="danger"]:hover:not(:disabled) { background: var(--danger-bg); }
.btn[data-variant="ghost"]:hover:not(:disabled) { background: var(--bg-4); color: var(--text-0); }
.chip:hover:not([aria-pressed="true"]) { background: var(--bg-4); }
.btn.modal-close:hover, .btn.sheet-close:hover, .btn.glossary-close:hover, .btn.mk-popover-close:hover, .btn.toast-dismiss:hover { background: var(--bg-4); }
```

Outside it — the filled variants' own presses (the base rule from P1-2 excludes them):

```css
.btn[data-variant="primary"]:active:not(:disabled) { background: var(--accent-active); border-color: var(--accent-active); }  /* the existing rule, recoloured */
.btn[data-variant="short"]:active:not(:disabled) { background: var(--loss-hover); border-color: var(--loss-hover); }
```

Chips (responsive building blocks): `.chip { border-radius: var(--radius); } .chip[aria-pressed="true"] { background: var(--bg-3); border-color: var(--accent); color: var(--text-0); font-weight: var(--fw-medium); }`.

Shared close button (buttons section, placed after `.btn[data-size="sm"]` — `.modal-close` carries `data-size="sm"`, and both rules are (0,2,0)):

```css
.btn.modal-close, .btn.glossary-close, .btn.mk-popover-close {
    width: max(var(--icon-btn), var(--tap-min)); height: max(var(--icon-btn), var(--tap-min));
    min-width: 0; min-height: 0; padding: 0; color: var(--text-0);
}
.btn.sheet-close, .btn.toast-dismiss { color: var(--text-0); } /* the sheet close keeps its 44 px from the sheets block; the toast dismiss keeps its sm-ghost box so the 'Dismiss' word it shows over an open sheet fits */
```

Remove the glossary section's own `.btn.glossary-close` size rule. `.icon` / `.icon-sm` are P2-1's — do not redefine them. Note for P5-8: the `.mk-popover-close` size rules in markets.css (~l.581, ~l.861) are now dead and get deleted there.

**Files.** `src/styles/components.css`.

**Why.** The short variant hovers with the only `filter` in the system, the red outline 'Close position' hovers neutral grey exactly when the user is about to press it, the primary lightens on press while everything else darkens, the 25/50/75 % chips are the only pills and 'pressed' is a 1 px border change; close ✕ boxes come in 24/28/32/44 px.

**Watch out.** `--danger` on `--danger-bg` is 5.19:1; text-inverse on `--loss-hover` / `--accent-active` stays > 6:1. Toast.tsx swaps the ✕ for the word 'Dismiss' on a phone over an open sheet (`html[data-sheet-open] .toast-region .toast-dismiss-word { display: inline }`), which a fixed square box would clip. `.sheet-close { min-width: 44px }` in the sheets block would otherwise combine with a 32 px height into a 44 × 32 rectangle on a tablet with a mouse.

#### P3-4 — Inputs drawn by the app: selects, fields, checkboxes, radios, Max (high)

**Change.** components.css inputs section:

```css
.select {
    appearance: none; height: var(--field-h); padding: 0 var(--select-arrow-w) 0 var(--sp-3); color: var(--text-0); text-overflow: ellipsis;
    background: var(--bg-3) url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16' fill='none' stroke='%23aab4bf' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M4 6l4 4 4-4'/%3E%3C/svg%3E") no-repeat right var(--sp-3) center / 16px;
}
.select:focus-visible { outline: 2px solid var(--focus); outline-offset: 0; }
```

(the arrow is `--text-1`'s hex, 7.35:1 on bg-3: a data URI cannot read a variable; `src/config/csp.ts` allows `img-src 'self' data:`). `.input-wrap { height: var(--field-h); }` replacing the coarse `calc(var(--control-h) + 2px)` override (~l.757): `--field-h` is 30 px with a mouse and 46 px under coarse from P1-4, so the inner `<input>` (`height: 100%` inside a 1 px border) still measures 44 px — `input` and `select` are PRIMARY controls in the gate. Disabled fields and Max:

```css
.input-wrap:has(> .input:disabled) { background: var(--bg-1); border-color: var(--line); }
.input-wrap:has(> .input:disabled) .input-suffix, .input:disabled { color: var(--text-2); cursor: not-allowed; }
.input-wrap .input-max { color: var(--text-1); }
@media (hover: hover) { .input-wrap .input-max:hover { color: var(--text-0); } }
```

Checkbox and radio (the create page has the only radios; style them generically):

```css
.checkbox input, input[type="radio"] {
    appearance: none; width: var(--check); height: var(--check); flex: none; display: inline-grid; place-content: center;
    background: var(--bg-3); border: 1px solid var(--line-strong); border-radius: var(--radius-sm); cursor: pointer;
}
input[type="radio"] { margin: 0; border-radius: 50%; }
.checkbox input:checked { background: var(--accent); border-color: var(--accent); }
.checkbox input:checked::before { content: ""; width: 5px; height: 9px; border: solid var(--text-inverse); border-width: 0 2px 2px 0; transform: translateY(-1px) rotate(45deg); }
input[type="radio"]:checked { border-color: var(--accent); }
input[type="radio"]:checked::before { content: ""; width: 8px; height: 8px; border-radius: 50%; background: var(--accent); }
.checkbox input:disabled { opacity: 0.5; cursor: not-allowed; }
```

In the existing `.checkbox input` rule drop `width: 16px; height: 16px; accent-color: var(--accent)` and keep its `margin-top: 2px` (the container is `align-items: flex-start` with a mouse); do not add `align-self`. In the coarse block keep `.checkbox { min-height; align-items: center; padding-block }` and `.checkbox input { margin-top: 0; }`, dropping only its `width/height: 20px` (the `--check` token carries 20 under coarse). Keep base.css's `:focus-visible` outline. markets.css's radio `accent-color` and coarse 20 px rules are harmless under `appearance: none` until P5-6 deletes them.

**Files.** `src/styles/components.css`.

**Why.** The selects show Chrome's white chevron among custom-drawn controls and sit 2 px shorter than the fields under a finger; checkboxes and radios are the OS's accent-colour widgets (different in Safari and Firefox), including the acknowledgement that gates every signature; disabled fields look enabled; the gold 'Max' text means 'press me' 60 px above gold text that means 'be careful'.

**Watch out.** A 44 px `--field-h` would yield a 42 px input and fail 'primary controls under 44px' at the five touch viewports. The gate measures a checkbox / radio by its `<label>` rect; `.checkbox`, `.mk-choice`, `.mk-pool-choice` keep `min-height: var(--tap-min)`.

#### P3-5 — The acknowledgement as a control row; the details chevron (high)

**Change.** disclosure section (scope stays `.disclosure .checkbox`, so the markets toolbar checkbox is untouched):

```css
.disclosure .checkbox { margin-top: var(--sp-3); padding: var(--sp-2) var(--sp-3); background: var(--bg-3); border: 1px solid var(--line-strong); border-radius: var(--radius); font-weight: var(--fw-medium); }
.disclosure .checkbox:has(input:checked) { border-color: var(--accent); }
.disclosure .checkbox:has(input:focus-visible) { outline: 2px solid var(--focus); outline-offset: 1px; }  /* one ring, on the row, keyboard only */
.disclosure .checkbox input:focus-visible { outline: none; }
.disclosure .checkbox input { width: calc(var(--check) + 4px); height: calc(var(--check) + 4px); margin-top: 0; }  /* 22 px with a mouse, 24 under coarse */
```

Details chevron, aligned to the icon set: `details > summary::before { width: 7px; height: 7px; border-width: 0 2px 2px 0; margin: 0 var(--sp-1) 0 2px; }` (2 px borders: a 1.6 px border snaps to 1 px on a 1× screen).

**Files.** `src/styles/components.css`.

**Why.** 'I understand my position can be liquidated or settled in SOL.' is a 16 px grey box at the end of eight bullets in the same type as the bullets; users read the foot's reason, look for a button and do not recognise the checkbox as the thing to tap.

**Watch out.** Every e2e reaches the ack through `T.disclosureAck(id)` on the input (fixtures.ts, keyboard.spec, responsive.spec's `cap.tap(…, 'check')`), which is unchanged; the SPEC sentence stays the checkbox's label verbatim (components.test 'Disclosure'). The checked state has the tick itself as a non-colour cue.

#### P3-6 — Tabs: hover, two-sided overflow fade, the sub-level variant (high)

**Change.** Tabs.tsx: add `variant?: 'sub'` and emit `data-variant={variant}` on the `.tabs` div; in `update()` compute `start = el.scrollLeft > 4` and `end = el.scrollWidth - el.clientWidth - el.scrollLeft > 4` and set `data-overflow` to `'both' | 'start' | 'end'` (remove it when neither). Roles, ids, `aria-controls`, `tabIndex`, testids and `data-tone` unchanged; the jsdom path (ResizeObserver guard) untouched. components.css tabs section:

```css
@media (hover: hover) { .tab:hover:not([aria-selected="true"]) { color: var(--text-0); border-bottom-color: var(--line-strong); } }
.tabs[data-overflow="end"] { mask-image: linear-gradient(to right, #000 85%, transparent); }     /* replaces the bare [data-overflow] rule */
.tabs[data-overflow="start"] { mask-image: linear-gradient(to right, transparent, #000 15%); }
.tabs[data-overflow="both"] { mask-image: linear-gradient(to right, transparent, #000 15%, #000 85%, transparent); }
.tabs[data-variant="sub"] { display: inline-flex; align-self: flex-start; max-width: 100%; gap: 2px; padding: 2px; border: 1px solid var(--line); border-radius: var(--radius); background: var(--bg-2); }
.tabs[data-variant="sub"] .tab { height: max(28px, var(--tap-min)); padding: 0 var(--sp-3); border-bottom: 0; border-radius: var(--radius-sm); font-size: var(--fs-sm); }
.tabs[data-variant="sub"] .tab[aria-selected="true"] { background: var(--bg-3); color: var(--text-0); box-shadow: inset 0 0 0 1px var(--line-strong); }
```

No visible change until P7-2 passes `variant="sub"` from LendPanel.

**Files.** `src/components/Tabs.tsx`, `src/styles/components.css`.

**Why.** Tabs have no hover state at all on a PC; 'Short | Lend' and 'Deposit | Withdraw | Claim' use one identical underline style 46 px apart, so the sheet opens on two rows of eight equal options; scrolled to the end, the tab row hard-clips 'Le|nders' on the left with no fade.

**Watch out.** `.tab` is a PRIMARY control: `max(28px, var(--tap-min))` gives 44 under coarse. Nothing in src tests or e2e asserts `data-overflow`. The selected sub-tab carries `aria-selected` plus a fill and an inset ring (a shape cue, like today's underline).

#### P3-7 — Field: the error and the hint together (medium)

**Change.** Field.tsx: render `{error && <div id={errId} className="field-error" role="alert">{error}</div>}` and then `{hint && <div id={hintId} className="field-hint">{hint}</div>}` — drop the `!error` guard (`aria-describedby` already lists both ids). components.css: `.field-error + .field-hint { margin-top: calc(-1 * var(--sp-1) + 1px); }`.

**Files.** `src/components/Field.tsx`, `src/styles/components.css`.

**Why.** An error replaces the very hint the user needs to fix the input: 'Above the balance of this account.' removes the balance, a size error removes 'Max now 4,716,275.645', 'More shares than you hold.' removes the holdings — the error is shown twice (field and foot) and the limit zero times.

**Watch out.** Error-then-hint is the new DOM order and the reading order. components.test 'AmountInput' asserts the alert text and its absence only; no src test or e2e asserts a hint's absence.

#### P3-8 — Badges: grey tags, a bounded neutral, capitalised enums (medium)

**Change.** Badge.tsx: add `className?: string`, merged after `badge` (keeps `data-tone`, `title`, `data-testid`). components.css badges section:

```css
.badge[data-tone="info"] { background: var(--tag-bg); color: var(--tag); }
.badge[data-tone="neutral"] { background: var(--bg-2); box-shadow: inset 0 0 0 1px var(--line-strong); }
.badge-status { text-transform: capitalize; }   /* single-word enum badges only: position status, indexer state, settle condition; P5 / P6 pass the class and P6 removes .exit-badge's own capitalize */
```

Do not touch `.badge.tier`.

**Files.** `src/components/Badge.tsx`, `src/styles/components.css`.

**Why.** Every 'Mock (test)' row pill and the 'open' / 'Market creator' badges are link blue, so static tags look clickable dozens of times per screen; the neutral badge (bg-3 on a bg-1 row, 1.18:1) has no visible bound, so 'Lend only' reads as stray text; 'open' / 'live' sit beside 'Safe' / 'Enabled' in two casings.

**Watch out.** `capitalize` is the permitted trivial casing; Playwright's `toHaveText` / `getByText` compare textContent, so 'open' assertions are unaffected. Look at a filled info tag and a ringed neutral side by side in the VENUE column at 1440 and in the phone cards before merging.

#### P3-9 — One grammar for toned boxes (medium)

**Change.** Replace the `.banner` block (~l.349–388) with:

```css
.banner { border: 1px solid var(--line-panel); border-left: 3px solid var(--text-2); border-radius: var(--radius); background: var(--surface-note); }
.banner[data-tone="warn"]   { border-left-color: var(--warn);   background: var(--warn-bg); }
.banner[data-tone="danger"] { border-left-color: var(--danger); background: var(--danger-bg); }
.banner[data-tone="info"]   { border-left-color: var(--info);   background: var(--info-bg); }
.banner[data-tone="ok"]     { border-left-color: var(--ok);     background: var(--ok-bg); }
```

(drop the full-border tone colours; padding unchanged so nothing moves at the gate; keep the phone `flex-wrap` rule). Banner = short notice with tint and bar; disclosure = long text with bar, no tint; toasts and `.mk-reason` already follow it.

**Files.** `src/styles/components.css`.

**Why.** The green full-bordered 'Healthy…' banner sits directly above red and blue 3 px left-bar disclosures; the same coverage message appears as a left-bar box in the page and a full-border box in the trade panel 400 px apart.

**Watch out.** Testids and roles stay (`position-health` role=status, `T.txError`, `T.lendSweepNotice`, the market banners `focusBanner` targets); the only feature override is `.short-blocked .banner { margin-bottom: 0 }`. Severity is still worded in the text, so the bar is not a colour-only cue.

#### P3-10 — Glossary marks, the Reveal, copy and external glyphs (medium)

**Change.** glossary section: `.glossary-tip { color: var(--icon-muted); vertical-align: calc(0.36em - 6px); } .glossary-tip-icon { width: 12px; height: 12px; } .tbl th button.glossary-tip { color: var(--icon-muted); }` (the section comment derives the vertical-align from the icon size; it was `calc(0.36em - 6.5px)` for the larger icon); hover (`--text-0`, background → `var(--bg-4)`) and `[aria-expanded="true"]` (`--info`) stay; the 24 px box and the sideways-only `.hit` areas are untouched. Glossary.tsx: the popover's close button renders `<NavIcon name="close" className="icon" />` (P3-3's shared rule sizes it; `T.glossaryClose` and the 'Close' aria-label stay). Reveal section:

```css
.reveal { align-items: center; min-height: 24px; border-bottom: 0; text-decoration: underline dotted var(--line-strong); text-underline-offset: 3px; }  /* the box the gate passes today; the underline now sits on the text's baseline */
.reveal-pop { box-shadow: var(--shadow-pop); border-color: var(--line-panel); }
.reveal-pop::backdrop { background: var(--bg-overlay); }
```

No `.reveal.hit::after` rule and no `hit` class on the Reveal. Reveal.tsx: '↗' → `<NavIcon name="external" className="icon-sm" />`. Address.tsx: the '⧉' copy glyph → `<NavIcon name="copy" className="icon-sm" />`, the copied state → `<NavIcon name="check" className="icon-sm" />` (aria-labels 'Copy …' / 'Copied' unchanged); `.address-text .btn { color: var(--text-2); }`, `--ok` while copied, and inside `@media (hover: hover)` `.address-text .btn:hover { color: var(--text-1); }`.

**Files.** `src/styles/components.css`, `src/components/Glossary.tsx`, `src/components/Reveal.tsx`, `src/components/Address.tsx`.

**Why.** Six ⓘ per header row and one on half the kv rows are typeset at label tone and weight, so they double the noise of every list and an orphaned one looks like a broken header; the Reveal's dotted underline sits 8 px under the glyphs on a 24 px box so the market header's second line has three baselines; copy and explorer are ~9 px font glyphs that depend on the platform font.

**Watch out.** The gate's under-24 check measures the control's own rect, never a `.hit::after` area, and `.reveal { all: unset }` leaves it unpositioned anyway. `--icon-muted` is ≥ 3:1 on bg-0/1/2 (4.68 / 4.39 / 4.10) for an icon-only control (WCAG 1.4.11). Address.tsx keeps its `.btn hit` sm box (`--tap-sm` 24 / 36).

**Verification (P3).** Desktop 1440: 10-market-overview (the Short panel's 'Connect wallet' is the only button-shaped control in the foot, the disabled submit a grey caption under it; selects show the app's chevron; the acknowledgement is a bordered bg-3 row; ⓘ marks are dimmer than their labels; the mint's dotted underline hugs the text; the panel title 'PREVIEW' outranks 'MARKET' / 'EXITS'), 40-lend-deposit (the error line above the surviving balance hint; sub-tabs as a segmented row once P7-2 passes the variant), 31-position-detail ('Close position' hovers to a red tint, not grey; the health banner has a 3 px left bar; 'MANAGE' is 13 px white over 'ADD MARGIN' 12 px grey over 'AMOUNT (SOL)' 12 px muted), 21-short-review (the disabled 'Sign' differs from 'Refresh' by colour and weight). Phone 390: trade-sheet-no-wallet (two rows: CTA + caption, 44 px each), trade-sheet-lend-withdraw (checked box drawn by the app, pressed 50 % chip filled), market-events (fade on the left edge when scrolled), create-market (no fade strip over the empty page above the fixed foot). Gate: every disabled `.form-foot .btn` ≥ 44 px under coarse at 320–844; `input` / `select` ≥ 44 under coarse; `.reveal` ≥ 24 everywhere; axe clean. e2e: mobile-trade.spec:221–222 and mobile-lend.spec:254 unchanged (texts and visibility kept); components.test.tsx, responsive.test.tsx, Glossary.test.tsx pass.

---

### P4 — Tables, key/value lists, stat strips and numbers

**Goal.** Dense data that reads at a glance: zeros quiet, real numbers loud, headers that stick and sit on the label tier, a visibly sorted column, rows that answer the pointer, decimals aligned in SOL columns, and one 28 px row grid. P4 also defines the `Stat` `hero` prop (moved here from P8-4 so it exists before P7 consumes it).

**Owned files.**
- `src/styles/components.css` — sections 'key / value grid (stats, previews)' including the stat strip rules, 'table', 'numbers'
- `src/components/DataTable.tsx`, `src/components/Num.tsx`, `src/components/States.tsx` (the `Stat` prop only), `src/lib/format.ts`
- `src/components/responsive.test.tsx` (two textContent expectations)

Tokens consumed (P1): `--bg-hover`, `--line-panel`, `--label-fs/-fw/-ls/-color`, `--kv-row-h`, `--lh-tight`. Icons (P2-1): `sort`, `chevron`.

Package contract for later packages: every plain `.num` zero in a table cell also carries `data-zero=""` (P5 sets it on UtilBar's percentage span and on the open-positions count; P7 on any plain-span zero in the lend table) so a row never mixes grey and white zeros; `pad` goes on SOL columns and only on token columns that show fractional amounts (whole-token quantities such as the lender's VALUE stay unpadded); P6 moves `.position-facts` rows to `--kv-row-h`.

**Items.**

#### P4-1 — Zeros quiet (high)

**Change.** Num.tsx: `Sol` emits `data-zero=""` on its `.num` span when `lamports === 0n`, `Tokens` when `raw === 0n`, `Bps` when `value === 0 || value === 0n` (value is `number | bigint`); `Pnl` flat already uses `--flat`. components.css numbers section, placed **before** the `.pnl[data-sign]` rules (equal specificity, source order decides):

```css
.num[data-zero] { color: var(--text-2); }
.num[data-zero] > .unit { color: inherit; }
.btn .num[data-zero] { color: inherit; }   /* 'Claim all · <Sol>' can read 0 SOL inside the gold primary button: text-2 on accent is 1.7:1 */
```

`.kv dd[data-zero]` stays.

**Files.** `src/components/Num.tsx`, `src/styles/components.css`.

**Why.** In the markets list six of eight rows read '0 SOL / 0% / 0 SOL / 0' in full white at the weight of the one busy market's 301.14 SOL; the Lenders tab is four rows of white zeros; the kv lists on the same pages already mute zeros. Muting is not hiding: text-2 is ≥ 5.08:1 on every surface.

**Watch out.** The markets row's `0%` comes from UtilBar and `0 · 0` from CompactTokens (markets/shared/cells.tsx), not from `Bps` / `Tokens` — P5-5 adds their `data-zero`, or a row reads grey '0.00 SOL', white '0%'.

#### P4-2 — Rows answer the pointer (high)

**Change.** table section, inside the existing `@media (hover: hover)` block: `.tbl tbody tr:hover { background: var(--bg-hover); }`. Outside it: `.tbl tbody tr[data-interactive="true"]:focus-visible { outline: 2px solid var(--focus); outline-offset: -2px; }` (the inset keeps the ring unclipped under `.tbl-wrap { overflow: auto }` below 1024 px). Leave the existing `.tbl tbody tr[aria-selected="true"]` rule untouched (dead CSS: DataTable never sets `aria-selected`; only Tabs and SearchBox do). kv section: delete the existing invisible `.kv[data-lines] .kv-row:hover > *` rule (~l.564) rather than strengthen it — kv rows are read-only and must not look clickable.

**Files.** `src/styles/components.css`.

**Why.** Row hover is bg-2 on bg-1 (1.07:1): every markets / positions / lenders row is `data-interactive` yet does not visibly respond, and the focused row cannot be tracked.

**Watch out.** `--bg-hover` is P1's token for exactly this; it equals `--bg-3` and the base press rule, so hover and press on a row are one colour (the focus ring is the second cue). If the lift reads too faint at 1440, retune `--bg-hover` in tokens.css (≤ #262d36 so `--text-2` keeps 4.5:1 on muted zeros in a hovered row); never point rows at `--bg-4`. A `box-shadow` on a `tr` in a collapsed table is not painted by WebKit — none is used.

#### P4-3 — Table header: the label tier, the sorted column, sort icons, the empty list, a header that sticks (high)

**Change.** components.css table section — in the `.tbl th { position: sticky; … }` rule at ~l.930 (it follows the `(max-width: 1279.98px)` block whose `padding` shorthand would otherwise reset an earlier `padding-block` below 1280 px), add:

```css
z-index: 1;   /* positioned row content later in the DOM (.btn.hit copy buttons, .glossary-tip.hit) must not paint over a stuck header */
font-size: var(--label-fs); font-weight: var(--label-fw); letter-spacing: var(--label-ls); color: var(--label-color);
line-height: var(--lh-tight); padding-block: var(--sp-1);   /* a 24 px sort button + 2 + 2 = 28 px: the header row stays on the grid; vertical-align stays middle */
```

Sorted column: `.tbl th[aria-sort] button { color: var(--text-0); } .tbl th[aria-sort] { box-shadow: inset 0 -2px 0 var(--accent); }`. Sort marks — DataTable.tsx renders `<NavIcon name="sort" className="sort-hint" />` for a sortable unsorted column, `<NavIcon name="chevron" className="sort-dir" />` for the active column and before 'Ascending' / 'Descending' in the card sort bar (words and `data-order` unchanged; ▲/▼ stay P&L-only):

```css
.tbl .sort-hint, .tbl .sort-dir, .tbl-sortbar .sort-dir { width: 12px; height: 12px; opacity: 1; color: var(--text-2); }
.tbl th[aria-sort] .sort-dir { color: var(--text-1); }
[data-order="asc"] .sort-dir, .tbl th[aria-sort="ascending"] .sort-dir { rotate: 180deg; }
@media (hover: hover) { .tbl th button:hover .sort-hint { color: var(--text-1); } }
```

Update `src/components/responsive.test.tsx` l.73 to `'Order: Descending'` and l.82 to `'Order: Ascending'` (the arrow glyph was aria-hidden and leaves the button's textContent; the visually-hidden 'Order: ' stays). Leave `.tbl-sortbar`'s padding as it is (it shares the 12 px left edge with the card titles). Empty list (DataTable renders `.empty` as the table's sibling inside `.tbl-wrap`):

```css
.tbl-wrap:has(> .empty) > .tbl > thead, .tbl-wrap:has(> .empty) > .tbl-sortbar { display: none; }
.tbl-wrap > .empty { padding: var(--sp-7) var(--sp-5); }   /* loading keeps header + skeleton rows */
```

A sticky header that sticks (every table fits without sideways scroll at 1024 / 1280 / 1440 per §7.3; the positions and lend tables, whose rows carry buttons, are excluded until a tap-cover run on a long seeded list clears them; the review's deltas table has no `.tbl-wrap`, so the selector never reaches it):

```css
@media (min-width: 1024px) {
    .page .tbl-wrap:not(.positions-tbl, .lend-table) { overflow: visible; }
    .page .tbl-wrap:not(.positions-tbl, .lend-table) .tbl th { top: var(--header-h); }
    .page .tbl-wrap:not(.positions-tbl, .lend-table) .tbl tbody :focus-visible { scroll-margin-top: calc(var(--header-h) + 3 * var(--row-h)); } /* a Shift+Tab-focused row or in-row control clears a stuck header up to three lines tall (base.css clears only the app header) */
}
```

**Files.** `src/styles/components.css`, `src/components/DataTable.tsx`, `src/components/responsive.test.tsx`.

**Why.** At 1024 the header row is ~56 px with five label heights and ⓘ marks dangling under single words; the active sort 'TVL ▼' is a 20-unit grey step among seven ⓘ and four ↕ specks and reuses the P&L glyph; `.tbl-wrap { overflow: auto }` makes the wrap the th's scroll container, so the promised sticky header never sticks and a 30-row list on a laptop is unlabeled numbers one scroll down; eleven header controls frame a one-line empty state.

**Watch out.** The bottom-aligned header (vertical-align: bottom with 4 / 3 px padding) is deferred: a sort button is a 24 px box while a plain header is a 15 px line, so at 1440 plain headers would sit 4–5 px lower than sortable ones. A stuck thead with sort buttons is judged by `e2e/focus-obscured.ts` (not in its BARS list) — the scroll-margin rule is what keeps focus.spec's markets walk passing; with 8 seeded markets no desktop viewport scrolls the list, so test the walk on a long seeded list. components.test.tsx's `columnheader { name: /TVL/ }` still passes.

#### P4-4 — Card hero, padded decimals, slot and age formatting (medium)

**Change.** DataTable.tsx `cellAttrs(c, sort)`: add `'data-sorted': sort?.key === c.key ? '' : undefined` on th and td. components.css inside the existing `@container tbl (max-width: 559.98px)` block: `[data-cards] > .tbl > tbody > tr > td[data-sorted] { font-size: var(--fs-lg); font-weight: var(--fw-medium); } [data-cards] td[data-sorted] .num { font-size: inherit; }` (the `::before` label stays 12 px). Num.tsx: add `pad?: boolean` to `Sol` and `Tokens` — `Sol` calls `formatSol(lamports, frac, pad ? frac : 0)`; format.ts gives `formatTokens` a fourth parameter, `formatTokens(raw, decimals, maxFrac = Math.min(decimals, 4), pad = false)` returning `formatFixed(raw, decimals, maxFrac, pad ? maxFrac : 0)`, and `Tokens` calls `formatTokens(raw, decimals, frac, pad)` (defaults unchanged, so format.test.ts holds). format.ts: add `formatSlot(n: bigint | number | null): string` (grouped with `toLocaleString('en-US')`, '—' for null) and `formatAge(seconds: number | bigint): string` ('45s' under a minute, then '11m', '3h 12m', '2d 4h'); `formatDuration` and `Countdown` untouched. Add unit tests for `formatSlot` / `formatAge` and the `pad` path.

**Files.** `src/components/DataTable.tsx`, `src/components/Num.tsx`, `src/lib/format.ts`.

**Why.** A card is scanned by one number, but the markets cards show PRICE/TVL/UTILISATION/OI/POSITIONS at one 13 px weight while the sort bar says 'Sort by TVL'; right-aligned SOL columns read '301.14 / 67.32 / 60 / 15 / 10.1' with decimal points at four offsets, defeating tabular-nums; slots are formatted four ways and ages churn seconds that refresh every 15–30 s.

**Watch out.** Any later package that passes `pad` re-checks e2e `toHaveText` on that column (lend-claim-sync.spec.ts:102–164 and mobile-lend.spec.ts:200–203 assert exact '… 0 SOL' strings; those come from ClaimLabel's `formatSol` and are unaffected unless padded). The review's delta rows are not padded (P9-4).

#### P4-5 — kv rows on the 28 px grid, with an emphasis hook (medium)

**Change.** kv section:

```css
.kv[data-lines] dt, .kv[data-lines] dd { min-height: var(--kv-row-h); padding-block: calc((var(--kv-row-h) - 1lh - 1px) / 2); }  /* 28 px rows, inline flow kept (text-align: right survives, wrapped values grow); 1lh is already used in this file */
.kv dd[data-emph], .kv dd > [data-emph] { color: var(--text-0); }                  /* colour on the dd, so .pnl[data-sign] and .cr[data-risk] children keep theirs */
.kv dd[data-emph] > .num, .kv dd > [data-emph] > .num { font-size: var(--fs-md); font-weight: var(--fw-medium); }
.kv dd[data-emph] .unit, .kv dd > [data-emph] .unit { font-size: var(--fs-xs); }
.kv dt:has(+ dd[data-emph]), .kv dt:has(+ dd > [data-emph]) { color: var(--text-0); }
```

The `dd > [data-emph]` variants let a form that renders its rows through `Kv` (which has no attribute slot) wrap the value in `<span data-emph>` (P7-2); P6 sets the attribute on its own `dd`s (Total cost, CR at open, Liquidation price, You receive, P&L), P7 on Shares minted and You may withdraw now.

**Files.** `src/styles/components.css`.

**Why.** Hairline kv rows are 26.4 px (12 × 1.45 + 8 + 1) against 28 px table rows in the same tab, so long lists step 26/27 px; the trade ticket's decisive numbers are typeset exactly like 'Minimum rent 0.000014 SOL'. A shared emphasis hook keeps the preview's hierarchy a kv rule, not a feature one-off.

**Watch out.** No `display: flex` on dt/dd: it would blockify inline children and drop the whitespace between a `<Sol/>` and a following span. The size and weight sit on the `> .num`, not the dd, so a sub-line inside the same dd (`.short-preview-sub`) does not go medium.

#### P4-6 — Stat strip: dividers, label tier, the hero size (medium)

**Change.** stat strip rules in the kv section:

```css
.stat { background: transparent; }   /* each cell's 1 px hairlines now show between columns and rows (the strip paints bg-1 and clips the outer edges) — matching the position hero */
.stat-strip { border-color: var(--line-panel); }
.stat-label { font-size: var(--label-fs); font-weight: var(--label-fw); letter-spacing: var(--label-ls); color: var(--label-color); }
.stat[data-hero] .stat-value { font-size: var(--fs-xl); font-weight: var(--fw-medium); }   /* the attribute comes from P4-7's Stat hero prop; P7-1 marks the lender's SOL claimable */
```

**Files.** `src/styles/components.css`.

**Why.** The position hero shows cell dividers while the market and lender strips hide theirs under each cell's opaque background; the lender's headline number ('SOL claimable 0.007283 SOL') is the same 15 px as 'Markets 1'.

**Watch out.** `--fs-xl` is 20 px under 640 px; P7-1 caps the hero at `--fs-lg` under `(max-width: 399.98px)` in lend.css so a 320 px 2-up cell never clips `0.016481 SOL` inside the strip's `overflow: hidden`. The 1,280 px nowrap fit of `.mk-stats .stat-label` with the 500 weight is handled by its existing `flex-basis: clamp(…)`; the gate's 'no stat alone on a row' check catches a regression.

#### P4-7 — `Stat` gains the `hero` hook (medium; moved here from P8-4)

**Change.** States.tsx `Stat`: add `hero?: boolean` and emit `data-hero={hero ? '' : undefined}` on the `.stat` div. `Empty`, `ErrorState` and `Kv` markup unchanged (P8 owns the rest of States.tsx's behaviour but makes no further edit to the file).

**Files.** `src/components/States.tsx`.

**Why.** A strip needs a hierarchy of its own; a generic hero hook lets the lender's claimable KPI — and later the market's RATE (APR) — step up without a feature override. It is defined here, beside the CSS that styles it, so P7 can consume it.

**Watch out.** `data-hero=""` only when `hero` is true; markets/responsive.test reads `.stat-label` text and `dataset.priority`, responsive.spec.ts:910 reads `.mk-stats > .stat` positions — none changes.

**Verification (P4).** Desktop 1440: 01-markets (zeros grey, the 'E2EMAIN' row's numbers white; 'TVL' header white with a 2 px gold underline; headers on the label tier in a 28 px row; hovering a row visibly lifts it; scrolling a long seeded list keeps the header under the app header), 11-market-tab-lenders (zero columns quiet), 10-market-overview (hairlines between the seven stats; kv rows land on y multiples of 28), 02-markets-search-no-market (no header row or sort bar over the empty message). 1024: 16-markets-1024 (headers wrap as before, on the tier). Phone 390: markets-cards (TVL is the 16 px hero of each card; the sort bar reads 'Descending' with a chevron), positions (CR hero unchanged). Gate: h-scroll 0 at 1024 / 1280 / 1440 with the sticky header; tapCovers clean with the header stuck; focus.spec's markets walk on a seeded list long enough to scroll. Unit: format.test.ts unchanged plus the new formatSlot / formatAge / pad tests; responsive.test.tsx with the two updated expectations; components.test.tsx.

---

### P5 — Markets list, market page, create market, protocol status

**Goal.** The markets list shows a market on the first phone screen and makes rows obviously openable; the market header sits on one baseline with the price as the hero; a market that cannot be shorted opens on Lend; the create and status pages lose their stretched, half-empty look.

**Owned files.**
- `src/features/markets/markets.css`
- `src/features/markets/MarketsPage.tsx`, `MarketsIntro.tsx`, `MarketPage.tsx`, `CreateMarketPage.tsx`, `ProtocolStatusPage.tsx`
- `src/features/markets/market/MarketHeader.tsx`, `StatStrip.tsx`, `OverviewTab.tsx`, `HistoryTabs.tsx`, `CreatorDeposits.tsx`, `TierWhyPopover.tsx`, `MarketBanners.tsx`
- `src/features/markets/shared/cells.tsx`, `shared/marketFlags.ts`
- one-liners outside the list: `src/config/testids.ts` (P5-7), `src/styles/breakpoints.ts` comment (P5-1, only with the owner's yes)

Consumed: tokens `--label-fs/-fw/-ls/-color`, `--fs-hero`, `--meter-track`, `--surface-nested`, `--line-panel`, `--shadow-pop`, `--tag`, `--tag-bg`, `--side-col`, `--page-gap`, `--form-measure` (P1); icons `close`, `copy` + `.icon` / `.icon-sm` (P2); `Badge className`, `.badge-status`, the shared close rule, drawn radios (P3); `Sol pad`, `.num[data-zero]`, `formatAge`, `formatSlot` (P4). Write `var(--token, <today's value>)` where the items say so.

Screenshots that change by design: 13 / 14 and the gate's paused / t22 shots (P5-2), 01 / 16 and the phone markets shots (P5-1, P5-4, P5-5), 02 and 51 (P5-7), 05 / 06 / 07 (P5-6), 03 (P5-8), 10's header baselines (P5-3).

**Items.**

#### P5-1 — Phone markets first screen (high; one owner decision inside)

**Change.** markets.css, in the `(max-width: 639.98px)` block — the toolbar on one row, merged into the sort bar:

```css
.mk-toolbar { flex-wrap: wrap; border-bottom: 0; padding-bottom: var(--sp-2); }  /* keep wrap: at 320 px the count wraps to its own right-aligned line */
.mk-search { flex: 1 1 140px; min-width: 0; }
.mk-filters-toggle { flex: none; }
.mk-count { flex: none; margin-left: auto; }
```

(the open `.mk-filters` keeps `flex: 1 0 100%`). Do not add the intro-type rules (`.mk-intro-part p:not(.mk-intro-links) { font-size: var(--fs-sm) }` is already the base rule at l.44–49; the two margin rules save 8 px for a tighter recipe than the desktop strip). Measured saving ≈ 50–60 px: the first card's price / TVL, utilisation / OI and positions rows land above the status strip at 390 × 844.

**OWNER DECISION — implement only with an explicit yes** (it extends the 2026-10-11 short-phone decision to every phone): add `(max-width: 639.98px)` as a third comma-separated query to the compact-strip prelude at markets.css l.94–95, **keeping** `(max-width: 399.98px) and (max-height: 699.98px)` in it (breakpoints.test.ts l.90 requires markets.css to still contain that exact query), so every phone keeps the heading row, 'What can go wrong', its sentence and its two links (the two explanatory parts are one tap away on How it works); with it the first card lands at ≈ y 430. In the same change update the l.88–93 comment and the breakpoints.ts l.163–175 wording; FRONTEND.md §3.3 l.274–280 and §7.6 l.2012–2016 are P10's. Without the yes, leave l.94–95 untouched. `data-testid` marketsIntro and marketsIntroLink(*) stay in the DOM either way.

**Files.** `src/features/markets/markets.css` (+ `src/styles/breakpoints.ts` comment with the yes).

**Why.** At 390 × 844 the intro strip (y 112–430), a three-row toolbar and the sort bar push the first card to y 620 and the status strip clips it at 760: the user taps 'Markets' and sees three paragraphs and three controls, no market.

**Watch out.** Replacing the short-phone query with the phone query (it subsumes it) fails breakpoints.test.ts — add, never replace. The 844 × 390 landscape viewport must keep the filter on the first screen.

#### P5-2 — Default the trade panel to the side that works (high)

**Change.** marketFlags.ts: `export const SHORT_BLOCKED: ReadonlySet<MarketFlagKey> = new Set(['paused-opens', 'coverage-paused', 'route-broken', 'list-only', 'migrating']);` (the states where `openBlocked` returns a blocking code; 'warming-up', 'pool-window', 'graduation-halted', 'mint-unsafe' and 'venue-upgraded' stay shortable by default — short-lived or partial states, or ones where the Lend side is not verified to work). MarketPage.tsx: `const canShort = !data.flags.some((f) => SHORT_BLOCKED.has(f)); const panel = search.panel ?? (canShort ? 'short' : 'lend');` (an explicit `?panel=short` still wins — router.tsx l.139 validates it; the Short tab, its notice and 'Read the full coverage notice' remain). Action bar: `data-unavailable={canShort ? undefined : ''}` on the Short button (testid, aria-label, click → sheet unchanged) and `data-variant={canShort ? undefined : 'primary'}` on the Lend button. markets.css:

```css
.btn.mk-action[data-variant="short"][data-unavailable] { background: var(--bg-2); border-color: var(--line-strong); color: var(--text-1); }
@media (hover: hover) { .btn.mk-action[data-variant="short"][data-unavailable]:hover:not(:disabled) { background: var(--bg-3); filter: none; } }
```

(the `.btn.` prefix and the explicit hover rule keep P3-3's `.btn[data-variant="short"]:hover` from turning the unavailable button red again on a hovering tablet). The banner above already states the reason in words.

**Files.** `src/features/markets/MarketPage.tsx`, `src/features/markets/shared/marketFlags.ts`, `src/features/markets/markets.css`.

**Why.** On a lend-only or coverage-paused market the panel opens on Short with a red/amber notice and an empty column while the Lend tab — the only thing one can do — is unselected, and the phone action bar still offers the loudest control on the page for the one action that cannot happen.

**Watch out.** coverageLink.test.tsx's phone case taps the action bar's Short, which sets `?panel=short` explicitly (sheetParam.ts l.33) and still gets the Short panel; MarketPage.test.tsx l.102 and router.test.tsx:78 assert the Short panel on an unflagged market; screens.spec:127 and responsive.spec:957 assert only the flag badge on t22. Re-run MarketPage.test.tsx, coverageLink.test.tsx, router.test.tsx and accept the new 13 / 14 shots.

#### P5-3 — Market header baselines and section typography (medium)

**Change.** markets.css:

```css
.mk-head { align-items: flex-end; align-items: last baseline; }
.mk-head-prices { align-items: flex-end; align-items: last baseline; }
.mk-head-sub { align-items: baseline; }
.mk-head-price { font-size: var(--fs-hero); line-height: var(--lh-tight); }   /* 22 px desktop / 24 phone: the headline price outranks the 20 px title (P1-1); was --fs-xl */
.mk-head-price-label { font-size: var(--label-fs, var(--fs-xs)); font-weight: var(--label-fw, var(--fw-medium)); letter-spacing: var(--label-ls, 0.04em); color: var(--label-color, var(--text-2)); }
.mk-banners { gap: var(--sp-3); }
```

Remove `.mk-intro-part h3`'s own `font-size` (l.35) so it inherits the section tier from base h3; `.mk-section-title`, `.mk-creator h3` and `.mk-popover-head h3` carry no size / weight / tracking overrides today (margins only) — nothing to remove; `.mk-intro-risk-title { color: var(--warn) }` stays (now amber). Keep `.mk-head-title h1 { font-size: var(--fs-xl) }` at l.808–810 (the phone symbol stays 20 px). Desktop ladder: symbol 20 (h1), EMA price 22 (hero), stats 15, rows 12.

**Files.** `src/features/markets/markets.css`.

**Why.** The hero line of every market page shows three baselines: the name (13 px / 1.45), the mint inside a 24 px Reveal box and the 18 px price align box-bottoms of blocks with different line-heights; six uppercase-label recipes sit side by side in one header; and the price a trader reads first was the same size as the title.

**Watch out.** `last baseline` is right because `.mk-head` and `.mk-head-prices` are column flex items whose first baseline would be the label, not the number. The phone block (l.796–806) keeps the stacked column; check phone/market for the 24 px price in the stacked head.

#### P5-4 — Markets rows that look openable, venue pills quiet (medium)

**Change.** MarketsPage.tsx token column: wrap `TokenLabel` in `<Link to="/m/$mint" params={{ mint }} className="target-link" onClick={(e) => e.stopPropagation()}>` exactly as PositionsPage l.166–173 does (DataTable's `fromControl` routes the click to the link; the row stays activatable by Enter and elsewhere). markets.css:

```css
.mk-tbl td[data-col="token"] > a .mk-token-symbol { color: inherit; }   /* .mk-token-symbol sets --text-0 (l.367) and would beat the link's blue; also fixes the status page's lists, which are already links. --info on bg-1 = 6.96:1 */
.mk-markets-tbl td[data-col="venue"] .badge[data-tone="info"] { background: transparent; padding: 0; color: var(--text-1); font-weight: var(--fw-regular); box-shadow: none; }
@container tbl (max-width: 559.98px) {
    .mk-markets-tbl td[data-col="venue"] .badge[data-tone="info"] { background: var(--tag-bg, var(--bg-3)); color: var(--tag, var(--text-1)); padding: 0 var(--sp-2); }
}
```

(Migrating stays warn, Lend only stays the neutral pill.) Pass `pad` on the tvl and oi `Sol` cells (P4-4).

**Files.** `src/features/markets/MarketsPage.tsx`, `src/features/markets/markets.css`.

**Why.** Positions and Lending teach that a blue symbol opens the market, then the markets list — the one whose rows open — shows plain bold text with a 1.15:1 hover and nothing at all on touch; eight identical saturated venue pills out-shout the money columns.

**Watch out.** Without the `color: inherit` rule nothing visible changes (03-status proves it: its list already wraps TokenLabel in a Link and 'E2EPAUSE' renders white). The gate's tap-cover check skips descendant / ancestor controls, so a link inside an interactive row never counts as a cover; `.target-link` is 24 px (36 under coarse). One extra Tab stop per row, as PositionsPage already pays — re-run focus.spec / mobile-focus.spec; markets.spec.ts:89 (Enter on the row) still passes.

#### P5-5 — Utilisation meter and cell data (medium)

**Change.** markets.css: `.mk-util-track { background: var(--meter-track, var(--bg-3)); } .mk-util-fill:not([data-empty]) { min-width: 2px; }`. No stat-strip stacking: the strip keeps bar → number on one row (DOM order bar → value; a column would drop the value's baseline ≈ 6 px below 'Rate' and 'TVL' beside it). cells.tsx: `UtilBar` sets `data-empty` on the fill and `data-zero` on its `.num` when `bps === 0` (P4-1's `.num[data-zero]` mutes it); plain counts rendered as `<span className="num">` — Positions (MarketsPage.tsx ~l.134), Proposals (HistoryTabs.tsx ~l.400), Lenders (StatStrip.tsx ~l.164) — get `data-zero={n === 0 ? '' : undefined}`; `Age` uses `formatAge` (UTC in `title`, `data-value` unchanged). HistoryTabs.tsx: `pad` on its SOL columns (v, m, paid, buyback, lenders, keeper, borrower).

**Files.** `src/features/markets/markets.css`, `src/features/markets/shared/cells.tsx`, `src/features/markets/market/HistoryTabs.tsx`.

**Why.** The 48 × 6 px utilisation track is a 1.18:1 smear with a ≤ 2 px fill at 0.7 %, so it costs 56 px per row and tells nothing; age columns show seconds that are up to 30 s stale.

**Watch out.** `UtilBar`'s `style={{ width }}` is CSSOM via React (allowed, as Skeleton's comment notes) and is unchanged. No unit or e2e test asserts the age text.

#### P5-6 — Create market: the shared side column, a form measure, hairline facts, drawn radios (medium)

**Change.** markets.css:

```css
.mk-create { grid-template-columns: minmax(0, 1fr) var(--side-col, 360px); gap: var(--page-gap, var(--sp-3)); }  /* drop the 380 / 340 literals and the (max-width: 1179.98px) override at l.876–879 (P1-4's --side-col is 320 under 1,180 px); keep the (max-width: 1023.98px) one-column rule at l.1001 */
.mk-create .panel-body .field, .mk-create .panel-body .kv, .mk-create .mk-choice, .mk-create .mk-reasons { max-width: var(--form-measure, 64ch); }  /* never .tbl-wrap: the pools DataTable stays full width */
```

(or add P2-7's `layout-side` class to the element in CreateMarketPage.tsx and delete the grid rule — same columns). CreateMarketPage.tsx `MintInfo`: render the token facts with `KvLines` from `./shared/KvLines.tsx` instead of `Kv` (same rows; `data-testid={T.createMintInfo}` stays on the wrapper; 'Supply 0' then reads muted like the Overview's zeros — `Kv` has no `data-lines` prop, so that is the only way to get the hairlines). Delete the radio `accent-color` rule (l.968–971) and the coarse 20 px radio rule (l.979–982): P3-4 draws radios from `--check`, which is 20 px under coarse.

**Files.** `src/features/markets/markets.css`, `src/features/markets/CreateMarketPage.tsx`.

**Why.** 'Decimals … 6' and 'Supply … 0' put ~900 px between label and value with no hairline, the 44-character mint input and a single radio span a 1,030 px panel, and the right column is 20 px wider than the trade and Manage panels for no visible reason.

**Watch out.** With `--side-col` undefined the grid is invalid and the page collapses to one column — the fallback is there for that. CreateMarketPage.test.tsx l.181 / 247 read `createMintInfo` textContent only.

#### P5-7 — Empty and indexer-down states carry their action (medium)

**Change.** `src/config/testids.ts`: add `marketsEmptyCreate: 'markets-empty-create'` (one line; its §11.2 row is P10's). MarketsPage.tsx: when the filter is a mint without a market, render in the DataTable `empty` slot, after the sentence, `<Link to="/markets/new" search={{ mint }} className="btn" data-variant="primary" data-testid={T.marketsEmptyCreate}>Create a market for this mint</Link>` (the approved label); while it shows (`mintWithoutMarket`) give the head link `data-variant="default"` — its text, href and `T.marketsCreateLink` testid unchanged (markets.spec.ts:99 clicks it by testid) — so one gold button is on screen. markets.css: `.mk-open-mint > label, .mk-open-mint-label { font-size: var(--fs-sm); color: var(--text-1); text-transform: none; letter-spacing: 0; }` (casing only; text unchanged). OpenMint's 'Open' button is already `variant="primary"` (it reads neutral in 51-fallback-markets only because it is `disabled={!mint}`) — nothing to change there.

**Files.** `src/config/testids.ts`, `src/features/markets/MarketsPage.tsx`, `src/features/markets/markets.css`.

**Why.** The empty state says 'use "Create a market for this mint"' and the button it names is 1,000 px away at the top right; in the indexer-down state the one thing that works — open a market by mint — is labelled with a 12 px uppercase tracked label where a sentence-length instruction belongs.

**Watch out.** A literal testid string in TSX breaks the test-id contract (every id comes from `config/testids.ts` and is listed in §11.2). `a.btn` is 30 px / 44 under coarse; `.mk-open-mint > label` (0,1,1) beats `.field-label` (0,1,0).

#### P5-8 — Status page and markets surfaces; close, copy and the tier mark (low)

**Change.** markets.css:

```css
.mk-status-pause > .badge { font-size: var(--fs-md); height: 22px; padding-inline: var(--sp-3); }
.mk-status-grid { align-items: start; }   /* panels as tall as their content; the §3.9 l.779 'equal height' sentence is P10's */
.mk-creator > .disclosure { background: var(--surface-nested); border-top-color: var(--line-panel, var(--line)); border-right-color: var(--line-panel, var(--line)); border-bottom-color: var(--line-panel, var(--line)); }  /* never border-color: markets.css loads after components.css and would grey the severity left bar */
.mk-intro { border-top-color: var(--line-panel, var(--line)); border-right-color: var(--line-panel, var(--line)); border-bottom-color: var(--line-panel, var(--line)); }  /* its blue info bar stays */
.mk-popover { border-color: var(--line-panel, var(--line)); box-shadow: var(--shadow-pop, 0 12px 32px rgb(0 0 0 / 55%)); }
```

Drop `.mk-creator .tbl th { background: var(--bg-3) }`; `.mk-banner` is a bare wrapper with no border — leave it out. Delete the `.mk-popover-close` size rules (l.581–586, l.860–865): P3-3's shared rule sizes it (32 px; 44 under coarse). ProtocolStatusPage.tsx: print the confirmed / finalized / guard / deploy slots with `formatSlot` (P4-4); the indexer-state badge gets `<Badge className="badge-status" …>` (P3-8) with one casing path, `{ live: 'Live', lagging: 'Lagging', stale: 'Stale', down: 'Unavailable' }`. TierWhyPopover.tsx: the ✕ renders `<NavIcon name="close" className="icon" />`. MarketHeader.tsx: keep `MintText` (a local component, not `AddressText` — swapping would change the Reveal label and the `reveal-mint` testid) and replace only its '⧉' glyph with `<NavIcon name="copy" className="icon-sm" />` (aria-label 'Copy <mint>' unchanged; m9-review.test.tsx:48 queries by that name). The tier badge keeps its inline SVG ⓘ (`.mk-tier-info`): no chevron and no dotted-underline hover — 'Tier C ▾' inside a badge reads as a tier picker, and §3.4 records 'Tier C ⓘ'. aria-labels and the `market-tier-why` testids unchanged.

**Files.** `src/features/markets/markets.css`, `src/features/markets/ProtocolStatusPage.tsx`, `src/features/markets/market/TierWhyPopover.tsx`, `src/features/markets/market/MarketHeader.tsx`.

**Why.** 'New shorts: Open' is a 12 px badge beside grey prose and a stretched panel leaves 180 px of empty frame; the venue programs table prints '449280566' under 'Confirmed slot 1,619'; the creator box nests bg-2 in bg-2 with a lifted th just to show.

**Watch out.** Tests read the indexer badge's `data-state` only (status.spec:19, ProtocolStatusPage.test:93 / 114); l.71 needs the v4 row to contain '222', which a grouped slot still does. Deleting the popover close's size rules before P3-3's shared rule exists would shrink the ✕ under a finger — P3 lands first.

**Verification (P5).** Phone 390: markets (one toolbar row — filter box, 'Filters ▾', '8 markets'; with the owner's yes the first card's numbers are on the first screen at ≈ y 430, otherwise at ≈ y 530–565), market (the utilisation stat shows bar then value on one baseline; the price at 24 px), coverage-link and the list-only market (the sheet and panel open on Lend; the action bar's Short is grey, Lend gold). Desktop 1440: 10-market-overview (name, mint and price on one baseline; the price 22 px over the 20 px symbol; 'MARKET' / 'EXITS' as the section tier), 01-markets (venue as quiet text, 'Lend only' a bordered pill, token symbols blue links; TVL / OI decimals aligned), 02-markets-search-no-market (a gold 'Create a market for this mint' under the sentence; the head link default), 05-create-market-ok (facts with hairlines within a 64ch measure; the cost column 360 px; app-drawn radio), 03-status ('Open' as a 13 px badge; panels as tall as their content; every slot grouped). 1024: 15-market-1024 header baselines. Gate: 320 px toolbar no sideways overflow; landscape 844 × 390 keeps the filter on the first screen. e2e: screens.spec (13 / 14 re-shot), responsive.spec:957, markets.spec, mobile-markets.spec, mobile-guide.spec, status.spec, focus / mobile-focus (one more Tab stop per markets row); unit: MarketPage.test.tsx, CreateMarketPage.test.tsx, router.test.tsx, coverageLink.test.tsx, breakpoints.test.ts.

---

### P6 — Short: trade ticket, position page, positions dashboard, Manage panel

**Goal.** The numbers a short decides on are unmissable before the signature; the position page leads with its CR; the dashboards share one KPI hierarchy; the Manage panel scrolls visibly and its two forms read as two.

**Owned files.**
- `src/features/short/short.css`
- `src/features/short/ShortPanel.tsx`, `PositionsPage.tsx`, `PositionDetailPage.tsx`, `CapsSummary.tsx`
- `src/features/short/positions/AddMarginForm.tsx`, `CloseForm.tsx`, `SettlementRisk.tsx`

Consumed: tokens `--surface-well`, `--line-panel`, `--label-fs/-fw/-ls/-color`, `--fs-hero`, `--kv-row-h`, `--field-narrow-w` (P1); `.layout-side` + `.side-sticky` (P2-7); `Badge className`, `.badge-status` (P3-8); `.kv dd[data-emph]` (P4-5), `Sol pad` (P4-4). short.css is bundled after components.css (lazy route chunk), so any short.css rule of equal specificity wins by order — the reason behind the `:not(.pnl)` scoping below.

**Items.**

#### P6-1 — The trade ticket's hierarchy (high)

**Change.** ShortPanel.tsx `Row`: add `emph?: boolean` → `<dd data-emph="">` and set it on Total cost, CR at open (spot), CR at open (EMA) and Liquidation price; render `sub` as a second `<dd className="short-preview-sub">` after the value's dd (a dt may have several dd; the value dd keeps its testid and `data-value`); put `data-group-start` on the dt of 'CR at open (spot)' and 'Rent rate (APR)' only (not Borrow: a hairline directly under the PREVIEW heading gains nothing; the caps are their own list, see below). While Row is open, swap its wrapper `style={{ display: 'contents' }}` (l.70) for `className="kv-row"` (components.css `.kv-row` is exactly `display: contents`) — one inline style fewer — and give the caps wrapper at ~l.440 the same class. CloseForm.tsx: `data-emph` on 'You receive' and 'P&L', sub as a dd the same way. short.css:

```css
.short-preview { background: var(--surface-well); border-color: var(--line-panel); }
.short-preview .kv { column-gap: 0; }                 /* a group separator is one line, as .mk-kv does */
.short-preview .kv dt { padding-right: var(--sp-4); } /* geometry unchanged */
.short-preview .kv dd.short-preview-sub,
.manage-panel .kv dd.short-preview-sub { grid-column: 1 / -1; margin-top: calc(-1 * var(--sp-1)); color: var(--text-2); font-size: var(--fs-xs); text-wrap: balance; }  /* spans the row (three ragged lines become two), stays right-aligned under the number it bounds */
.short-preview .kv dt[data-group-start],
.short-preview .kv dt[data-group-start] + dd { margin-top: var(--sp-3); padding-top: var(--sp-3); border-top: 1px solid var(--line); }
.short-preview .short-caps { margin-top: var(--sp-3); padding-top: var(--sp-3); border-top: 1px solid var(--line); }  /* the caps are a separate <dl>: the separator sits on the list */
.short-caps dt:not([data-binding="true"]),
.short-caps dd:not([data-binding="true"]) { color: var(--text-2); }  /* data-binding sits on the dt / dd themselves; the binding row keeps .short-caps [data-binding="true"] amber; every cap stays exact (the preview is the designated place for the exact maximum) */
```

Keep the generic `.short-preview-sub { display: block; color; font-size }` rule (short.css:86–90): CapsSummary.tsx:403 renders that class inside the market stat strip, outside any `.kv`. The `.kv dd[data-emph]` style itself is P4-5's.

**Files.** `src/features/short/ShortPanel.tsx`, `src/features/short/positions/CloseForm.tsx`, `src/features/short/short.css`.

**Why.** The preview is eighteen equal 12 px rows: 'Total cost 0.5073 SOL' and 'Liquidation price 0.0₅1994 SOL' weigh the same as 'Minimum rent 0.000014 SOL', the only emphasis being the binding cap's colour; explanatory sub-lines wrap into three ragged lines under their numbers; the Short preview is a sunken bg-0 box while the Lend summary in the same panel is a raised bg-2 one (now both are the well).

**Watch out.** A `:has([data-binding])` selector would match the binding dd too (the attribute is on the dd, not a descendant) and grey the binding cap. e2e reads the value dd's `data-value` only (ShortPanel.test.tsx:111–119, positions.test.tsx:96–99, short-open / short-close / screens / keyboard / mobile-trade), so the second `<dd>` is safe; `data-emph=""` and `data-group-start=""` render as empty attributes in React, which the selectors expect.

#### P6-2 — Position page and dashboard hierarchy (high)

**Change.** short.css:

```css
.position-hero-cr .cr { font-size: var(--fs-hero); }   /* 22 px desktop / 24 phone: the largest thing on the page (P1-1) */
.position-hero, .positions-totals { border-color: var(--line-panel); }
.position-hero-label { font-size: var(--label-fs); font-weight: var(--label-fw); letter-spacing: var(--label-ls); color: var(--label-color); }
.position-head h1 > .num { font-size: var(--fs-sm); font-weight: var(--fw-regular); letter-spacing: 0; color: var(--text-1); }
@media (max-width: 639.98px) { .position-head h1 > .num { display: block; margin-top: var(--sp-1); } }
```

(PositionDetailPage.tsx may add `className="position-id"` to that span for a cleaner hook; `title={position}` keeps the full id.) Totals as a strip, matching the lender KPI strip (which spans the table's width after P7-1):

```css
.positions-totals { display: flex; margin: 0; background: var(--bg-1); border: 1px solid var(--line-panel); border-radius: var(--radius-lg); overflow: hidden; font-size: var(--label-fs); }  /* margin: 0 — base.css gives every <p> a bottom margin */
.positions-totals > span { flex: 1 1 0; min-width: 0; display: flex; flex-direction: column; gap: var(--sp-1); padding: var(--sp-3) var(--sp-4); box-shadow: 1px 0 0 var(--line), 0 1px 0 var(--line); font-weight: var(--label-fw); letter-spacing: var(--label-ls); text-transform: uppercase; color: var(--label-color); }
.positions-totals > span:first-child { flex-direction: row; align-items: baseline; gap: var(--sp-2); }  /* '1 OPEN' inline with the number large; DOM and copy untouched (casing only) */
.positions-totals .num { font-size: var(--fs-lg); text-transform: none; letter-spacing: 0; overflow-wrap: anywhere; }  /* a long vault figure wraps at 320 px instead of being clipped by the strip */
.positions-totals .num:not(.pnl) { color: var(--text-0); }   /* an unscoped colour would beat .pnl[data-sign] by source order and flatten the P&L total */
.positions-totals .num .unit { font-size: var(--fs-xs); }
@media (max-width: 639.98px) { .positions-totals { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); } }
```

No `.positions-totals .pnl { color: inherit }` rule (it would win). Two cells (the closed / liquidated / expired filters omit Vault and Rent due) fill the box with flex; no fixed `repeat(4, …)` on a desktop.

**Files.** `src/features/short/short.css`, `src/features/short/PositionDetailPage.tsx`.

**Why.** 'CR NOW 303 %' — the number that decides a liquidation — is 18 px, the same as the page title and 3 px over the P&L; the headline carries a 12-character account id at title size and weight, taking 45 % of the phone title line; the Positions totals (count, vault, P&L, rent due) are the smallest text on the page while Lending gives the same numbers a KPI strip.

**Watch out.** The id at `--fs-sm` is 12 px on a desktop (the floor). `positions-totals` and `data-count` stay on the `<p>` (mobile-trade.spec.ts:165 only needs it visible). If the owner finds the full-width strip too sparse at 1440, cap both dashboards together (this one and `.lend-kpis`), never one.

#### P6-3 — Manage panel and the sticky column (medium)

**Change.** PositionDetailPage.tsx: add `layout-side` to the `.position-layout` element (l.301) and `side-sticky` to the `.manage-panel` element (l.451), keeping both existing classes (mobile-trade.spec.ts:177–201's `.manage-panel` locator, `#add-margin-title` / `#close-title` focus and useSheetMode's initialFocus are untouched). short.css: once `.layout-side` and `@media (min-width: 1024px) { .side-sticky … }` are confirmed in components.css (P2-7, with the one-column rule under 1024 and `--side-col` 360 / 320), delete l.190–222 (the duplicated 360 / 320 / one-column grid and the ≥ 1024 sticky block); keep `.manage-panel[data-sheet="open"] > .panel-head`, the heading focus rules and `.manage-panel .form-foot`. `.manage-panel #close { margin-top: var(--sp-3); padding-top: var(--sp-5); border-top: 1px solid var(--line); }` (the hairline stays; the 32 px of air is what fixes the finding); `.manage-panel .short-section-title` keeps the section tier (no restyle). Rows on the grid: `.position-facts .kv-row { min-height: var(--kv-row-h); padding-block: 0; align-items: center; } .timeline li { min-height: var(--kv-row-h); align-items: center; }`. Delete the existing invisible `.position-facts .kv-row:hover` rule and add no replacement: read-only rows do not answer the pointer (the same call as P4-2).

**Files.** `src/features/short/PositionDetailPage.tsx`, `src/features/short/short.css`.

**Why.** The Manage panel's sticky scroll region has no affordance (content truncated under a hairline, a thin low-contrast scrollbar) and its sticky recipe duplicates the trade panel's; 'ADD MARGIN' and 'CLOSE' are separated only by a hairline so the disabled 'Add margin' appears to belong to the slippage field below it; facts rows are 26.4 px.

**Watch out.** P6's reviewers accepted a visible facts-row hover; the plan drops it for consistency with P4-2 — reinstate both or neither. The Scrim is the grid's third child (l.490), fine because `.scrim { display: none }` on a desktop. The `.sheet-host[data-sheet="open"]` fixed-sheet rules keep winning under 1024 only while the sticky rule stays inside `(min-width: 1024px)`.

#### P6-4 — Short inputs on phones (medium)

**Change.** short.css:

```css
@media (max-width: 639.98px) {
    .short-inputs { grid-template-columns: minmax(0, 1fr); }
    .short-inputs > .field + .field { margin-top: var(--sp-2); }
    .short-inputs > .field:last-child > .input-wrap { max-width: var(--field-narrow-w); }   /* 120 px, the same as .lend-tolerance */
}
```

Size full width, slippage a short box under it (the Lend form's own pattern; the subgrid rows keep working with one column, as the ≥ 1024 block at short.css:38–46 already proves).

**Files.** `src/features/short/short.css`.

**Why.** At 390 px the size digits get ≈ 138 px beside the 96 px slippage box, ≈ 105 px at 360 and ≈ 67 px at 320, while Max writes a 12-character value — the number the user is about to short scrolls out of its own box after the tap that fills it.

**Watch out.** `max-width` on the wrap never reduces the input's 44 px height under coarse; the 640–1023 side sheet keeps the side-by-side layout.

#### P6-5 — Positions table and badges (low)

**Change.** PositionsPage.tsx (l.320) and PositionDetailPage.tsx (l.200, and the ClosedSummary status badge at l.114): `className="badge-status"` on the status badges (P3-8's prop and rule; its info tone is already the grey tag). short.css: delete the dead `.exit-badge { text-transform: capitalize }` (l.516; no TSX uses it); add `@media (min-width: 1280px) { .positions-tbl > .tbl > thead th[data-col="risk"] { white-space: nowrap; } }` (1280, not 1024: the column is `width: 1%`, nowrap widens it ≈ 26 px, and 1024 is the width where tables are tightest). Pass `pad` on the vault and rent `Sol` cells (P4-4). short.css has no `.position-health` tone rule — nothing to remove; P3-9 already gives the health banner its left bar.

**Files.** `src/features/short/PositionsPage.tsx`, `src/features/short/PositionDetailPage.tsx`, `src/features/short/short.css`.

**Why.** 'open' sits beside 'Safe' in two casings on the page a user watches most; the 1 % RISK column puts its ⓘ on a second line at 1440 and makes the whole header 44 px for one glyph; VAULT reads '6.527 / 0.6016' with wandering decimals.

**Watch out.** e2e asserts `data-status` / `data-value`, never the badge tone or casing. An overflow from the nowrap would scroll inside `.tbl-wrap` (not a gate failure) but would break §7.3's 'every table fits at 1,024' — hence 1280.

**Verification (P6).** Desktop 1440: 20-short-preview (Total cost, CR at open and Liquidation price in 13 px white with white labels; sub-lines spanning the row under their numbers, two lines at most; hairlines before the risk and terms groups and before the caps; the four non-binding caps grey, the binding one amber; the preview a bg-0 well), 31-position-detail ('303 %' at 22 px is the largest text; the id a 12 px regular grey sub-element; 'ADD MARGIN' and 'CLOSE' 24 px apart with a hairline; the Manage foot casts an upward shadow and the column shows a visible scrollbar thumb; facts rows on 28 px with no hover), 30-positions (a totals strip spanning the table's width with 15 px numbers and a coloured P&L; 'Open' capitalised; RISK on one line). Phone 390: trade-sheet-short (size full width, slippage 120 px under it), position (24 px CR; 'Short E2EMAIN' with the id as a sub-line), positions (2 × 2 totals strip; under the Closed filter, 2 cells). Gate: 320 px sheet no overflow; tapCovers clean. e2e: short-open.spec / short-close.spec read `data-value` — unchanged; mobile-trade.spec's `.manage-panel` flow; unit: ShortPanel.test.tsx, positions.test.tsx, stacking.test.ts.

---

### P7 — Lend: KPI strip, lend panel forms, lender table

**Goal.** The lender dashboard anchors on the number a lender came for, and the lend panel's controls match the rest of the terminal (16 px select under a finger, sub-level tabs, the well surface, aligned decimals).

**Owned files.**
- `src/features/lend/lend.css`
- `src/features/lend/LendPanel.tsx`, `LenderDashboardPage.tsx`, `deposit/DepositForm.tsx`, `withdraw/WithdrawForm.tsx`, `claim/ClaimForm.tsx`, `ui.tsx`

Consumed: tokens `--surface-well`, `--line-panel`, `--field-narrow-w` (P1); `Tabs variant="sub"` (P3-6); `Stat hero` (P4-7), `Sol pad` (P4-4), the `.kv dd > [data-emph]` selector (P4-5), `.stat[data-hero]` (P4-6). P7-3 waits for P8-3 (see the item). Everything else in P7 is implementable in position 7.

**Items.**

#### P7-1 — KPI strip and the card hero (high)

**Change.** lend.css: delete `.lend-dashboard .stat-strip { max-width: 880px }` and add nothing in its place (no `(min-width: 1024px)` flex-basis rule: `.stat` is `flex: 1 1 clamp(…)` in a wrapping strip and four cells already split the width equally once the cap goes). Inside the existing `@container tbl (max-width: 959.98px)` block: `.lend-table > .tbl > tbody > tr > td[data-col="claimable"] { font-size: var(--fs-lg); font-weight: var(--fw-medium); }` (the lender card's fixed hero). Cap the strip's hero on narrow phones so a 2-up cell never clips the number inside the strip's `overflow: hidden`: `@media (max-width: 399.98px) { .lend-kpis > .stat[data-hero] .stat-value { font-size: var(--fs-lg); } }`. LenderDashboardPage.tsx: pass `hero` on 'SOL claimable' only — 'Markets', 'Total value' (an estimate: ≈, 'n of m priced') and 'Rent credited' stay 15 px, so the strip has one beat and it is the number the page's only primary action repeats; pass `pad` on the claimable and vesting `Sol` cells. The claimable column's `frac` is per row (`solFrac` gives 6 or 9), so decimals align within rows of equal precision only — do not force one frac (dust rows need 9) and do not claim fully aligned decimals for it.

**Files.** `src/features/lend/lend.css`, `src/features/lend/LenderDashboardPage.tsx`.

**Why.** The four KPIs stop at 888 px above a 1,432 px table with 550 px of nothing beside them, and 'SOL claimable 0.007283 SOL' is the same 15 px as 'Markets 1' — the only prominent figure on the page is inside the Claim all button; on the phone CLAIMABLE is no bigger than 'Locked 0'.

**Watch out.** Two heroes out of four would read as a checkerboard on the phone's 2 × 2 grid. mobile-lend.spec.ts checks testids and the card `thead` position only; lend-claim-sync.spec.ts:102–164 and mobile-lend.spec.ts:200–203 assert '… 0 SOL' strings that come from ClaimLabel's `formatSol`, which is not padded.

#### P7-2 — Lend panel controls (high)

**Change.** lend.css: delete the line `font-size: var(--fs-sm);` from `.lend-select` (l.32) — base.css's `input, select, .input, .select { font-size: var(--fs-input) }` then applies: 13 px with a mouse, 16 px under coarse; `font-family: var(--font-num)` stays; long labels truncate under P3-4's ellipsis. `.lend-position { background: var(--surface-well); border-color: var(--line-panel); }`; `.lend-tolerance { max-width: var(--field-narrow-w); }` (120 px, the value it has today). LendPanel.tsx: `<Tabs variant="sub" …>` on Deposit / Withdraw / Claim (P3-6; the `T.lendTab(…)` testids stay on the tab buttons). DepositForm.tsx / WithdrawForm.tsx: `Kv` renders the dd with only a testid, so put the emphasis on a wrapper inside the value — `<span data-emph><Tokens … testId={T.lendPreview('shares')} /></span>` for 'Shares minted' and `<span data-emph><Tokens … testId={T.withdrawPreview('allowance')} /></span>` for 'You may withdraw now' (P4-5's `.kv dd > [data-emph]` rules style them; testids stay on the Tokens span). Keep the amount's `disabled` predicate as it is (P3-4's CSS now shows it).

**Files.** `src/features/lend/lend.css`, `src/features/lend/LendPanel.tsx`, `src/features/lend/deposit/DepositForm.tsx`, `src/features/lend/withdraw/WithdrawForm.tsx`.

**Why.** The FROM ACCOUNT select is 13 px mono directly above a 16 px amount field (iOS zooms the sheet when it is tapped; the review's Priority select is 16 px in the same flow); the Lend position box is raised bg-2 while the Short preview beside it is a well; the two lend tablists are indistinguishable from the Short | Lend row.

**Watch out.** An undefined `--line-panel` would render a `currentColor` frame round the position box — P1 is in. P3-6's sub variant keeps `.tab` at `max(28px, var(--tap-min))`, 44 under coarse, so the gate's target check holds.

#### P7-3 — Hand the no-wallet CTA width to the shared empty state (medium; lands after P8-3)

**Change.** As P7's final step, after P8-3 has landed: delete lend.css l.204–212 — the comment and the two rules `.lend-dashboard .empty[data-layout="block"] > .empty-action { align-self: stretch }` and `.lend-dashboard .empty-action > .connect-cta { width: min(360px, 100%) }` — once components.css contains P8-3's `.empty[data-layout="block"] > .empty-action { align-self: stretch }` and `.empty-action > .connect-cta { width: min(360px, 100%) }` with those exact values. If P8-3 chose a different width, keep the lend rules until the owner picks one look for both pages. Testids unchanged.

**Files.** `src/features/lend/lend.css`.

**Why.** Positions and Lending give the same no-wallet state two looks (a 120 px centred button vs a full-width one) because only lend.css widens the CTA; the rule belongs to the shared state.

**Watch out.** Deleting before P8-3 leaves /lend with the 120 px button in between — the visible regression the item exists to end. Only P7 may touch lend.css, so this is sequencing, not ownership.

**Verification (P7).** Desktop 1440: 41-lender-dashboard (four KPIs span to x 1,432; 'SOL claimable' at 18 px, the other three 15 px; CLAIMABLE padded within each row's precision and aligned with VESTING where the precision matches), 40-lend-deposit (segmented sub-tabs under the Short | Lend underline; the position box a bg-0 well; 'Shares minted' white 13 px). Phone 390: trade-sheet-lend (the account select's text as large as the amount's; tolerance 120 px), lend-dashboard (CLAIMABLE 16 px hero in the card; the strip's hero 16 px at 320 and 18 px at 390), lend-dashboard-no-wallet (after P7-3: CTA 360 px wide inside a framed block, identical to positions-no-wallet). Gate: 320 px lend sheet no overflow; sub-tabs ≥ 44 px under coarse; 320 px KPI cell not clipped. e2e: mobile-lend.spec, lend-deposit.spec, lend-claim-sync.spec by testid unchanged; unit: LendPanel.test, DepositForm / WithdrawForm tests.

---

### P8 — Reading pages and shared states: How it works, Security, Empty / Error

**Goal.** The two pages built for reading get a reading hierarchy and wayfinding; the shared empty and error states look designed at every width. (The `Stat` hero hook that was P8-4 is P4-7.)

**Owned files.**
- `src/features/guide/guide.css`, `src/features/guide/HowItWorksPage.tsx`
- `src/features/security/security.css`, `src/features/security/SecurityPolicyPage.tsx`
- `src/styles/components.css` — section 'states' (~l.1776–1850, including the `.stale-badge` and `.route-stub` rules there)

Consumed: tokens `--fs-prose`, `--lh-prose`, `--line-panel` (P1); fall back to `--fs-md`, `--lh`, `--line` only if a token is missing — never a literal in guide.css / security.css and never an undefined variable.

**Items.**

#### P8-1 — Reading type, copy untouched (high)

**Change.** guide.css:

```css
.guide-lede { font-size: var(--fs-lg); max-width: 60ch; color: var(--text-1); }       /* 8.6:1 */
.guide-body > p, .guide-body > ul, .guide-list { font-size: var(--fs-prose); line-height: var(--lh-prose); }  /* on the prose selectors, not .guide-body, so the embedded disclosures keep --lh */
.guide-body .disclosure { font-size: var(--fs-prose); }   /* the risk boxes read at the prose size: same box, bar and title; the trade-panel disclosures elsewhere are unaffected */
.guide-body h2 { font-size: var(--fs-xl); margin-top: var(--sp-7); padding-top: var(--sp-5); }
.guide-body h3 { margin-top: var(--sp-5); margin-bottom: var(--sp-2); }   /* keeps the section tier from base: uppercase 12 px bold, now with air above so it groups downward */
.guide-toc { border-color: var(--line-panel); }
```

No `.guide-page h1` rule: the page title keeps the shared 20 px (a 22 px title on one page would be a fourth title size; the ladder is title 20 > h2 18 > prose 14 > 12 px section tier). security.css: `.sec-page h3 { margin: var(--sp-5) 0 var(--sp-2); } .sec-page .panel-body > p:not(.sec-note), .sec-list { font-size: var(--fs-prose); line-height: var(--lh-prose); }` (the Scope panel's 12 px `.sec-note` would otherwise grow; the closing 'Machine-readable' note is outside a panel and unaffected).

**Files.** `src/features/guide/guide.css`, `src/features/security/security.css`.

**Why.** On the two pages written for newcomers the type is the data-table's: 15 px headings one step above 13 px body, uppercase labels where sub-headings belong, a lede indistinguishable from the body — a 3,143 px wall of near-equal text at 68ch.

**Watch out.** HowItWorksPage.test keys on data-testid, `.skeleton` and `.guide-missing` only. `.sec-page .panel-body > p` (0,2,1) outranks `.sec-note` (0,1,0) — hence the `:not(.sec-note)`.

#### P8-2 — Table of contents: wrapped on the phone, a sticky rail from 1,280 px (medium)

**Change.** guide.css phone block: replace the column override with `.guide-toc ul { flex-wrap: wrap; gap: 0 var(--sp-5); }` (the base rules already give the links their 44 px min-height; no `padding-inline` on the links — it would shift the first link 4 px off the 'ON THIS PAGE' title's left edge for nothing); keep `.guide-list { padding-left: var(--sp-4) }` in the block. Desktop rail — DOM order (page-head, lede, toc, optional guide-error, body, foot) kept, no `order`:

```css
@media (min-width: 1280px) {
    .guide-page { display: grid; grid-template-columns: minmax(0, 760px) 200px; column-gap: var(--sp-7); justify-content: center; max-width: none; }
    .guide-page > * { grid-column: 1; }   /* head, lede, [error], body, foot stack in column 1 in DOM order; nothing is pinned to a row */
    .guide-page > .guide-toc { grid-column: 2; grid-row: 3 / span 3; align-self: start; position: sticky; top: calc(var(--header-h) + var(--sp-4)); margin: 0; }  /* beside the body it indexes; the span keeps its sticky area over error + body + foot */
    .guide-toc ul { flex-direction: column; gap: 0; }
}
```

`.page`'s `gap` still supplies the row gap. Verified in Chromium at 1440 px with and without the config-read error present: head / lede / [error] / body / foot stack at x = 224 in order, the toc at x = 1016 beside the body.

**Files.** `src/features/guide/guide.css`.

**Why.** On the phone a 290 px 'On this page' column of six 44 px rows fills the first screen and the explanation starts below the fold; on the desktop the toc scrolls away on a 3,100 px page and nothing marks the way back.

**Watch out.** Pinning `grid-row: 3` on `.guide-body` while the foot and the conditional error auto-place drops 'Back to markets' (and the error) into the rail column beside the lede. 44 px rows with 0 row-gap touch but do not overlap (tap-cover gate unaffected).

#### P8-3 — Shared empty and error states (medium)

**Change.** components.css states section — merge into the existing `.empty[data-layout="block"]` rule:

```css
.empty[data-layout="block"] { background: var(--bg-1); border: 1px solid var(--line-panel, var(--line)); border-radius: var(--radius-lg); padding: var(--sp-7) var(--sp-5); min-height: 200px; justify-content: center; }
.empty[data-layout="block"] > .empty-action { align-self: stretch; }
.empty-action > .connect-cta { width: min(360px, 100%); margin-inline: auto; }
.error-state { color: var(--text-0); text-align: left; display: flex; align-items: center; flex-wrap: wrap; gap: var(--sp-3); padding: var(--sp-3) var(--sp-4); min-width: 0; overflow-wrap: anywhere; border-left: 3px solid var(--danger); background: var(--danger-bg); border-radius: var(--radius); }  /* text-0 on --danger-bg 13:1; an unbroken token in an API error (a URL, a signature) wraps instead of pushing the box past 320 px */
.error-state .btn { min-height: var(--tap-min); }   /* keep: Retry is data-size="sm" and would drop to 36 px under a finger without it */
```

`role="alert"` and Retry kept; `.empty` without `data-layout` (table empties) untouched. The `.lend-dashboard`-scoped copies of the two empty-action rules in lend.css become redundant (same values) — P7-3 deletes them.

**Files.** `src/styles/components.css`.

**Why.** No-wallet Positions is a floating bold sentence with a 120 px button on the raw background, Not found three lines at the top of a 900 px void, and the indexer-down panel a centred red sentence — stubs rather than designed states; red prose for a whole sentence is alarm, not information.

**Watch out.** Markup unchanged (CreateMarketPage.test asserts `className === 'error-state'`; MarketsPage.test and responsive.spec.ts:811 select `.error-state`; NotFound keeps role=alert). None of the block empties is nested in a panel, so no doubled borders; `min(360px, 100%)` fits 320 px.

**Verification (P8).** Desktop 1440 full-page 72-how-it-works: 20 px title, 15 px lede, 14 px body at 1.55, the embedded risk boxes at 14 px, 18 px h2 with 32 px above; the toc a sticky 200 px rail right of the prose that stays in view while scrolling, 'Back to markets' under the body, and with the config read failing the error box in the text column. 60-security: 14 px lists, h3 with 16 px above, the Scope note still 12 px. Phone 390: how-it-works (toc as 2–3 wrapped rows; 'Shorting a token' starts on the first screen), positions-no-wallet and lend-dashboard-no-wallet (one framed bg-1 block each, CTA 360 px), not-found (framed). 51-fallback-markets: the indexer error a left-bar red-tinted note with Retry beside it. focus.spec (How it works walk) passes; breakpoints.test.ts passes (`min-width: 1280px` is in the set).

---

### P9 — Overlays: sheets, modal, review, toasts

**Goal.** Everything that floats over the page is placed and lit consistently and survives the phone's safe areas: sheet heads that stay under the notch, a review body that uses the height it has, a toast that takes the title strip cleanly, one shadow and surface per layer, and a visible priority choice.

**Owned files.**
- `src/styles/components.css` — sections 'modal', 'review: the plan's instructions', 'toast', 'review: deltas, sign hint', and inside 'responsive building blocks' the sheet rules (`.sheet-host`, `.sheet-head`, `.scrim`, the sheet keyframes)
- `src/components/Modal.tsx`, `Sheet.tsx`, `Toast.tsx`, `src/tx/TxReviewModal.tsx`

Consumed: tokens `--sheet-head-h`, `--shadow-modal`, `--shadow-sheet`, `--shadow-toast`, `--surface-well`, `--line-panel`, `--safe-t` (P1); icon `close` + `.icon` (P2-1); the `.chip[aria-pressed]` idiom and the shared close rule (P3-3).

**Items.**

#### P9-1 — Sheet head under the notch (high)

**Change.** components.css, the sheet rules in the `(max-width: 1023.98px)` block (mirrors `.modal-head`; replaces `padding-top: var(--safe-t)` on the host and `top: calc(-1 * var(--safe-t))` on the head):

```css
.sheet-host[data-sheet="open"] { padding-top: 0; scroll-padding-top: calc(var(--sheet-head-h) + var(--safe-t) + var(--sp-3)); }  /* keeps today's 8 px clearance for a focused control; short.css's .short-section-title scroll-margin is additive and stays */
.sheet-host[data-sheet="open"] .sheet-head { top: 0; padding-top: var(--safe-t); min-height: calc(var(--sheet-head-h) + var(--safe-t)); }  /* padding-top written AFTER the existing padding shorthand (0 var(--sp-2) 0 max(var(--sp-4), var(--safe-l))) in the same rule */
```

`--sheet-head-h` is P1-4's `calc(var(--header-h) + var(--sp-2))` = 52 px in the compact tiers (56 under coarse). Verify with `--safe-t: 47px` forced in DevTools.

**Files.** `src/styles/components.css`.

**Why.** A scroll container's padding-top scrolls away with its content; a sticky head with a negative `top` is then pinned above the scrollport, so in a wallet's in-app browser (47 px inset; index.html sets viewport-fit=cover) only ~5 px of 'Trade E2EMAIN' and its ✕ remain once the form scrolls.

**Watch out.** An undefined `--sheet-head-h` inside `calc()` invalidates `min-height` and `scroll-padding-top` (the head drops to content height, the 2.4.11 padding disappears) — grep tokens.css first. Changes nothing at a 0 inset; the tablet side sheet follows through the same rule.

#### P9-2 — Modal body height, modal and tx-steps surfaces, the More sheet (high)

**Change.** modal section: `.modal[open] { display: flex; flex-direction: column; } .modal-body { flex: 1; min-height: 0; max-height: none; }` (drop `calc(100dvh - 200px)`; the dialog's own `max-height: calc(100dvh - 64px)` bounds it; the UA's `margin: auto` keeps centring; head and foot keep `min-height: auto` so only the body shrinks). `.modal { box-shadow: var(--shadow-modal); }` — keep `border: 1px solid var(--line-strong)` (over the 72 % scrim the dialog's bg-1 and the dimmed page are within ~1.1:1, so the edge is what makes the dialog read as a window; every other floating surface shares it). `.tx-steps { background: var(--surface-well); border-color: var(--line-panel); }`. Inside the existing `@media (max-width: 639.98px)` modal block, next to `.modal[open][data-variant="sheet"]`:

```css
.modal[open][data-variant="sheet"] { background: var(--bg-2); box-shadow: var(--shadow-sheet); }   /* More renders only under 640 px */
.modal[data-variant="sheet"] .modal-head { background: var(--bg-2); }   /* the phone's sticky head is bg-1 by default; a darker title band on a bg-2 sheet would be a surface reuse (as .menu-head does for the wallet sheet) */
```

Verify the sheet still hugs the bottom.

**Files.** `src/styles/components.css`.

**Why.** On a 900 px desktop the review's body is capped at 700 px although the dialog may be 836 and head + foot take ~100: the body gains the ~36 px the 200 px constant wastes (the acknowledgement still lands below the fold at 900 px; the fix is structural, not a cure); the More sheet is bg-1 with no shadow while every other bottom sheet is bg-2 with one; the tx-steps box is bg-2 among bg-2 disclosures.

**Watch out.** The phone block's identical `.modal[open]` selector still wins later; the gate's unroll targets `dialog[open] .modal-body` (responsive.spec.ts:263–272), which remains the scroller.

#### P9-3 — The toast over an open sheet takes the title strip (medium)

**Change.** toast section — every rule below inside the existing `@media (max-width: 639.98px)` block beside the current `html[data-sheet-open] .toast-region` rules (never at top level: the tablet's side sheet shows the toast bottom-left and keeps its title):

```css
html[data-sheet-open] .toast-region { top: var(--safe-t); left: 0; right: calc(var(--sp-2) + 44px + var(--sp-2)); }  /* ends 4 px short of the ✕ box and 4 px past the title box */
html[data-sheet-open] .toast-region .toast { min-height: var(--sheet-head-h); align-items: center; border-radius: 0; border-width: 0 0 1px 3px; box-shadow: none; }
```

At top level of the section: `.toast { box-shadow: var(--shadow-toast); }`. No rule on `.sheet-title`: the opaque strip is as tall as the head's content box (top = `--safe-t`, min-height = `--sheet-head-h`, matching P9-1) and 4 px wider than the title box, so it covers a two-line 16 px title (46 px < 52); if a title ever shows under the strip, blank it with `html[data-sheet-open]:has(.toast-region .toast) .sheet-title { opacity: 0; }` — never `color: transparent` (axe reads it as 1:1) or `visibility: hidden` (it would remove the dialog's name).

**Files.** `src/styles/components.css`.

**Why.** The confirmation toast is ~36 px placed 4 px from the top over a 52 px head, so 'Trade E2EMAIN' bleeds out under the toast's bottom edge (phone/trade-sheet-lend-withdraw.png shows it; the state recurs because a toast's 6 s do not count while a sheet covers it). Sized to the head, it takes the strip cleanly; the ✕ stays clear.

**Watch out.** The strip ends 4 px clear of the ✕ (FRONTEND §7.6 says 16 px; P10 records the change). mobile-trade.spec.ts:135–141 checks visibility and href only; the gate treats `.toast-region` as a bar.

#### P9-4 — Priority as a visible choice (low)

**Change.** TxReviewModal.tsx: replace the `Select` with `<div role="group" aria-label="Priority fee" className="chip-group">` holding `<button type="button" className="chip hit" aria-pressed={speed === 'normal'} onClick={() => setSpeed('normal')}>Normal</button>` and the same for `Fast` (the `.chip[aria-pressed="true"]` idiom from P3-3; `hit` gives 44 px under coarse) — a `<fieldset className="chip-group">` with a visually-hidden `<legend>Priority fee</legend>` is equally fine, as WithdrawForm does; remove the now-unused `Select` import. components.css review section: `.modal-body .kv dd:has(> .chip-group) { display: flex; justify-content: flex-end; }`. The delta rows are **not** padded: DeltaRow formats through `formatFixed(v, scale, min(scale, 6))`, not `Sol` / `Tokens`, and padding a round 'before' ('1,000.000000 → 999.492642') adds seven zeros of noise on a phone's two-line row and breaks the app-wide trailing-zero trimming.

**Files.** `src/tx/TxReviewModal.tsx`, `src/styles/components.css`.

**Why.** The one decision the review offers hides in an 80 px native 'Normal ▾' menu styled like the read-only rows around it, with 'Fast' invisible until opened; FRONTEND §5.3 still calls it the priority selector (P10 writes the chip pair).

**Watch out.** 'Normal' / 'Fast' / 'Priority' copy unchanged; no unit test or e2e spec references the select (only `setSpeed` mocks and the FeeSpeed reducer). `.chip` is not in the gate's coarse PRIMARY list; the 4 px gap and ±4 px hit area do not cross into the neighbour.

#### P9-5 — Close icons (low)

**Change.** Modal.tsx and Sheet.tsx (`SheetHead`) render `<NavIcon name="close" className="icon" />` in place of the text '✕'; Toast.tsx renders `<NavIcon name="close" className="icon toast-dismiss-x" />` in place of its ✕ span and keeps the hidden 'Dismiss' word span (the open-sheet rule keys on `.toast-dismiss-x`, which the svg now carries). aria-labels 'Close' / 'Dismiss' and testids unchanged. Keep `.modal-close { flex: none; }` (it is the shrink guard beside a long `.grow` title, not a size). Keep the coarse `.btn.modal-close, .btn.sheet-close { min-width: 44px; min-height: 44px; }` override unless the gate's primaryUnder44 passes at 320 / 360 / 390 / 768 with it removed (P3-3's shared rule gives `.modal-close` `max(--icon-btn, --tap-min)` = 44 under coarse and sits after `.btn[data-size="sm"]`, so it should, but prove it before deleting).

**Files.** `src/components/Modal.tsx`, `src/components/Sheet.tsx`, `src/components/Toast.tsx`, `src/styles/components.css`.

**Why.** The most important escape hatch on a full-screen sheet is a faint ~8–11 px font glyph that renders differently on Android and iOS and changes size between the review, the sheets, the wallet and the toast.

**Watch out.** TxReviewModal.test.tsx:187 and mobile-trade.spec.ts:157 select the close by name. After P9 the wallet sheet's (P2-8) and the glossary's (P3-10) ✕ are already SVGs, so no font-glyph close should remain.

**Verification (P9).** Phone 390: trade-sheet-short with `--safe-t: 47px` forced — the head (title, 16 px ✕ icon) stays fully visible after scrolling the form; trade-sheet-lend-withdraw after a send (reopen the Lend sheet within 6 s of a confirmed deposit) — the CONFIRMED strip spans the head's height with no title ink under it and the ✕ clear; 768: the side sheet's toast stays bottom-left with the title visible; more (bg-2 sheet with a shadow and a bg-2 head); review-sheet (Normal / Fast chips, Normal pressed, 44 px each). Desktop 1440 and 1280×800: 21-short-review — the body reaches the dialog's bottom minus the foot; the dialog keeps its `--line-strong` edge; 'Your SOL 1,000 → 999.492642' unchanged. Gate: primaryUnder44 for `.modal-close` / `.sheet-close` at 320 / 390 / 768 before and after any deletion of the coarse override; tapCovers clean on the chip row. e2e: mobile-focus.spec and mobile-trade.spec (sheet toast assertions) pass; axe clean on the review; TxReviewModal.test.tsx passes.

---

### P10 — Closing docs pass (FRONTEND.md), after P2–P9 have landed

One PR editing `docs/design/FRONTEND.md` only, each row citing the package that built it and written only if that package landed as described:

- §3.1 Shell: 'ShortWin visually hidden < 400' → 'hidden under 400 px only while the cluster badge shows' (P2-2); ⌕ and Connect form the trailing group (P2-4); the status bar inset is `--gutter` (P2-4).
- §3.3 and §7.6: the compact intro strip on every phone — only if the owner said yes to P5-1 (with the breakpoints.ts and markets.css comments already updated in P5).
- §3.9 l.779: the status page's panels are as tall as their content (P5-8).
- §5.3: the priority selector is a Normal / Fast chip pair (P9-4).
- §7.2: the h-scroll counts re-recorded at 1,280 / 1,440 after P1 (gutter and scrollbar gutter); SOL table columns padded to their column's precision (P4-4, P5-4, P5-5, P6-5, P7-1).
- §7.4 Button row: 'a disabled button keeps a `--line-strong` edge and regular weight; with a Connect CTA in the same foot it is a quiet caption-like row' (P3-2); the filled variants' own hover / press colours (P3-3); the shared close-button size (P3-3). DataTable row: 'the header sticks under the app header from 1,024 px except in the positions and lend tables; rows carry `scroll-margin-top: calc(var(--header-h) + 3 * var(--row-h))` and the focus walks (focus.spec.ts, Tab and Shift+Tab over the markets list) pass'; the sorted column's gold line and SVG sort marks; the empty list hides its header (P4-3); the card hero (P4-4). Badge row: info = grey tag, neutral = ringed, `.badge-status` capitalised (P3-8). Tabs: the `sub` variant and the two-sided fade (P3-6). Field: error above the surviving hint (P3-7). Stat: the `hero` hook (P4-7). Empty / ErrorState: the framed block and the left-bar error (P8-3).
- §7.6: a landscape phone gets the phone's status strip, and a 640–665 px mouse window still clips the end of 'Security' (P2-6); the sheet head keeps its height under the notch (P9-1); the toast strip over a sheet is 4 px, not 16 px, clear of its ✕ (P9-3); the trade and Manage panels share `.layout-side` / `.side-sticky` (P2-7, P5, P6).
- §7.7: the tablet row says a landscape phone gets the strip (P2-6); the market page opens on Lend when shorting is blocked (P5-2).
- §11.2: `markets-empty-create` (P5-7).

---

## 4. Dropped

### Dropped at synthesis (finding ids)

| id | why |
| --- | --- |
| component-consistency-8 | Contradicts colour-surface-states-6 on direction only; resolved for the well (bg-0). The trade panel's notes under the preview are already bg-2 boxes, so a bg-2 preview would make the panel one monotone stack; a sunken well separates 'read-only output' from 'note'. The lend position box moves to the well instead (P7-2). |
| phone-shell-9 (strip bg-0) | Contradicts colour-surface-states-7: a bg-0 strip would merge into the page and could not match its own expanded list (bg-2); the strip becomes bg-2 so handle and sheet read as one object (P2-3). The action bar's upward shadow and dropped border from the same finding are kept (P2-5). |
| colour-surface-states-2 (tinted disabled rings) | The documented rule says a disabled button is neutral whatever its variant; a dim gold or red ring next to the red-outlined danger button muddles the vocabulary. The `--line-strong` edge alone restores the control's shape (P3-2). |
| forms-flows-1 (display:none / visually-hidden reason) | The one-line reason is the FormFoot contract's live status region and mobile-trade.spec:222 / mobile-lend.spec:254 assert it; hiding or clipping it risks `toBeVisible` and changes §3.6/§3.7. Demoting the duplicate submit to a caption (P3-2) already leaves one button-shaped control. |
| data-presentation-2 (compact caps in the preview) | CapsSummary.tsx designates the panel's preview as the place for the exact maximum (the stat strip is compact because the preview is exact); compacting the five caps would leave the exact values only in title attributes, i.e. hidden numbers (constraint 9). The caps are demoted as a grouped grey block instead (P6-1). |
| forms-flows-5 (sentence-case white Manage headings) | Superseded by the three-tier type system: section headings (bold text-1) now differ from field labels (medium text-2) by weight and colour, and a fourth sentence-case style would break the tiers. Its spacing between the two forms is kept (P6-3). |
| desktop-shell-2 (optional 18 px wordmark) | The wordmark must not outrank the page title, which moves to 20 px; the brand stays 15 px and gains presence from the mark instead. |
| desktop-shell-3 (12 px gutter at 1024) | FRONTEND §7.2 records the 1,024 px tables as fitting with a few px to spare; removing 8 px of width there risks the gate's h-scroll count. The 12 px gutter applies from 1,280 px, after measuring (P1-4). |
| desktop-shell-5 (56 px header) | 56 px costs 8 px of every phone screen the markets-fold items fight for; 52 px gives the 44 px controls 4 px of air each side (P1-4). |
| phone-shell-12 (deep-link initial focus on the sheet host) | Needs a new `useSheetMode` option across the sheets and markets packages for a ring that is correct behaviour for keyboard users and appears only after a programmatic deep-link focus; low value for the coupling. The two-sided overflow fade from the same finding is kept (P3-6). |
| first-use-intuition-3 (two-up intro grid fallback) | Computed: at 390 px each explanatory paragraph wraps to ~7 lines in a 170 px column, so side by side they are as tall as stacked — no saving. The toolbar row and the owner-gated strip compaction remain (P5-1). |
| data-presentation-3 (utilisation kink marker) | A CSSOM-positioned gradient tick per row adds a custom property pipeline for a 1 px mark that cannot be read on a 48 px track; the visible track and 2 px minimum fill deliver the meter (P5-5). |
| forms-flows-12 (disabling the tolerance input in TSX) | Would change which controls the no-wallet lend sheet's focus walk visits (mobile-focus.spec); the CSS disabled look (P3-4) delivers the cue without touching behaviour. |
| desktop-shell-1 (animation-timeline fade) | Scroll-driven animations are not needed: the sticky foot's upward shadow and the static 12 px fade (P2-7, P3-2) are the standard cue and work everywhere. |
| hierarchy-type-space-6 (swapping the '1 open' nodes) | Reordering the words changes approved copy order; the first totals cell is styled inline ('1 open' with the number large) while the other three stack (P6-2). |
| hierarchy-type-space-4 (duplicate of first-use-intuition-3) | Merged into P5-1: both describe the phone markets fold; the CSS-only savings are firm, the strip compaction on every phone is one owner decision carried once. |
| phone-shell-3 (formatSlot helper used by StatusBar) | Folded: the StatusBar formats the slot inline exactly as its summary already does (P2-3), so the shell package does not depend on the data package's new helper; ProtocolStatusPage uses `formatSlot` (P5-8). |

### Dropped or moved during verification (sub-items of accepted items)

| item | what | why |
| --- | --- | --- |
| P1-3 / P1-4 | `--brand-mark`, `--brand-tile`, `--brand-mark-size` | The logo task owns the mark; Logo.tsx draws in `currentColor` and `--accent`, so no brand token is consumed. |
| P1-5 | `scrollbar-gutter: stable` on `html` unconditionally | Narrows every classic-scrollbar viewport by 15 px at rest; screens.spec's 1024 × 720 markets-table fit would fail. Scoped to `(min-width: 1280px)`. |
| P1-6 | FRONTEND rows for other packages (Button, DataTable, §5.3 chips, P5-1 note) | A row recorded before its package lands is a false contract. Written in P10. |
| P2-1 | `BrandMark` in icons.tsx; favicon regeneration | Would give the product two brand drawings; both belong to the logo task. |
| P2-2 | `.brand-mark` span wrapper, `.brand-mark svg`, `.brand-mark-tile`; unscoped `.nav { margin-left }` | Would overwrite the logo task's rules and re-wrap a mark not drawn for a tile; the fixed phone tab bar would shift 8 px. |
| P2-3 | `data-sheet-keep` on the scrim; scrim `z-index: calc(var(--z-bars) - 1)`; `.statusbar-item:last-child { border-bottom: 0 }` | Inert (inherited from the footer); inside the fixed footer's stacking context it would dim the list and swallow its taps; it stripped the Security button's bottom edge. |
| P2-4 | `.search-toggle { color }` at (0,1,0); `.menu-item:hover` without `:not(:disabled)`; `--shadow-pop` on the phone search overlay; a blur-only header edge | No-op against the ghost variant; a disabled wallet entry would light up; a stray band over the input row; a terminal header must keep a hairline. |
| P2-6 | `a.statusbar-item { flex: none }` | A `white-space: nowrap` flex item never shrinks; `overflow: hidden` clips the end of the row whatever the flex values. |
| P2-7 | `.stack { gap: var(--page-gap) }` unscoped | `.stack` is also the lend / Manage forms, SettlementRisk and the toast region — in-box density stays untouched. |
| P2-8 | `.wallet-links { overflow: hidden }` | Clips the rows' `:focus-visible` rings (constraint 5). |
| P2-9 | CSS-border `::after` chevron | A fifth chevron drawing beside the registered one. |
| P3-2 | 48 px claim for the demoted submit | The (0,4,0) rule outranks the coarse 48 px floor: it is 44 px under coarse (the gate's floor), 24 with a mouse. |
| P3-3 | `.toast-dismiss` and `.sheet-close` in the square close rule; `.icon` / `.icon-sm` redefinition | The toast shows the word 'Dismiss' over an open sheet; the sheet close would become 44 × 32 on a tablet with a mouse; the classes are P2-1's. |
| P3-4 | `align-self: start` on `.checkbox input` | Under coarse the box would top-align ~10 px above a centred one-line label; the container's `align-items` already decides. |
| P3-5 | `:focus-within` ring; literal 20 / 24 px box sizes | Fires on pointer focus and doubles the input's own ring; the `--check` token exists. |
| P3-10 | `.reveal { min-height: 0 }` + `.reveal.hit::after` | The gate measures the control's own rect (a 15–20 px button fails at all eight viewports) and `all: unset` leaves the button unpositioned. |
| P4-2 | `tr[aria-selected="true"]` restyle; kv-row hover (`--bg-4` for rows) | Dead CSS (no `tr` ever gets `aria-selected`); read-only rows must not look clickable; rows take P1's `--bg-hover`. |
| P4-3 | bottom-aligned header (`vertical-align: bottom`, 4 / 3 px padding); `.tbl-sortbar` padding | Plain headers would sit 4–5 px lower than sortable ones at 1440 and the header row would leave the 28 px grid; the sort bar shares the card titles' 12 px left edge. |
| P4-4 | `pad` on the review's before / after | DeltaRow formats via `formatFixed`, not `Sol` / `Tokens`, and padding a round 'before' is noise (P9-4). |
| P4-5 | `display: flex` on `data-lines` dt / dd; literal `12px` unit | Flex blockifies inline children and drops inter-item whitespace; `--fs-xs` exists. |
| P5-1 | the three intro-type rules | `.mk-intro-part p:not(.mk-intro-links)` is already `--fs-sm` at every width; the margins save 8 px for a tighter recipe than the desktop's. Saving restated as ≈ 50–60 px, not 90. |
| P5-5 | stacking the utilisation meter over its value in the stat strip | `order` is banned, so the bar would sit above the number and drop its baseline ≈ 6 px below the six neighbours. |
| P5-6 | `<Kv data-lines>` | `Kv` has no such prop; `KvLines` renders the hairline list. |
| P5-7 | `data-variant="primary"` on OpenMint; a second gold button | Already primary (neutral only while disabled); the head link goes default while the empty-slot CTA shows. |
| P5-8 | tier badge ⓘ → chevron with a dotted-underline hover; `.mk-banner` in the border selector; `border-color` shorthand on the creator box and intro | 'Tier C ▾' reads as a picker and §3.4 records 'Tier C ⓘ'; `.mk-banner` has no border; the shorthand would grey the severity / info left bars. |
| P6-1 | `:has([data-binding])` caps selectors; `data-group-start` on Borrow and on the caps dt; `text-align: left` on sub-lines | The attribute sits on the dt / dd themselves (the binding cap would turn grey); a hairline under the heading gains nothing and the caps are their own `<dl>`; a number on the left edge breaks the numbers-right rule. |
| P6-2 | `.positions-totals .pnl { color: inherit }`; unscoped `.num { color }`; fixed `repeat(4, …)` desktop grid; `--fs-2xl` literal for the CR | Both colour rules would flatten the P&L total by source order; four fixed columns leave half the strip empty under the Closed / Liquidated / Expired filters; the CR consumes `--fs-hero`. |
| P6-3 | `.position-facts .kv-row:hover` with `--bg-hover` | Read-only rows do not answer the pointer (P4-2). |
| P6-5 | `(min-width: 1024px)` for the RISK nowrap | The 1 % column would widen ≈ 26 px at the width where tables are tightest; 1280. |
| P7-1 | `(min-width: 1024px) { .lend-kpis > .stat { flex-basis: calc(25% - 1px) } }`; `hero` on 'Total value' | A no-op (`flex-grow: 1` already splits the width); two heroes dilute the one number and checkerboard the phone's 2 × 2 grid. |
| P7-2 | `.lend-select { font-size: var(--fs-input) }`; `data-emph` on a `Kv`-rendered dd | Re-declares base.css; the fix is deleting the `--fs-sm` override. `Kv` has no attribute slot; the value is wrapped instead. |
| P8-1 | `.guide-page h1 { font-size: var(--fs-2xl) }`; literal `line-height: 1.55`; line-height on `.guide-body` | A fourth title size on one page; tokens first (`--lh-prose`); the embedded disclosures would inherit it. |
| P8-2 | `grid-row: 3` pinned on `.guide-body`; `.guide-toc a { padding-inline }` | The foot and the conditional error auto-placed into the rail column (verified in Chromium); the link would sit 4 px off the title's edge. |
| P8-4 | `Stat hero` prop defined in P8 | Consumed by P7-1: moved to P4-7. |
| P9-2 | `.modal { border-color: var(--line-panel) }` | Over the scrim the `--line-strong` edge is what makes the dialog read as a window. |
| P9-3 | `.sheet-title { color: transparent }`; rules at the toast section's top level | The opaque strip already covers the title and a transparent title hides the dialog's visible name for the toast's life (opacity: 0 is the fallback); top-level rules would blank the tablet side sheet's title. |
| P9-4 | `pad` on the delta rows | Not reachable (`formatFixed`), and seven zeros of noise on a phone's two-line row. |
| P9-5 | deleting `.modal-close { flex: none }` and the coarse 44 px close override | The shrink guard beside a long title; the override is what the gate's primaryUnder44 relies on until the shared rule is proven. |
