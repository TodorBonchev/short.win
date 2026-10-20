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
import { chromium, devices } from '@playwright/test';
import type { Address } from '@solana/kit';
import { chromiumPath, E2E_DIR, URLS } from '../env.ts';
import { App, E2E_WALLET_NAME, expect, T, walletInitScript } from '../fixtures.ts';
import { fullWipe, shortPosition } from '../validator/actions.ts';
import { ata, clock, getAccount, marketKeys, waitForClock } from '../validator/chain.ts';
import { crowdBuy, curveCoin, curveState, freshCurveOpenPrice, marketVenue } from '../validator/curve.ts';
import { CURVE_LEND_RAW } from '../validator/genesis.ts';
import { loadState } from '../validator/state.ts';
import { DemoGuide, installPointer } from './guide.ts';
import { liquidateSeededPosition } from './liquidate.ts';

const PHONE = process.argv.includes('--phone');
const VIEWPORT = PHONE ? { width: 390, height: 780 } : { width: 1440, height: 900 };
const DIR = join(E2E_DIR, 'demo', ...(PHONE ? ['phone'] : []));
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
        ({ title, detail, phone }) => {
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
                padding: phone ? '24px' : '0',
            });
            const heading = document.createElement('div');
            heading.textContent = title;
            Object.assign(heading.style, {
                color: '#f0b90b',
                fontSize: phone ? '30px' : '46px',
                fontWeight: '750',
                letterSpacing: '.02em',
            });
            const sub = document.createElement('div');
            sub.textContent = detail;
            Object.assign(sub.style, { maxWidth: '950px', fontSize: phone ? '18px' : '23px', lineHeight: '1.4' });
            overlay.append(heading, sub);
            document.body.append(overlay);
        },
        { title, detail, phone: PHONE },
    );
    await page.waitForTimeout(ms);
    await page.evaluate(() => document.getElementById('shortwin-demo-overlay')?.remove());
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
    const priorEvents = await fetch(`${URLS.indexer}/v1/markets/${coin.mint}/events`);
    if (priorEvents.ok) {
        const body = (await priorEvents.json()) as { data?: { kind?: string }[] };
        if (body.data?.some((event) => event.kind === 'MarketCreated')) {
            throw new Error('indexer contains a prior take; stop the stack and archive .e2e/pg before reseeding');
        }
    }

    const browser = await chromium.launch({ executablePath: chromiumPath(), headless: true });
    const context = await browser.newContext({
        baseURL: URLS.app,
        viewport: VIEWPORT,
        deviceScaleFactor: PHONE ? 2 : 1,
        ...(PHONE ? { isMobile: true, hasTouch: true, userAgent: devices['Pixel 7'].userAgent } : {}),
        colorScheme: 'dark',
        locale: 'en-US',
        timezoneId: 'UTC',
        // Playwright records CSS pixels; a larger size pads instead of increasing detail.
        recordVideo: { dir: RAW_DIR, size: VIEWPORT },
    });
    await context.addInitScript({ content: walletInitScript('curveLender') });
    await installPointer(context, PHONE);
    const page = await context.newPage();
    const app = new App(page, state, 'curveLender');
    const started = Date.now();
    const marks: Mark[] = [];
    const cues: { text: string; seconds: number }[] = [];
    const waits: { kind: 'warmup' | 'liquidation' | 'settlement'; start: number; end: number; speed: number }[] = [];
    const signatures: Record<string, string> = {};
    const elapsed = () => (Date.now() - started) / 1000;
    const cue = (text: string) => cues.push({ text, seconds: elapsed() });
    const guide = new DemoGuide(page, cue, PHONE);
    const sheetReady = async (selector: string): Promise<void> => {
        await expect(page.locator(selector)).toHaveAttribute('data-sheet', 'open');
        await page.evaluate(() =>
            Promise.all(
                document
                    .getAnimations()
                    .filter(
                        (animation) =>
                            animation.playState === 'running' && animation.effect?.getTiming().iterations !== Infinity,
                    )
                    .map((animation) => animation.finished.catch(() => undefined)),
            ).then(() => undefined),
        );
    };
    const showExitHistory = async (mint: string): Promise<void> => {
        await app.goto(`/m/${mint}?tab=${PHONE ? 'positions' : 'liquidations'}`, 'market');
        if (PHONE) {
            await guide.click(app.id(T.marketTab('liquidations')), 'Tap Liquidations · inspect the confirmed exit');
            await expect(page).toHaveURL(/tab=liquidations/);
        }
    };
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
    const waitForIndexedExit = async (mint: string, signature: string): Promise<void> => {
        await expect
            .poll(
                async () => {
                    const response = await fetch(`${URLS.indexer}/v1/markets/${mint}/liquidations`);
                    if (!response.ok) return false;
                    const body = (await response.json()) as { data?: { signature?: string }[] };
                    return body.data?.some((event) => event.signature === signature) ?? false;
                },
                { timeout: 45_000, intervals: [500, 1_000] },
            )
            .toBe(true);
        // The API allows two seconds of HTTP caching. Let any earlier empty response expire
        // before navigating, so the result hold shows the real exit without a manual refresh.
        await page.waitForTimeout(2_100);
    };
    const sign = async (label: string): Promise<string> => {
        await guide.click(app.inModal(T.txSign), label);
        cue('Transaction submitted · waiting for local-validator confirmation');
        await app.expectStage('confirmed', 90_000);
        const signature = await app.inModal(T.txSignature).getAttribute('data-value');
        if (!signature) throw new Error('confirmed transaction has no signature');
        cue('Confirmed on the local validator');
        await page.waitForTimeout(1_600);
        await guide.click(app.modal.getByRole('button', { name: 'Done' }), 'Done · return to the market');
        await expect(app.modal).toHaveCount(0);
        return signature;
    };
    const mark = (step: string) => {
        const seconds = elapsed();
        marks.push({ step, seconds });
        console.log(`[demo] ${seconds.toFixed(1)}s ${step}`);
    };
    const pageErrors: string[] = [];
    page.on('pageerror', (error) => pageErrors.push(error.message));
    let failure: unknown;
    try {
        cue(
            PHONE
                ? 'Gold rings mark taps · real transactions on a local validator'
                : 'Follow the gold cursor · rings mark clicks · all transactions use a local validator',
        );
        await app.goto('/markets', 'markets');
        await card(
            page,
            'short.win',
            'A market, a curve short, and two ways a position can end · local validator',
            3_000,
        );

        mark('create-market');
        cue('Create a lending market for a token on the Pump.fun curve');
        await app.goto(`/markets/new?mint=${coin.mint}`, 'create-market');
        await card(page, '01 · Create a market', 'Detect the Pump.fun curve, review the transaction, then sign.');
        await guide.click(app.id(T.walletButton), 'Connect wallet · use the funded local test wallet');
        await guide.click(app.id(T.walletOption(E2E_WALLET_NAME)), 'Select the ShortWin E2E wallet');
        await expect(app.id(T.walletAddress)).toHaveAttribute('data-value', app.walletAddress);
        await expect(app.id(T.createVerdict)).toHaveAttribute('data-verdict', 'ok');
        await expect(app.id(T.createVenue)).toHaveAttribute('data-value', 'pump_curve');
        await guide.click(app.id(T.createSubmit), 'Create market · simulate the transaction first');
        if ((await app.simulated()) !== 'review') throw new Error('create-market simulation failed');
        cue('Simulation passed · review the accounts and network fee');
        await page.waitForTimeout(2_500);
        await guide.click(app.inModal(T.txSign), 'Sign in wallet · create the market on the local validator');
        await expect(page).toHaveURL(new RegExp(`/m/${coin.mint}`), { timeout: 90_000 });
        if (!(await getAccount(keys.market))) throw new Error('market create did not land');
        const m = { mint: coin.mint, market: keys.market, createdAt: (await clock()).unixTimestamp };
        cue('Market created · the lender can now supply token inventory');
        await page.waitForTimeout(1_500);

        mark('deposit');
        cue('Supply tokens to the market so borrowers can open shorts');
        await app.goto(`/m/${coin.mint}?panel=lend&action=deposit`, 'market');
        await app.connectWallet();
        await card(
            page,
            '02 · Deposit tokens',
            'A lender supplies inventory; the SOL repayment risk is shown before signing.',
        );
        if (PHONE) {
            await guide.click(app.id(T.marketActionLend), 'Tap Lend · open the deposit sheet');
            await sheetReady('.trade-panel');
        }
        const panel = app.id(T.lendPanel);
        const source = await ata(app.walletAddress, coin.mint, coin.tokenProgram);
        await guide.move(
            panel.getByTestId(T.lendTokenAccount),
            PHONE ? 'Use the lender’s funded token account' : 'Choose the lender’s funded token account',
        );
        await app.selectTokenAccount(panel.getByTestId(T.lendTokenAccount), source);
        await guide.fill(
            panel.getByTestId(T.lendAmountInput),
            (CURVE_LEND_RAW / 1_000_000n).toString(),
            'Enter 60,000,000 tokens to lend',
        );
        await expect(panel.getByTestId(T.lendPreview('shares'))).toHaveAttribute('data-value', /^[1-9]\d*$/);
        if (PHONE) {
            await guide.click(
                panel.getByTestId(T.lendDepositReason).getByRole('button'),
                'Tap the reminder · read the repayment disclosure',
            );
        }
        await guide.check(
            panel.getByTestId(T.disclosureAck('lender.solRepayment')),
            'Acknowledge that repayment may be in SOL',
        );
        await guide.click(panel.getByTestId(T.lendSubmit), 'Deposit · preview the token transfer and lender shares');
        if ((await app.simulated()) !== 'review') throw new Error('deposit simulation failed');
        cue('Review the deposit · tokens leave the wallet and lender shares are issued');
        await page.waitForTimeout(2_200);
        await guide.check(app.inModal(T.disclosureAck('lender.solRepayment')), 'Confirm the SOL repayment disclosure');
        signatures.deposit = await sign('Sign in wallet · confirm the token deposit');

        // Pump's first validator trade tops up the dumped curve to the local rent minimum.
        await crowdBuy(1_000_000_000n, 1_000_000_000n);
        cue('The market must be 50 seconds old before borrowing can begin');
        const warmupStart = elapsed();
        await waitForClock(m.createdAt + 52n);
        await freshCurveOpenPrice(m);
        waits.push({ kind: 'warmup', start: warmupStart, end: elapsed(), speed: 8 });

        mark('open-curve-short');
        cue('Borrow tokens and sell them on the curve · sale proceeds stay locked as collateral');
        await app.goto(`/m/${coin.mint}?panel=short`, 'market');
        await card(
            page,
            '03 · Open a curve short',
            'Borrow tokens, sell them on the Pump.fun curve, and lock the proceeds.',
        );
        if (PHONE) {
            await guide.click(app.id(T.marketActionShort), 'Tap Short · open the borrowing sheet');
            await sheetReady('.trade-panel');
        }
        const short = app.id(T.shortPanel);
        await expect(short.getByTestId(T.shortBlocked)).toHaveCount(0);
        await guide.fill(short.getByTestId(T.shortQInput), '1500000', 'Enter a short size of 1,500,000 tokens');
        await expect(short.getByTestId(T.shortPreview('proceeds'))).toHaveAttribute('data-value', /^[1-9]\d*$/);
        const shortAck = short.getByTestId(T.disclosureAck('borrower.liquidation'));
        if (PHONE && (await shortAck.count())) {
            await guide.click(
                short.getByTestId(T.shortSubmitReason).getByRole('button'),
                'Tap the reminder · read the liquidation risk',
            );
        }
        if (await shortAck.count()) await guide.check(shortAck, 'Acknowledge the liquidation and SOL settlement risk');
        await guide.click(short.getByTestId(T.shortSubmit), 'Open short · preview the borrow, sale and collateral');
        if ((await app.simulated()) !== 'review') throw new Error('curve open simulation failed');
        const opened = await app.deltaOfNew('q', [app.walletAddress, keys.market]);
        const position = opened.address as Address;
        if (opened.change !== Q) throw new Error(`wrong open size: ${opened.change}`);
        cue('Review the borrowed tokens, sale proceeds and locked collateral');
        await page.waitForTimeout(2_400);
        await guide.check(
            app.inModal(T.disclosureAck('borrower.liquidation')),
            'Confirm the risk disclosure before signing',
        );
        signatures.open = await sign('Sign in wallet · open the curve short');
        if (!(await shortPosition(position))) throw new Error('curve short did not land');

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
        cue('Close the borrower’s position at the current buyback cost');
        await app.goto(`/positions/${position}`, 'position');
        await card(page, '04 · Close the short', 'The borrower buys back the tokens and settles the position.');
        if (PHONE) {
            await guide.click(app.id(T.positionActionClose), 'Tap Close · open the position management sheet');
            await sheetReady('.manage-panel');
        }
        await expect(app.id(T.closePreview('buyback'))).toHaveAttribute('data-value', /^[1-9]\d*$/);
        await guide.click(app.id(T.closeSubmit), 'Close short · preview the buyback and settlement');
        if ((await app.simulated()) !== 'review') throw new Error('curve close simulation failed');
        cue('Review the final buyback cost and the amount returned to the borrower');
        await page.waitForTimeout(2_400);
        await guide.click(app.inModal(T.txSign), 'Sign in wallet · buy back tokens and close the short');
        await expect.poll(async () => shortPosition(position), { timeout: 90_000 }).toBeNull();
        if ((await marketVenue(keys.market)) !== 2) throw new Error('the curve did not move to PumpMigrating');
        cue('Closed on chain · loading the confirmed exit history');
        signatures.close = await indexedSignature('Close');
        await expect
            .poll(
                async () => {
                    const response = await fetch(`${URLS.indexer}/v1/positions/${position}`);
                    const body = (await response.json()) as { position?: { status?: string } };
                    return body.position?.status;
                },
                { timeout: 30_000 },
            )
            .toBe('closed');
        // Reload the real indexed result so the success hold does not retain the pre-close
        // query cache beside the already-closed chain account.
        await app.goto(`/positions/${position}`, 'position');
        await expect(app.id(T.positionDetail).getByText('closed', { exact: true }).first()).toBeVisible();
        cue('Short closed · the position account is settled on the local validator');
        await page.waitForTimeout(3_000);

        // The create and close pages navigate away from their review dialogs on confirmation;
        // capture those signatures from the real indexer's event log for the evidence manifest.
        signatures.create = await indexedSignature('MarketCreated');

        mark('scripted-liquidation');
        cue('Switch to a separate mock market to demonstrate a keeper liquidation');
        const move = state.markets.move;
        const toLiquidate = state.positions.closer;
        await app.goto(`/m/${move.mint}?tab=positions`, 'market');
        await card(
            page,
            '05 · Scripted liquidation',
            'A price rise lowers the collateral ratio; a keeper closes the at-risk short.',
        );
        cue('Scripted price rise · the keeper waits for the position to become liquidatable');
        const liquidationStart = elapsed();
        const liquidation = await liquidateSeededPosition(move, toLiquidate);
        await waitForIndexedExit(move.mint, liquidation.signature);
        waits.push({ kind: 'liquidation', start: liquidationStart, end: elapsed(), speed: 8 });
        signatures.liquidation = liquidation.signature;
        cue('Liquidation confirmed · open the market’s liquidation history');
        await showExitHistory(move.mint);
        await expect(app.id(T.marketLiquidationRow(liquidation.signature))).toHaveAttribute(
            'data-kind',
            'liquidation',
            {
                timeout: 45_000,
            },
        );
        await guide.move(
            PHONE
                ? app.id(T.marketLiquidationRow(liquidation.signature)).locator('[data-col="kind"]')
                : app.id(T.marketLiquidationRow(liquidation.signature)),
            'Liquidation recorded · the keeper closed the at-risk position',
        );
        await page.waitForTimeout(4_000);

        mark('s1-settlement');
        cue('Switch to another mock market to demonstrate settlement when token buyback fails');
        const wipe = state.markets.wipe;
        await app.goto(`/m/${wipe.mint}?tab=positions`, 'market');
        await card(
            page,
            '06 · S1 SOL settlement',
            'A larger price gap makes token buyback impossible; lenders receive the remaining SOL.',
        );
        cue('Scripted price gap · the keeper settles the position in SOL');
        const settlementStart = elapsed();
        const settlement = await fullWipe(wipe, 'wiped', state.tokenAccounts.wipe.lender, state.positions.wipe);
        await waitForIndexedExit(wipe.mint, settlement.signature);
        waits.push({ kind: 'settlement', start: settlementStart, end: elapsed(), speed: 8 });
        if (settlement.settle.condition !== 1) throw new Error('settlement was not S1');
        signatures.s1 = settlement.signature;
        cue('S1 settlement confirmed · inspect the exit record');
        await showExitHistory(wipe.mint);
        await expect(app.id(T.marketLiquidationRow(settlement.signature))).toHaveAttribute('data-condition', 'S1', {
            timeout: 45_000,
        });
        await guide.move(
            PHONE
                ? app.id(T.marketLiquidationRow(settlement.signature)).locator('[data-col="condition"]')
                : app.id(T.marketLiquidationRow(settlement.signature)),
            'S1 settlement recorded · lenders receive the remaining SOL',
        );
        if (PHONE) {
            await guide.move(
                app.id(T.marketLiquidationRow(settlement.signature)).locator('[data-col="lenders"]'),
                'Lenders’ SOL · remaining collateral is distributed in SOL',
            );
        }
        await page.waitForTimeout(4_000);

        mark('outro');
        cue('Six confirmed actions · create, deposit, open, close, liquidation and S1 settlement');
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
            `${JSON.stringify({ version, variant: PHONE ? 'phone' : 'desktop', viewport: VIEWPORT, started: new Date(started).toISOString(), pointer: true, marks, cues, waits, signatures, raw: RAW, failure: failure ? String(failure) : null, pageErrors }, null, 2)}\n`,
        );
        console.log(`[demo] raw recording: ${RAW}`);
    }
    if (failure) throw failure;
}

await main();
