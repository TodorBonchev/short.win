/**
 * Prints the per-viewport counts and the worst page states of a responsive capture
 * (`e2e/specs/responsive.spec.ts`, FRONTEND.md §10.5):
 *
 *   node e2e/responsive/summary.ts [<SHORTWIN_SHOTS_DIR>]   # default apps/web/.e2e/responsive
 *   node e2e/responsive/summary.ts <dir> --pages            # also one line per page state
 *   node e2e/responsive/summary.ts <before> <after>         # the two side by side ("before → after")
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { E2E_DIR } from '../env.ts';
import { type MetricsFile, score } from './metrics.ts';

const args = process.argv.slice(2);
const dirs = args.filter((a) => !a.startsWith('--'));
const dir = dirs[0] ?? process.env.SHORTWIN_SHOTS_DIR ?? join(E2E_DIR, 'responsive');
const load = (d: string) => JSON.parse(readFileSync(join(d, 'metrics.json'), 'utf8')) as MetricsFile;
const m = load(dir);
const ORDER = ['phone-xs', 'phone-s', 'phone', 'phone-land', 'tablet', 'laptop-s', 'laptop', 'desktop'];
const byOrder = (a: string, b: string) => ORDER.indexOf(a) - ORDER.indexOf(b);
const vps = Object.keys(m.summary).sort(byOrder);

const cols = [
    ['shots', 'shots'],
    ['errors', 'errors'],
    ['notes', 'notes'],
    ['widerLayout', 'wide-layout'],
    ['minZoom', 'min-zoom'],
    ['pagesWithDocOverflow', 'doc-ovfl'],
    ['pagesWithMainOverflow', 'main-ovfl'],
    ['maxOverflowX', 'max-ovfl-px'],
    ['offscreen', 'offscreen'],
    ['hScrollers', 'h-scroll'],
    ['targetsUnder44', '<44px'],
    ['targetsUnder24', '<24px'],
    ['overlaps', 'overlaps'],
    ['tapCovers', 'tap-cover'],
    ['smallTextNodes', 'text<12'],
    ['titles', 'title-tips'],
    ['axeViolations', 'axe'],
    ['axeNodes', 'axe-nodes'],
] as const;

if (dirs.length >= 2) {
    // BEFORE → AFTER: the viewports either capture has, each count as "before → after".
    const after = load(dirs[1] as string);
    const all = [...new Set([...Object.keys(m.summary), ...Object.keys(after.summary)])].sort(byOrder);
    const keep = cols.filter(([k]) => k !== 'notes' && k !== 'pagesWithMainOverflow');
    console.log(`before ${dir}\nafter  ${dirs[1]}\n`);
    console.log(['viewport'.padEnd(10), ...keep.map(([, h]) => h.padStart(13))].join(''));
    for (const vp of all) {
        const b = m.summary[vp];
        const a = after.summary[vp];
        const cell = (k: (typeof keep)[number][0]) => `${String(b?.[k] ?? '–')} → ${String(a?.[k] ?? '–')}`;
        console.log([vp.padEnd(10), ...keep.map(([k]) => cell(k).padStart(13))].join(''));
    }
    console.log('\naxe rules after:');
    for (const vp of all) console.log(`  ${vp}: ${after.summary[vp]?.axeRules.join(', ') || '—'}`);
    process.exit(0);
}

console.log(`${dir}/metrics.json (updated ${m.updatedAt})\n`);
console.log(['viewport'.padEnd(10), ...cols.map(([, h]) => h.padStart(11))].join(''));
for (const vp of vps) {
    const s = m.summary[vp];
    if (!s) continue;
    console.log([vp.padEnd(10), ...cols.map(([k]) => String(s[k] ?? '–').padStart(11))].join(''));
}
console.log('\naxe rules per viewport:');
for (const vp of vps) console.log(`  ${vp}: ${m.summary[vp]?.axeRules.join(', ') || '—'}`);
console.log('\nworst page states:');
for (const w of m.worst) console.log(`  ${String(w.score).padStart(5)}  ${w.viewport}/${w.name}`);

if (args.includes('--pages')) {
    for (const vp of vps) {
        console.log(`\n${vp}:`);
        for (const r of Object.values(m.shots[vp] ?? {})) {
            const l = r.layout;
            const bits = l
                ? [
                      `layout ${l.innerWidth}px`,
                      `ovfl ${Math.max(l.docOverflowX, l.mainOverflowX)}`,
                      `off ${l.offscreen.count}`,
                      `<44 ${l.targets.under44}`,
                      `<24 ${l.targets.under24}`,
                      `txt<12 ${l.smallText.nodes}`,
                      `axe ${r.axe.length}`,
                  ]
                : ['no metrics'];
            console.log(
                `  ${r.name.padEnd(34)} ${String(Math.round(score(r))).padStart(5)}  ${bits.join(' · ')}${r.error ? `  ERROR ${r.error}` : ''}`,
            );
        }
    }
}
