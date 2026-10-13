/**
 * mobile-mwa — the Mobile Wallet Adapter on an Android browser (O-6, lifted 2026-10-13; FRONTEND.md
 * §5.4 (6), §10.2; mobile.md §5.2): project `<profile>-android` (a Pixel 7: 412 × 839, touch,
 * DPR 2.625, an Android Chrome user agent). No wallet is injected (`wallet: null`), so the only
 * registered wallet is the adapter the app registers; the `mwa` fixture stands in for Android
 * around it: `wallet/mwa-hooks.js` records each intent (`association`, `port`, whether the tap's
 * user activation was still active) and plays the app switch (a `blur`), and `wallet/fake-mwa.ts`
 * answers the association socket at the protocol level as a wallet app signing as the seeded
 * `android` wallet. Every step is a real tap, axe runs at 0 on each state and the fixture's zero-CSP
 * guard holds.
 *
 * Flag on (the default build): M1 the chooser · M2 connect (the activation, the library's spinner
 * inserted and removed at once) · M3 the silent reconnect and Disconnect · M4 sign and confirm ·
 * M5 the activation survives a slow RPC (no network await between Sign and the wallet) · M6 a
 * decline · M7 a tampered transaction · M8 a launch nobody answers (the retry notice; Disconnect
 * then does not reopen the chooser) · M9 the session closed · M10 / M10b a wallet that cannot sign
 * through `solana:signTransaction` gets no signer · M11 the CSP lets the real socket and the prompt
 * request through · M12 a social app's in-app browser · M13 / M14 the reauthorization declined or
 * closed (R-S2) · M15 the wallet never answers the sign (R-S1) · M16 the wallet never answers the
 * connect (R-C1), with M17 no library overlay to tap meanwhile · M18 another account at the
 * reauthorization (R-S2) · T-UIR Blink does not upgrade `ws://localhost` / `http://localhost`.
 *
 * Flag off (`SHORTWIN_E2E_MWA=0 pnpm --filter web e2e --project chain-android`, which rebuilds the
 * app): no entry, today's no-wallet dialog, the library's chunk never fetched, and the M11 probes
 * refused by the CSP.
 *
 * Signs 1-token deposits into `main` with `android` (M4, M5, M8, M15). Writes
 * `android/wallet-mwa.png` and `android/wallet-in-app-browser.png` (CSS scale) to
 * `.e2e/screenshots/`, or to the reviewed set `e2e/screenshots/` with `SHORTWIN_E2E_SCREENSHOTS=update`.
 */
import { mkdirSync } from 'node:fs';
import { join } from 'node:path';
import type { Page } from '@playwright/test';
import { walletCopy } from '../../src/config/copyWallet.ts';
import { UA } from '../../src/test/userAgents.ts';
import { browseLink, MOBILE_WALLETS } from '../../src/wallet/deepLinks.ts';
import { MWA_NOT_FOUND_FRESH_MS, MWA_WALLET_NAME } from '../../src/wallet/mwa.ts';
import { MWA_RETURN_GRACE_MS, MWA_SIGN_CANCEL_AFTER_MS } from '../../src/wallet/mwaCall.ts';
import { E2E_DIR, ENV, URLS, WEB_DIR } from '../env.ts';
import { type App, expect, need, T, test } from '../fixtures.ts';
import { signatureKnown } from '../validator/actions.ts';
import { getTokenAmount } from '../validator/chain.ts';
import type { E2EState } from '../validator/state.ts';
import { FAKE_MWA_AUTH_TOKEN, type FakeMwa } from '../wallet/fake-mwa.ts';

const DIR = join(
    process.env.SHORTWIN_E2E_SCREENSHOTS === 'update'
        ? join(WEB_DIR, 'e2e', 'screenshots')
        : join(E2E_DIR, 'screenshots'),
    'android',
);
mkdirSync(DIR, { recursive: true });

const MWA_OPTION = T.walletOption(MWA_WALLET_NAME);
/** The library's own marker (its authorization cache's storage key): only its chunk carries it. */
const LIBRARY_MARKER = 'SolanaMobileWalletAdapterDefaultAuthorizationCache';

/** The first screen, at CSS scale (what the user sees). */
async function shot(page: Page, name: string): Promise<void> {
    await expect(page.locator('.skeleton:visible')).toHaveCount(0, { timeout: 20_000 });
    await page.screenshot({ path: join(DIR, `${name}.png`), animations: 'disabled', scale: 'css' });
}

function fakeOf(f: FakeMwa | null): FakeMwa {
    return need(f, 'fake MWA wallet (test.use({ mwa }))');
}

/** Opens the header chooser and taps "Use installed wallet"; the fake wallet authorizes. */
async function connectMwa(app: App): Promise<void> {
    await app.id(T.walletButton).tap();
    await expect(app.id(T.walletMenu)).toBeVisible();
    await app.id(MWA_OPTION).tap();
    await expect(app.id(T.walletAddress)).toHaveAttribute('data-value', app.walletAddress);
}

/** `android`'s E2EMAIN account (the deposit's source). */
function androidAta(state: E2EState) {
    return need(state.tokenAccounts.mainAta.android, 'android main ATA');
}

/**
 * In one page load (an account change must reach the plugin as the user's own pick): the market
 * page, connected through the adapter, Lend → the trade sheet, a deposit of `amount`, acknowledged
 * and reviewed, Sign enabled. With `reconnect` (the test connected on an earlier page load), the
 * silent reconnect is awaited instead of the chooser.
 */
async function depositReview(
    app: App,
    state: E2EState,
    { amount = '1', reconnect = false }: { amount?: string; reconnect?: boolean } = {},
): Promise<void> {
    const main = state.markets.main;
    await app.goto(`/m/${main.mint}?panel=lend&action=deposit`, 'market');
    if (reconnect) {
        const address = app.id(T.walletAddress);
        await expect(address).toHaveAttribute('data-value', app.walletAddress, { timeout: 15_000 });
    } else if (!(await app.id(T.walletAddress).isVisible())) await connectMwa(app);
    await app.id(T.marketActionLend).tap();
    const panel = app.id(T.lendPanel);
    await expect(panel).toBeVisible();
    await expect(panel).toHaveAttribute('data-action', 'deposit');
    const input = panel.getByTestId(T.lendAmountInput);
    await input.tap();
    await input.fill(amount);
    const reason = app.id(T.lendDepositReason);
    await expect(reason).toContainText('Tick the acknowledgement above');
    // The reason's button brings the acknowledgement into view and focuses it (mobile-lend).
    await reason.getByRole('button').tap();
    const ack = panel.getByTestId(T.disclosureAck('lender.solRepayment'));
    await expect(ack).toBeFocused();
    await ack.tap();
    await expect(ack).toBeChecked();
    await expect(app.id(T.lendSubmit)).toBeEnabled();
    await app.id(T.lendSubmit).tap();
    expect(await app.simulated()).toBe('review');
    await app.inModal(T.disclosureAck('lender.solRepayment')).tap();
    await expect(app.inModal(T.txSign)).toBeEnabled();
}

/** Taps Sign and waits for `confirmed`; returns the signature (`tx-signature` `data-value`). */
async function tapSignAndConfirm(app: App): Promise<string> {
    await expect(app.inModal(T.txSign)).toBeEnabled();
    await app.inModal(T.txSign).tap();
    await app.expectStage('confirmed', 90_000);
    return need(await app.inModal(T.txSignature).getAttribute('data-value'), 'tx-signature');
}

/** Nothing moved: `android`'s E2EMAIN balance is what it was. */
async function expectNothingLanded(state: E2EState, before: bigint | null): Promise<void> {
    expect(await getTokenAmount(androidAta(state))).toBe(before);
}

/**
 * From now on the page notes when `selector` first matches, on the wall clock
 * (`performance.timeOrigin + performance.now()`: the clock of Node's `Date.now()`, so of the fake
 * wallet's `answeredAt`); returns a reader for that moment (`null` until it matched).
 */
async function firstMatchAt(page: Page, selector: string): Promise<() => Promise<number | null>> {
    await page.evaluate((sel) => {
        const w = window as unknown as { __firstMatchAt?: Record<string, number> };
        const seen = w.__firstMatchAt ?? {};
        w.__firstMatchAt = seen;
        const check = () => {
            if (seen[sel] === undefined && document.querySelector(sel))
                seen[sel] = performance.timeOrigin + performance.now();
        };
        new MutationObserver(check).observe(document.documentElement, {
            subtree: true,
            childList: true,
            attributes: true,
        });
        check();
    }, selector);
    return () =>
        page.evaluate(
            (sel) => (window as unknown as { __firstMatchAt?: Record<string, number> }).__firstMatchAt?.[sel] ?? null,
            selector,
        );
}

/** In the page: the CSP violations a native WebSocket to a loopback port and the prompt request raise. */
async function cspProbes(page: Page): Promise<string[]> {
    await page.route('http://localhost/', (r) =>
        r.fulfill({ status: 204, headers: { 'access-control-allow-origin': '*' } }),
    );
    return page.evaluate(async () => {
        const seen: string[] = [];
        document.addEventListener('securitypolicyviolation', (e) =>
            seen.push(`${e.violatedDirective} ${e.blockedURI}`),
        );
        try {
            // Outside the routed `/solana-wallet` path: a native socket, which the CSP governs (the
            // routed association socket is a page-side mock that bypasses connect-src).
            const ws = new WebSocket('ws://localhost:49151/csp-probe');
            ws.onerror = () => {};
        } catch (e) {
            seen.push(`WebSocket threw: ${(e as Error).name}`);
        }
        try {
            await fetch('http://localhost/', { mode: 'no-cors', credentials: 'omit', cache: 'no-store' });
        } catch (e) {
            seen.push(`fetch failed: ${(e as Error).message}`);
        }
        await new Promise((r) => setTimeout(r, 1_000));
        return seen;
    });
}

test.describe('Mobile Wallet Adapter (flag on)', { tag: '@scaffold' }, () => {
    test.skip(!ENV.mwa, 'the app is built with the Mobile Wallet Adapter off (SHORTWIN_E2E_MWA=0)');
    test.use({ wallet: null, mwa: {} });

    test('M1 the chooser: "Use installed wallet", then the open-in links outside the menu', async ({ app, page }) => {
        await app.goto('/markets', 'markets');
        await app.id(T.walletButton).tap();
        const menu = app.id(T.walletMenu);
        await expect(menu).toBeVisible();
        const entry = app.id(MWA_OPTION);
        await expect(entry).toBeVisible();
        await expect(entry).toContainText(walletCopy('wallet.mwa.label'));
        await expect(entry.getByTestId(T.walletMwaHint)).toHaveText(walletCopy('wallet.mwa.hint'));
        await expect(entry).toHaveAttribute('data-mwa', '');
        await expect(entry).toHaveAttribute('data-local-network', 'ready');
        // role="menu" holds the entry alone; the links follow outside it.
        await expect(menu.getByRole('menuitem')).toHaveCount(1);
        const block = app.id(T.walletOpenElsewhere);
        await expect(block).toContainText(walletCopy('wallet.openIn.more'));
        expect(await block.evaluate((el) => el.closest('[role="menu"]') === null)).toBe(true);
        const url = page.url();
        const origin = new URL(url).origin;
        for (const w of MOBILE_WALLETS) {
            await expect(block.getByTestId(T.walletOpenIn(w))).toHaveAttribute('href', browseLink(w, url, origin));
        }
        await expect(block).toContainText('Set your wallet to localnet first.');
        await expect(app.id(T.walletSendsItself)).toHaveCount(0);
        await app.a11y('android chooser with the Mobile Wallet Adapter');
        await shot(page, 'wallet-mwa');
    });

    test('M2 connect: one association within the tap’s activation; the library’s spinner removed at insertion', async ({
        app,
        page,
        fakeMwa,
    }) => {
        const fake = fakeOf(fakeMwa);
        await app.goto('/markets', 'markets');
        const url = page.url();
        await app.id(T.walletButton).tap();
        // The entry shows once the package is loaded, and the app's host observer is installed then.
        await expect(app.id(MWA_OPTION)).toBeVisible();
        // Every <div> appended to or removed from <body>, and whether it is the library's spinner host.
        await page.evaluate(() => {
            const seen: { op: 'add' | 'remove'; spinner: boolean }[] = [];
            (window as unknown as { __bodyDivs: typeof seen }).__bodyDivs = seen;
            const spinner = (n: Node) =>
                n instanceof HTMLDivElement && n.style.getPropertyValue('--spinner-color') !== '';
            new MutationObserver((records) => {
                for (const r of records) {
                    for (const n of r.addedNodes)
                        if (n instanceof HTMLDivElement) seen.push({ op: 'add', spinner: spinner(n) });
                    for (const n of r.removedNodes)
                        if (n instanceof HTMLDivElement) seen.push({ op: 'remove', spinner: spinner(n) });
                }
            }).observe(document.body, { childList: true });
        });
        await app.id(MWA_OPTION).tap();
        await expect(app.id(T.walletAddress)).toHaveAttribute('data-value', app.walletAddress);
        const launches = await app.mwaLaunches();
        expect(launches).toHaveLength(1);
        expect(launches[0]?.active).toBe(true);
        // `location.assign('solana-wallet:…')` left the page where it was: headless Chromium has no
        // app for the scheme (were it to navigate, CDP's Page.frameRequestedNavigation would have to
        // stand in for the hooks).
        expect(page.url(), 'the page stays after the intent').toBe(url);
        expect([...fake.methods()].sort()).toEqual(['authorize', 'get_capabilities']);
        expect(fake.sessions).toEqual([expect.objectContaining({ established: true, closedByWallet: false })]);
        // The association inserted the spinner's host once (its <style> and attribute passed the CSP:
        // the fixture's guard) and the app removed it at once.
        const divs = await page.evaluate(
            () => (window as unknown as { __bodyDivs: { op: string; spinner: boolean }[] }).__bodyDivs,
        );
        expect(divs.filter((d) => d.spinner)).toEqual([
            { op: 'add', spinner: true },
            { op: 'remove', spinner: true },
        ]);
        expect(
            await page.evaluate(() =>
                [...document.querySelectorAll<HTMLElement>('body > div')].some(
                    (d) => d.style.getPropertyValue('--spinner-color') !== '',
                ),
            ),
        ).toBe(false);
        await app.a11y('android connected through the Mobile Wallet Adapter');
    });

    test('M3 a reload reconnects without an association; after Disconnect it does not', async ({
        app,
        page,
        fakeMwa,
    }) => {
        const fake = fakeOf(fakeMwa);
        await app.goto('/markets', 'markets');
        await connectMwa(app);
        await page.reload();
        await expect(app.id(T.header)).toBeVisible();
        await expect(app.id(T.walletAddress)).toHaveAttribute('data-value', app.walletAddress, { timeout: 15_000 });
        expect(await app.mwaLaunches()).toEqual([]);
        expect(fake.sessions).toHaveLength(1);
        await app.a11y('android silently reconnected');
        await app.id(T.walletButton).tap();
        await app.id(T.walletDisconnect).tap();
        await expect(app.id(T.walletAddress)).toHaveCount(0);
        await page.reload();
        await expect(app.id(T.walletButton)).toHaveAccessibleName('Connect wallet', { timeout: 15_000 });
        await expect(app.id(T.walletAddress)).toHaveCount(0);
        expect(await app.mwaLaunches()).toEqual([]);
        expect(fake.sessions).toHaveLength(1);
    });

    test('M11 the CSP lets a native loopback socket and the prompt request through', async ({ app, page }) => {
        await app.goto('/markets', 'markets');
        expect(await cspProbes(page)).toEqual([]);
    });

    test('M16 / M17 a connect the wallet never answers ends 10 s after the page is back; no overlay meanwhile', async ({
        app,
        page,
        fakeMwa,
    }) => {
        const fake = fakeOf(fakeMwa);
        // A page whose middle is text (a tap there must not navigate).
        await app.goto('/security', 'security');
        fake.next('silentConnect');
        await app.id(T.walletButton).tap();
        await app.id(MWA_OPTION).tap();
        const button = app.id(T.walletButton);
        await expect(button).toHaveAttribute('aria-busy', 'true');
        await expect(button).toBeDisabled();
        await expect.poll(() => fake.sessions.at(-1)?.established ?? false, { timeout: 10_000 }).toBe(true);
        await expect
            .poll(() => fake.calls.find((c) => c.method === 'authorize')?.outcome, { timeout: 10_000 })
            .toBe('pending');

        await test.step('M17: a tap where the library’s overlay would be changes nothing', async () => {
            const url = page.url();
            const vp = need(page.viewportSize(), 'a viewport');
            const hit = await page.evaluate(
                ([x, y, root]) => {
                    const el = document.elementFromPoint(x, y);
                    return {
                        inPage: !!el?.closest(`[data-testid="${root}"]`),
                        control: !!el?.closest('a, button, input, select, textarea'),
                        tag: el?.tagName ?? null,
                    };
                },
                [vp.width / 2, vp.height / 2, T.page('security')] as const,
            );
            expect(hit.control, `the middle of the screen is no control (${JSON.stringify(hit)})`).toBe(false);
            expect(hit.inPage, `the middle of the screen is the page (${JSON.stringify(hit)})`).toBe(true);
            await page.mouse.click(vp.width / 2, vp.height / 2);
            await page.waitForTimeout(500);
            expect(page.url()).toBe(url);
            expect(fake.sessions.at(-1)).toMatchObject({ closedByPage: false, closedByWallet: false });
            expect(fake.calls.find((c) => c.method === 'authorize')?.outcome).toBe('pending');
            expect(
                await page.evaluate(() =>
                    [...document.querySelectorAll<HTMLElement>('body > div')].some(
                        (d) => d.style.getPropertyValue('--spinner-color') !== '',
                    ),
                ),
            ).toBe(false);
            await expect(button).toBeDisabled();
        });

        // The browser comes back to the front without the wallet answering: the connect ends 10 s later.
        await app.mwaReturnToPage();
        const back = Date.now();
        await expect(button).toBeEnabled({ timeout: MWA_RETURN_GRACE_MS + 15_000 });
        expect(Date.now() - back).toBeGreaterThanOrEqual(MWA_RETURN_GRACE_MS - 1_000);
        await expect(button).toHaveAccessibleName('Connect wallet');
        await app.a11y('android after an unanswered connect');
        // MWA's own connect lock was reset: the next tap launches the wallet again.
        const launches = (await app.mwaLaunches()).length;
        await connectMwa(app);
        expect(await app.mwaLaunches()).toHaveLength(launches + 1);
    });

    test.describe('M12 a social app’s in-app browser (Telegram for Android)', () => {
        // A WebView user agent: the library is never registered there.
        test.use({ userAgent: UA.telegramAndroid, permissions: ['clipboard-read', 'clipboard-write'] });

        test('no entry; "Open this page in your browser" and Copy link first, then the open-in links', async ({
            app,
            page,
        }) => {
            await app.goto('/markets', 'markets');
            await app.id(T.walletButton).tap();
            const menu = app.id(T.walletMenu);
            await expect(menu).toBeVisible();
            await expect(app.id(MWA_OPTION)).toHaveCount(0);
            await expect(menu).toHaveAttribute('role', 'dialog');
            const block = app.id(T.walletInAppBrowser);
            await expect(block).toContainText(walletCopy('wallet.webview.open'));
            // First in the chooser: before the open-in links.
            const first = MOBILE_WALLETS[0];
            expect(
                await page.evaluate(
                    ([a, b]) => {
                        const box = document.querySelector(`[data-testid="${a}"]`);
                        const link = document.querySelector(`[data-testid="${b}"]`);
                        return (
                            !!box && !!link && !!(box.compareDocumentPosition(link) & Node.DOCUMENT_POSITION_FOLLOWING)
                        );
                    },
                    [T.walletInAppBrowser, T.walletOpenIn(first)] as const,
                ),
            ).toBe(true);
            const copy = app.id(T.walletCopyLink);
            await expect(copy).toHaveText(walletCopy('wallet.webview.copy'));
            await copy.tap();
            await expect(copy).toHaveText('Copied');
            expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(page.url());
            // The confirmation is brief: the button reads "Copy link" again (and the PNG shows the
            // chooser at rest, not a race against the 1.2 s timer).
            await expect(copy).toHaveText(walletCopy('wallet.webview.copy'));
            const url = page.url();
            const origin = new URL(url).origin;
            for (const w of MOBILE_WALLETS) {
                await expect(app.id(T.walletOpenIn(w))).toHaveAttribute('href', browseLink(w, url, origin));
            }
            await app.a11y('android in-app browser chooser');
            await shot(page, 'wallet-in-app-browser');
        });
    });

    test('T-UIR Blink does not upgrade ws://localhost or http://localhost under upgrade-insecure-requests', async ({
        page,
    }) => {
        await page.route('https://uir.test/', (r) =>
            r.fulfill({
                status: 200,
                contentType: 'text/html',
                body: '<!doctype html><html lang="en"><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="upgrade-insecure-requests; connect-src ws://localhost:* http://localhost"><title>UIR probe</title><p>UIR probe</p></html>',
            }),
        );
        await page.route('http://localhost/', (r) => r.fulfill({ status: 204 }));
        const cdp = await page.context().newCDPSession(page);
        await cdp.send('Network.enable');
        const sockets: string[] = [];
        const requests: string[] = [];
        cdp.on('Network.webSocketCreated', (e) => sockets.push(e.url));
        cdp.on('Network.requestWillBeSent', (e) => requests.push(e.request.url));
        await page.goto('https://uir.test/');
        await page.evaluate(async () => {
            try {
                const ws = new WebSocket('ws://localhost:49152/x');
                ws.onerror = () => {};
            } catch {
                // a refusal shows as a missing socket below
            }
            // Bounded: should Chromium's own local-network check hold the request for a permission
            // nobody answers (headless shows no prompt), the CDP events below still carry its URL.
            await Promise.race([
                fetch('http://localhost/', { mode: 'no-cors', cache: 'no-store' }).catch(() => undefined),
                new Promise((r) => setTimeout(r, 2_000)),
            ]);
            await new Promise((r) => setTimeout(r, 1_000));
        });
        expect(sockets.filter((u) => u.includes('localhost:49152'))).toEqual(['ws://localhost:49152/x']);
        expect(requests.filter((u) => /\/\/localhost[:/]/.test(u))).toContain('http://localhost/');
        expect(requests.some((u) => u.startsWith('https://localhost'))).toBe(false);
    });
});

test.describe('Mobile Wallet Adapter: signing (flag on)', { tag: '@lend' }, () => {
    test.skip(!ENV.mwa, 'the app is built with the Mobile Wallet Adapter off (SHORTWIN_E2E_MWA=0)');
    test.use({ wallet: null, mwa: {} });

    test('M4 sign and confirm: a new association, the reauthorization, sign_transactions; it lands', async ({
        app,
        state,
        fakeMwa,
    }) => {
        const fake = fakeOf(fakeMwa);
        const main = state.markets.main;
        const before = need(await getTokenAmount(androidAta(state)), 'android balance');
        await depositReview(app, state);
        const wallet = app.inModal(T.txWallet);
        await expect(wallet).toContainText(MWA_WALLET_NAME);
        await expect(wallet).toHaveAttribute('data-value', app.walletAddress);
        await expect(wallet).toHaveAttribute('data-sends-itself', 'false');
        await app.a11y('android deposit review (Mobile Wallet Adapter)');
        const launches = (await app.mwaLaunches()).length;
        const signature = await tapSignAndConfirm(app);
        expect(await app.mwaLaunches()).toHaveLength(launches + 1);
        expect(fake.methods().slice(-2)).toEqual(['authorize', 'sign_transactions']);
        expect(fake.calls.at(-2)?.params.auth_token).toBe('e2e');
        expect(fake.methods()).not.toContain('sign_and_send_transactions');
        expect(fake.signed.at(-1)).toEqual({ signature, tampered: false });
        expect(await signatureKnown(signature)).toBe(true);
        expect(await getTokenAmount(androidAta(state))).toBe(before - 10n ** BigInt(main.decimals));
        await app.a11y('android deposit confirmed');
    });

    test('M5 the tap’s activation reaches the wallet although getLatestBlockhash takes 6 s', async ({
        app,
        page,
        state,
    }) => {
        await depositReview(app, state);
        const launches = (await app.mwaLaunches()).length;
        // From here on every getLatestBlockhash to the E2E RPC is held 6 s: before the prefetch the
        // Sign path awaited one, and the activation (5 s) lapsed before the intent.
        await page.route(
            (url) => url.origin === URLS.rpc,
            async (route) => {
                if ((route.request().postData() ?? '').includes('"getLatestBlockhash"'))
                    await new Promise((r) => setTimeout(r, 6_000));
                await route.fallback();
            },
        );
        const tapAt = await page.evaluate(() => performance.now());
        await app.inModal(T.txSign).tap();
        await expect.poll(async () => (await app.mwaLaunches()).length, { timeout: 5_000 }).toBe(launches + 1);
        const launch = need((await app.mwaLaunches()).at(-1), 'the association');
        expect(launch.active).toBe(true);
        expect(launch.at - tapAt).toBeLessThan(1_000);
        await app.expectStage('confirmed', 90_000);
        await app.a11y('android deposit confirmed after a slow RPC');
    });

    test('M6 a decline in the wallet: "Cancelled in your wallet"; Try again', async ({ app, state, fakeMwa }) => {
        const fake = fakeOf(fakeMwa);
        const before = await getTokenAmount(androidAta(state));
        await depositReview(app, state);
        fake.next('decline');
        await app.inModal(T.txSign).tap();
        await app.expectStage('failed');
        await expect(app.inModal(T.txStage)).toHaveText('Cancelled');
        await expect(app.inModal(T.txError)).toHaveAttribute('data-kind', 'wallet-rejected');
        await expect(app.inModal(T.txRetry)).toHaveText('Try again');
        expect(fake.calls.at(-1)).toMatchObject({ method: 'sign_transactions', outcome: 'error' });
        await app.a11y('android review: declined in the wallet');
        await expectNothingLanded(state, before);
    });

    test('M7 a tampered transaction is refused before anything is sent', async ({ app, state, fakeMwa }) => {
        const fake = fakeOf(fakeMwa);
        const before = await getTokenAmount(androidAta(state));
        await depositReview(app, state);
        fake.next('tamper');
        await app.inModal(T.txSign).tap();
        await app.expectStage('failed');
        await expect(app.inModal(T.txError)).toHaveAttribute('data-kind', 'integrity');
        const signed = need(fake.signed.at(-1), 'the tampered signature');
        expect(signed.tampered).toBe(true);
        expect(await signatureKnown(signed.signature)).toBe(false);
        await app.a11y('android review: a tampered transaction refused');
        await expectNothingLanded(state, before);
    });

    test('M8 a launch nobody answers: the retry notice, Sign again confirms; Disconnect does not reopen the chooser', async ({
        app,
        page,
        state,
    }) => {
        await depositReview(app, state);
        await app.mwaLaunch('not-found');
        await app.inModal(T.txSign).tap();
        const notice = app.inModal(T.txSignNotice);
        await expect(notice).toHaveAttribute('data-reason', 'launch', { timeout: 10_000 });
        await expect(notice).toHaveText(walletCopy('wallet.mwa.retry'));
        await app.expectStage('review');
        await expect(app.inModal(T.txSign)).toBeEnabled();
        await app.a11y('android review: the wallet app could not be opened');
        await app.mwaLaunch('ok');
        await tapSignAndConfirm(app);
        // Done closes the review and the trade sheet.
        await app.modal.getByRole('button', { name: 'Done' }).tap();
        await expect(app.modal).toHaveCount(0);
        // Then Disconnect. The chooser would reopen only for a not-found younger than
        // MWA_NOT_FOUND_FRESH_MS, which the confirm above may outlast: so a second launch nobody
        // answers, on a new page load, and Disconnect right after its notice.
        await depositReview(app, state, { reconnect: true });
        await app.mwaLaunch('not-found');
        const tapAt = Date.now(); // before the library's not-found (about 3 s after the intent)
        await app.inModal(T.txSign).tap();
        await expect(notice).toHaveAttribute('data-reason', 'launch', { timeout: 10_000 });
        await app.inModal(T.txCancel).tap();
        await expect(app.modal).toHaveCount(0);
        await app.id(T.tradeSheetClose).tap();
        await app.id(T.walletButton).tap();
        await app.id(T.walletDisconnect).tap();
        await expect(app.id(T.walletAddress)).toHaveCount(0);
        // Not vacuous: had the not-found at Sign been recorded (the wallet was connected, so it is
        // ignored), it would still be fresh here, and the chooser would reopen.
        expect(Date.now() - tapAt, 'Disconnect inside the not-found window').toBeLessThan(MWA_NOT_FOUND_FRESH_MS);
        await page.waitForTimeout(1_500);
        await expect(app.id(T.walletMenu)).toHaveCount(0);
    });

    test('M9 the wallet closes the session during sign_transactions: the generic wallet error, Retry', async ({
        app,
        state,
        fakeMwa,
    }) => {
        const fake = fakeOf(fakeMwa);
        const before = await getTokenAmount(androidAta(state));
        await depositReview(app, state);
        fake.next('close');
        await app.inModal(T.txSign).tap();
        await app.expectStage('failed');
        await expect(app.inModal(T.txError)).toHaveAttribute('data-kind', 'wallet');
        await expect(app.inModal(T.txRetry)).toBeVisible();
        expect(fake.calls.at(-1)).toMatchObject({ method: 'sign_transactions', outcome: 'closed' });
        await app.a11y('android review: the session closed');
        await app.inModal(T.txCancel).tap();
        await expect(app.modal).toHaveCount(0);
        await expectNothingLanded(state, before);
    });

    for (const [name, b] of [
        ['M13 the reauthorization declined', 'reauthDecline'],
        ['M14 the session closed during the reauthorization', 'reauthClose'],
    ] as const) {
        test(`${name}: "Connect a wallet that can sign.", Close and review again; the header disconnected`, async ({
            app,
            page,
            state,
            fakeMwa,
        }) => {
            const fake = fakeOf(fakeMwa);
            const before = await getTokenAmount(androidAta(state));
            await depositReview(app, state);
            const shownAt = await firstMatchAt(page, `[data-testid="${T.txError}"][data-kind="wallet-changed"]`);
            fake.next(b);
            await app.inModal(T.txSign).tap();
            // R-S2 ends it at once: well before the 5 s way out of a sign that never settles …
            const error = app.inModal(T.txError);
            await expect(error).toHaveAttribute('data-kind', 'wallet-changed', {
                timeout: MWA_SIGN_CANCEL_AFTER_MS - 1_000,
            });
            // … and within 1 s of the wallet's answer to the reauthorization (its decline, or the
            // socket it closed), timed on the one clock the page and the fake wallet share.
            const reauth = need(fake.calls.at(-1), 'the reauthorization');
            expect(reauth).toMatchObject({
                method: 'authorize',
                outcome: b === 'reauthDecline' ? 'error' : 'closed',
            });
            expect(reauth.params.auth_token).toBe(FAKE_MWA_AUTH_TOKEN);
            const shown = need(await shownAt(), 'the wallet-changed error');
            expect(shown - need(reauth.answeredAt, 'the wallet’s answer')).toBeLessThan(1_000);
            await expect(error).toContainText('Connect a wallet that can sign.');
            const retry = app.inModal(T.txRetry);
            await expect(retry).toHaveAttribute('data-mode', 'close');
            await expect(retry).toHaveText('Close and review again');
            await expect(app.id(T.walletAddress)).toHaveCount(0);
            await expect(app.id(T.walletButton)).toHaveAccessibleName('Connect wallet');
            expect(fake.methods()).not.toContain('sign_transactions');
            await app.a11y(`android review: ${b}`);
            await expectNothingLanded(state, before);
        });
    }

    test('M15 the wallet never answers the sign: Cancel works after 5 s; a new Sign confirms', async ({
        app,
        state,
        fakeMwa,
    }) => {
        const fake = fakeOf(fakeMwa);
        const before = need(await getTokenAmount(androidAta(state)), 'android balance');
        await depositReview(app, state);
        fake.next('silent');
        await app.inModal(T.txSign).tap();
        await app.expectStage('signing', 10_000);
        const cancel = app.inModal(T.txCancel);
        await expect(cancel).toBeDisabled();
        await expect(cancel).toHaveAttribute('data-cancellable', 'false');
        await expect(cancel).toHaveAttribute('data-cancellable', 'true', { timeout: MWA_SIGN_CANCEL_AFTER_MS + 5_000 });
        await expect(cancel).toBeEnabled();
        await app.a11y('android review: waiting for the wallet, Cancel enabled');
        await cancel.tap();
        await expect(app.modal).toHaveCount(0);
        expect(fake.calls.at(-1)).toMatchObject({ method: 'sign_transactions', outcome: 'pending' });
        await expectNothingLanded(state, before);
        // The sheet stays for another try: review again and sign through a new association.
        const launches = (await app.mwaLaunches()).length;
        const reason = app.id(T.lendDepositReason);
        if ((await reason.textContent())?.includes('Tick the acknowledgement')) {
            await reason.getByRole('button').tap();
            await app.id(T.lendPanel).getByTestId(T.disclosureAck('lender.solRepayment')).tap();
        }
        await expect(app.id(T.lendSubmit)).toBeEnabled();
        await app.id(T.lendSubmit).tap();
        expect(await app.simulated()).toBe('review');
        await app.inModal(T.disclosureAck('lender.solRepayment')).tap();
        await tapSignAndConfirm(app);
        expect(await app.mwaLaunches()).toHaveLength(launches + 1);
        expect(await getTokenAmount(androidAta(state))).toBe(before - 10n ** BigInt(state.markets.main.decimals));
    });

    test('M18 another account at the reauthorization: the sign ends; the header shows the new account', async ({
        app,
        state,
        fakeMwa,
    }) => {
        const fake = fakeOf(fakeMwa);
        const before = await getTokenAmount(androidAta(state));
        await depositReview(app, state);
        fake.next('reauthSwitch');
        await app.inModal(T.txSign).tap();
        await expect(app.inModal(T.txError)).toHaveAttribute('data-kind', 'wallet-changed', { timeout: 10_000 });
        await expect(app.inModal(T.txRetry)).toHaveAttribute('data-mode', 'close');
        const shown = app.id(T.walletAddress);
        await expect(shown).toHaveAttribute('data-value', /^[1-9A-HJ-NP-Za-km-z]{32,44}$/);
        expect(await shown.getAttribute('data-value')).not.toBe(app.walletAddress);
        await app.a11y('android review: the account changed in the wallet');
        await expectNothingLanded(state, before);
    });
});

test.describe('Mobile Wallet Adapter: wallets that cannot sign through it (flag on)', { tag: '@lend' }, () => {
    test.skip(!ENV.mwa, 'the app is built with the Mobile Wallet Adapter off (SHORTWIN_E2E_MWA=0)');

    for (const [name, mwa] of [
        ['M10 a sending-only wallet (no solana:signTransactions)', { signFeature: false }],
        [
            'M10b per-account features in MWA ids',
            { accountFeatures: ['solana:signTransactions', 'solana:signAndSendTransaction'] },
        ],
    ] as const) {
        test.describe(name, () => {
            test.use({ wallet: null, mwa });

            test('connects, but gets no signer: the deposit form’s no-signer state; never the sending path', async ({
                app,
                page,
                state,
                fakeMwa,
            }) => {
                const fake = fakeOf(fakeMwa);
                await app.goto(`/m/${state.markets.main.mint}?panel=lend&action=deposit`, 'market');
                await connectMwa(app);
                await app.id(T.marketActionLend).tap();
                const panel = app.id(T.lendPanel);
                await expect(panel).toBeVisible();
                await expect(panel).toContainText('Connect a wallet to lend this token.');
                await expect(app.id(T.lendDepositReason)).toContainText('Connect a wallet first');
                await expect(app.id(T.lendSubmit)).toBeDisabled();
                await expect(page.locator('[data-sends-itself="true"]')).toHaveCount(0);
                await app.a11y(`android ${name}: no signer`);
                await app.id(T.tradeSheetClose).tap();
                await app.id(T.walletButton).tap();
                await app.id(T.walletDisconnect).tap();
                await expect(app.id(T.walletAddress)).toHaveCount(0);
                await app.id(T.walletButton).tap();
                await expect(app.id(MWA_OPTION)).toBeVisible();
                await expect(app.id(T.walletSendsItself)).toHaveCount(0);
                expect(fake.methods()).not.toContain('sign_transactions');
                expect(fake.methods()).not.toContain('sign_and_send_transactions');
            });
        });
    }
});

test.describe('Mobile Wallet Adapter (flag off)', { tag: '@scaffold' }, () => {
    test.skip(ENV.mwa, 'needs the flag-off build (SHORTWIN_E2E_MWA=0)');
    test.use({ wallet: null });

    test('no entry; today’s no-wallet dialog; the library’s chunk is never fetched', async ({ app, page }) => {
        const scripts: Promise<string>[] = [];
        page.on('response', (r) => {
            if (r.request().resourceType() === 'script') scripts.push(r.text().catch(() => ''));
        });
        await app.goto('/markets', 'markets');
        await app.id(T.walletButton).tap();
        const menu = app.id(T.walletMenu);
        await expect(menu).toBeVisible();
        await expect(menu).toHaveAttribute('role', 'dialog');
        await expect(menu.locator('[role="menu"]')).toHaveCount(0);
        await expect(app.id(MWA_OPTION)).toHaveCount(0);
        await expect(app.id(T.walletOpenElsewhere)).toHaveCount(0);
        const url = page.url();
        const origin = new URL(url).origin;
        for (const w of MOBILE_WALLETS) {
            await expect(app.id(T.walletOpenIn(w))).toHaveAttribute('href', browseLink(w, url, origin));
        }
        await app.a11y('android no-wallet dialog (flag off)');
        const bodies = await Promise.all(scripts);
        expect(bodies.length).toBeGreaterThan(0);
        expect(bodies.filter((b) => b.includes(LIBRARY_MARKER))).toEqual([]);
    });

    test.describe('the CSP without the flag', () => {
        test.use({ allowCsp: /localhost/ });

        test('M11 refuses the loopback socket and the prompt request', async ({ app, page }) => {
            await app.goto('/markets', 'markets');
            const seen = await cspProbes(page);
            expect(seen.filter((s) => s.startsWith('connect-src')).length).toBeGreaterThanOrEqual(2);
        });
    });
});
