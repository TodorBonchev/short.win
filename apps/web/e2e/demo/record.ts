/**
 * One-take local-validator growth demo. Run on a fresh full-profile E2E seed and the E2E
 * production preview (docs/DEVELOPMENT.md, "Run the app against a local validator").
 *
 *   SHORTWIN_E2E_PROFILE=full SHORTWIN_INDEXER_BIN=../../services/target/release/shortwin-indexer \
 *     node e2e/validator/start.ts --env
 *   node --input-type=module -e "import {loadState} from './e2e/validator/state.ts'; \
 *     import {buildApp,startPreview} from './e2e/validator/app.ts'; \
 *     buildApp(loadState().protocolAlt); await startPreview()"
 *   node e2e/demo/record.ts
 *
 * The recording uses real on-chain writes and the real local indexer. It changes the test
 * validator's state; stop and reseed it before E2E tests that expect the original seed.
 */
import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { chromium } from '@playwright/test';
import type { Address } from '@solana/kit';
import { chromiumPath, E2E_DIR, URLS } from '../env.ts';
import { App, expect, T, walletInitScript } from '../fixtures.ts';
import { fullWipe, shortPosition } from '../validator/actions.ts';
import { ata, clock, getAccount, marketKeys, waitForClock } from '../validator/chain.ts';
import { crowdBuy, curveCoin, curveState, freshCurveOpenPrice, marketVenue } from '../validator/curve.ts';
import { CURVE_LEND_RAW } from '../validator/genesis.ts';
import { loadState } from '../validator/state.ts';
import { liquidateSeededPosition } from './liquidate.ts';

const DIR = join(E2E_DIR, 'demo');
const RAW_DIR = join(DIR, 'playwright');
const RAW = join(DIR, 'raw.webm');
const MANIFEST = join(DIR, 'recording.json');
const Q = 1_500_000n * 1_000_000n;

interface Mark {
    readonly step: string;
    readonly seconds: number;
}

/** Text cards are browser overlays; product UI and all transactions remain untouched. */
async function card(page: import('@playwright/test').Page, title: string, detail: string, ms = 2_200): Promise<void> {
    await page.evaluate(
        ({ title, detail }) => {
            document.getElementById('shortwin-demo-overlay')?.remove();
            const overlay = document.createElement('div');
            overlay.id = 'shortwin-demo-overlay';
            Object.assign(overlay.style, {
                position: 'fixed',
                inset: '0',
                zIndex: '2147483647',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                flexDirection: 'column',
                gap: '18px',
                background: '#0b0e11',
                color: '#f3f5f7',
                fontFamily: 'Inter, system-ui, sans-serif',
                textAlign: 'center',
            });
            const heading = document.createElement('div');
            heading.textContent = title;
            Object.assign(heading.style, {
                color: '#f0b90b',
                fontSize: '46px',
                fontWeight: '750',
                letterSpacing: '.02em',
            });
            const sub = document.createElement('div');
            sub.textContent = detail;
            Object.assign(sub.style, { maxWidth: '950px', fontSize: '23px', lineHeight: '1.4' });
            overlay.append(heading, sub);
            document.body.append(overlay);
        },
        { title, detail },
    );
    await page.waitForTimeout(ms);
    await page.evaluate(() => document.getElementById('shortwin-demo-overlay')?.remove());
}

async function note(page: import('@playwright/test').Page, message: string | null): Promise<void> {
    await page.evaluate((text) => {
        document.getElementById('shortwin-demo-note')?.remove();
        if (!text) return;
        const n = document.createElement('div');
        n.id = 'shortwin-demo-note';
        n.textContent = text;
        Object.assign(n.style, {
            position: 'fixed',
            left: '24px',
            bottom: '48px',
            zIndex: '2147483647',
            padding: '11px 16px',
            maxWidth: '680px',
            border: '1px solid #f0b90b',
            background: '#0b0e11ee',
            color: '#f3f5f7',
            borderRadius: '8px',
            font: '600 17px Inter, system-ui, sans-serif',
        });
        document.body.append(n);
    }, message);
}

async function main(): Promise<void> {
    mkdirSync(RAW_DIR, { recursive: true });
    const state = loadState();
    if (state.program !== 'test-venue' || !state.venues || !state.positions) {
        throw new Error('the demo needs a fresh test-venue seed with venue fixtures and open positions');
    }
    const cargo = readFileSync(join(E2E_DIR, '..', '..', '..', 'programs', 'shortwin', 'Cargo.toml'), 'utf8');
    const version = /^version\s*=\s*"([^"]+)"/m.exec(cargo)?.[1];
    if (!version) throw new Error('cannot read program version');
    const coin = curveCoin();
    const keys = await marketKeys(coin.mint, coin.tokenProgram);
    if (await getAccount(keys.market)) throw new Error('the demo curve market already exists; reseed the validator');
    if (!(await fetch(`${URLS.indexer}/v1/status`)).ok) throw new Error('the real local indexer is not running');

    const browser = await chromium.launch({ executablePath: chromiumPath(), headless: true });
    const context = await browser.newContext({
        baseURL: URLS.app,
        viewport: { width: 1440, height: 900 },
        deviceScaleFactor: 1,
        colorScheme: 'dark',
        locale: 'en-US',
        timezoneId: 'UTC',
        recordVideo: { dir: RAW_DIR, size: { width: 1440, height: 900 } },
    });
    await context.addInitScript({ content: walletInitScript('curveLender') });
    const page = await context.newPage();
    const app = new App(page, state, 'curveLender');
    const started = Date.now();
    const marks: Mark[] = [];
    const signatures: Record<string, string> = {};
    const mark = (step: string) => {
        const seconds = (Date.now() - started) / 1000;
        marks.push({ step, seconds });
        console.log(`[demo] ${seconds.toFixed(1)}s ${step}`);
    };
    const pageErrors: string[] = [];
    page.on('pageerror', (error) => pageErrors.push(error.message));
    let failure: unknown;
    try {
        await app.goto('/markets', 'markets');
        await card(
            page,
            'short.win',
            'A market, a curve short, and two ways a position can end · local validator',
            3_000,
        );

        mark('create-market');
        await app.goto(`/markets/new?mint=${coin.mint}`, 'create-market');
        await app.connectWallet();
        await expect(app.id(T.createVerdict)).toHaveAttribute('data-verdict', 'ok');
        await expect(app.id(T.createVenue)).toHaveAttribute('data-value', 'pump_curve');
        await card(page, '01 · Create a market', 'The app detects the live Pump.fun curve and checks the mint.');
        await page.waitForTimeout(2_500);
        await app.id(T.createSubmit).click();
        if ((await app.simulated()) !== 'review') throw new Error('create-market simulation failed');
        await page.waitForTimeout(2_500);
        await app.inModal(T.txSign).click();
        await expect(page).toHaveURL(new RegExp(`/m/${coin.mint}`), { timeout: 90_000 });
        if (!(await getAccount(keys.market))) throw new Error('market create did not land');
        const m = { mint: coin.mint, market: keys.market, createdAt: (await clock()).unixTimestamp };
        await page.waitForTimeout(1_500);

        mark('deposit');
        await app.goto(`/m/${coin.mint}?panel=lend&action=deposit`, 'market');
        await app.connectWallet();
        await card(
            page,
            '02 · Deposit tokens',
            'A lender supplies inventory; the SOL repayment risk is shown before signing.',
        );
        const panel = app.id(T.lendPanel);
        const source = await ata(app.walletAddress, coin.mint, coin.tokenProgram);
        await app.selectTokenAccount(panel.getByTestId(T.lendTokenAccount), source);
        await panel.getByTestId(T.lendAmountInput).fill((CURVE_LEND_RAW / 1_000_000n).toString());
        await expect(panel.getByTestId(T.lendPreview('shares'))).toHaveAttribute('data-value', /^[1-9]\d*$/);
        await panel.getByTestId(T.disclosureAck('lender.solRepayment')).check();
        await page.waitForTimeout(2_200);
        await panel.getByTestId(T.lendSubmit).click();
        if ((await app.simulated()) !== 'review') throw new Error('deposit simulation failed');
        await page.waitForTimeout(2_200);
        await app.ackInReview('lender.solRepayment');
        signatures.deposit = await app.signAndConfirm();
        await page.waitForTimeout(1_800);
        await app.closeModal();

        // Pump's first validator trade tops up the dumped curve to the local rent minimum.
        await crowdBuy(1_000_000_000n, 1_000_000_000n);
        await note(page, 'The protocol enforces a 50-second market warm-up. Recording continues in one take.');
        await waitForClock(m.createdAt + 52n);
        await freshCurveOpenPrice(m);
        await note(page, null);

        mark('open-curve-short');
        await app.goto(`/m/${coin.mint}?panel=short`, 'market');
        await card(
            page,
            '03 · Open a curve short',
            'Borrow tokens, sell them on the Pump.fun curve, and lock the proceeds.',
        );
        const short = app.id(T.shortPanel);
        await expect(short.getByTestId(T.shortBlocked)).toHaveCount(0);
        await short.getByTestId(T.shortQInput).fill('1500000');
        await expect(short.getByTestId(T.shortPreview('proceeds'))).toHaveAttribute('data-value', /^[1-9]\d*$/);
        const shortAck = short.getByTestId(T.disclosureAck('borrower.liquidation'));
        if (await shortAck.count()) await shortAck.check();
        await page.waitForTimeout(2_400);
        await short.getByTestId(T.shortSubmit).click();
        if ((await app.simulated()) !== 'review') throw new Error('curve open simulation failed');
        const opened = await app.deltaOfNew('q', [app.walletAddress, keys.market]);
        const position = opened.address as Address;
        if (opened.change !== Q) throw new Error(`wrong open size: ${opened.change}`);
        await page.waitForTimeout(2_400);
        await app.ackInReview('borrower.liquidation');
        signatures.open = await app.signAndConfirm();
        if (!(await shortPosition(position))) throw new Error('curve short did not land');
        await page.waitForTimeout(1_500);
        await app.closeModal();

        // The existing pump v3 E2E scenario moves the curve across rT; the UI then closes the
        // position in one transaction through pump's synthetic migration path.
        await card(
            page,
            'A third-party trade moves the curve',
            'The close preview updates to the current buyback cost.',
        );
        const beforeBuy = await curveState();
        await crowdBuy(beforeBuy.rt - Q / 2n, 100n * 1_000_000_000n);
        mark('close-curve-short');
        await app.goto(`/positions/${position}`, 'position');
        await expect(app.id(T.closePreview('buyback'))).toHaveAttribute('data-value', /^[1-9]\d*$/);
        await card(page, '04 · Close the short', 'The borrower buys back the tokens and settles the position.');
        await page.waitForTimeout(2_400);
        await app.id(T.closeSubmit).click();
        if ((await app.simulated()) !== 'review') throw new Error('curve close simulation failed');
        await page.waitForTimeout(2_400);
        await app.inModal(T.txSign).click();
        await expect.poll(async () => shortPosition(position), { timeout: 90_000 }).toBeNull();
        if ((await marketVenue(keys.market)) !== 2) throw new Error('the curve did not move to PumpMigrating');
        await page.waitForTimeout(2_000);

        // The create and close pages navigate away from their review dialogs on confirmation;
        // capture those signatures from the real indexer's event log for the evidence manifest.
        const indexedSignature = async (kind: string): Promise<string> => {
            for (let attempt = 0; attempt < 30; attempt++) {
                const response = await fetch(`${URLS.indexer}/v1/markets/${coin.mint}/events`);
                if (response.ok) {
                    const body = (await response.json()) as { data?: { kind?: string; signature?: string }[] };
                    const signature = body.data?.find((event) => event.kind === kind)?.signature;
                    if (signature) return signature;
                }
                await page.waitForTimeout(1_000);
            }
            throw new Error(`indexer did not report ${kind}`);
        };
        signatures.create = await indexedSignature('MarketCreated');
        signatures.close = await indexedSignature('Close');

        mark('scripted-liquidation');
        const move = state.markets.move;
        const toLiquidate = state.positions.closer;
        await app.goto(`/m/${move.mint}?tab=positions`, 'market');
        await card(
            page,
            '05 · Scripted liquidation',
            'A local price rise lowers collateral; a keeper closes the at-risk short.',
        );
        await note(page, 'Scripted local price rise · the keeper checks each price bucket before liquidation.');
        const liquidation = await liquidateSeededPosition(move, toLiquidate);
        signatures.liquidation = liquidation.signature;
        await note(page, null);
        await app.goto(`/m/${move.mint}?tab=liquidations`, 'market');
        await expect(app.id(T.marketLiquidationRow(liquidation.signature))).toHaveAttribute(
            'data-kind',
            'liquidation',
            {
                timeout: 45_000,
            },
        );
        await page.waitForTimeout(4_000);

        mark('s1-settlement');
        const wipe = state.markets.wipe;
        await app.goto(`/m/${wipe.mint}?tab=positions`, 'market');
        await card(
            page,
            '06 · S1 SOL settlement',
            'A larger price gap makes token buyback impossible; lenders receive the remaining SOL.',
        );
        await note(page, 'Scripted price gap · the keeper runs crank_price + sol_settle until S1 lands.');
        const settlement = await fullWipe(wipe, 'wiped', state.tokenAccounts.wipe.lender, state.positions.wipe);
        if (settlement.settle.condition !== 1) throw new Error('settlement was not S1');
        signatures.s1 = settlement.signature;
        await note(page, null);
        await app.goto(`/m/${wipe.mint}?tab=liquidations`, 'market');
        await expect(app.id(T.marketLiquidationRow(settlement.signature))).toHaveAttribute('data-condition', 'S1', {
            timeout: 45_000,
        });
        await page.waitForTimeout(4_000);

        mark('outro');
        await card(
            page,
            'Six local-validator actions',
            'Curve close, then liquidation and S1 settlement on separate test markets.',
            4_000,
        );
        if (pageErrors.length) throw new Error(`browser errors: ${pageErrors.join(' | ')}`);
    } catch (error) {
        failure = error;
        console.error('[demo] recording failed:', error);
    } finally {
        await context.close();
        const path = await page.video()?.path();
        if (path) copyFileSync(path, RAW);
        await browser.close();
        writeFileSync(
            MANIFEST,
            `${JSON.stringify({ version, started: new Date(started).toISOString(), marks, signatures, raw: RAW, failure: failure ? String(failure) : null, pageErrors }, null, 2)}\n`,
        );
        console.log(`[demo] raw recording: ${RAW}`);
    }
    if (failure) throw failure;
}

await main();
