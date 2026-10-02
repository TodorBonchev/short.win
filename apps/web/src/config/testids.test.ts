import { describe, expect, it } from 'vitest';
import { readRepo } from '../test/repo.ts';
import { T } from './testids.ts';

describe('data-testid contract', () => {
    const statics = Object.entries(T).filter(([, v]) => typeof v === 'string') as [string, string][];
    const prefixes = Object.entries(T)
        .filter(([, v]) => typeof v === 'function')
        .map(([k, f]) => [k, (f as (x: string) => string)('KEY').replace(/-KEY$/, '')] as const);

    it('has unique ids and prefixes', () => {
        const all = [...statics.map(([, v]) => v), ...prefixes.map(([, v]) => v)];
        expect(new Set(all).size).toBe(all.length);
        for (const v of all) expect(v).toMatch(/^[a-z0-9]+(-[a-z0-9]+)*$/);
    });

    it('is documented in FRONTEND.md §11', () => {
        const doc = readRepo('docs/design/FRONTEND.md');
        for (const [k, v] of statics) expect(doc, k).toContain(`\`${v}\``);
        for (const [k, v] of prefixes) expect(doc, k).toContain(`\`${v}-<`);
    });
});
