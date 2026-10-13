/**
 * Playwright fixtures of the E2E suite (FRONTEND.md §10.2): the seeded scenario (`state`), the
 * injected "ShortWin E2E" wallet signing as one seeded wallet per spec group (`test.use({ wallet })`),
 * the indexer (`mock` — routed and built from the chain —, `down` for the fallback specs, `real` in
 * profile `full`), and `app`, a small driver over the §11 `data-testid` contract: navigation,
 * wallet connection, the transaction review (stage, deltas, acknowledgements, Sign), keyboard focus
 * and axe. Specs select only by `T` (src/config/testids.ts) — never by text or CSS.
 *
 * O-6 (FRONTEND.md §5.4 (6), §10.2): `test.use({ mwa: {…} })` installs the Mobile Wallet Adapter
 * harness — the init-script hooks that stand in for Android's app switch (`wallet/mwa-hooks.js`) and
 * the protocol-level fake wallet app answering the association socket (`wallet/fake-mwa.ts`, signing
 * as the seeded `android` wallet; the `fakeMwa` fixture). `allowCsp` lets one probe expect CSP
 * violations (the flag-off build's); any other violation still fails the test.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import AxeBuilder from '@axe-core/playwright';
import { test as base, expect, type Locator, type Page } from '@playwright/test';
import type { Address, Base64EncodedWireTransaction } from '@solana/kit';
import type { DisclosureId } from '../src/config/disclosures.ts';
import { T } from '../src/config/testids.ts';
import type { TxStage } from '../src/tx/types.ts';
import { ENV, WEB_DIR } from './env.ts';
import { IndexerMock, type IndexerMode } from './indexer-mock.ts';
import { rpc } from './validator/chain.ts';
import { loadSecretKey, type WalletName } from './validator/keys.ts';
import { type E2EState, loadState } from './validator/state.ts';
import { FakeMwa, MWA_SOCKET_URL, type MwaLaunch } from './wallet/fake-mwa.ts';

export { expect, T };

const WALLET_SOURCE = readFileSync(join(WEB_DIR, 'e2e', 'wallet', 'injected-wallet.js'), 'utf8');
const MWA_HOOKS_SOURCE = readFileSync(join(WEB_DIR, 'e2e', 'wallet', 'mwa-hooks.js'), 'utf8');
/** The seeded wallet the fake Mobile Wallet Adapter wallet signs as. */
export const MWA_E2E_WALLET: WalletName = 'android';

/** The fake MWA wallet's variants (`fake-mwa.ts`); `{}` is a wallet that signs. */
export interface MwaOptions {
    /** `false`: no `solana:signTransactions` in its capabilities (a sending-only wallet). */
    readonly signFeature?: boolean;
    /** Per-account `features` on the accounts it authorizes. */
    readonly accountFeatures?: readonly string[];
}
/** The name the injected wallet registers under (`wallet-option-<name>`). */
export const E2E_WALLET_NAME = 'ShortWin E2E';

/** The init script that installs the test wallet with `wallet`'s generated key. */
export function walletInitScript(wallet: WalletName): string {
    const cfg = { secretKey: Array.from(loadSecretKey(wallet)), chains: ['solana:localnet'], label: wallet };
    return `${WALLET_SOURCE}\n;window.__shortwinInstallE2EWallet(${JSON.stringify(cfg)});`;
}

export interface E2EOptions {
    /** The seeded wallet the injected wallet signs as; `null` installs no wallet. */
    readonly wallet: WalletName | null;
    /** `mock` (profile `chain`), `down` (the indexer is unreachable: RPC fallback), `real` (profile `full`). */
    readonly indexer: IndexerMode | 'real';
    /** O-6: the Mobile Wallet Adapter harness (hooks and fake wallet app); `null` installs none. */
    readonly mwa: MwaOptions | null;
    /** CSP console errors this test expects (matching it); any other one still fails the test. */
    readonly allowCsp: RegExp | null;
}

export interface E2EFixtures {
    readonly state: E2EState;
    /** The routed indexer (`null` when `indexer` is `real`). */
    readonly indexerMock: IndexerMock | null;
    /** The fake MWA wallet app (`mwa` set), else `null`. */
    readonly fakeMwa: FakeMwa | null;
    readonly app: App;
}

/** The raw integer of an element's `data-value` (§11: raw integers, never parsed from text). */
export async function dataValue(l: Locator): Promise<bigint> {
    const v = await l.getAttribute('data-value');
    if (v === null || !/^-?\d+$/.test(v))
        throw new Error(`no integer data-value on ${l.toString()} (got ${String(v)})`);
    return BigInt(v);
}

const STAGES_DONE: readonly TxStage[] = ['review', 'failed'];

/** A driver over the §11 test ids. */
export class App {
    readonly page: Page;
    readonly state: E2EState;
    readonly walletName: WalletName | null;

    constructor(page: Page, state: E2EState, walletName: WalletName | null) {
        this.page = page;
        this.state = state;
        this.walletName = walletName;
    }

    id(testId: string): Locator {
        return this.page.getByTestId(testId);
    }

    get walletAddress(): Address {
        if (!this.walletName) throw new Error('this test installs no wallet');
        return this.state.wallets[this.walletName].address;
    }

    /** Navigates and waits for the shell (header and the routed page root). */
    async goto(path: string, page?: Parameters<typeof T.page>[0]): Promise<void> {
        await this.page.goto(path);
        await expect(this.id(T.header)).toBeVisible();
        if (page) await expect(this.id(T.page(page))).toBeVisible();
    }

    /**
     * Connects the injected wallet through the header's wallet menu (Wallet Standard discovery). On
     * a page the wallet already approved (a reload or `goto` after a connect), the app reconnects
     * silently instead: wait for that rather than opening the menu.
     */
    async connectWallet(): Promise<void> {
        const address = this.id(T.walletAddress);
        const approved = await this.page.evaluate(
            (a) => window.localStorage.getItem('shortwin-e2e-wallet:authorized') === a,
            this.walletAddress,
        );
        if (approved) {
            await expect(address).toHaveAttribute('data-value', this.walletAddress, { timeout: 15_000 });
            return;
        }
        if (await address.isVisible()) return;
        await this.id(T.walletButton).click();
        await expect(this.id(T.walletMenu)).toBeVisible();
        await this.id(T.walletOption(E2E_WALLET_NAME)).click();
        await expect(address).toHaveAttribute('data-value', this.walletAddress);
    }

    async disconnectWallet(): Promise<void> {
        await this.id(T.walletButton).click();
        await this.id(T.walletDisconnect).click();
        await expect(this.id(T.walletAddress)).toHaveCount(0);
    }

    // ---- the transaction review (FRONTEND.md §5.3, §11.1) ---------------------------------

    /**
     * The open review `<dialog>`. Every flow renders its own (closed ones stay in the DOM,
     * src/components/Modal.tsx), and at most one is open at a time.
     */
    get modal(): Locator {
        return this.page.locator(`dialog[data-testid="${T.txModal}"][open]`);
    }

    /** Inside the review dialog (a panel and the review can render the same disclosure id). */
    inModal(testId: string): Locator {
        return this.modal.getByTestId(testId);
    }

    async stage(): Promise<TxStage> {
        return (await this.inModal(T.txStage).getAttribute('data-stage')) as TxStage;
    }

    async expectStage(stage: TxStage, timeout = 60_000): Promise<void> {
        await expect(this.inModal(T.txStage)).toHaveAttribute('data-stage', stage, { timeout });
    }

    /** Waits for the simulation to finish (stage `review` or `failed`) and returns the stage. */
    async simulated(timeout = 60_000): Promise<TxStage> {
        await expect(this.modal).toBeVisible();
        await expect
            .poll(async () => STAGES_DONE.includes(await this.stage()), { timeout, message: 'simulation finished' })
            .toBe(true);
        return this.stage();
    }

    /** Ticks a disclosure's acknowledgement inside the review. */
    async ackInReview(id: DisclosureId): Promise<void> {
        await this.inModal(T.disclosureAck(id)).check();
    }

    /** The signed change of a watched account field (`<address>:<field>`), from `tx-delta-…`. */
    async delta(address: Address, field: string): Promise<bigint> {
        return dataValue(this.inModal(T.txDelta(`${address}:${field}`)));
    }

    /**
     * Every simulated delta of the review as `{ address, field, change }` — for accounts whose
     * address the spec cannot know in advance (a new position's PDA has a random nonce).
     */
    async deltas(): Promise<{ address: Address; field: string; change: bigint }[]> {
        const rows = await this.modal.locator('[data-testid^="tx-delta-"]').evaluateAll((els) =>
            els.map((e) => ({
                id: e.getAttribute('data-testid') ?? '',
                value: e.getAttribute('data-value') ?? '',
            })),
        );
        return rows.map((r) => {
            const key = r.id.slice('tx-delta-'.length);
            const i = key.lastIndexOf(':');
            return { address: key.slice(0, i) as Address, field: key.slice(i + 1), change: BigInt(r.value) };
        });
    }

    /** The single delta row with `field` whose account is not one of `exclude`. */
    async deltaOfNew(field: string, exclude: readonly Address[] = []): Promise<{ address: Address; change: bigint }> {
        const hits = (await this.deltas()).filter((d) => d.field === field && !exclude.includes(d.address));
        if (hits.length !== 1)
            throw new Error(`expected one ${field} delta, got ${JSON.stringify(hits.map((h) => h.address))}`);
        const [h] = hits;
        if (!h) throw new Error('unreachable');
        return { address: h.address, change: h.change };
    }

    /** Clicks Sign and waits for `confirmed`; returns the signature (`tx-signature` `data-value`). */
    async signAndConfirm(timeout = 90_000): Promise<string> {
        await expect(this.inModal(T.txSign)).toBeEnabled();
        await this.inModal(T.txSign).click();
        await this.expectStage('confirmed', timeout);
        const sig = await this.inModal(T.txSignature).getAttribute('data-value');
        if (!sig) throw new Error('no tx-signature after confirmation');
        return sig;
    }

    /** Closes the dialog (Cancel, or Done once confirmed) and waits until it is closed. */
    async closeModal(): Promise<void> {
        if ((await this.stage()) === 'confirmed') {
            await this.modal.getByRole('button', { name: 'Done' }).click();
        } else {
            await this.inModal(T.txCancel).click();
        }
        await expect(this.modal).toHaveCount(0);
    }

    // ---- the injected wallet's test controls ----------------------------------------------

    async walletRejectNext(n = 1): Promise<void> {
        await this.page.evaluate((k) => window.__shortwinE2E?.rejectNext(k), n);
    }

    async walletTamperNext(mode: 'data' | 'drop' = 'data'): Promise<void> {
        await this.page.evaluate((m) => window.__shortwinE2E?.tamperNext(m), mode);
    }

    async walletRaiseFeeNext(microLamportsPerCu: number): Promise<void> {
        await this.page.evaluate((p) => window.__shortwinE2E?.raiseFeeNext(p), microLamportsPerCu);
    }

    async walletSigned(): Promise<SignedRecord[]> {
        return this.page.evaluate(() => window.__shortwinE2E?.signed() ?? []);
    }

    // ---- the Mobile Wallet Adapter harness (`mwa`; wallet/mwa-hooks.js) ---------------------

    /** The associations the page launched (the intents), in order. */
    async mwaLaunches(): Promise<MwaLaunch[]> {
        return this.page.evaluate(() => window.__mwaE2E?.launches ?? []);
    }

    /** `'not-found'`: no wallet app comes to the front after the next intents (until set back). */
    async mwaLaunch(mode: 'ok' | 'not-found'): Promise<void> {
        await this.page.evaluate((m) => {
            if (window.__mwaE2E) window.__mwaE2E.launch = m;
        }, mode);
    }

    /** The browser comes back to the front without the wallet answering (`focus`). */
    async mwaReturnToPage(): Promise<void> {
        await this.page.evaluate(() => window.__mwaE2E?.returnToPage());
    }

    // ---- inputs ----------------------------------------------------------------------------

    /**
     * Selects the option of a token-account `<select>` that stands for `address`: by value when the
     * panel uses the address as the option value, else by an option label that shows the address
     * (full, or its first and last 4 characters).
     */
    async selectTokenAccount(select: Locator, address: Address): Promise<void> {
        let value: string | null = null;
        // The owner's accounts load after the wallet connects: wait for the option to appear.
        await expect
            .poll(
                async () => {
                    const options = await select
                        .locator('option')
                        .evaluateAll((os) =>
                            os.map((o) => ({ value: (o as HTMLOptionElement).value, label: o.textContent ?? '' })),
                        );
                    const hit =
                        options.find((o) => o.value === address) ??
                        options.find((o) => o.label.includes(address)) ??
                        options.find(
                            (o) => o.label.includes(address.slice(0, 4)) && o.label.includes(address.slice(-4)),
                        );
                    value = hit?.value ?? null;
                    return value !== null;
                },
                { timeout: 20_000, message: `an option for token account ${address}` },
            )
            .toBe(true);
        await select.selectOption(value ?? '');
    }

    // ---- keyboard (FRONTEND.md §8) ---------------------------------------------------------

    /**
     * Presses Tab (Shift+Tab with `back`) until the element with `testId` has focus — inside the
     * review dialog with `inModal` (the panel and the review can carry the same disclosure id).
     */
    async tabTo(testId: string, opts: { max?: number; back?: boolean; inModal?: boolean } = {}): Promise<void> {
        const max = opts.max ?? 150;
        for (let i = 0; i < max; i++) {
            const focused = await this.page.evaluate(() => {
                const a = document.activeElement;
                return {
                    id: a?.getAttribute('data-testid') ?? null,
                    inModal: !!a?.closest('[data-testid="tx-modal"]'),
                };
            });
            if (focused.id === testId && (!opts.inModal || focused.inModal)) return;
            await this.page.keyboard.press(opts.back ? 'Shift+Tab' : 'Tab');
        }
        throw new Error(`${testId} never received keyboard focus within ${max} Tab presses`);
    }

    // ---- accessibility (FRONTEND.md §8; "axe on every page") -------------------------------

    /** WCAG 2.2 A/AA axe scan of the current page (or `scope`); fails on any violation. */
    async a11y(label: string, scope?: string): Promise<void> {
        // A sheet that just opened still fades in (`--t-sheet`): axe would read its text half
        // transparent (colour contrast). Measure the settled page: wait out running animations.
        await this.page.evaluate(() =>
            Promise.all(
                document
                    .getAnimations()
                    .filter((a) => a.playState === 'running' && a.effect?.getTiming().iterations !== Infinity)
                    .map((a) => a.finished.catch(() => undefined)),
            ).then(() => undefined),
        );
        let b = new AxeBuilder({ page: this.page }).withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa']);
        if (scope) b = b.include(scope);
        const r = await b.analyze();
        const summary = r.violations.map(
            (v) => `${v.id} (${v.impact ?? 'n/a'}): ${v.help} — ${v.nodes.map((n) => n.target.join(' ')).join(', ')}`,
        );
        expect(summary, `axe violations on ${label}`).toEqual([]);
    }
}

export interface SignedRecord {
    readonly wire: string;
    readonly signature: string;
    readonly tampered: boolean;
    readonly raisedFeeTo: string | null;
}

declare global {
    interface Window {
        __shortwinE2E?: {
            readonly address: string;
            readonly name: string;
            connected(): boolean;
            rejectNext(n?: number): void;
            tamperNext(mode?: 'data' | 'drop'): void;
            raiseFeeNext(microLamportsPerCu: number | bigint): void;
            signed(): SignedRecord[];
            reset(): void;
            signWireForTest(base64: string): Promise<string>;
        };
    }
}

/** The fake MWA wallet of each page (created by the `page` fixture, which routes its socket). */
const fakeOf = new WeakMap<Page, FakeMwa>();

export const test = base.extend<E2EOptions & E2EFixtures>({
    wallet: ['trader', { option: true }],
    indexer: [ENV.profile === 'full' ? 'real' : 'mock', { option: true }],
    mwa: [null, { option: true }],
    allowCsp: [null, { option: true }],
    // biome-ignore lint/correctness/noEmptyPattern: Playwright's fixture signature
    state: async ({}, use) => {
        await use(loadState());
    },
    indexerMock: async ({ state, indexer }, use) => {
        const mock = indexer === 'real' ? null : new IndexerMock(state, indexer);
        await use(mock);
        mock?.dispose();
    },
    page: async ({ page, wallet, indexerMock, mwa, allowCsp }, use) => {
        if (wallet) await page.addInitScript({ content: walletInitScript(wallet) });
        if (indexerMock) await indexerMock.install(page);
        let fake: FakeMwa | null = null;
        if (mwa) {
            const f = new FakeMwa({
                secretKey: loadSecretKey(MWA_E2E_WALLET),
                launches: () => page.evaluate(() => window.__mwaE2E?.launches ?? []),
                sendTransaction: (wire) =>
                    rpc()
                        .sendTransaction(wire as Base64EncodedWireTransaction, {
                            encoding: 'base64',
                            preflightCommitment: 'confirmed',
                        })
                        .send(),
                signFeature: mwa.signFeature,
                accountFeatures: mwa.accountFeatures,
            });
            fake = f;
            fakeOf.set(page, f);
            await page.addInitScript({ content: MWA_HOOKS_SOURCE });
            await page.routeWebSocket(MWA_SOCKET_URL, (ws) => f.session(ws));
        }
        const errors: string[] = [];
        page.on('console', (m) => {
            if (m.type() !== 'error' || !/Content Security Policy|Refused to/.test(m.text())) return;
            if (allowCsp?.test(m.text())) return;
            errors.push(m.text());
        });
        await use(page);
        // The production CSP (§9.3) must never block the app (a violation is a defect, not noise).
        expect(errors, 'CSP violations').toEqual([]);
        // The fake wallet app understood every message the page's library sent it.
        if (fake) expect(fake.errors, 'fake MWA wallet errors').toEqual([]);
    },
    fakeMwa: async ({ page }, use) => {
        await use(fakeOf.get(page) ?? null);
    },
    app: async ({ page, state, wallet, mwa }, use) => {
        await use(new App(page, state, wallet ?? (mwa ? MWA_E2E_WALLET : null)));
    },
});

/** `v`, or a failure naming what the seed should have provided. */
export function need<V>(v: V | null | undefined, what: string): V {
    if (v === null || v === undefined) throw new Error(`the seed has no ${what}`);
    return v;
}

/** Skips the test on a run without the mock venue (SHORTWIN_E2E_PROGRAM=release): nothing is shorted. */
export function requireTrading(state: E2EState): void {
    test.skip(state.program !== 'test-venue' || !state.positions, 'needs the test-venue build (mock venue)');
}

/** Skips the test when the dumped venue fixtures are not loaded (SHORTWIN_E2E_VENUES=0). */
export function requireVenueFixtures(state: E2EState): void {
    test.skip(!state.venueMints, 'needs the dumped venue fixtures (SHORTWIN_E2E_VENUES=1)');
}
