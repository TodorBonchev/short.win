export const meta = {
  name: 'design-implement',
  description: 'Implement the design-polish plan package by package: implementer with the live screenshot loop, then an adversarial visual QA reviewer, then a fixer round; gates after each',
  phases: [
    { title: 'Foundations', detail: 'P1 tokens/base → P2 shell → P3 components → P4 tables; sequential, each committed' },
    { title: 'Shared states', detail: 'P8 reading pages and shared empty/error states' },
    { title: 'Pages and overlays', detail: 'P5 markets, P6 short, P7 lend, P9 overlays — parallel, disjoint files' },
  ],
}

// args: { planFile, readme, baselineDir, shotsDir, stages: [{ title, parallel, packages: [{ key, title, files, routes, wallet, extraShots }] }] }
const A = args
const REPO = '/home/todor_dev/Projects/short.win'
const WEB = `${REPO}/apps/web`

const RULES = `HARD RULES (tests enforce several; breaking one fails the package):
1. Keep the dark trading terminal: dense, dark, data-first. Polish, do not redesign or lighten.
2. One DOM at every width: width switches are CSS only. No duplicated markup per breakpoint.
3. Closed breakpoint set (src/styles/breakpoints.test.ts): only @media (max-width: 399.98px | 639.98px | 1023.98px | 1179.98px | 1279.98px), (min-width: 640px | 1024px | 1280px | 1680px), (pointer: coarse), (hover: hover), (prefers-reduced-motion: reduce); (orientation: landscape) only with coarse in the tablet tier; the only height query is (max-width: 399.98px) and (max-height: 699.98px). No range syntax. Container queries only on .tbl-wrap (tbl) and .modal-body (dialog).
4. CSP: no inline style attributes, no <style> in TSX; SVG presentation attributes are fine. No web fonts (system stacks --font-ui / --font-num only). No remote images.
5. WCAG 2.2 AA: text ≥ 4.5:1 on its background, borders/focus ≥ 3:1; colour never the only cue; NO text under 12 px at any width; targets ≥ 24 px, primary controls ≥ 44 px under (pointer: coarse); hover styles only inside @media (hover: hover); transitions use the --t tokens (0 under reduced motion); keep :focus-visible rings visible.
6. Reading order = DOM order: no CSS order, no row-reverse.
7. Tokens first: a new colour/size/radius/shadow becomes a variable in tokens.css (only the tokens package may add tokens; other packages use existing tokens — if you truly need a new one, define it as a local custom property in your own CSS file's top-level :root block is NOT allowed either: use an existing token or a literal derived from one with color-mix()/calc()).
8. Copy is owner-approved: never reword user-facing strings (trivial casing excepted); the header wordmark stays "ShortWin".
9. Never hide information (risk, staleness, numbers). Keep every data-testid and every class name the CSS or e2e already keys on (add classes freely; do not rename or remove).
10. Only touch the files your package owns (listed below). If a change needs another file, do not make it — report it in blockers.
11. Owner decisions are not yours: where the plan marks something as an owner decision ("explicit yes only", "the owner picks"), implement only the parts that need no decision and report the open decision in notes.
12. The responsive gate fails on: sideways overflow, element off screen, axe violation, target < 24 px, primary control < 44 px under coarse, text < 12 px, a control under another control's tap area — at 320, 360, 390, 768, 844×390, 1024, 1280, 1440 px. Check 320 px (viewport phone-xs) whenever you change the shell or a header.`

const REPORT_SCHEMA = {
  type: 'object',
  properties: {
    done: { type: 'array', items: { type: 'object', properties: { id: { type: 'string' }, what: { type: 'string' } }, required: ['id', 'what'] } },
    skipped: { type: 'array', items: { type: 'object', properties: { id: { type: 'string' }, why: { type: 'string' } }, required: ['id', 'why'] } },
    files_changed: { type: 'array', items: { type: 'string' } },
    gates: { type: 'object', properties: { lint: { type: 'string' }, typecheck: { type: 'string' }, test: { type: 'string' } }, required: ['lint', 'typecheck', 'test'] },
    after_shots_dir: { type: 'string' },
    blockers: { type: 'string' },
    notes: { type: 'string' },
  },
  required: ['done', 'skipped', 'files_changed', 'gates', 'after_shots_dir', 'blockers'],
}

const QA_SCHEMA = {
  type: 'object',
  properties: {
    approved: { type: 'boolean' },
    issues: { type: 'array', items: { type: 'object', properties: { item: { type: 'string' }, severity: { type: 'string', enum: ['blocker', 'major', 'minor'] }, problem: { type: 'string' }, fix: { type: 'string' }, evidence: { type: 'string' } }, required: ['item', 'severity', 'problem', 'fix', 'evidence'] } },
    gates_ok: { type: 'boolean' },
    praise: { type: 'string' },
  },
  required: ['approved', 'issues', 'gates_ok'],
}

function shotsCmd(pkg, dir) {
  const routes = pkg.routes.join(',')
  const wallet = pkg.wallet || 'margin'
  return `node .e2e/shots.ts --out ${dir} --viewports phone,desktop --wallet ${wallet} --routes ${routes}${pkg.extraShots ? `\n  and also: ${pkg.extraShots}` : ''}`
}

function implPrompt(pkg, stageTitle, round, issues, stage) {
  const gitRule = stage.commit
    ? `When every gate passes, commit YOUR OWNED FILES ONLY (never \`git add -A\`): \`cd ${REPO} && git add <your files> && git commit -F <msgfile>\` with the subject \`feat(web): ${pkg.title.toLowerCase()} (design pass ${pkg.key})\`${round > 1 ? ` — for this fix round use \`fix(web): ${pkg.key} review fixes\`` : ''}, a body of 3-8 lines saying what changed and why, and these two trailer lines at the end:\nCo-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_01Jjc86sGqnZkMwrtg9gyVtS\nNever stash, reset, checkout or amend; never push.`
    : `Do NOT commit, stash, reset or checkout anything: the lead commits this stage. Other engineers are editing OTHER files in the same working tree right now (other packages of this stage: ${(stage.packages || []).map((p) => p.key).filter((k) => k !== pkg.key).join(', ')}): touch only your owned files; if \`lint\`, \`typecheck\` or \`test\` fails in a file you do not own, wait 60 s and retry (up to 3 times) — someone is mid-edit — and only a failure in your own files is yours to fix; if a screenshot shows something odd outside your package's area, note it, do not fix it.`
  const base = `You are a senior front-end engineer and designer implementing ONE work package of the short.win design polish on the branch design-polish (working tree ${REPO}; the web app is ${WEB}). Work directly in the working tree. ${gitRule} Never run a repo-wide formatter write (use \`pnpm --filter web exec biome check --write <your files>\`).

READ FIRST: ${A.readme} (the live screenshot loop: the dev server with HMR and the seeded validator are already running), then your package section in ${A.planFile} (section "${pkg.title}"; the plan's "visual direction" at the top applies to everything), then docs/design/FRONTEND.md §7.6 (grep -n '### 7.6' ${REPO}/docs/design/FRONTEND.md and read that section) once.

PACKAGE ${pkg.key} — ${pkg.title}
Owned files (the only files you may edit): ${pkg.files.join(', ')}

${RULES}

METHOD:
1. Capture BEFORE shots of your routes (${pkg.routes.join(', ')}) into ${A.shotsDir}/${pkg.key}/before (cd ${WEB} && ${shotsCmd(pkg, `${A.shotsDir}/${pkg.key}/before`)}) and LOOK at them (Read tool) so you know what you are changing. The baseline from before any package is in ${A.baselineDir}.
2. Implement every item of your package, in the plan's order, exactly as specified (selectors, tokens, px values), folding in any amendment. Where the plan leaves a value open, choose what looks best and say so. Keep the CSS organised in the file's existing sections with a one-line comment per new rule block in the file's existing comment style.
3. After each few items: capture AFTER shots into ${A.shotsDir}/${pkg.key}/after (same command with /after) and LOOK at phone AND desktop. Compare with before. Fix anything that looks worse, clipped, misaligned, overflowing or low-contrast. Iterate until it looks genuinely great. Also check phone-xs (320 px) for anything in the header/shell/cards: --viewports phone-xs.
4. Run the gates from ${WEB}: \`pnpm --filter web exec biome check --write <your files>\` then \`pnpm --filter web lint\`, \`pnpm --filter web typecheck\`, \`pnpm --filter web test\`. All three must pass (a failing breakpoints.test.ts means a forbidden media query: fix it). If a vitest snapshot/assertion in a test file you do not own fails because of an intended visual change, report it in blockers instead of editing the test.
5. Return the structured report: every plan item as done (what you did) or skipped (why), the files changed, gate results, the after-shots dir, blockers (needs another package's file; a test you could not touch), notes.`
  if (round === 1) return base
  return `${base}\n\nTHIS IS FIX ROUND ${round}. A reviewer rejected the previous attempt with these issues — resolve each (or justify in notes why not), then re-run the shots and the gates:\n${JSON.stringify(issues, null, 1)}`
}

function qaPrompt(pkg, report, stage) {
  return `You are the adversarial visual QA reviewer for ONE work package of the short.win design polish (branch design-polish, ${REPO}). Your job is to catch anything that would make the owner say "this looks worse" or that would fail the gates. Be concrete and harsh; approve only what you would ship.

Read ${A.readme} (how to capture shots) and the package section "${pkg.title}" in ${A.planFile}.

PACKAGE ${pkg.key} — ${pkg.title}
Owned files: ${pkg.files.join(', ')}
Implementer's report: ${JSON.stringify(report, null, 1)}

${RULES}

NOTE: ${stage && stage.parallel ? `other packages (${(stage.packages || []).map((p) => p.key).filter((k) => k !== pkg.key).join(', ')}) are being implemented in the same working tree right now on other pages/files; judge only this package's items and owned files, and report (do not fail the package for) oddities clearly outside them. Do not commit anything.` : 'this package has been committed by the implementer; the diff to judge is `git diff HEAD~1 -- <owned files>` if the report says it committed, else the working tree diff.'}

CHECK:
1. LOOK (Read tool) at the implementer's after shots in ${report.after_shots_dir} (phone/ and desktop/, first-screen and -full) against the before shots in ${A.shotsDir}/${pkg.key}/before and the pre-polish baseline ${A.baselineDir}. Then capture your OWN fresh shots (cd ${WEB} && ${shotsCmd(pkg, `${A.shotsDir}/${pkg.key}/qa`)}) plus --viewports phone-xs,tablet for the same routes, and look at those too.
2. For every plan item marked done: did it land as specified, and does it look right at phone and desktop? For every item skipped: was the reason valid?
3. Hunt regressions: clipped or overlapping text, sideways overflow, misaligned numbers, lost hover/focus states, contrast below 4.5:1 (compute it for any new colour pair), text under 12 px, controls under 24/44 px, a bar covering a control, broken card mode, anything that now looks inconsistent with the rest of the app.
4. Read the diff of the owned files (\`cd ${REPO} && git diff HEAD~1 -- <owned paths>\` when the implementer committed, else \`git diff -- <owned paths>\`; the owned files are listed above): any forbidden media query, inline style, removed class/data-testid, hover outside (hover: hover), hard-coded colour that should be a token, DOM reorder?
5. Run the gates yourself: \`pnpm --filter web lint && pnpm --filter web typecheck && pnpm --filter web test\` in ${WEB}.
Return approved=true only if there are no blocker or major issues and the gates pass. List every issue with the exact fix.`
}

async function runPackage(pkg, stageTitle, stage) {
  let report = await agent(implPrompt(pkg, stageTitle, 1, [], stage), { label: `impl:${pkg.key}`, phase: stageTitle, schema: REPORT_SCHEMA })
  if (!report) return { key: pkg.key, status: 'implementer-failed' }
  let qa = await agent(qaPrompt(pkg, report, stage), { label: `qa:${pkg.key}`, phase: stageTitle, schema: QA_SCHEMA })
  let round = 1
  while (qa && !qa.approved && round < 3) {
    round += 1
    const issues = qa.issues.filter((i) => i.severity !== 'minor')
    log(`${pkg.key}: QA rejected (${issues.length} issues) — fix round ${round}`)
    const fixed = await agent(implPrompt(pkg, stageTitle, round, issues, stage), { label: `fix${round}:${pkg.key}`, phase: stageTitle, schema: REPORT_SCHEMA })
    if (fixed) report = { ...fixed, done: [...report.done, ...fixed.done] }
    qa = await agent(qaPrompt(pkg, report, stage), { label: `qa${round}:${pkg.key}`, phase: stageTitle, schema: QA_SCHEMA })
  }
  log(`${pkg.key}: ${qa && qa.approved ? 'APPROVED' : 'NOT APPROVED'} after ${round} round(s)`)
  return { key: pkg.key, title: pkg.title, status: qa && qa.approved ? 'approved' : 'not-approved', rounds: round, report, qa }
}

const results = []
for (const stage of A.stages) {
  phase(stage.title)
  log(`stage ${stage.title}: ${stage.packages.map((p) => p.key).join(', ')} (${stage.parallel ? 'parallel' : 'sequential'})`)
  if (stage.parallel) {
    const rs = await parallel(stage.packages.map((p) => () => runPackage(p, stage.title, stage)))
    results.push(...rs.filter(Boolean))
  } else {
    for (const p of stage.packages) results.push(await runPackage(p, stage.title, stage))
  }
}
return results
