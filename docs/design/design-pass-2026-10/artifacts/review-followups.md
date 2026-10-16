# Follow-ups collected from the package QA reviews (for the final cleanup pass, file ownership lifted)

## From P1 QA (minor)
- FRONTEND.md §7.1 `--bg-2` row says hover/press moved to `--bg-hover`/`--bg-4`; check that no live hover rule still paints `--bg-2` once P4–P9 land (components.css `.kv[data-lines] .kv-row:hover` and friends were listed).
- Record in FRONTEND.md that phone h3 is 13 px (was 14) — plan-internal inconsistency, not a defect.

## From P2 QA2 (minor)
- Landscape phone (844×390, 667×375): the expanded status list is 15–30 px taller than its max-height, so the two link buttons at its bottom are clipped until the list scrolls — give the list its bottom padding inside the scroll box or lower the max-height by the links' row.
- Stale CSS comment on `.status-scrim` ("a press on it folds the list") — the closer is now the scrim's click.
- Suggested e2e regression in mobile-shell.spec.ts: expand the strip, tap over a link under the scrim, assert URL unchanged and list hidden.
- P2-8 pre-existing: the wallet sheet's `.menu-scrim` closes on document pointerdown so a tap over a link clicks through (same ghost-tap class as the status scrim had) — fix the same way.

## From P3 impl/QA (cross-package)
- markets.css `.mk-popover-close` size rules (~l.581 and the coarse one) are dead now that the shared `.btn.mk-popover-close` rule wins — P5-8 deletes them (check it did).
- markets.css `.mk-choice / .mk-pool-choice input[type="radio"]` accent-color — P5-6 drawn radios (check).
- Modal.tsx, TierWhyPopover.tsx, Toast.tsx still render the font glyph ✕ inside the new 32 px shared close box — P9-5 / P5-8 (check they switched to the `close` icon).
- lend.css Withdraw preview at 1440 with a value typed: label column collapses to one word per line and "Locked shares after" overlaps its value (pre-existing; P7 should fix — check).

## From P4 QA (minor)
- 768 px markets table: the 12 px sort/chevron SVGs widen OPEN INTEREST / POSITIONS header buttons → the table is 750 px in a 742 px wrap (8 px sideways scroll in the wrap). Consider `priority` or a narrower sort gap at the tablet tier.
- `.tbl tbody tr:hover { background: var(--bg-hover) }` lifts rows of non-interactive tables too (status Venue programs, market Lenders/Pools/fees, security Scope) — scope the hover to `tr[data-interactive]` / rows with a link, or make it opt-out.
- Unit tests for formatSlot / formatAge / pad exist only as a scratch node test: /tmp/claude-1000/-home-todor-dev-Projects-short-win/042f05f3-c79b-4ca9-be93-bfe6ebc27df2/scratchpad/p4-tests/format.p4.test.ts — port them into apps/web/src/lib/format.test.ts.
- formatAge zero-pads minutes after an hour ("3h 05m"); owner may prefer "3h 5m" — leave, but note in docs.
- Card sort bar: the select's own chevron and the direction chevron are two down-pointing marks 20 px apart with different meanings — consider an arrow glyph (↓/↑) for direction.
- Skeleton cells carry data-sorted and inherit the card hero's 16 px, so the sorted column's placeholder is taller while loading — exclude skeleton rows from data-sorted or reset `.skeleton` height there.

## From P1 notes (owner decision, not implemented)
- Scrollbar gutter for 1,024–1,279 px (only safe form: `scrollbar-gutter: stable` on the dialog/sheet lock rule).

## From stage 2 QA (P8, P5, P6, P7, P9) — minor unless marked
- P8-1: on phones (≤639.98px) `.guide-body h2` at --fs-xl = 20 px equals the 20 px page title; add to guide.css's phone block `.guide-body h2 { font-size: calc(var(--fs-xl) - 2px) }` (18 px) so the title outranks the sections.
- P8-1: `.guide-lede { max-width: 60ch }` at 15 px leaves a one-word widow ("sign.") at 768–1440; try 64ch or `text-wrap: pretty` (safe: a hint).
- P8-3: `.error-state` is now a padded tinted box with a left bar; where it renders directly inside `.panel-body.flush` (ProtocolStatusPage's scan panel) check its margins (it may touch the panel edge).
- P8-3 vs P7-3: the shared CTA rule landed as `.empty-action > :where(.connect-cta)` — P7-3 deleted lend.css's copies; verify /lend no-wallet still shows the 360 px CTA (P7 QA said yes).
- P5-2: the unavailable grey Short action-bar button (`.btn.mk-action[data-variant="short"][data-unavailable]`) still gets the red `:active` press from components.css by cascade order — add a press rule for the grey state (bg-4) with higher specificity.
- P5-2: the panel default is decided before the market view loads (`flags: []` while loading → mounts on Short, flips to Lend) — defer the default until the view exists (useMarketData) or render the tabs disabled while loading.
- P5-4: the status-card token link rule `.mk-tbl td[data-col="token"] > a { min-height: 24px }` (0,2,2) beats `.target-link`'s 44 px coarse height — raise the coarse rule's specificity or drop min-height there.
- P5 (outside): `/markets?q=<digits only>` is JSON-parsed to a number by TanStack Router's default search parser and dropped by validateSearch — pre-existing; consider a custom `parseSearch` that keeps strings.
- P5-5 / P4-4: `Sol` mutes exact zeros only; a non-zero amount that rounds to 0.0000 renders bright next to muted zeros (Lenders tab 'SOL paid out') — mute by the displayed value, or show "<0.0001".
- P6-2/P6-5: the positions KPI strip shows 'VAULT 6.527 SOL' (unpadded) while the padded table cell shows '6.5270 SOL' — pad the strip's SOL values the same way.
- P6-4: slippage box is 120 px under 640 px and the Lend tolerance box 120 px everywhere, but the desktop Short slippage box is still full width — align (`max-width: var(--field-narrow-w)` from 1024 px too) if it reads better.
- P6-3: `side-sticky` turns on `.side-sticky .form-foot { box-shadow: var(--shadow-up) }` for both Manage feet; the Add-margin foot (mid-panel) casts a shadow onto the Close form below — scope the shadow to the last form foot or to the sticky-stuck state.
- P6 (pre-existing): the acknowledgement checkbox is 22×22 with a mouse (`--check`), under the 24 px floor if it ever becomes a judged target — the responsive gate measures the label row (44 px), so no failure; consider `--check: 24px` with a mouse.
- P7-1: on the 390 px phone the KPI strip's hero is --fs-lg (16 px), the plan expected 18 px at 390 (only 320 gets 16) — owner call; it reads fine.
- P7: 320 px withdraw preview: `.lend-nowrap` date value runs off-screen (x 153→356 in a 148 px dd) — allow wrapping at the date's "until" or drop nowrap under 400 px.
- P7: desktop 360 px column: the widest withdraw preview value (~257 px) squeezes the labels to three lines — let the value wrap after "until" (a `<wbr>` or `overflow-wrap`) instead of nowrap above 640 px.
- P7: leftover comment in lend.css (~l.120) now heads `.lend-kpi-note` which it does not describe — fix the comment.
- P7: `.lend-position` pads 8 px all round while `.short-preview` pads 8/12 — align to 8/12.
- P4/Num (outside P7): `.kv dd { overflow-wrap: anywhere }` + `Tokens` rendering no space before `<bdi class="unit">` lets "19…E2EMAIN" break mid-word at 320 px — add a thin space or `white-space: nowrap` on the unit pair.
- P9-3: tablet (768) toast over the side sheet: the bottom-left toast (x 12–372) overlaps the 440 px sheet (from x 329) by 44 px — pre-existing; shift the toast left column to `min(360px, 100vw - sheet)` or raise it above the sheet foot on tablets.
- P9-2: review dialog on a landscape phone (844×390): body 186 px — pre-existing; consider a shorter head/foot under `(orientation: landscape) and (pointer: coarse)` in the tablet tier.
- P9 docs drift: FRONTEND.md §7.6 "16 px clear of its ✕" → now 4 px clear of the 44 px ✕ box, sized to the head; §5.3 "priority selector" → a Normal / Fast chip pair. (P10)
- P5 owner decision left open: the compact intro strip on every phone (add (max-width: 639.98px) to markets.css's compact-strip prelude + breakpoints.ts comment) — explicit yes only.
