/**
 * Design-pass screenshot helper (not part of the repo: `.e2e/` is gitignored).
 *
 * Captures routes of the running app at phone / tablet / desktop viewports with the seeded E2E
 * wallet installed and pre-authorised and the indexer mock routed, as the E2E specs do — but against
 * the Vite dev server (`pnpm dev:e2e`, HMR), so a CSS change shows up in seconds.
 *
 *   node .e2e/shots.ts --out <dir> [--base http://127.0.0.1:5173] [--viewports phone,desktop]
 *                      [--wallet margin|claimer|trader|none] [--disconnected]
 *                      [--routes /markets,/m/$main,...] [--no-full] [--prefix name-]
 *                      [--click 'css1,,css2'] [--type 'css::text'] [--wait ms] [--scroll px]
 *
 * Viewports: phone-xs 320×640, phone-s 360×780, phone 390×844, phone-landscape 844×390, tablet 768×1024,
 * laptop 1024×768, laptop-l 1280×800, desktop 1440×900.
 * Route placeholders: $main $move $paused $t22 $wipe $pot $chain → the seeded market's mint;
 * $pos-margin $pos-closer $pos-holder → the seeded position's address.
 * Output: <dir>/<viewport>/<slug>.png (the first screen) and <slug>-full.png (the whole document).
 * --click: selectors clicked in order after the page settles (separator ",,"); --type: fill a field.
 */
import { mkdirSync, readFileSync } from 'node:fs';
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
const has = (name: string): boolean => args.includes(`--${name}`);

const out = opt('out', '');
if (!out) {
    console.error('--out <dir> is required');
    process.exit(2);
}
const base = opt('base', 'http://127.0.0.1:5173');
const viewportNames = opt('viewports', 'phone,desktop').split(',');
const walletName = opt('wallet', 'margin');
const full = !has('no-full');
const prefix = opt('prefix', '');
const clicks = opt('click', '') ? opt('click', '').split(',,') : [];
const typeArg = opt('type', '');
const waitMs = Number(opt('wait', '0'));
const scrollPx = Number(opt('scroll', '0'));

const ipad = devices['iPad (gen 7)'] ?? devices['iPhone 13'];
const VIEWPORTS: Record<
    string,
    { width: number; height: number; isMobile: boolean; hasTouch: boolean; deviceScaleFactor: number; userAgent?: string }
> = {
    'phone-xs': { width: 320, height: 640, isMobile: true, hasTouch: true, deviceScaleFactor: 2, userAgent: devices['iPhone 13'].userAgent },
    'phone-s': { width: 360, height: 780, isMobile: true, hasTouch: true, deviceScaleFactor: 2, userAgent: devices['Pixel 7'].userAgent },
    phone: { width: 390, height: 844, isMobile: true, hasTouch: true, deviceScaleFactor: 2, userAgent: devices['iPhone 13'].userAgent },
    'phone-landscape': { width: 844, height: 390, isMobile: true, hasTouch: true, deviceScaleFactor: 2, userAgent: devices['iPhone 13'].userAgent },
    tablet: { width: 768, height: 1024, isMobile: true, hasTouch: true, deviceScaleFactor: 2, userAgent: ipad.userAgent },
    laptop: { width: 1024, height: 768, isMobile: false, hasTouch: false, deviceScaleFactor: 1 },
    'laptop-l': { width: 1280, height: 800, isMobile: false, hasTouch: false, deviceScaleFactor: 1 },
    desktop: { width: 1440, height: 900, isMobile: false, hasTouch: false, deviceScaleFactor: 1 },
};

const DEFAULT_ROUTES = [
    '/markets',
    '/m/$main',
    '/positions',
    '/positions/$pos-margin',
    '/lend',
    '/status',
    '/how-it-works',
    '/security',
    '/markets/new',
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
function slugOf(route: string): string {
    const s = route.replace(/^\//, '').replace(/[^a-zA-Z0-9]+/g, '-').replace(/^-+|-+$/g, '');
    return s || 'home';
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
        if (!vp) {
            console.error(`unknown viewport ${vn} (known: ${Object.keys(VIEWPORTS).join(', ')})`);
            continue;
        }
        const context = await browser.newContext({
            viewport: { width: vp.width, height: vp.height },
            isMobile: vp.isMobile,
            hasTouch: vp.hasTouch,
            deviceScaleFactor: vp.deviceScaleFactor,
            userAgent: vp.userAgent,
            colorScheme: 'dark',
            locale: 'en-US',
            timezoneId: 'UTC',
            baseURL: base,
        });
        if (walletName !== 'none') {
            const secret = loadSecretKey(walletName as WalletName);
            const address = publicKeyOf(secret);
            const cfg = { secretKey: Array.from(secret), chains: ['solana:localnet'], label: walletName };
            await context.addInitScript({
                content: `${WALLET_SOURCE}\n;window.__shortwinInstallE2EWallet(${JSON.stringify(cfg)});`,
            });
            if (!has('disconnected')) {
                // The injected wallet remembers the approved site, and the app's wallet plugin
                // remembers which account to reconnect to silently (WalletButton.tsx WALLET_STORAGE_KEY).
                await context.addInitScript({
                    content: `try { localStorage.setItem('shortwin-e2e-wallet:authorized', ${JSON.stringify(address)}); localStorage.setItem('shortwin:wallet', ${JSON.stringify(`ShortWin E2E:${address}`)}); } catch {}`,
                });
            }
        }
        const mock = new IndexerMock(state, 'mock');
        const page = await context.newPage();
        await mock.install(page);
        const dir = join(out, vn);
        mkdirSync(dir, { recursive: true });
        for (const route of routes) {
            const slug = prefix + slugOf(route);
            try {
                await page.goto(resolve(route));
                await settle(page);
                for (const sel of clicks) {
                    await page.locator(sel).first().click({ timeout: 10_000 });
                    await page.waitForTimeout(250);
                }
                if (typeArg) {
                    const [sel, text] = typeArg.split('::');
                    if (sel && text !== undefined) await page.locator(sel).first().fill(text);
                    await page.waitForTimeout(600);
                }
                if (scrollPx > 0) {
                    await page.evaluate((y) => window.scrollTo(0, y), scrollPx);
                    await page.waitForTimeout(200);
                }
                if (waitMs > 0) await page.waitForTimeout(waitMs);
                await page.screenshot({ path: join(dir, `${slug}.png`), animations: 'disabled' });
                if (full) {
                    const extra = await page.evaluate(() =>
                        Math.max(0, document.documentElement.scrollHeight - window.innerHeight),
                    );
                    if (extra > 0) {
                        await page.setViewportSize({ width: vp.width, height: Math.min(vp.height + extra, 6_000) });
                        await page.waitForTimeout(150);
                        await page.screenshot({ path: join(dir, `${slug}-full.png`), animations: 'disabled' });
                        await page.setViewportSize({ width: vp.width, height: vp.height });
                    }
                }
                console.log(`${vn}/${slug}`);
            } catch (e) {
                console.error(`${vn}/${slug}: ${(e as Error).message.split('\n')[0]}`);
                await page.screenshot({ path: join(dir, `${slug}-ERROR.png`) }).catch(() => undefined);
            }
        }
        mock.dispose();
        await context.close();
    }
} finally {
    await browser.close();
}
