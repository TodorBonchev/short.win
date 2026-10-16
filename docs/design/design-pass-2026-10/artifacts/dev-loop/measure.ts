/**
 * Design-pass measurement helper (not part of the repo: `.e2e/` is gitignored). Loads routes at
 * desktop viewports against the dev server and prints, per page state, the document's sideways
 * overflow, whether the page scrolls vertically, and every element that scrolls or clips sideways
 * (the responsive gate's h-scroller rule: scrollWidth > clientWidth + 1), with `.tbl-wrap` singled out.
 *
 *   node .e2e/measure.ts [--viewports laptop-l,desktop] [--wallet margin] [--routes '/markets,/m/$main@@Positions']
 *
 * A route may end in `@@<Tab name>`: that tab is clicked after the page settles.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { chromium, devices, type Page } from '@playwright/test';
import { WEB_DIR } from '../e2e/env.ts';
import { IndexerMock } from '../e2e/indexer-mock.ts';
import { loadSecretKey, publicKeyOf, type WalletName } from '../e2e/validator/keys.ts';
import { loadState } from '../e2e/validator/state.ts';

const args = process.argv.slice(2);
function opt(name: string, dflt: string): string {
    const i = args.indexOf(`--${name}`);
    const v = i >= 0 ? args[i + 1] : undefined;
    return v === undefined ? dflt : v;
}
const base = opt('base', 'http://127.0.0.1:5173');
const viewportNames = opt('viewports', 'laptop-l,desktop').split(',');
const walletName = opt('wallet', 'margin');
const VIEWPORTS: Record<string, { width: number; height: number; isMobile: boolean; hasTouch: boolean; deviceScaleFactor: number; userAgent?: string }> = {
    'phone-landscape': { width: 844, height: 390, isMobile: true, hasTouch: true, deviceScaleFactor: 2, userAgent: devices['iPhone 13'].userAgent },
    tablet: { width: 768, height: 1024, isMobile: true, hasTouch: true, deviceScaleFactor: 2, userAgent: devices['iPad (gen 7)'].userAgent },
    laptop: { width: 1024, height: 768, isMobile: false, hasTouch: false, deviceScaleFactor: 1 },
    'laptop-l': { width: 1280, height: 800, isMobile: false, hasTouch: false, deviceScaleFactor: 1 },
    desktop: { width: 1440, height: 900, isMobile: false, hasTouch: false, deviceScaleFactor: 1 },
    wide: { width: 1680, height: 1000, isMobile: false, hasTouch: false, deviceScaleFactor: 1 },
};
const DEFAULT_ROUTES = [
    '/markets', '/m/$main', '/m/$main@@Positions', '/m/$main@@Lenders', '/m/$main@@Liquidations', '/m/$main@@Pools',
    '/m/$main@@Coverage', '/m/$main@@Events', '/positions', '/positions/$pos-margin', '/lend', '/status', '/how-it-works',
    '/security', '/markets/new',
];
const routes = opt('routes', '') ? opt('routes', '').split(',') : DEFAULT_ROUTES;
const state = loadState();
function resolve(route: string): string {
    return route
        .replace(/\$pos-(\w+)/g, (m, k: string) => {
            const p = state.positions?.[k as keyof NonNullable<typeof state.positions>];
            return p ? p.position : m;
        })
        .replace(/\$(\w+)/g, (m, k: string) => {
            const mk = state.markets[k as keyof typeof state.markets];
            return mk ? mk.mint : m;
        });
}
async function settle(page: Page): Promise<void> {
    await page.locator('[data-testid="app-header"]').waitFor({ state: 'visible', timeout: 30_000 });
    const deadline = Date.now() + 20_000;
    while (Date.now() < deadline) {
        if ((await page.locator('.skeleton').count()) === 0) break;
        await page.waitForTimeout(200);
    }
    await page.waitForTimeout(400);
}
const WALLET_SOURCE = readFileSync(join(WEB_DIR, 'e2e', 'wallet', 'injected-wallet.js'), 'utf8');
const browser = await chromium.launch({ executablePath: process.env.SHORTWIN_CHROMIUM ?? '/usr/bin/chromium' });
try {
    for (const vn of viewportNames) {
        const vp = VIEWPORTS[vn];
        if (!vp) { console.error(`unknown viewport ${vn}`); continue; }
        const context = await browser.newContext({
            viewport: { width: vp.width, height: vp.height }, isMobile: vp.isMobile, hasTouch: vp.hasTouch,
            deviceScaleFactor: vp.deviceScaleFactor, userAgent: vp.userAgent, colorScheme: 'dark', locale: 'en-US', timezoneId: 'UTC', baseURL: base,
        });
        if (walletName !== 'none') {
            const secret = loadSecretKey(walletName as WalletName);
            const address = publicKeyOf(secret);
            const cfg = { secretKey: Array.from(secret), chains: ['solana:localnet'], label: walletName };
            await context.addInitScript({ content: `${WALLET_SOURCE}\n;window.__shortwinInstallE2EWallet(${JSON.stringify(cfg)});` });
            await context.addInitScript({ content: `try { localStorage.setItem('shortwin-e2e-wallet:authorized', ${JSON.stringify(address)}); localStorage.setItem('shortwin:wallet', ${JSON.stringify(`ShortWin E2E:${address}`)}); } catch {}` });
        }
        const mock = new IndexerMock(state, 'mock');
        const page = await context.newPage();
        await mock.install(page);
        console.log(`== ${vn} ${vp.width}x${vp.height} wallet=${walletName}`);
        for (const spec of routes) {
            const [route, tab] = spec.split('@@');
            try {
                await page.goto(resolve(route ?? '/'));
                await settle(page);
                if (tab) { await page.getByRole('tab', { name: tab, exact: true }).click({ timeout: 10_000 }); await page.waitForTimeout(500); await settle(page); }
                const m = await page.evaluate((deviceWidth) => {
                    const describe = (el: Element) => {
                        const id = el.getAttribute('data-testid');
                        return `${el.tagName.toLowerCase()}${id ? `[${id}]` : ''}${el.className && typeof el.className === 'string' ? `.${el.className.trim().split(/\s+/).slice(0, 2).join('.')}` : ''}`;
                    };
                    const items: string[] = [];
                    for (const el of document.querySelectorAll('*')) {
                        if (el.clientWidth === 0 || el.scrollWidth <= el.clientWidth + 1) continue;
                        const ox = getComputedStyle(el).overflowX;
                        const kind = ox === 'auto' || ox === 'scroll' ? 'SCROLL' : ox === 'hidden' || ox === 'clip' ? 'clip' : 'vis';
                        if (kind === 'vis') continue;
                        items.push(`${kind} ${describe(el)} ${el.scrollWidth}>${el.clientWidth}`);
                    }
                    const tbl = [...document.querySelectorAll('.tbl-wrap')].map((w) => `${w.clientWidth - w.scrollWidth}`);
                    return {
                        docOverflowX: document.documentElement.scrollWidth - deviceWidth,
                        contentW: document.documentElement.clientWidth,
                        vScroll: document.documentElement.scrollHeight > window.innerHeight,
                        gutter: getComputedStyle(document.documentElement).getPropertyValue('--gutter').trim(),
                        headerH: (document.querySelector('[data-testid="app-header"]') as HTMLElement | null)?.getBoundingClientRect().height,
                        tblSpare: tbl,
                        items,
                    };
                }, vp.width);
                console.log(`${spec.padEnd(28)} docX=${m.docOverflowX} contentW=${m.contentW} vScroll=${m.vScroll} gutter=${m.gutter} header=${m.headerH} tblSpare=[${m.tblSpare.join(' ')}]${m.items.length ? `\n    ${m.items.join('\n    ')}` : ''}`);
            } catch (e) {
                console.error(`${spec}: ${(e as Error).message.split('\n')[0]}`);
            }
        }
        mock.dispose();
        await context.close();
    }
} finally {
    await browser.close();
}
