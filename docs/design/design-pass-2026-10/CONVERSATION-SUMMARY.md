# Design polish: conversation summary and handoff

A Claude Code session on 2026-10-16 did this work and put it on `main`. This file explains what was asked, what
happened in order, which decisions were taken and why, and how to continue. For the done/left checklist see
[STATUS.md](STATUS.md).

All commit hashes in this folder are the ones on `main`. The work was first committed on a branch and then
rebased, so the hashes the session reported before the rebase no longer exist outside the archive bundle
described in STATUS.md.

## The request

> pull latest branch and switch to it. then on a new branch (branched from latest) Polish the logo of short.win
> and make it look great. Also start a workflow to polish the design of the website so website looks great and
> intuitive and easy to use both on phone and pc

Two later requests in the same session:

> Can you summarize what is left and what was done in a file locally and summarize this conversation in a file so
> someone else can continue?

> Check if main is latest branch and commit everything to main including to folder with whats done and whats left
> and remove other branches so main is the only source of truth

The session ran at maximum effort with multi-agent workflows switched on for the design work. Claude acted as the lead: it scouted
the code itself, wrote each workflow script, delegated design and implementation to subagents, checked their
output, ran the gates and made every commit decision. The owner was not consulted during the work. Every choice
that the plan marked as the owner's was left undone and is listed in STATUS.md.

## What happened, in order

1. **Branch.** `git fetch` showed `main` as the newest branch (`c0e0900`, 2026-10-13). `air-followups` pointed at
   the same commit. The checked-out branch, `m1-core`, was 223 commits behind. Claude checked out `main`,
   fast-forwarded it and created `design-polish`.
2. **Scouting.** The web app is `apps/web`: Vite 8, React 19, TanStack Router and Query. Its design contract is
   `docs/design/FRONTEND.md` §7, a dense dark trading terminal with hard rules (listed below). The styles are
   `tokens.css`, `base.css` and a 2,758-line `components.css` plus feature stylesheets. The E2E harness runs a
   local `solana-test-validator` with seeded markets, positions and lenders. The committed reference screenshots
   showed the old identity: a red "▼" text glyph before "ShortWin" and a red falling-line favicon.
3. **A live visual loop.** The full Playwright run takes about 9 minutes, so Claude built a fast loop:
   - the E2E validator and seed, left running;
   - the Vite dev server with HMR;
   - a screenshot helper, `apps/web/.e2e/shots.ts`, that renders any route at phone, tablet or desktop size with a
     seeded test wallet already connected.

   Three problems were fixed on the way: Vite bound only IPv6 `localhost`; the dev server crashed five routes with
   "process is not defined" (fixed with a wrapper config); and `wasm-bindgen-cli` 0.2.129 was missing (installed,
   then the math WASM package rebuilt).
4. **Logo, round 1.** Six designer agents each explored one direction:
   - fall-to-check: a falling line that becomes a check;
   - an S/W monogram;
   - a W made of two down-arrows;
   - bear candles;
   - a cut tile;
   - wordmark-first: the W of "Win" drawn as a price chart.

   A seventh agent built a contact sheet of every concept at 16, 20, 24, 32 and 48 px, light and dark. Three
   judges scored them through brand, product-UI and icon lenses. The scores are in the logo section below.
5. **Logo, round 2.** One agent refined the W into final files and a second measured them pixel by pixel. Claude
   wired the mark into the header (`Logo.tsx`, CSS, `index.html`, the manifest, a FRONTEND.md paragraph), ran
   the gates and committed `5ac926d`.
6. **Design audit.** Eight agents audited the committed screenshots and the code, one lens each:
   - hierarchy, type and spacing;
   - colour, surfaces and states;
   - the phone shell;
   - the desktop shell;
   - data presentation;
   - forms and flows;
   - first-use intuitiveness;
   - component consistency.

   They returned 100 findings. A synthesis agent merged them into 9 work packages, 56 items, with disjoint file
   ownership, and dropped 16 findings with reasons. Two skeptics reviewed every package: one against the hard
   constraints and tests, one for whether the change really looks better. The verified plan, 1,513 lines, is in
   `artifacts/design-plan.md`.
7. **Implementation, stage 1: foundations.** P1 tokens, P2 shell, P3 components and P4 tables ran one after
   another. Each had an implementer working against the live app, then an independent QA agent with fresh
   screenshots (320 px and tablet included), a diff review and the gates, then up to two fix rounds. Each
   implementer committed only its own files. P2's QA rejected a status-strip scrim that let a tap fall through to
   the page; that was fixed in `abe049a`. A session usage limit interrupted the run after P2. It resumed from the
   workflow cache with nothing lost.
8. **First E2E check.** This run reused the dev-loop validator. It found two real problems, both fixed in
   `fcaea1b`:
   - P4 hid an empty list's header but left an empty `role="table"`, which axe rejects. Now the whole table hides.
   - A harness helper crashed on a BigInt while formatting a simulation error, which hid the real error.
9. **Stage 2.** P8 (reading pages and shared empty/error states) ran first because P7 depends on it. Then P5
   markets, P6 short, P7 lend and P9 overlays ran in parallel in the same working tree, each touching only its own
   files. Claude committed them separately by ownership. P5 needed one docs row it did not own: FRONTEND.md §11.2
   must list every test id, and `testids.test.ts` enforces it. Claude applied that patch.
10. **Cleanup, P11.** One agent with ownership lifted worked through every QA note from the nine reviews. It
    closed 34, left 6 (owner decisions and docs-only notes) and was approved in one round (`1ba6805`).
11. **Final verification.** Claude stopped the dev loop and ran the full suite and the responsive gate, each on a
    freshly seeded validator, then refreshed the committed reference screenshots (`6b4ce96`). The results are in
    STATUS.md. Three lend chain-scenario specs failed and two responsive scenarios with the same chain setup
    failed. Every design-related check passed.
12. **Docs, P10.** One agent wrote the FRONTEND.md rows for everything that landed. A second agent checked every
    claim against the code and found five inaccuracies, which were corrected (`cb91e3d`).
13. **Bisect.** The same three specs were run on `main` in a separate git worktree against a fresh validator, and
    they fail there too. The local test-venue program binary predates 31 program commits, and the errors (an
    address mismatch on `event_authority`, events that do not decode) fit a binary older than the client. That
    points to a stale local build, but it is not proven.
14. **Wrap-up.** Claude wrote memory notes for future Claude sessions, stopped the validator and servers, and
    removed the worktree.
15. **Handoff files.** At the owner's request Claude wrote this folder. It also copied the plan, the review notes,
    the logo sources, the helpers and the workflow scripts out of the temporary scratch folder into `artifacts/`.
16. **Main had moved.** On the request to put everything on `main`, a fresh `git fetch` showed `origin/main`
    59 commits ahead of the branch's base: pump v3 and the Mobile Wallet Adapter, pushed that day. A new branch
    `pump-v3` pointed at the same commit (`d805c19`). Ten files had been changed on both sides.
17. **Rebase.** `main` has never had a merge commit, so the 15 design commits were rebased onto `d805c19`
    instead of merged. Git stopped once, in the P2 shell commit, with two conflicts:
    - `components.css`: upstream added a rule for the in-app browser block exactly where P2 added a comment.
      Both were kept.
    - `WalletChooser.tsx`: upstream moved the open-in links into an `<OpenInLinks />` component. P2's two edits
      (the external icon, the `wallet-note` class) were moved into that component.

    Everything else, FRONTEND.md included, merged without conflicts and was checked afterwards.
18. **A semantic conflict the merge could not see.** Upstream renders `public/icon-192.png`, the wallet adapter's
    app icon, from `favicon.svg`, and its test checks the pixels. The renderer only understood the old favicon's
    stroked line, so the icon still showed the old logo. Claude taught the renderer filled shapes and regenerated
    the icon (`1011177`).
19. **Full verification on the new base.** Claude built the 1.0.1 programs from the new `main` in a separate
    worktree and copied them into `target/`, replacing the stale 2026-10-01 builds. It then ran the full suite
    (desktop, phone and Android) and the responsive gate, each on a fresh validator, with screenshot update on so
    every reference set was re-shot (`9363b4c`).
    - The gate passed 96 of 96.
    - The three lend specs passed, which confirmed the stale-build diagnosis.
    - 14 Android wallet-adapter tests failed. Running that spec on untouched upstream `main` in the build
      worktree gave the same 14 failures, so they are not caused by the design pass.
20. **Record and push.** Claude rewrote this folder for the final state, added a design-pass section to TODO.md,
    committed with `[skip ci]` (the owner's convention since 2026-10-11, so no CI run was started),
    fast-forwarded `main` and pushed it.
21. **Branch cleanup.** Every other branch and leftover ref was archived into one verified git bundle and then
    removed, locally and on GitHub. STATUS.md lists each one and how to restore it.

## Decisions and why

### The logo

| Concept | Product-UI judge | Brand judge | Icon judge | Total |
|---|---|---|---|---|
| fall-to-check | 37 (1st) | 40 (1st) | 33 (2nd) | 110 |
| wordmark-first, the W (shipped) | 33 (3rd) | 37 (2nd) | 36 (1st) | 106 |
| w-of-arrows | 31 | 30 | 32 | 93 |
| bear-candle | 34 | 29 | 28 | 91 |
| cut-tile | 29 | 25 | 29 | 83 |
| sw-monogram | 28 | 24 | 24 | 76 |

Claude shipped the W although fall-to-check had the higher total. The reasons:

- **Favicon legibility.** The icon judge, whose lens matters most at 16 px, ranked the W first. It called the W
  "the only concept that is legible as a shape at 16 px". It said fall-to-check reads as a generic "verified"
  tick in a tab strip and that its red stroke looks cropped at the tile's edge.
- **Red stays a number colour.** Fall-to-check puts loss red into the brand. In this UI red means a P&L loss.
- **A font-independent lockup.** The W's lockup uses outlined letters, so it renders identically on every OS.

The trade-off the other judges raised: fall-to-check tells the "short, then win" story without a caption, and it
keeps gold out of the header so the Connect button stays the only gold there. **This was Claude's call and was not
put to the owner.** The runner-up's files are kept in `artifacts/logo/runner-up-fall-to-check/`.

Refinement decisions:

- No "shelf" variant. A flat price line entering the W read as a stray serif at 16 px and narrowed the left counter.
- No keyline on the favicon tile. At 16 px it smudged into a half-pixel rim.
- The lone mark is 30 px, not 28. On the 24-unit grid, 30 px puts the cap and baseline on whole pixels at every
  device pixel ratio; at 28 px they rendered as grey rows.
- The favicon was rebuilt on its own 32-unit grid: a stroke of 4 units, which is 2 solid pixels at 16 px, with
  the W spanning x 2 to 30 and the cap and baseline on even rows.
- Two icon sets in the manifest. The "any" icons keep transparent rounded corners for desktop install surfaces.
  The "maskable" icons are full-bleed with the W inside the central 80 % for Android's masks.

### The design direction

From the plan's §1; every package followed it.

- **Keep the dark, dense, data-first terminal.** The polish adds rhythm and states, not decoration.
- **Three label tiers do all the labelling.** A panel title is 13 px bold white, a section heading 12 px bold
  light grey, a label 12 px medium muted. The page title is 20 px. A hero number (the EMA price, a position's CR)
  is 22 to 24 px, so the number a trader reads first always outranks the title.
- **One meaning per surface.** bg-0 is the page and the sunken well for read-only figures. bg-1 is a panel with a
  crisp edge. bg-2 is a note. bg-3 is an input. bg-4 is a bg-2 surface under the pointer.
- **Gold means only the primary action or the selected state.** Caution moved to amber. Informational tags turned
  grey so nothing static looks clickable.
- **Spacing groups.** 12 px between boxes, 16 px under a page head. The density inside a box is unchanged.
- **Every control glyph is one SVG icon** from `src/shell/icons.tsx`. Font glyphs no longer draw controls.

### The constraints every change respected

Several of these are enforced by tests.

- One DOM at every width: width switches are CSS only.
- A closed set of breakpoints, enforced by `src/styles/breakpoints.test.ts`; no new media queries.
- CSP `style-src 'self'`: no inline styles, no web fonts, no remote images.
- WCAG 2.2 AA contrast, no text under 12 px at any width, 24 px targets and 44 px primary controls under a finger.
- Hover styles only under `(hover: hover)`. Reading order equals DOM order.
- New values become tokens in `tokens.css`.
- Owner-approved copy was never reworded, and every `data-testid` and every class the CSS or E2E relies on was kept.

### Process decisions

- **One file, one owner.** Each package owned its files, so parallel agents never edited the same file. The
  components.css sections were split between packages by their section banners.
- **Implementers and reviewers were separate agents.** A package counted as done only when an independent QA agent
  approved it with its own screenshots and gate run.
- **Owner decisions were held back.** Where the plan said "implement only with an explicit yes", nothing was
  implemented. Those items are listed in STATUS.md.
- **Rebase, not merge.** `main`'s history is strictly linear, so the design commits were replayed onto the new
  `main` instead of joined with a merge commit.
- **`[skip ci]` on the push.** Since 2026-10-11 the owner has pushed with `[skip ci]` and runs CI deliberately;
  TODO.md lists "CI on GitHub" as open for pump v3. The design push follows that convention, so local
  verification was made as complete as possible instead: freshly built programs, the full suite, the gate.
- **The icon renderer was generalised, not bypassed.** Pointing the wallet adapter at another icon file would have
  changed upstream's design. Teaching its renderer filled shapes keeps the icon a pixel-checked rendering of the
  favicon.
- **Archive before deleting.** The leftover `fix/`, `ki/` and `fz/` refs had commits that are not in `main` by
  hash. Their content is in `main` (checked by distinctive markers), but they were still archived into a bundle
  first, so the cleanup can be undone.
- **Restrained options in the cleanup.** Zeros are muted by the displayed digits instead of showing "<0.0001",
  because a new string would change owner-approved copy. The acknowledgement checkbox got its own 24 px floor
  instead of a token change. Digit-only searches are kept in `validateSearch` instead of a custom URL parser that
  would change every route's URL format.

## How to continue

1. Work from `main`, the only branch. `git log --oneline d805c19..main` lists the design pass.
2. Do the "Left to do" items in STATUS.md: the owner's review and decisions, a CI run on GitHub when wanted, and
   the 14 Android wallet-adapter tests that fail on this machine.
3. For more visual work, use the loop below and judge every change at phone and desktop width, 320 px included.

### The visual loop

From `apps/web`. The originals of the helpers live in `.e2e/`, which is gitignored; copies are in
`artifacts/dev-loop/`.

```bash
node e2e/validator/start.ts --env    # validator on :18899, seed, .env.e2e.local (about 2 minutes)
./node_modules/.bin/vite --config .e2e/vite.dev.config.ts --mode e2e --host 127.0.0.1   # http://127.0.0.1:5173
node .e2e/shots.ts --out /tmp/shots --viewports phone,desktop --wallet margin --routes '/markets,/m/$main'
node .e2e/measure.ts --viewports laptop-l,desktop --wallet margin   # sideways overflow per page
node e2e/validator/stop.ts           # stop the validator and any preview
```

Helper details:

- **Viewports:** `phone-xs` 320, `phone-s` 360, `phone` 390, `phone-landscape` 844×390, `tablet` 768,
  `laptop` 1024, `laptop-l` 1280, `desktop` 1440.
- **Wallets:**
  - `margin` has an open short on the main market.
  - `claimer` is a lender there.
  - `trader` is connected with nothing.
  - `none` installs no wallet; `--disconnected` installs one without connecting.
- **Route placeholders:** `$main`, `$move`, `$paused`, `$t22`, `$wipe`, `$pot` and `$chain` become a seeded
  market's mint; `$pos-margin` becomes a seeded position. `?sheet=trade` opens the phone trade sheet.
- **Interactions:** `--click` takes selectors separated by `,,` and `--type 'selector::text'` fills a field.
  Prefer the `data-testid` values in `src/config/testids.ts`.

### The gates

From `apps/web`:

```bash
pnpm --filter web lint && pnpm --filter web typecheck && pnpm --filter web test
pnpm --filter web e2e                     # full suite, desktop and phone (about 9 minutes)
SHORTWIN_E2E_RESPONSIVE=1 SHORTWIN_RESPONSIVE_GATE=1 pnpm --filter web e2e --grep @responsive   # about 16 minutes
pnpm --filter web exec node e2e/responsive/summary.ts --pages   # metrics of the last gate run
SHORTWIN_E2E_SCREENSHOTS=update pnpm --filter web e2e e2e/specs/screens.spec.ts   # desktop reference shots
SHORTWIN_E2E_SCREENSHOTS=update pnpm --filter web e2e e2e/specs/guide.spec.ts     # desktop shots 70 to 74
SHORTWIN_E2E_SCREENSHOTS=update pnpm --filter web e2e --project chain-phone       # phone reference shots
```

### Gotchas

- **Use a fresh validator for authoritative results.** Leave `SHORTWIN_E2E_REUSE` unset. The dev loop and
  earlier runs change chain state, and the first check in this session ran on a reused validator.
- **One validator, two users.** An E2E run without `SHORTWIN_E2E_REUSE` restarts the validator that the dev loop
  uses.
- **Expected oddities in dev-server screenshots.** They say "Live closed" or "Live feed reconnecting": React
  StrictMode closes the live socket once in development, and the production build shows "Live open". Headless
  desktop screenshots at 1,280 px and wider show a 15 px strip on the right, which is the reserved scrollbar
  gutter.
- **Browser binary.** Playwright and the helpers use `/usr/bin/chromium`. The bundled Playwright browser is not
  installed on this machine, which may be why the Android wallet-adapter tests fail here.
- **Program binaries.** `target/deploy/` and `target/test-venue/deploy/` hold builds of `main`'s programs as of
  2026-10-16. Rebuild them after any change under `programs/` (STATUS.md has the commands), or the lend chain
  scenarios fail in ways that look like app bugs.
- **Playwright's `--project` takes several values.** Put the spec path before it, as in
  `pnpm --filter web e2e e2e/specs/mobile-mwa.spec.ts --project=chain-android`, or the path is read as a
  project name.
- **Notes for future Claude sessions** are in `~/.claude/projects/-home-todor-dev-Projects-short-win/memory/`:
  `web-visual-dev-loop.md` and `shortwin-design-pass-2026-10.md`.
- **The session's scratch folder under `/tmp` will not survive.** Everything worth keeping from it is in
  `artifacts/`.

## The files in `artifacts/`

| Path | What it is |
|---|---|
| `design-plan.md` | The verified 9-package plan: principles, constraints, every item with its exact change, rationale, verification and the dropped findings. Its paths are relative to `apps/web/`; its line numbers refer to the files before each package landed. |
| `review-followups.md` | Every note the QA agents left. The cleanup pass worked from this list; STATUS.md records what is still open. |
| `logo/contact-sheet.png` | All six concepts at every size, light and dark. |
| `logo/final/` | The shipped logo's source files, its README (geometry and implementation notes), `preview.png`, and the Python scripts that generated the files. `preview.py` loads `InterVariable.ttf`, which is not copied; the other two scripts need no font. |
| `logo/runner-up-fall-to-check/` | The other finalist: its SVGs, README and preview. |
| `dev-loop/` | `shots.ts`, `measure.ts`, `vite.dev.config.ts`. Copy them into `apps/web/.e2e/` to use them. |
| `workflows/` | The five orchestration scripts: logo concepts, logo final, design audit, implementation, docs pass. Their absolute paths point to the scratch folder, which no longer exists. |
