/**
 * A form foot put back in the flow (`position: static`) also drops its stacking order: a flex
 * item's `z-index` applies without a position, so a static foot with the sticky foot's `z-index: 1`
 * paints over the open sheet's sticky head (its title and ✕) when the sheet scrolls it under the
 * head (the Manage sheet on a landscape phone; the fix round's review, FRONTEND.md §7.6).
 */
import { readdirSync, readFileSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const SRC = join(dirname(fileURLToPath(import.meta.url)), '..');

function cssFiles(dir: string): string[] {
    return readdirSync(dir, { withFileTypes: true }).flatMap((e) =>
        e.isDirectory() ? cssFiles(join(dir, e.name)) : e.name.endsWith('.css') ? [join(dir, e.name)] : [],
    );
}

/** Every innermost rule `selector { declarations }` (comments stripped; at-rule preludes skipped). */
function rules(css: string): { selector: string; body: string }[] {
    const clean = css.replace(/\/\*[\s\S]*?\*\//g, '');
    return [...clean.matchAll(/([^{};]+)\{([^{}]*)\}/g)].map((m) => ({
        selector: (m[1] ?? '').trim(),
        body: m[2] ?? '',
    }));
}

describe('a form foot back in the flow', () => {
    it('resets its z-index with its position', () => {
        const found: string[] = [];
        const bad: string[] = [];
        for (const f of cssFiles(SRC)) {
            for (const r of rules(readFileSync(f, 'utf8'))) {
                if (!r.selector.includes('.form-foot') || !/position\s*:\s*static/.test(r.body)) continue;
                const where = `${relative(SRC, f)}: ${r.selector}`;
                found.push(where);
                if (!/z-index\s*:\s*auto/.test(r.body)) bad.push(where);
            }
        }
        expect(found.length).toBeGreaterThanOrEqual(2);
        expect(bad).toEqual([]);
    });
});
