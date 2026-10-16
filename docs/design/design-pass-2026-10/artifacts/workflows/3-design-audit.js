export const meta = {
  name: 'design-audit',
  description: 'Audit the short.win web app design through eight lenses from the committed screenshots and the code, synthesise work packages, adversarially verify each, write the plan',
  phases: [
    { title: 'Audit', detail: 'eight lenses over desktop and phone screenshots + CSS/TSX' },
    { title: 'Synthesize', detail: 'merge, dedupe, rank, partition into work packages by file ownership' },
    { title: 'Verify', detail: 'two skeptics per package: constraints and design quality' },
    { title: 'Plan', detail: 'the implementation plan as markdown' },
  ],
}

const REPO = '/home/todor_dev/Projects/short.win'
const WEB = `${REPO}/apps/web`
const SCRATCH = '/tmp/claude-1000/-home-todor-dev-Projects-short-win/042f05f3-c79b-4ca9-be93-bfe6ebc27df2/scratchpad'
const SHOTS = `${WEB}/e2e/screenshots`

const DESKTOP_SHOTS = ['01-markets','02-markets-search-no-market','03-status','04-not-found','05-create-market-ok','06-create-market-rejected','07-create-market-pump-curve','10-market-overview','11-market-tab-coverage','11-market-tab-events','11-market-tab-lenders','11-market-tab-liquidations','11-market-tab-pools','11-market-tab-positions','12-market-tier-why','13-market-coverage-paused','14-market-list-only','15-market-1024','16-markets-1024','20-short-preview','21-short-review','22-short-blocked','30-positions','31-position-detail','40-lend-claim','40-lend-deposit','40-lend-withdraw','41-lender-dashboard','42-lend-cpi-guard-notice','43-lend-cpi-guard-bracket-review','50-fallback-market','51-fallback-markets','60-security','70-markets-intro','71-glossary-open','72-how-it-works','73-short-coverage-link','74-lend-deposit-order']
const PHONE_SHOTS = ['coverage-banner-focused','coverage-link','create-market','create-market-rejected','glossary-open','how-it-works','lend-dashboard-claim-all','lend-dashboard-no-wallet','lend-dashboard','lend-deposit-order','manage-sheet-close','manage-sheet','market-events','market','markets-cards','markets-filters','markets-intro','markets','market-tier-why','market-trade-sheet','more','not-found','position','positions-no-wallet','positions','review-sheet-deposit','review-sheet','search','security','status-expanded','status','toast-short-open','trade-sheet-lend-no-wallet','trade-sheet-lend','trade-sheet-lend-ready','trade-sheet-lend-withdraw','trade-sheet-no-wallet','trade-sheet-short','wallet-connected','wallet-no-wallet']

const CONTEXT = `PRODUCT: short.win — a web app to short any Solana token by borrowing it from lenders who earn rent. Vite + React 19 SPA at ${WEB}. The owner's standing request: "make it look great and intuitive and easy to use both on phone and PC", keeping the dark trading terminal.

CURRENT STATE: the committed screenshots are the app as it is now (production build, seeded local validator, 2026-10-11):
- desktop 1440×900 (full-page): ${SHOTS}/<name>.png for ${DESKTOP_SHOTS.join(', ')}
- phone 390×844 (iPhone emulation): ${SHOTS}/phone/<name>.png for ${PHONE_SHOTS.join(', ')}
Open them with the Read tool (it renders PNGs). Look at MANY of them, at both widths; your findings must cite what you saw.

CODE MAP:
- Styles: ${WEB}/src/styles/tokens.css (design tokens + the breakpoint rules in its header comment), base.css, components.css (2758 lines; sections marked /* ---- name ---- */: shell, shell tablet, shell phone, layouts, panel, key/value grid, stat strip, buttons, inputs, tabs, table, badges, numbers, disclosure, modal, review, toast, search, menus, states, untrusted text, sheets, form foot, action bar, chips, reveal, More sheet, wallet, review deltas, glossary).
- Feature CSS: ${WEB}/src/features/markets/markets.css (1075 lines), short/short.css (543), lend/lend.css (355), guide/guide.css (144), security/security.css (69).
- Shell TSX: ${WEB}/src/shell/{AppShell,StatusBar,MoreSheet,SearchBox,SlotAge,icons,NotFound}.tsx; wallet: ${WEB}/src/wallet/{WalletButton,WalletChooser,ConnectCta}.tsx.
- Components: ${WEB}/src/components/*.tsx (Button, Panel, DataTable, Field, Tabs, Badge, Modal, Toast, Sheet, Disclosure, States, Num, FormFoot, ActionBar, Reveal, Glossary, Address, TokenId, Countdown, PageBanners).
- Pages: ${WEB}/src/features/markets/{MarketsPage,MarketsIntro,MarketPage,CreateMarketPage,ProtocolStatusPage}.tsx, markets/market/{MarketHeader,StatStrip,OverviewTab,HistoryTabs,CreatorDeposits,TierWhyPopover,MarketBanners}.tsx; short/{ShortPanel,PositionsPage,PositionDetailPage,CapsSummary}.tsx, short/positions/{AddMarginForm,CloseForm,SettlementRisk}.tsx; lend/{LendPanel,LenderDashboardPage}.tsx + deposit/withdraw/claim forms; guide/HowItWorksPage.tsx; security/SecurityPolicyPage.tsx; ${WEB}/src/tx/TxReviewModal.tsx.
- The design contract: ${REPO}/docs/design/FRONTEND.md §7 (lines 1856-2123: tokens, typography, density, components, motion, responsive conventions, supported widths) and §3.1 (lines 180-232: the shell). Read §7.6 at least.

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
10. The responsive gate (e2e/specs/responsive.spec.ts) fails on sideways overflow, an element off screen, an axe violation, a target < 24 px, a primary control < 44 px under coarse, text < 12 px, or a control under another control's tap area — at 320, 360, 390, 768, 844×390, 1024, 1280 and 1440 px.`

const FINDINGS_SCHEMA = {
  type: 'object',
  properties: {
    findings: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          id: { type: 'string' },
          title: { type: 'string' },
          severity: { type: 'string', enum: ['high', 'medium', 'low'] },
          pages: { type: 'array', items: { type: 'string' } },
          viewports: { type: 'array', items: { type: 'string' } },
          evidence: { type: 'string' },
          problem: { type: 'string' },
          proposal: { type: 'string' },
          files: { type: 'array', items: { type: 'string' } },
          risks: { type: 'string' },
        },
        required: ['id', 'title', 'severity', 'pages', 'viewports', 'evidence', 'problem', 'proposal', 'files'],
      },
    },
    overall: { type: 'string' },
  },
  required: ['findings', 'overall'],
}

const LENSES = [
  { key: 'hierarchy-type-space', text: 'Visual hierarchy, typography and spacing rhythm. Page titles vs panel titles vs labels vs values; line heights; the 4 px grid; vertical rhythm between header, page header, panels and tables; wasted or cramped space; alignment of numbers and labels; uppercase label usage; heading sizes on phone vs desktop; where the eye lands first on each page and whether that is right.' },
  { key: 'colour-surface-states', text: 'Colour, surfaces, elevation and interactive states. Surface layering (bg-0/1/2/3), borders vs shadows, panel radius, how panels separate from the page; the accent gold vs red vs green balance (does red dominate? is the primary action obvious?); hover/focus/active/disabled/selected states of buttons, rows, tabs, inputs, links; badge tints; banner tones; whether the UI feels flat and muddy or crisp and layered.' },
  { key: 'phone-shell', text: 'The phone experience (< 640 px): header (mark, cluster badge, search button, wallet button), the bottom tab bar (icons, labels, active state, More), the More sheet, the status strip and its expanded list, the search overlay, full-screen trade/manage sheets and their heads/feet, the action bar, safe areas, thumb reach, scroll behaviour, what a first-time phone user would struggle with. Use the phone screenshots heavily and the shell CSS (components.css "shell: phone", sheets, action bar, More sheet, wallet).' },
  { key: 'desktop-shell', text: 'The desktop and tablet shell (≥ 640 px): the header (brand, cluster badge, nav underline, the search field and its "/" hint, the wallet button), the status bar (items, dots, links), page headers and their actions, the 1680 px page cap and gutters, the two-column trade layout, the sticky trade panel, how the tablet tier (640-1023) looks (15-market-1024.png, 16-markets-1024.png), and whether the whole reads as one polished product.' },
  { key: 'data-presentation', text: 'Data presentation: tables (header row, sort indicators, ⓘ marks, row height, zebra/hover, numeric alignment, units, subscript zeros, column widths, the utilisation bar), card mode on phone (the card title row, label/value pairs, actions), stat strips and KPIs, key/value lists (label/value contrast, hairlines), badges (tier, venue, risk, status), P&L colouring with arrows, empty cells (—, 0). Is the most important number the most visible? Is anything noisy or hard to scan?' },
  { key: 'forms-flows', text: 'Forms and transaction flows: the Short/Lend trade panel (size field with unit and Max, slippage, the preview key/value list, disclosure boxes, acknowledgement checkbox, submit button and its reason), the Manage panel (Add margin / Close), Create market, Deposit/Withdraw/Claim, the review modal (deltas, stage, Sign) and toasts, on desktop and as phone sheets. Judge clarity of the next step, input ergonomics (44 px, 16 px text), error/hint placement, how long previews read on a phone, and the visual weight of the submit.' },
  { key: 'first-use-intuition', text: 'First-use intuitiveness: what a newcomer sees first on /markets (the intro strip, the table), the How it works page (reading width, on-this-page nav, disclosure boxes), empty/loading/error/not-found states, the no-wallet states (positions-no-wallet, lend-dashboard-no-wallet, trade-sheet-no-wallet), the wallet chooser, the glossary ⓘ affordance, the fallback banner, the status page. Is it obvious what the product does, what to do first, and what the risks are? Where is it confusing, cluttered or intimidating? Propose only visual/structural fixes (copy is owner-approved).' },
  { key: 'component-consistency', text: 'Consistency and component polish across pages: buttons (variants, heights, radii, the "short" red outline vs gold primary vs default), inputs/selects/checkboxes (heights, borders, focus), tabs (underline, scroll fade), disclosure/details chevrons, panels and section titles, dividers, icons (the nav icons, ⓘ, chevrons, the copy icon, the ▼/▲ arrows), link styling, the cluster badge, badges, toasts — anything drawn two different ways in two places, anything that looks unfinished or default-browser (native select arrows, checkbox styling, scrollbars).' },
]

phase('Audit')
const audits = (await parallel(LENSES.map((l) => () =>
  agent(`${CONTEXT}\n\nYOUR LENS (${l.key}): ${l.text}\n\nTASK: audit the app through this lens only, at phone AND desktop, and return 6-14 findings that would make the app look great and feel intuitive and easy to use. Each finding: a precise problem with evidence (name the screenshot(s) and what you saw, and the CSS rule or component responsible when you found it), a concrete proposal at the level of CSS/markup changes (name tokens, values, selectors, files), the files to touch, and risks against the constraints above. Prefer high-leverage changes a user would notice over nitpicks; still include the small polish items that add up. Severity: high = a user notices immediately / hurts usability; medium = clear polish; low = fine detail. Give ids like ${l.key}-1, ${l.key}-2… Do NOT edit any files.`, {
    label: `audit:${l.key}`, phase: 'Audit', schema: FINDINGS_SCHEMA,
  })
))).filter(Boolean)
const allFindings = audits.flatMap((a) => a.findings)
log(`${allFindings.length} findings from ${audits.length} lenses`)

phase('Synthesize')
const PLAN_SCHEMA = {
  type: 'object',
  properties: {
    packages: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          key: { type: 'string' },
          title: { type: 'string' },
          goal: { type: 'string' },
          files: { type: 'array', items: { type: 'string' } },
          items: {
            type: 'array',
            items: {
              type: 'object',
              properties: {
                id: { type: 'string' },
                change: { type: 'string' },
                files: { type: 'array', items: { type: 'string' } },
                rationale: { type: 'string' },
                from_findings: { type: 'array', items: { type: 'string' } },
                severity: { type: 'string', enum: ['high', 'medium', 'low'] },
              },
              required: ['id', 'change', 'files', 'rationale', 'from_findings', 'severity'],
            },
          },
          verification: { type: 'string' },
        },
        required: ['key', 'title', 'goal', 'files', 'items', 'verification'],
      },
    },
    dropped: { type: 'array', items: { type: 'object', properties: { id: { type: 'string' }, why: { type: 'string' } }, required: ['id', 'why'] } },
    principles: { type: 'string' },
  },
  required: ['packages', 'dropped', 'principles'],
}
const plan = await agent(`${CONTEXT}\n\nYou are the design lead. Eight auditors reviewed the app; their findings follow as JSON. Merge duplicates (the same problem seen through two lenses becomes one item that cites both ids), resolve contradictions (pick the better-argued one and say why in dropped), drop anything that violates the constraints or that is not a real improvement, and rank by user impact.\n\nThen partition the surviving items into 7-9 WORK PACKAGES by FILE OWNERSHIP so that implementers can work one package at a time without touching another package's files: e.g. (1) tokens + base (tokens.css, base.css), (2) shell: header/nav/status bar/More/search/wallet (components.css shell sections + shell TSX), (3) core components: buttons/inputs/tabs/badges/panels/disclosure (components.css sections + components/*.tsx), (4) tables, cards, stat strips, kv lists (components.css table/kv/stat sections + DataTable.tsx), (5) markets pages (markets.css + markets TSX), (6) market page trade panel + positions (short.css + short TSX), (7) lend (lend.css + lend TSX), (8) guide/security/status/states (guide.css, security.css, States.tsx, ProtocolStatusPage), (9) modal/review/toast/sheets. Adjust the grouping to the findings, but every file must belong to exactly one package; if two packages need the same file, move the items so one package owns it. Each item: a precise, implementable change (selectors, tokens, values; what the result should look like), the files, the rationale, the finding ids, severity. Write for an implementer who has not seen the findings. In verification say what screenshots at which viewports prove the package landed. Write principles: 5-8 sentences on the visual direction all packages share (so they look like one hand did them).\n\nFINDINGS:\n${JSON.stringify(allFindings, null, 1)}`, {
  label: 'synthesize:plan', phase: 'Synthesize', schema: PLAN_SCHEMA,
})
if (!plan) throw new Error('synthesis failed')
log(`${plan.packages.length} packages, ${plan.packages.reduce((n, p) => n + p.items.length, 0)} items, ${plan.dropped.length} dropped`)

phase('Verify')
const VERDICT_SCHEMA = {
  type: 'object',
  properties: {
    verdicts: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          item_id: { type: 'string' },
          verdict: { type: 'string', enum: ['keep', 'amend', 'drop'] },
          reason: { type: 'string' },
          amendment: { type: 'string' },
        },
        required: ['item_id', 'verdict', 'reason'],
      },
    },
    package_note: { type: 'string' },
  },
  required: ['verdicts', 'package_note'],
}
const SKEPTICS = [
  { key: 'constraints', text: 'You are the skeptic for CONSTRAINTS. For every item, check it against the hard constraints (open the actual CSS/TSX files and tests it touches: breakpoints.test.ts, stacking.test.ts, components.test.tsx, responsive.test.tsx, the e2e specs that key on the classes). Would it add a forbidden media query? An inline style? Text under 12 px? Shrink a target? Break a data-testid or a class an e2e spec or a test asserts on? Change owner-approved copy? Reorder the DOM against reading order? Default to drop if an item cannot be implemented within the constraints; amend when a small change makes it compliant (say exactly how).' },
  { key: 'design-quality', text: 'You are the skeptic for DESIGN QUALITY. Look at the screenshots the item concerns (Read the PNGs) and judge honestly whether the change would make the app look better and be easier to use for a trader on a phone and on a PC — or whether it adds clutter, lowers density the owner wants, reduces contrast, makes numbers harder to scan, or is a matter of taste with no clear win. Default to drop if uncertain that it is an improvement; amend when a more restrained version would clearly help (say exactly how).' },
]
const verified = await pipeline(
  plan.packages,
  (pkg) => parallel(SKEPTICS.map((s) => () =>
    agent(`${CONTEXT}\n\n${s.text}\n\nThe shared visual direction: ${plan.principles}\n\nWORK PACKAGE ${pkg.key} — ${pkg.title}\nGoal: ${pkg.goal}\nFiles: ${pkg.files.join(', ')}\nItems:\n${JSON.stringify(pkg.items, null, 1)}\n\nReturn a verdict for EVERY item id (keep / amend with the exact amendment / drop with the reason), plus a package note.`, {
      label: `skeptic:${s.key}:${pkg.key}`, phase: 'Verify', schema: VERDICT_SCHEMA,
    })
  )).then((vs) => ({ pkg, verdicts: vs.filter(Boolean) })),
  ({ pkg, verdicts }) => {
    const byItem = new Map()
    for (const v of verdicts) for (const x of v.verdicts) {
      const arr = byItem.get(x.item_id) || []
      arr.push(x)
      byItem.set(x.item_id, arr)
    }
    const items = []
    const dropped = []
    for (const it of pkg.items) {
      const vs = byItem.get(it.id) || []
      const drops = vs.filter((v) => v.verdict === 'drop')
      const amends = vs.filter((v) => v.verdict === 'amend')
      if (drops.length >= 1 && (drops.length === vs.length || vs.length <= 1 || drops.some((d) => /constraint|breakpoint|inline style|12 px|12px|testid|test id|copy|owner|order|target|contrast|CSP/i.test(d.reason)))) {
        dropped.push({ id: it.id, why: drops.map((d) => d.reason).join(' | ') })
        continue
      }
      items.push({ ...it, amendments: amends.map((a) => a.amendment || a.reason), skeptic_reasons: vs.map((v) => `${v.verdict}: ${v.reason}`) })
    }
    return { ...pkg, items, dropped, notes: verdicts.map((v) => v.package_note) }
  },
)
const finalPackages = verified.filter(Boolean)
const kept = finalPackages.reduce((n, p) => n + p.items.length, 0)
const droppedN = finalPackages.reduce((n, p) => n + p.dropped.length, 0)
log(`verification: ${kept} items kept, ${droppedN} dropped`)

phase('Plan')
const PLANFILE_SCHEMA = { type: 'object', properties: { path: { type: 'string' }, summary: { type: 'string' } }, required: ['path', 'summary'] }
const written = await agent(`Write the implementation plan for the short.win design polish to ${SCRATCH}/design-plan.md as clear markdown an implementing engineer will follow package by package. Start with the shared visual direction (principles), then the constraints block verbatim (below), then one section per package in implementation order (foundations first: tokens/base, then shell, then components, then tables, then pages, then modal/toast), each with its goal, owned files, the items as a numbered list (change, files, rationale, any amendments the skeptics required — fold the amendment into the change text so the item reads as one instruction, and keep the skeptic reasons as a short "watch out" line), and the verification (which screenshots at which viewports). Add a "Dropped" appendix listing dropped ids with why. Keep every item's precision (selectors, tokens, px values). Return the path and a 10-line summary.\n\nCONSTRAINTS BLOCK:\n${CONTEXT.slice(CONTEXT.indexOf('HARD CONSTRAINTS'))}\n\nPRINCIPLES:\n${plan.principles}\n\nPACKAGES (verified):\n${JSON.stringify(finalPackages, null, 1)}\n\nDROPPED AT SYNTHESIS:\n${JSON.stringify(plan.dropped, null, 1)}`, {
  label: 'plan:write', phase: 'Plan', schema: PLANFILE_SCHEMA,
})

return { plan_file: written, packages: finalPackages.map((p) => ({ key: p.key, title: p.title, items: p.items.length, dropped: p.dropped.length })), principles: plan.principles, findings: allFindings.length }