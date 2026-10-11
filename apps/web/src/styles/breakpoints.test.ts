/**
 * The closed breakpoint set (FRONTEND.md §7.6): every `@media` / `@container` width in the app's CSS
 * is one of `breakpoints.ts`, written as `max-width: N.98px` / `min-width: Npx` (no range syntax),
 * and only `.tbl-wrap` and `.modal-body` are containers.
 */
import { readdirSync, readFileSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { CONTAINER_SELECTORS, CONTAINER_WIDTHS, MEDIA_HEIGHTS, MEDIA_WIDTHS, SHORT_PHONE } from './breakpoints.ts';

const SRC = join(dirname(fileURLToPath(import.meta.url)), '..');

function cssFiles(dir: string): string[] {
    return readdirSync(dir, { withFileTypes: true }).flatMap((e) =>
        e.isDirectory() ? cssFiles(join(dir, e.name)) : e.name.endsWith('.css') ? [join(dir, e.name)] : [],
    );
}

const FILES = cssFiles(SRC).map((f) => ({
    name: relative(SRC, f),
    css: readFileSync(f, 'utf8').replace(/\/\*[\s\S]*?\*\//g, ''),
}));

/** `@media` / `@container` preludes (the text up to the block's `{`). */
function preludes(css: string, at: 'media' | 'container'): string[] {
    return [...css.matchAll(new RegExp(`@${at}\\s+([^{]+)\\{`, 'g'))].map((m) => (m[1] ?? '').trim());
}

/** The width features in a prelude: `[feature, value]`. */
function widths(prelude: string): [string, string][] {
    return [...prelude.matchAll(/\(\s*((?:max|min)-width)\s*:\s*([^)]+?)\s*\)/g)].map((m) => [m[1] ?? '', m[2] ?? '']);
}

const RANGE = /\(\s*[^():]*\bwidth\s*[<>=]|[<>=]\s*width\b/;

describe('breakpoints (FRONTEND.md §7.6)', () => {
    it('finds the stylesheets', () => {
        expect(FILES.map((f) => f.name)).toEqual(expect.arrayContaining(['styles/components.css', 'styles/base.css']));
    });

    it('uses only the closed set of media widths, without range syntax', () => {
        const bad: string[] = [];
        for (const { name, css } of FILES) {
            for (const p of preludes(css, 'media')) {
                if (RANGE.test(p)) bad.push(`${name}: range syntax in @media ${p}`);
                for (const [feature, value] of widths(p)) {
                    const allowed: readonly string[] = feature === 'max-width' ? MEDIA_WIDTHS.max : MEDIA_WIDTHS.min;
                    if (!allowed.includes(value)) bad.push(`${name}: @media ${p}`);
                }
            }
        }
        expect(bad).toEqual([]);
    });

    it('uses only the closed set of media heights (one max-height), always with a width', () => {
        const bad: string[] = [];
        for (const { name, css } of FILES) {
            for (const p of preludes(css, 'media')) {
                for (const m of p.matchAll(/\(\s*((?:max|min)-height)\s*:\s*([^)]+?)\s*\)/g)) {
                    const allowed: readonly string[] = m[1] === 'max-height' ? MEDIA_HEIGHTS.max : [];
                    if (!allowed.includes(m[2] ?? '')) bad.push(`${name}: @media ${p}`);
                }
                // Each comma-separated query naming a height also names a width.
                for (const q of p.split(',')) {
                    if (/-height\s*:/.test(q) && widths(q).length === 0)
                        bad.push(`${name}: height alone in @media ${p}`);
                }
            }
        }
        expect(bad).toEqual([]);
    });

    it('names a height only in the owner-approved short-phone query (2026-10-11)', () => {
        expect(SHORT_PHONE).toBe('(max-width: 399.98px) and (max-height: 699.98px)');
        expect(SHORT_PHONE).toContain(`(max-height: ${MEDIA_HEIGHTS.max[0]})`);
        const found: string[] = [];
        const bad: string[] = [];
        for (const { name, css } of FILES) {
            for (const p of preludes(css, 'media')) {
                for (const q of p.split(',').map((x) => x.replace(/\s+/g, ' ').trim())) {
                    if (!/-height\s*:/.test(q)) continue;
                    if (q === SHORT_PHONE) found.push(name);
                    else bad.push(`${name}: @media ${q}`);
                }
            }
        }
        expect(bad).toEqual([]);
        // The markets intro strip's compact layout uses it (FRONTEND.md §3.3).
        expect(found).toContain(join('features', 'markets', 'markets.css'));
    });

    it('uses only the named containers and their widths', () => {
        const bad: string[] = [];
        for (const { name, css } of FILES) {
            for (const p of preludes(css, 'container')) {
                if (RANGE.test(p)) bad.push(`${name}: range syntax in @container ${p}`);
                const cname = /^([a-z][\w-]*)\s*\(/.exec(p)?.[1];
                const allowed: readonly string[] | undefined =
                    cname === 'tbl' ? CONTAINER_WIDTHS.tbl : cname === 'dialog' ? CONTAINER_WIDTHS.dialog : undefined;
                if (!allowed) {
                    bad.push(`${name}: unnamed or unknown container in @container ${p}`);
                    continue;
                }
                for (const [feature, value] of widths(p)) {
                    if (feature !== 'max-width' || !allowed.includes(value)) bad.push(`${name}: @container ${p}`);
                }
            }
        }
        expect(bad).toEqual([]);
    });

    it('makes only .tbl-wrap and .modal-body containers', () => {
        const bad: string[] = [];
        for (const { name, css } of FILES) {
            for (const m of css.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
                const selector = (m[1] ?? '').trim();
                const body = m[2] ?? '';
                if (!/(^|[;\s])container(-type|-name)?\s*:/.test(body)) continue;
                const sels = selector.split(',').map((s) => s.trim());
                for (const s of sels) {
                    if (!(CONTAINER_SELECTORS as readonly string[]).includes(s)) bad.push(`${name}: ${s}`);
                }
            }
        }
        expect(bad).toEqual([]);
    });

    it('rejects what it should (self-check of the parsers)', () => {
        expect(RANGE.test('(width < 640px)')).toBe(true);
        expect(RANGE.test('(400px <= width <= 700px)')).toBe(true);
        expect(RANGE.test('(max-width: 639.98px)')).toBe(false);
        expect(widths('(min-width: 640px) and (max-width: 1023.98px)')).toEqual([
            ['min-width', '640px'],
            ['max-width', '1023.98px'],
        ]);
    });
});
