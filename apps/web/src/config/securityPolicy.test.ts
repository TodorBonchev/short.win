import { describe, expect, it } from 'vitest';
import { readRepo } from '../test/repo.ts';
import { SECURITY_EMAIL, SECURITY_POLICY_URL, SECURITY_TXT_CANONICAL } from './securityPolicy.ts';
import { SECURITY_TXT_PLACEHOLDER, securityTxtProblems } from './securityTxt.ts';

const TXT = readRepo('apps/web/public/.well-known/security.txt');
const field = (name: string) => [...TXT.matchAll(new RegExp(`^${name}: (.*)$`, 'gm'))].map((m) => m[1]);

/** The owner's values (M10 Q3, 2026-10-02) in every copy: the web file, the page, SECURITY.md, both programs. */
describe('security policy values (M10 Q3)', () => {
    it('the committed security.txt carries the owner values and passes the production check', () => {
        expect(field('Contact')).toEqual([`mailto:${SECURITY_EMAIL}`]);
        expect(field('Policy')).toEqual([SECURITY_POLICY_URL]);
        expect(field('Canonical')).toEqual([SECURITY_TXT_CANONICAL]);
        expect(field('Preferred-Languages')).toEqual(['en']);
        expect(TXT).not.toContain(SECURITY_TXT_PLACEHOLDER);
        // Expires lies more than 30 days and at most a year after the values were set.
        expect(securityTxtProblems(TXT, { now: new Date('2026-10-02T00:00:00Z'), production: true })).toEqual([]);
    });

    it('SECURITY.md and both programs carry the same contact and policy', () => {
        const md = readRepo('SECURITY.md');
        expect(md).toContain(`mailto:${SECURITY_EMAIL}`);
        expect(md).toContain(SECURITY_POLICY_URL);
        expect(md).not.toContain(SECURITY_TXT_PLACEHOLDER);
        for (const lib of ['programs/shortwin/src/lib.rs', 'programs/complete_curve/src/lib.rs']) {
            const src = readRepo(lib);
            expect(src, lib).toContain(`contacts: "email:${SECURITY_EMAIL},link:${SECURITY_POLICY_URL}"`);
            expect(src, lib).toContain(`policy: "${SECURITY_POLICY_URL}"`);
            expect(src, lib).toContain('project_url: "https://short.win"');
            expect(src, lib).not.toContain(SECURITY_TXT_PLACEHOLDER);
        }
    });
});
