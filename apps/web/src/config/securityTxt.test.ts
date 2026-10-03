import { describe, expect, it } from 'vitest';
import { readRepo } from '../test/repo.ts';
import { SECURITY_TXT_PLACEHOLDER, securityTxtProblems } from './securityTxt.ts';

const NOW = new Date('2026-10-02T00:00:00Z');
const COMMITTED = readRepo('apps/web/public/.well-known/security.txt');

/** A filled-in file, as the owner's values will make it (RFC 9116 §2.5). */
const FILLED = [
    '# short.win vulnerability disclosure (RFC 9116).',
    'Contact: mailto:security@example.org',
    'Contact: https://example.org/report',
    'Expires: 2027-06-30T00:00:00.000Z',
    'Policy: https://example.org/security',
    'Preferred-Languages: en',
    'Canonical: https://example.org/.well-known/security.txt',
    '',
].join('\n');

const prod = (text: string, now = NOW) => securityTxtProblems(text, { now, production: true });
const swap = (from: string, to: string) => {
    expect(FILLED).toContain(from);
    return FILLED.replace(from, to);
};

describe('security.txt (RFC 9116; SECTXT-4)', () => {
    it('the committed file has every required field and parses (placeholders allowed outside production)', () => {
        expect(securityTxtProblems(COMMITTED, { now: NOW, production: false })).toEqual([]);
        for (const field of ['Contact', 'Expires', 'Policy', 'Canonical', 'Preferred-Languages']) {
            expect(COMMITTED).toMatch(new RegExp(`^${field}: `, 'm'));
        }
    });

    it('a production check refuses the committed file while it holds OWNER-FILL placeholders', () => {
        const problems = prod(COMMITTED);
        if (COMMITTED.includes(SECURITY_TXT_PLACEHOLDER)) {
            expect(problems.some((p) => p.includes(SECURITY_TXT_PLACEHOLDER))).toBe(true);
        } else {
            // Filled in by the owner: it passes the production check on a clock pinned 90 days
            // before its own Expires, so this test does not start failing on a calendar date.
            // The real-clock window is the deploy's: a mainnet build (vite.config.ts) and
            // `pnpm --filter web check:headers` (scripts/check-headers.ts) refuse an Expires
            // that is 30 days or less ahead, so the file is rotated before then (owner item).
            const expires = /^Expires: (.+)$/m.exec(COMMITTED)?.[1] ?? '';
            const pinned = new Date(Date.parse(expires) - 90 * 86_400_000);
            expect(Number.isNaN(pinned.getTime())).toBe(false);
            expect(prod(COMMITTED, pinned)).toEqual([]);
        }
    });

    it('accepts a filled-in file', () => {
        expect(prod(FILLED)).toEqual([]);
    });

    it('requires mailto: or https: contacts', () => {
        expect(prod(swap('mailto:security@example.org', 'security@example.org'))).toEqual([
            'Contact security@example.org is not mailto: or https:',
        ]);
        expect(prod(swap('https://example.org/report', 'http://example.org/report'))).toEqual([
            'Contact http://example.org/report is not mailto: or https:',
        ]);
        expect(prod(FILLED.replace(/^Contact: .*\n/gm, ''))).toEqual(['Contact is required']);
    });

    it('requires exactly one RFC 3339 Expires, more than 30 days and at most a year ahead', () => {
        const at = (e: string) => prod(swap('Expires: 2027-06-30T00:00:00.000Z', `Expires: ${e}`));
        expect(at('2026-11-02T00:00:00Z')).toEqual([]);
        expect(at('2027-10-02T00:00:00+00:00')).toEqual([]);
        expect(at('2026-11-01T00:00:00Z')).toEqual(['Expires 2026-11-01T00:00:00Z is not more than 30 days ahead']);
        expect(at('2026-09-01T00:00:00Z')).toEqual(['Expires 2026-09-01T00:00:00Z is not more than 30 days ahead']);
        expect(at('2027-10-02T00:00:01Z')).toEqual([
            'Expires 2027-10-02T00:00:01Z is more than a year ahead (RFC 9116 §2.5.5)',
        ]);
        expect(at('2027-06-30')).toEqual(['Expires 2027-06-30 is not an RFC 3339 date-time']);
        expect(prod(`${FILLED}Expires: 2027-01-01T00:00:00Z\n`)).toEqual([
            'Expires must appear exactly once (found 2)',
        ]);
        expect(prod(FILLED.replace(/^Expires: .*\n/m, ''))).toEqual(['Expires must appear exactly once (found 0)']);
    });

    it('judges the Expires window only in production (other builds check the format alone)', () => {
        const dev = (e: string) =>
            securityTxtProblems(swap('Expires: 2027-06-30T00:00:00.000Z', `Expires: ${e}`), {
                now: NOW,
                production: false,
            });
        expect(dev('2026-10-20T00:00:00Z')).toEqual([]);
        expect(dev('2025-01-01T00:00:00Z')).toEqual([]);
        expect(dev('2030-01-01T00:00:00Z')).toEqual([]);
        expect(dev('2026-10-20')).toEqual(['Expires 2026-10-20 is not an RFC 3339 date-time']);
        expect(prod(swap('2027-06-30T00:00:00.000Z', '2026-10-20T00:00:00Z'))).toEqual([
            'Expires 2026-10-20T00:00:00Z is not more than 30 days ahead',
        ]);
    });

    it('checks the signed text of an OpenPGP cleartext-signed file (RFC 9116 §2.3)', () => {
        const signed = (body: string, headers = 'Hash: SHA512', tail = '-----END PGP SIGNATURE-----') =>
            [
                '-----BEGIN PGP SIGNED MESSAGE-----',
                headers,
                '',
                body.trimEnd(),
                '-----BEGIN PGP SIGNATURE-----',
                '',
                'iQIzBAEBCgAdFiEEexample=',
                '=AbCd',
                tail,
                '',
            ].join('\n');
        expect(prod(signed(FILLED))).toEqual([]);
        // Dash-escaped lines (RFC 4880 §7.1) are unescaped before parsing.
        expect(prod(signed(FILLED.replace('Policy: ', '- Policy: ')))).toEqual([]);
        expect(prod(signed(`${FILLED}Bounty: yes`))).toEqual(['line 11: unknown field Bounty']);
        expect(prod(signed(FILLED, 'Comment: x'))).toEqual(['line 2: unexpected OpenPGP armor header Comment: x']);
        expect(prod(signed(FILLED, 'Hash: SHA512', ''))).toEqual(['-----END PGP SIGNATURE----- is missing']);
        expect(prod(signed(FILLED).replace('-----BEGIN PGP SIGNATURE-----', ''))).toEqual([
            'a cleartext-signed file needs a -----BEGIN PGP SIGNATURE----- block',
            'Contact is required',
            'Expires must appear exactly once (found 0)',
            'Canonical is required',
            'Policy is required',
        ]);
        // An unsigned file with a stray signature block is malformed, not silently cut.
        expect(prod(`${FILLED}-----BEGIN PGP SIGNATURE-----\n`)).toEqual([
            'line 8 is neither a comment nor a "Field: value" line',
        ]);
    });

    it('requires https Canonical and Policy, the Canonical at /.well-known/security.txt', () => {
        expect(prod(swap('Policy: https://', 'Policy: http://'))).toEqual([
            'Policy http://example.org/security is not an https URL',
        ]);
        expect(prod(swap('Canonical: https://', 'Canonical: http://'))).toEqual([
            'Canonical http://example.org/.well-known/security.txt is not an https URL',
        ]);
        expect(prod(swap('.well-known/security.txt', 'security.txt'))).toEqual([
            'Canonical https://example.org/security.txt is not served at /.well-known/security.txt',
        ]);
        expect(prod(FILLED.replace(/^Policy: .*\n/m, ''))).toEqual(['Policy is required']);
        expect(prod(FILLED.replace(/^Canonical: .*\n/m, ''))).toEqual(['Canonical is required']);
    });

    it('refuses unknown fields, malformed lines and placeholders in production', () => {
        expect(prod(`${FILLED}Bounty: yes\n`)).toEqual(['line 8: unknown field Bounty']);
        expect(prod(`${FILLED}just text\n`)).toEqual(['line 8 is neither a comment nor a "Field: value" line']);
        const held = swap('mailto:security@example.org', 'mailto:OWNER-FILL-security-mailbox');
        expect(prod(held)).toEqual(['Contact is still an OWNER-FILL placeholder']);
        expect(securityTxtProblems(held, { now: NOW, production: false })).toEqual([]);
    });
});
