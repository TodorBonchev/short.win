import { describe, expect, it } from 'vitest';
import { browseLink, MOBILE_WALLETS } from './deepLinks.ts';

const HREF = 'https://x.test/m/5TNPqAbCdEfGhJkLmNoPqRsTuVwXyZ12TYqJ?panel=lend&action=deposit#a';
const ORIGIN = 'https://x.test';
const ENC_HREF =
    'https%3A%2F%2Fx.test%2Fm%2F5TNPqAbCdEfGhJkLmNoPqRsTuVwXyZ12TYqJ%3Fpanel%3Dlend%26action%3Ddeposit%23a';

describe('wallet browse links (FRONTEND.md §5.4)', () => {
    it('encode path, query and hash, keep the case, and send the origin only as ref', () => {
        expect(browseLink('backpack', HREF, ORIGIN)).toBe(
            `https://backpack.app/ul/v1/browse/${ENC_HREF}?ref=https%3A%2F%2Fx.test`,
        );
        expect(browseLink('phantom', HREF, ORIGIN)).toBe(
            `https://phantom.com/ul/browse/${ENC_HREF}?ref=https%3A%2F%2Fx.test`,
        );
        expect(browseLink('solflare', HREF, ORIGIN)).toBe(
            `https://solflare.com/ul/v1/browse/${ENC_HREF}?ref=https%3A%2F%2Fx.test`,
        );
    });

    it('lists the wallets alphabetically', () => {
        expect([...MOBILE_WALLETS]).toEqual([...MOBILE_WALLETS].sort());
    });

    it('never produces another host, whatever the page URL', () => {
        const hosts = new Set(['backpack.app', 'phantom.com', 'solflare.com']);
        for (const w of MOBILE_WALLETS) {
            for (const href of [HREF, 'https://evil.test/@x', '//evil.test', 'javascript:alert(1)']) {
                const u = new URL(browseLink(w, href, 'https://evil.test/'));
                expect(hosts.has(u.host)).toBe(true);
                expect(u.protocol).toBe('https:');
                expect(u.searchParams.getAll('ref')).toHaveLength(1);
            }
        }
    });
});
