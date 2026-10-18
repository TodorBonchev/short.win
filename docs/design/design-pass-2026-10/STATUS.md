# Design polish: status

As of 2026-10-16. The design pass is on `main`. Its commits were rebased onto `main` at `d805c19` (pump v3 and the
Mobile Wallet Adapter, which landed the same day) and pushed to `origin` with `[skip ci]`, the owner's convention
since 2026-10-11. Every other branch was then removed, so `main` is the only branch, locally and on GitHub. The
removed refs are archived (see "Branch cleanup" below).

How the work was done, the decisions behind it and how to pick it up are in
[CONVERSATION-SUMMARY.md](CONVERSATION-SUMMARY.md).

## Summary

The logo was replaced and the web app's visual design was polished across every page, at phone and desktop
widths, inside the existing dark-terminal design system. On `main`, with freshly built 1.0.1 programs:

- Lint, typecheck and all unit tests pass.
- The responsive gate passes in full.
- The Playwright suite passes on desktop and phone.
- The 14 Android wallet-adapter failures were traced to Chromium's native external-protocol handoff and
  fixed in the test harness on 2026-10-18; the Android MWA spec now passes (19 passed, 2 flag-off skips).

What is left is the GitHub CI evidence.

## Done

### Logo (`5ac926d`)

- **The mark.** The "▼" text glyph in the header became a drawn SVG: the W of "Win" as a price chart. A
  full-height fall in the text colour (the short) joins through a low, sharp apex to a gold check (the win). It
  lives in `apps/web/src/shell/Logo.tsx`. It sits at 24 px beside "ShortWin" and at 30 px alone under 400 px.
- **Colour rules.** The fall half is `currentColor`. The win half is `fill: var(--accent)` through
  `.brand-mark-win`. Loss red is deliberately not used, so red keeps meaning a P&L loss.
- **Favicon and app icons** in `apps/web/public/`:
  - `favicon.svg`, rebuilt on the 16 px grid, plus `favicon-16.png`, `favicon-32.png` and `favicon.ico`.
  - `apple-touch-icon.png` (180 px).
  - `manifest.webmanifest` with rounded "any" icons and full-bleed "maskable" icons.
  - `icon-192.png`, the mobile wallet adapter's app icon, re-rendered from the new favicon (`1011177`).
- **How it was chosen.** Six concepts were rendered at every real size and scored by three judges, then the
  finalist was refined and measured pixel by pixel. The judges' totals slightly favoured another concept; see
  the logo section of the summary file.

### Design polish

Each package was implemented against the live app, reviewed by an independent QA agent with fresh screenshots,
and fixed until approved. Package ids (P1 to P11) match the commit messages.

| Commit | Package | What changed |
|---|---|---|
| `c41ba84` | P1 Tokens and base | Three label tiers (panel title 13 px bold, section 12 px bold, label 12 px muted). A 20 px page title and a 22/24 px hero size. New surfaces (`--bg-4`, `--bg-hover`, `--line-panel`, well, note, nested) and six shadows. Lighter muted text, amber `--warn` so gold means only the primary action, grey informational tags. Structural sizes: page gaps, 28 px key/value rows, a 360/320 px side column, a 52 px header under a finger. A 12 px gutter and a reserved scrollbar gutter from 1,280 px. |
| `e10a5fd`, `abe049a` | P2 Shell | One SVG icon registry (`src/shell/icons.tsx`). Under 400 px the wordmark hides only while the cluster badge shows, so it stays on mainnet. Header grouping and shadow. The phone status strip as a tinted band with a drawn chevron and a scrim that closes on click, so a tap never falls through to the page. Landscape phones get the strip. Gold tab-bar indicator, action-bar shadow, shared page rhythm and side column, wallet sheet and chooser polish, More sheet rows with icons and chevrons. |
| `21dd64c` | P3 Core components | Panels on the title tier with a crisp edge. Disabled, busy and long submit buttons. Hover and press per button variant. One shared close button. App-drawn selects, checkboxes and radios. The acknowledgement as a full control row. Tabs with hover, a two-sided overflow fade and a `sub` variant. Field errors above the hint. Grey tags, ringed neutral badges, capitalised status badges. One grammar for toned boxes. Glossary marks, Reveal, copy and external icons. |
| `f3c7500` | P4 Tables and numbers | Muted zeros, row hover, headers on the label tier, a gold line under the sorted column, SVG sort marks. A sticky table header from 1,024 px, except the positions and lend tables. A hero figure in phone cards, padded decimals, `formatSlot` and `formatAge`. Key/value rows on the 28 px grid with an emphasis hook. Stat-strip dividers and a `Stat` `hero` prop. |
| `16c8906` | P8 Reading pages and states | How it works and Security get a 15 px lede, a 14 px body at 1.55 line height and clearer section spacing. Their contents box wraps on phones and becomes a sticky side rail from 1,280 px. Empty states become a framed block with a 360 px Connect button. Errors become a left-bar note with Retry. |
| `fcaea1b` | Fix | An empty list hides its whole table, which fixes the axe `aria-required-children` failure on every empty list. The E2E harness no longer throws on a BigInt inside a simulation error. |
| `91a48fe` | P5 Markets | Phone toolbar on one row. The trade panel opens on Lend where shorting is blocked. The EMA price is the market page's hero number. Rows look openable; venue tags are quiet. Utilisation meter on a visible track. Create market on the shared side column with drawn radios. Empty and indexer-down states carry their action, with a new test id `markets-empty-create`. Status page surfaces. |
| `e3b6da6` | P6 Short | Trade-ticket hierarchy: the preview as a sunken well, emphasised key rows, caps as one grey block with the binding cap in amber. The position page's CR as its hero. A positions KPI strip. The Manage panel on the shared sticky column. A narrow slippage input. |
| `e129fa4` | P7 Lend | A page-wide KPI strip with "SOL claimable" as the hero. Deposit, Withdraw and Claim as segmented sub-tabs. The position summary as a well. The no-wallet button moved to the shared empty state. |
| `bc25144` | P9 Overlays | The sheet head clears the notch. Modal body height and surfaces. A toast over an open sheet takes the title strip. The priority fee as Normal / Fast chips. Close icons everywhere. |
| `1ba6805` | P11 Cleanup | 34 follow-ups from the package reviews; see the next section. |
| `cb91e3d` | P10 Docs | `docs/design/FRONTEND.md` records the pass as built: shell, every page, review dialog, numbers, components, motion, responsive rules and the width table. A second reader checked every claim against the code and caught five inaccuracies, all corrected. |
| `1011177` | Fix after the rebase | `icon-192.png`, the wallet adapter's app icon that upstream renders from `favicon.svg`, showed the old logo, because the renderer only understood the old favicon's stroked line. The renderer now also draws filled shapes, and the icon is regenerated. |
| `6b4ce96`, `9363b4c` | Screenshots | Every reference set re-shot against the production build: desktop (38, including 70–74), phone (40) and the Android set the wallet adapter added (9). |

The folder holding this file and the TODO.md entry are in the final commit.

### The cleanup pass (`1ba6805`)

It closed every reviewer note that needed no owner decision. The main ones:

- **Landscape phones:** the expanded status list flows two items a row, so its links are no longer clipped.
- **New regression test** in `e2e/specs/mobile-shell.spec.ts`: a tap on the page under the status scrim only folds the list.
- **768 px markets table:** fits its width; the token cell wraps symbol over mint.
- **Row hover:** only interactive rows react to the pointer. Read-only tables stay still.
- **Card sort bar:** the direction button uses an arrow icon, distinct from the select's chevron.
- **Skeleton rows:** no longer take the sorted column's hero size while loading.
- **How it works on phones:** section headings are 18 px, below the 20 px title. The intro no longer ends in a one-word line.
- **Error states:** inset inside flush panels.
- **Market page:** the grey Short button has its own press state. The Short/Lend panel default waits for the market data, so it no longer flips after load.
- **Touch targets and search:** the status page's token links get a 36 px target under a finger. `/markets?q=<digits>` keeps the digits as text.
- **Zeros:** muted by the displayed digits, so `0.0000` never shows bright beside muted zeros.
- **Positions:** vault and rent are padded in the KPI strip. The desktop slippage box is 120 px. Only the last Manage form casts a footer shadow.
- **Acknowledgement checkbox:** at least 24 px with a mouse.
- **Lend:** dates wrap between date and time. Withdraw-preview labels are no longer squeezed. The position well's padding matches the short preview.
- **Units** never break mid-word.
- **Overlays:** the tablet toast clears the side sheet. The landscape review dialog body is taller.

### Tests added or changed

- `src/lib/format.test.ts`: `formatSlot`, `formatAge` (including the "3h 05m" case) and the `pad` path.
- `src/components/components.test.tsx`: `data-zero` by displayed digits, and no `data-sorted` on skeleton rows.
- `src/components/responsive.test.tsx`: the sort control's text expectations follow the SVG icons.
- `src/router.test.tsx`: the `/m/$mint` row follows the deferred panel default.
- `src/wallet/mobileWalletAdapter.test.ts`: its icon check recolours the gold half instead of the old red stroke.
- `e2e/specs/mobile-shell.spec.ts`: the scrim ghost-tap regression.
- `e2e/validator/actions.ts`: the BigInt-safe simulation error.

## Verification

This was run on `main` as pushed. The programs were freshly built from it (`anchor build --tools-version v1.57
--arch v3` and `scripts/build-test-venue.sh`), the validator freshly seeded, and the app the production build.

| Check | Result |
|---|---|
| `pnpm --filter web lint` | pass |
| `pnpm --filter web typecheck` | pass |
| `pnpm --filter web test` | 1,026 passed, 1 skipped |
| Playwright full suite: desktop, phone and Android projects | 110 passed, 2 skipped, 14 failed |
| Responsive gate, 8 viewports, `SHORTWIN_RESPONSIVE_GATE=1` | 96 passed |
| GitHub CI | not run (pushed with `[skip ci]`) |

**The 14 failures** are all in `e2e/specs/mobile-mwa.spec.ts` on the `chain-android` project: M3, M4 to M9,
M10, M10b, M13 to M16/17 and M18. Each times out after about 30 seconds, typically on a tap. Untouched upstream
`main` (`d805c19`), run in a separate worktree with the same fresh programs, fails exactly the same 14 tests on
this machine. They are therefore not caused by the design pass. The owner's own gate on 2026-10-16 recorded the
web E2E as green ("122 passed", TODO.md), so the difference is this machine's environment. One untested guess:
this machine runs the system Chromium, because Playwright's bundled browser is not installed.

**Android follow-up, 2026-10-18:** reproduced M3's timeout with system Chromium 152.0.7977.82, then reduced
it to a two-button HTML page: the first button calls `location.assign('solana-wallet:…')`; the next real
touch hangs at Playwright's "performing tap action". The same failure occurs using the browser executable
directly, without the system launcher's user flags. The fake wallet had already authorized successfully;
the native external-protocol handoff was blocking later input, including after a reload.

`e2e/wallet/mwa-hooks.js` now cancels only `solana-wallet:` navigations with the Navigation API. The harness
already supplies Android's app switch and wallet; cancelling the desktop handoff leaves the real adapter,
association handshake, signing and CSP checks in place. M2 now also taps the connected wallet button to
check that touch input still works. M3 passed in 3.5 s after previously timing out at 33.7 s. The complete
spec, `SHORTWIN_E2E_KEEP=1 SHORTWIN_E2E_REUSE=1 pnpm --filter web e2e mobile-mwa.spec.ts
--project=chain-android`, passed **19 tests**, with the **2 flag-off tests skipped** (1.5 min). No timeout
was increased.

The same spec was then rerun with a fresh ledger and the draft audit-fix test-venue build
`c3689cc688690ffdaa247a25e16d652a73d504e887a2d97df95d65ef4f791897`: **19 passed, 2 skipped** (3.2 min,
including seeding). The seed now records and completes fresh price buckets with real cranks after its
50-second listing warm-up, before opening the seeded positions, to satisfy the new open-price age guard.

**Resolved since the first report:**

- The three lend chain-scenario specs (`lend-claim-sync`, `lend-post-wipe`, `lend-claim-pot-chain`) pass.
- The two responsive scenarios built on them ("lend: a shortfall", "a second lender") pass.

The cause was the stale local program build, as suspected; `target/` now holds fresh 1.0.1 builds.

## Left to do

1. **CI on GitHub.** Nothing in this pass has run in CI, like the pump v3 work before it (TODO.md lists that too).
   Run the `ci` workflow.

### Known minor issues, reviewed and not fixed

- **Markets table in mouse windows 768 to 985 px wide.** The token cell wraps to two lines, so rows are taller
  there. This is a side effect of the 768 px fit fix, which keys on the table's container width only.
- **Deposit form on desktop.** Some labels wrap to two lines beside short values. This is a side effect of the
  1fr / 1.2fr key/value columns in `lend.css`.
- **Precision mismatch.** The position detail page shows "Vault 6.527 SOL" unpadded while the positions list
  shows "6.5270 SOL".
- **Portrait tablets 640 to 700 px wide.** The toast's width floor is 176 to 236 px, so toast text wraps more.
- **Desktop screenshots.** Headless Chromium at ≥ 1,280 px shows a 15 px empty strip on the right: the reserved
  scrollbar gutter. This is expected.
- **FRONTEND.md figures taken from code comments and not re-measured:**
  - the ≈ 666 px fit of the desktop status bar, below which a mouse window clips "Security";
  - the landscape review dialog body growing from 186 to 242 px;
  - the tinted status strip's contrast ratios.

### Found outside the design scope

- **`pnpm dev:e2e` crashes five routes** with "process is not defined", thrown from
  `clients/js/src/generated/errors/shortwin.ts`. The dev server does not define `process.env.NODE_ENV` for the
  workspace client's TypeScript source. The production build is unaffected. The workaround used here is
  `apps/web/.e2e/vite.dev.config.ts`, also copied to `artifacts/dev-loop/`. A real fix belongs in
  `apps/web/vite.config.ts` as a dev-only define, or in the client generator.
- **Building the web app needs `wasm-bindgen-cli` 0.2.129.** It was installed on this machine during the session
  with `cargo install wasm-bindgen-cli --version =0.2.129 --locked`.

## Branch cleanup

On 2026-10-16, after the push, every branch except `main` was removed. All of them were checked first.

| Removed | Where | Its work |
|---|---|---|
| `design-polish` | local | rebased onto `main` and fast-forwarded into it |
| `m1-core` | local and `origin` | an ancestor of `main` |
| `air-followups`, `ai-review-fixes`, `pump-v3` | `origin` | ancestors of `main` |
| `fix/d`, `ki/r11`, `ki/fe30`, `ki/ki65`, `ki/ki65b`, `ki/ki66`, `ki/ki67`, `fz/a`, `fz/b` | local remote-tracking refs with no remote | see below |

The last group are leftovers fetched from earlier sessions' scratch clones under `~/.cache/shortwin-work/m10/`.
Their commits are not in `main` by hash or patch, because that work was squashed or reworded on its way in. A
distinctive marker from each (for example `FUZZ-KI67-FIX-1`, `absent_owners_at_the_wind_down`,
`FUZZ-M10-24H-D-B`) is present in `main`'s tree, and the clones they came from are still on disk.

**Archive.** Every removed ref, and the pre-rebase tip of `design-polish` (`e5e626b`), is in one verified bundle:
`/home/todor_dev/Projects/short.win-archived-branches-2026-10-16.bundle` (80 MB). To restore one:

```bash
git fetch /home/todor_dev/Projects/short.win-archived-branches-2026-10-16.bundle refs/remotes/ki/ki66:refs/heads/ki66
```

Left alone: the tags (`audit-1`, `v1.0.0`), and two detached worktrees of earlier fuzz runs
(`~/.cache/shortwin-work/fuzz24c/repo`, `fuzz24d/repo`). Worktrees are not branches.

## Where things are

- **Logo:** `apps/web/src/shell/Logo.tsx`, `apps/web/public/` (icons and manifest; `scripts/icon-192.ts` renders
  `icon-192.png`), `.brand*` rules in `apps/web/src/styles/components.css`.
- **Design system:** `apps/web/src/styles/tokens.css`, `base.css`, `components.css`, and the feature stylesheets
  under `apps/web/src/features/*/`.
- **The contract:** `docs/design/FRONTEND.md` §7 (brand paragraph and package rows) and §3 (pages as built).
- **This folder's `artifacts/`:**
  - `design-plan.md`: the 1,513-line verified plan the packages followed.
  - `review-followups.md`: every QA note the cleanup pass worked from.
  - `logo/`: the concept contact sheet, the final logo files with their generator scripts, and the runner-up.
  - `dev-loop/`: the screenshot and measurement helpers.
  - `workflows/`: the five orchestration scripts used.
- **Local helpers:** the originals of the dev-loop helpers are in `apps/web/.e2e/`. That folder is gitignored and
  E2E runs write into it, so keep the copies in `artifacts/dev-loop/`.
