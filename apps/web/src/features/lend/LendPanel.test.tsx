/**
 * The lend panel in the DOM (FRONTEND.md §3.6, §4.1, §10.1, §10.3): the SPEC §5.3 sentence verbatim
 * with its required checkbox gating the review; the CPI Guard notice, the plain deposit disabled
 * and the opt-in bracket; the memo-required destination and `memo_program`; the quoted `min_sol`;
 * the vest-forfeit acknowledgement; claims, the deficit cover and the epoch sync; blocked states.
 * Chain reads, the wallet and the transaction flow are stubbed; the previews run the real WASM.
 */
import {
    bigIntToPodU128,
    findShareEpochPda,
    getCoverDeficitInstructionDataDecoder,
    getWithdrawInstructionDataDecoder,
    identifyShortwinInstruction,
    type LenderPosition,
    type MarketVault,
    ShortwinInstruction,
} from '@shortwin/client';
import { AccountRole, address, createNoopSigner } from '@solana/kit';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { ToastProvider } from '../../components/Toast.tsx';
import { LENDER_SOL_REPAYMENT } from '../../config/disclosures.ts';
import { config } from '../../config/env.ts';
import { MEMO_PROGRAM, TOKEN_2022_PROGRAM } from '../../config/programs.ts';
import { T } from '../../config/testids.ts';
import { formatBps, formatSol } from '../../lib/format.ts';
import { formatUtc } from '../../lib/time.ts';
import { initMathSync, type MathModule } from '../../math/load.ts';
import { createAppClient } from '../../rpc/client.ts';
import { AppClientProvider } from '../../rpc/context.tsx';
import { readRepoBytes } from '../../test/repo.ts';
import { TxReviewModal } from '../../tx/TxReviewModal.tsx';
import type { TokenAccountInfo } from '../../tx/tokenAccount.ts';
import type { TxPlan } from '../../tx/types.ts';
import type { TxFlow } from '../../tx/useTxFlow.ts';
import type { LendChain } from './chain.ts';
import { LendPanelView } from './LendPanel.tsx';
import { MARKET_FLAG, previewClaimView, previewDepositView, unpaidRentExposure } from './previews.ts';
import { fakeMarket, simpleLender, simplePool } from './testkit.ts';

const h = vi.hoisted(() => ({
    wallet: { current: null as unknown },
    lend: { current: null as unknown },
    math: { current: undefined as unknown },
    flow: { current: null as unknown },
    simulate: { current: null as unknown },
}));

vi.mock('../../wallet/useWallet.ts', () => ({ useWallet: () => h.wallet.current }));
vi.mock('../../math/useMath.ts', () => ({ useMath: () => ({ math: h.math.current, error: null }) }));
vi.mock('./hooks.ts', () => ({
    useLendState: () => h.lend.current,
    useNowSeconds: () => 1_000,
    useAccountChanges: () => {},
}));
vi.mock('../../tx/useTxFlow.ts', async (orig) => ({
    ...(await orig<typeof import('../../tx/useTxFlow.ts')>()),
    useTxFlow: () => h.flow.current,
}));
vi.mock('../../tx/simulate.ts', () => ({
    simulatePlan: (...args: unknown[]) => (h.simulate.current as (...a: unknown[]) => unknown)(...args),
}));

const MINT = address('EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v');
const OWNER = address('9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin');
const MARKET = address('7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE');
const VAULT = address('Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB');
const LENDER = address('mSoLzYCxHdYgdzU16g5QSh3i5K3z3KZK7ytfqcJm7So');
const ATA = address('4k3Dyjzvzp8eMZWUXbBCjEvwSkkk59S5iCNLY3QrkX6R');
const GUARDED = address('JUPyiwrYJFskUPiHa7hkeR8VUtAeFoSYbKedZNsDvCN');
const MEMO_DEST = address('8sLbNZoA1cfnvMJLPfp98ZLAnFSYCFApfJKMbiXNLwxj');

let m: MathModule;
let started: TxPlan[];

beforeAll(() => {
    m = initMathSync(readRepoBytes('packages/math-wasm/pkg/shortwin_math_wasm_bg.wasm'));
});

function flowStub(): TxFlow {
    return {
        state: {
            stage: 'idle',
            plan: null,
            report: null,
            acks: new Set(),
            speed: 'normal',
            signature: null,
            error: null,
            refreshed: false,
        },
        start: (p: TxPlan) => {
            started.push(p);
        },
        refresh: () => {},
        setAck: () => {},
        setSpeed: () => {},
        sign: () => {},
        close: () => {},
    };
}

const account = (address_: string, amount: bigint, o: Partial<TokenAccountInfo> = {}): TokenAccountInfo => ({
    address: address(address_),
    program: 'token-2022',
    mint: MINT,
    owner: OWNER,
    amount,
    cpiGuard: false,
    memoRequired: false,
    ...o,
});

const lenderAccount = (l: ReturnType<typeof simpleLender>): LenderPosition => ({
    ...l,
    discriminator: new Uint8Array(8),
    owner: OWNER,
    market: MARKET,
    bump: 255,
    fwdShares: 0n,
    fwdEpoch: 0,
    fwd: 0,
    fwdLag: 0,
    reserved: new Uint8Array(10),
});

function chainOf(o: {
    pool?: Parameters<typeof simplePool>[0];
    lender?: LenderPosition | null;
    accounts?: TokenAccountInfo[];
    recordsComplete?: boolean;
    records?: LendChain['records'];
    market?: null;
    marketFields?: Parameters<typeof fakeMarket>[1];
}): LendChain {
    const pool = o.pool ?? { tokensFree: 1_000_000_000n, totalShares: 1_000_000_000_000n, lenderSol: 1_000_000n };
    return {
        mint: MINT,
        marketAddress: MARKET,
        vaultAddress: VAULT,
        market:
            o.market === null
                ? null
                : fakeMarket(pool, { tokenProgram: TOKEN_2022_PROGRAM, mint: MINT, ...o.marketFields }),
        vault: simplePool(pool).vault as unknown as MarketVault,
        config: { params: { lockGraceS: 86_400 } } as unknown as LendChain['config'],
        mintInfo: null,
        owner: OWNER,
        lenderAddress: LENDER,
        lender: o.lender ?? null,
        records: o.records ?? [],
        recordsComplete: o.recordsComplete ?? true,
        tokenAccounts: o.accounts ?? [account(ATA, 5_000_000n), account(GUARDED, 9_000_000n, { cpiGuard: true })],
        ata: ATA,
        walletLamports: 10n ** 10n,
        slot: 1n,
        // A fresh read (an old one would block the actions, FRONTEND.md §6.6).
        readAt: Date.now(),
    };
}

function setChain(c: LendChain) {
    h.lend.current = { isPending: false, isError: false, data: c, error: null, refetch: () => {} };
}

class NoSocket {
    readyState = 0;
    send() {}
    close() {}
    addEventListener() {}
    removeEventListener() {}
}

beforeEach(() => {
    started = [];
    vi.stubGlobal('fetch', () => new Promise(() => {}));
    vi.stubGlobal('WebSocket', NoSocket);
    h.math.current = m;
    h.flow.current = flowStub();
    h.wallet.current = {
        ready: true,
        status: 'connected',
        connected: { address: OWNER, signer: createNoopSigner(OWNER), walletName: 'Test', supportsV0: true },
    };
    h.simulate.current = async () => {
        throw new Error('no simulation in this test');
    };
    setChain(chainOf({}));
});

afterEach(() => {
    vi.unstubAllGlobals();
});

function renderPanel(action: 'deposit' | 'withdraw' | 'claim', onAction = vi.fn()) {
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const wrap = (children: ReactNode) => (
        <AppClientProvider client={createAppClient(config)}>
            <QueryClientProvider client={qc}>
                <ToastProvider>{children}</ToastProvider>
            </QueryClientProvider>
        </AppClientProvider>
    );
    const r = render(wrap(<LendPanelView mint={MINT} action={action} onAction={onAction} />));
    return { ...r, onAction, panel: () => screen.getByTestId(T.lendPanel) };
}

const submit = () => screen.getByTestId(T.lendSubmit) as HTMLButtonElement;

describe('Lend → Deposit', () => {
    it('shows the SPEC §5.3 sentence verbatim and keeps the review closed until it is ticked', async () => {
        const { panel } = renderPanel('deposit');
        expect(panel().dataset.action).toBe('deposit');
        expect(panel().dataset.mint).toBe(MINT);
        const disclosure = screen.getByTestId(T.disclosure('lender.solRepayment'));
        expect(disclosure.textContent).toContain(LENDER_SOL_REPAYMENT);
        const ack = within(disclosure).getByTestId(T.disclosureAck('lender.solRepayment'));
        expect(ack.closest('label')?.textContent).toBe(LENDER_SOL_REPAYMENT);
        expect(submit().disabled).toBe(true);
        fireEvent.change(screen.getByTestId(T.lendAmountInput), { target: { value: '1.5' } });
        expect(submit().disabled).toBe(true);
        const expected = previewDepositView(
            m,
            { market: chainOf({}).market as never, vault: chainOf({}).vault as never, lender: null, records: [] },
            1_000n,
            1_500_000n,
            50,
        );
        expect(expected.ok).toBe(true);
        if (!expected.ok) return;
        expect(screen.getByTestId(T.lendPreview('shares')).dataset.value).toBe(expected.value.shares.toString());
        expect(screen.getByTestId(T.lendPreview('share-bps')).dataset.value).toBe(String(expected.value.shareBps));
        expect(screen.getByTestId(T.lendPreview('value')).dataset.value).toBe(expected.value.value.toString());
        fireEvent.click(ack);
        expect(submit().disabled).toBe(false);
        fireEvent.click(submit());
        await waitFor(() => expect(started).toHaveLength(1));
        const plan = started[0] as TxPlan;
        expect(plan.kind).toBe('deposit');
        expect(plan.disclosures).toEqual(['lender.solRepayment']);
        expect(plan.instructions).toHaveLength(1);
    });

    it('fills Max from the source balance and refuses more than it', () => {
        renderPanel('deposit');
        fireEvent.click(screen.getByTestId(T.lendMax));
        expect((screen.getByTestId(T.lendAmountInput) as HTMLInputElement).value).toBe('5');
        fireEvent.change(screen.getByTestId(T.lendAmountInput), { target: { value: '6' } });
        fireEvent.click(screen.getByTestId(T.disclosureAck('lender.solRepayment')));
        expect(screen.getByText('Above the balance of this account.')).toBeTruthy();
        expect(submit().disabled).toBe(true);
    });

    it('blocks a CPI-Guarded source and offers the bracket (FE-1, FRONTEND.md §10.3)', async () => {
        renderPanel('deposit');
        expect(screen.queryByTestId(T.lendCpiGuardNotice)).toBeNull();
        fireEvent.change(screen.getByTestId(T.lendTokenAccount), { target: { value: GUARDED } });
        const notice = screen.getByTestId(T.lendCpiGuardNotice);
        expect(notice.dataset.state).toBe('blocked');
        expect(within(notice).getByTestId(T.disclosure('token.cpiGuard')).textContent).toContain(
            'CPI Guard blocks transfers that programs make on your behalf',
        );
        fireEvent.change(screen.getByTestId(T.lendAmountInput), { target: { value: '1' } });
        fireEvent.click(screen.getByTestId(T.disclosureAck('lender.solRepayment')));
        expect(submit().disabled).toBe(true);
        fireEvent.click(screen.getByTestId(T.lendCpiGuardBracket));
        expect(screen.getByTestId(T.lendCpiGuardNotice).dataset.state).toBe('bracket');
        expect(submit().disabled).toBe(false);
        fireEvent.click(submit());
        await waitFor(() => expect(started).toHaveLength(1));
        const plan = started[0] as TxPlan;
        expect(plan.instructions.map((i) => i.programAddress)).toEqual([
            TOKEN_2022_PROGRAM,
            expect.any(String),
            TOKEN_2022_PROGRAM,
        ]);
        expect(plan.disclosures).toContain('token.cpiGuard');
        // Back to the ATA: no notice, the plain deposit again.
        fireEvent.change(screen.getByTestId(T.lendTokenAccount), { target: { value: ATA } });
        expect(screen.queryByTestId(T.lendCpiGuardNotice)).toBeNull();
    });

    it('blocks deposits on a mint flagged unsafe', () => {
        setChain(chainOf({ pool: { tokensFree: 10n, totalShares: 10_000n, flags: MARKET_FLAG.mintUnsafe } }));
        renderPanel('deposit');
        expect(screen.getByTestId(T.lendBlocked).dataset.code).toBe('MintUnsafeFlagged');
        expect(screen.getByTestId(T.disclosure('market.mintUnsafe'))).toBeTruthy();
        expect(submit().disabled).toBe(true);
    });

    it('prepends the post-wipe sweep bundle to the first deposit after a full wipe', async () => {
        setChain(chainOf({ pool: { tokensFree: 0n, totalShares: 0n, flags: MARKET_FLAG.wipeLive, shareEpoch: 2 } }));
        renderPanel('deposit');
        expect(screen.getByTestId(T.lendSweepNotice)).toBeTruthy();
        fireEvent.change(screen.getByTestId(T.lendAmountInput), { target: { value: '1' } });
        // The preview reads the pool with the sweep done: 10⁶ raw tokens × 1,000 virtual shares.
        expect(screen.getByTestId(T.lendPreview('shares')).dataset.value).toBe('1000000000');
        fireEvent.click(screen.getByTestId(T.disclosureAck('lender.solRepayment')));
        fireEvent.click(submit());
        await waitFor(() => expect(started).toHaveLength(1));
        const plan = started[0] as TxPlan;
        expect(plan.instructions).toHaveLength(2);
        const sweep = plan.instructions[0];
        expect(identifyShortwinInstruction(sweep?.data as Uint8Array)).toBe(ShortwinInstruction.SweepTrader);
        // The live wipe record (epoch 1) closes the sweep's accounts, writable.
        const [record] = await findShareEpochPda({ market: MARKET, epoch: 1 });
        expect(sweep?.accounts?.[sweep.accounts.length - 1]).toEqual({ address: record, role: AccountRole.WRITABLE });
        expect(plan.title).toContain('post-wipe sweep');
    });

    it('shows no unpaid-rent rows on a pool whose rent is all paid in', () => {
        renderPanel('deposit');
        fireEvent.change(screen.getByTestId(T.lendAmountInput), { target: { value: '1.5' } });
        expect(screen.getByTestId(T.lendPreview('shares'))).toBeTruthy();
        expect(screen.queryByTestId(T.lendPreview('unpaid-rent'))).toBeNull();
        expect(screen.queryByTestId(T.disclosure('lender.unpaidRent'))).toBeNull();
    });

    it('shows the pool’s unpaid rent and the deposit’s part of it, collapsed while immaterial', async () => {
        const pool = { tokensFree: 1_000_000_000n, totalShares: 1_000_000_000_000n, receivable: 1_000_000_000n };
        setChain(chainOf({ pool }));
        renderPanel('deposit');
        expect(screen.getByTestId(T.lendPreview('unpaid-rent')).dataset.value).toBe('1000000000');
        fireEvent.change(screen.getByTestId(T.lendAmountInput), { target: { value: '1.5' } });
        const shares = BigInt(screen.getByTestId(T.lendPreview('shares')).dataset.value ?? '0');
        const expected = unpaidRentExposure(pool.receivable, pool.totalShares, shares, 1_500_000n, 0n);
        expect(expected.level).toBe('info');
        const cell = screen.getByTestId(T.lendPreview('unpaid-rent-share'));
        expect(cell.dataset.value).toBe(expected.exposure.toString());
        expect(cell.closest('[data-level]')?.getAttribute('data-level')).toBe('info');
        const disclosure = screen.getByTestId(T.disclosure('lender.unpaidRent'));
        expect(disclosure.tagName).toBe('DETAILS');
        expect(disclosure.textContent).toContain(
            'This pool has credited 1 SOL of rent that borrowers have not paid yet',
        );
        fireEvent.click(screen.getByTestId(T.disclosureAck('lender.solRepayment')));
        fireEvent.click(submit());
        await waitFor(() => expect(started).toHaveLength(1));
        expect((started[0] as TxPlan).disclosures).toEqual(['lender.solRepayment']);
    });

    it('warns when the deposit’s part is material and repeats the disclosure in the review', async () => {
        // 1,000 SOL unpaid: a 0.15 % deposit carries about 1.5 SOL of it (≥ 1 SOL; no price on this market).
        const pool = { tokensFree: 1_000_000_000n, totalShares: 1_000_000_000_000n, receivable: 10n ** 12n };
        setChain(chainOf({ pool }));
        renderPanel('deposit');
        fireEvent.change(screen.getByTestId(T.lendAmountInput), { target: { value: '1.5' } });
        const shares = BigInt(screen.getByTestId(T.lendPreview('shares')).dataset.value ?? '0');
        const expected = unpaidRentExposure(pool.receivable, pool.totalShares, shares, 1_500_000n, 0n);
        expect(expected.level).toBe('warn');
        expect(expected.exposure).toBeGreaterThanOrEqual(10n ** 9n);
        const cell = screen.getByTestId(T.lendPreview('unpaid-rent-share'));
        expect(cell.dataset.value).toBe(expected.exposure.toString());
        expect(cell.closest('[data-level]')?.getAttribute('data-level')).toBe('warn');
        const disclosure = screen.getByTestId(T.disclosure('lender.unpaidRent'));
        expect(disclosure.tagName).toBe('SECTION');
        expect(disclosure.dataset.severity).toBe('warn');
        expect(disclosure.textContent).toContain(
            `this deposit would carry ${formatSol(expected.exposure, 9)} SOL of it`,
        );
        expect(disclosure.textContent).toContain('(Cover deficit, on the Claim tab)');
        fireEvent.click(screen.getByTestId(T.disclosureAck('lender.solRepayment')));
        fireEvent.click(submit());
        await waitFor(() => expect(started).toHaveLength(1));
        const plan = started[0] as TxPlan;
        expect(plan.disclosures).toEqual(['lender.solRepayment', 'lender.unpaidRent']);
        expect(plan.disclosureVars?.['lender.unpaidRent']).toEqual({
            receivable: '1,000',
            exposure: formatSol(expected.exposure, 9),
            share: formatBps(expected.shareBps),
        });
    });

    it('counts the rent accrued since the last accrual: a pool nothing has touched still shows it', () => {
        // Stored receivable 0, one open position accruing since t = 0 (the panel's clock reads 1,000 s):
        // the deposit's prologue credits it first.
        const pool = { tokensFree: 1_000_000_000n, totalShares: 1_000_000_000_000n };
        const lastAccrualTs = 0n;
        const aggr = [1_000n, 5_000n, 100_000n].map((c) => 1_000_000_000n * c * 8_500n);
        setChain(
            chainOf({
                pool,
                marketFields: {
                    lastAccrualTs,
                    emaPrice: bigIntToPodU128(10n ** 18n),
                    rentAggr: aggr.map((a) => bigIntToPodU128(a)),
                },
            }),
        );
        renderPanel('deposit');
        // 1,000 s of 85 % of 10 % APR on 1 SOL of tokens: ⌊8.5·10⁷·1,000/31,536,000⌋.
        expect(screen.getByTestId(T.lendPreview('unpaid-rent')).dataset.value).toBe('2695');
        fireEvent.change(screen.getByTestId(T.lendAmountInput), { target: { value: '1.5' } });
        const shares = BigInt(screen.getByTestId(T.lendPreview('shares')).dataset.value ?? '0');
        const expected = unpaidRentExposure(2_695n, pool.totalShares, shares, 1_500_000n, 10n ** 18n);
        expect(expected.exposure).toBeGreaterThan(0n);
        expect(screen.getByTestId(T.lendPreview('unpaid-rent-share')).dataset.value).toBe(expected.exposure.toString());
        expect(screen.getByTestId(T.disclosure('lender.unpaidRent'))).toBeTruthy();
    });

    it('labels a top-up’s figure as this deposit’s part, beside the position’s whole share', () => {
        // The wallet holds half the pool already; the 1.5-token deposit adds about 0.15 %.
        const pool = { tokensFree: 1_000_000_000n, totalShares: 1_000_000_000_000n, receivable: 1_000_000_000n };
        setChain(chainOf({ pool, lender: lenderAccount(simpleLender(500_000_000_000n)) }));
        renderPanel('deposit');
        fireEvent.change(screen.getByTestId(T.lendAmountInput), { target: { value: '1.5' } });
        const shares = BigInt(screen.getByTestId(T.lendPreview('shares')).dataset.value ?? '0');
        const expected = unpaidRentExposure(pool.receivable, pool.totalShares, shares, 1_500_000n, 0n);
        expect(screen.getByText('This deposit’s part if it is never paid')).toBeTruthy();
        expect(screen.getByTestId(T.lendPreview('unpaid-rent-share')).dataset.value).toBe(expected.exposure.toString());
        // The position's share counts the old shares too; the disclosure's share is this deposit's.
        const whole = Number(screen.getByTestId(T.lendPreview('share-bps')).dataset.value);
        expect(whole).toBeGreaterThan(5_000);
        expect(expected.shareBps).toBeLessThan(20);
        expect(screen.getByTestId(T.disclosure('lender.unpaidRent')).textContent).toContain(
            `(${formatBps(expected.shareBps)} of the pool from this deposit)`,
        );
    });

    it('asks for a wallet, and explains a missing market', () => {
        h.wallet.current = { ready: true, status: 'disconnected', connected: null };
        renderPanel('deposit');
        expect(screen.getByText('Connect a wallet to lend this token.')).toBeTruthy();
        expect(submit().disabled).toBe(true);
    });

    it('offers to create a market that does not exist', () => {
        setChain(chainOf({ market: null }));
        renderPanel('deposit');
        expect(screen.getByText(/No short.win market for this token yet/)).toBeTruthy();
        expect(screen.getByTestId(T.lendPanel)).toBeTruthy();
    });
});

describe('Lend → Withdraw', () => {
    const holding = () =>
        setChain(
            chainOf({
                lender: lenderAccount(simpleLender(500_000_000_000n)),
                accounts: [account(ATA, 0n), account(MEMO_DEST, 0n, { memoRequired: true })],
            }),
        );

    it('passes memo_program for a memo-required destination and min_sol from the simulated quote', async () => {
        holding();
        h.simulate.current = async () => ({
            report: {
                ok: true,
                deltas: [
                    { key: `${VAULT}:lamports`, label: 'v', before: 9_000n, after: 1_000n, unit: 'SOL', scale: 9 },
                ],
            },
            lifetime: null,
        });
        renderPanel('withdraw');
        expect(screen.getByTestId(T.lendPosition('shares')).dataset.value).toBe('500000000000');
        expect(screen.queryByTestId(T.withdrawMemoNotice)).toBeNull();
        const select = screen.getByTestId(T.withdrawDestination) as HTMLSelectElement;
        expect(select.dataset.memo).toBe('false');
        fireEvent.change(select, { target: { value: MEMO_DEST } });
        expect((screen.getByTestId(T.withdrawDestination) as HTMLSelectElement).dataset.memo).toBe('true');
        expect(
            within(screen.getByTestId(T.withdrawMemoNotice)).getByTestId(T.disclosure('token.memoRequired')),
        ).toBeTruthy();
        fireEvent.click(screen.getByTestId(T.withdrawMax));
        const max = (screen.getByTestId(T.withdrawSharesInput) as HTMLInputElement).value;
        expect(Number(max)).toBeGreaterThan(0);
        expect(screen.getByTestId(T.withdrawPreview('allowance')).dataset.value).toBe('500000000');
        const tokens = screen.getByTestId(T.withdrawPreview('tokens')).dataset.value;
        expect(BigInt(tokens ?? '0') <= 500_000_000n).toBe(true);
        const button = screen.getByTestId(T.withdrawSubmit) as HTMLButtonElement;
        expect(button.disabled).toBe(false);
        fireEvent.click(button);
        await waitFor(() => expect(started).toHaveLength(1));
        const plan = started[0] as TxPlan;
        const ix = plan.instructions[plan.instructions.length - 1];
        expect(ix?.accounts?.[10]?.address).toBe(MEMO_PROGRAM);
        const args = getWithdrawInstructionDataDecoder().decode(ix?.data as Uint8Array);
        expect(args.minSol).toBe(8_000n);
        expect(args.minTokens).toBe((BigInt(tokens ?? '0') * 9_950n) / 10_000n);
        expect(plan.disclosures).toContain('token.memoRequired');
    });

    it('creates a missing ATA destination', async () => {
        setChain(chainOf({ lender: lenderAccount(simpleLender(500_000_000_000n)), accounts: [] }));
        renderPanel('withdraw');
        const select = screen.getByTestId(T.withdrawDestination) as HTMLSelectElement;
        expect(select.value).toBe(ATA);
        expect(select.textContent).toContain('created with the withdrawal');
        fireEvent.change(screen.getByTestId(T.withdrawSharesInput), { target: { value: '1000' } });
        fireEvent.click(screen.getByTestId(T.withdrawSubmit));
        await waitFor(() => expect(started).toHaveLength(1));
        expect((started[0] as TxPlan).instructions).toHaveLength(2);
    });

    it('requires the vest-forfeit acknowledgement when unreleased vest stays with the others', async () => {
        setChain(
            chainOf({
                pool: {
                    tokensFree: 1_000_000_000n,
                    totalShares: 1_000_000_000_000n,
                    vestBal: 2_000_000_000n,
                    vestLast: 0n,
                    vestEnd: 21_600n,
                    lenderSol: 2_000_000_000n,
                },
                lender: lenderAccount(simpleLender(500_000_000_000n)),
                accounts: [account(ATA, 0n)],
            }),
        );
        renderPanel('withdraw');
        fireEvent.click(screen.getByTestId(T.withdrawPercent('25')));
        const forfeit = screen.getByTestId(T.withdrawPreview('vesting-forfeit'));
        expect(BigInt(forfeit.dataset.value ?? '0') > 0n).toBe(true);
        const d = screen.getByTestId(T.disclosure('lender.exitForfeitsVest'));
        // 2 SOL vesting from 0 to 21,600 s, at 1,000 s: 1.907… SOL unreleased; this lender holds half.
        expect(d.textContent).toContain('You have 0.953703 SOL of income still vesting.');
        const button = screen.getByTestId(T.withdrawSubmit) as HTMLButtonElement;
        expect(button.disabled).toBe(true);
        fireEvent.click(within(d).getByTestId(T.disclosureAck('lender.exitForfeitsVest')));
        expect(button.disabled).toBe(false);
        fireEvent.click(button);
        await waitFor(() => expect(started).toHaveLength(1));
        expect((started[0] as TxPlan).disclosures).toEqual(['lender.exitForfeitsVest']);
    });

    it('explains a position parked on a wipe record', () => {
        setChain(
            chainOf({
                pool: { tokensFree: 0n, totalShares: 0n, flags: MARKET_FLAG.wipeLive, shareEpoch: 2 },
                lender: lenderAccount(simpleLender(500n, { epoch: 1 })),
            }),
        );
        renderPanel('withdraw');
        expect(screen.getByText(/A full settlement wiped this pool out/)).toBeTruthy();
        expect(screen.queryByTestId(T.withdrawSubmit)).toBeNull();
    });

    it('maps a refusal and points a negative balance to the deficit cover', () => {
        setChain(chainOf({ lender: lenderAccount(simpleLender(500_000_000_000n, { pending: -7n })) }));
        const { onAction } = renderPanel('withdraw');
        fireEvent.change(screen.getByTestId(T.withdrawSharesInput), { target: { value: '1' } });
        expect(screen.getByTestId(T.lendBlocked).dataset.code).toBe('NegativePending');
        fireEvent.click(screen.getByText('Go to cover deficit'));
        expect(onAction).toHaveBeenCalledWith('claim');
    });
});

describe('Lend → Claim', () => {
    it('shows the claim values from the chain and claims', async () => {
        const lender = lenderAccount(simpleLender(500_000_000_000n, { pending: 300_000n, real: 300_000n }));
        setChain(chainOf({ lender }));
        renderPanel('claim');
        const s = chainOf({ lender });
        const view = previewClaimView(
            m,
            { market: s.market as never, vault: s.vault as never, lender, records: [] },
            1_000n,
        );
        expect(view.ok && view.value.claimable).toBe(300_000n);
        expect(screen.getByTestId(T.claimValue('claimable')).dataset.value).toBe('300000');
        expect(screen.getByTestId(T.claimValue('deficit')).dataset.value).toBe('0');
        expect(screen.getByTestId(T.claimValue('pot')).dataset.value).toBe('0');
        expect(screen.queryByTestId(T.coverDeficitSubmit)).toBeNull();
        fireEvent.click(screen.getByTestId(T.claimSubmit));
        await waitFor(() => expect(started).toHaveLength(1));
        expect((started[0] as TxPlan).kind).toBe('claim_sol');
    });

    it('covers a deficit exactly', async () => {
        setChain(chainOf({ lender: lenderAccount(simpleLender(5n, { pending: -12_345n })) }));
        renderPanel('claim');
        expect(screen.getByTestId(T.claimValue('deficit')).dataset.value).toBe('12345');
        expect(screen.getByTestId(T.lendBlocked).dataset.code).toBe('NegativePending');
        fireEvent.click(screen.getByTestId(T.coverDeficitSubmit));
        await waitFor(() => expect(started).toHaveLength(1));
        const plan = started[0] as TxPlan;
        expect(plan.kind).toBe('cover_deficit');
        expect(getCoverDeficitInstructionDataDecoder().decode(plan.instructions[0]?.data as Uint8Array).lamports).toBe(
            12_345n,
        );
    });
});

describe('Lend → a sub-unit conversion while income vests (KI-65; the owner’s decision of 2026-10-07)', () => {
    // After a 1/1,000 rebase the wallet holds 600 old shares (less than one new share) and the
    // pool vests 3 SOL until 21,600 s; the panel's clock reads 1,000 s.
    const rebase = {
        epoch: 0,
        accEnd: 0n,
        accREnd: 0n,
        sOld: 7_600n,
        carry: 0n,
        carryR: 0n,
        num: 1,
        denLog1000: 1,
        convRem: 0n,
    } as unknown as LendChain['records'][number];
    const pool = {
        tokensFree: 10n,
        totalShares: 7n,
        shareEpoch: 1,
        vestBal: 3_000_000_000n,
        vestEnd: 21_600n,
        lenderSol: 4_000_000_000n,
    };
    const lender = lenderAccount(simpleLender(600n, { epoch: 0, pending: 300_000n, real: 300_000n }));

    it('puts lender.subUnitConversion with the vest’s end into the claim’s review', async () => {
        setChain(chainOf({ pool, lender, records: [rebase] }));
        renderPanel('claim');
        fireEvent.click(screen.getByTestId(T.claimSubmit));
        await waitFor(() => expect(started).toHaveLength(1));
        const plan = started[0] as TxPlan;
        expect(plan.kind).toBe('claim_sol');
        // Converted to 0 shares and paid in full, the position closes (KI-67): said before the
        // sub-unit warning, which stays last.
        expect(plan.disclosures).toEqual(['lender.claimCloses', 'lender.subUnitConversion']);
        const vestEnd = formatUtc(21_600n);
        expect(plan.disclosureVars?.['lender.subUnitConversion']).toEqual({ vestEnd });
        // The review renders it, the vest's end filled in.
        const review = render(
            <TxReviewModal flow={{ ...flowStub(), state: { ...flowStub().state, stage: 'review', plan } }} />,
        );
        const d = within(review.container).getByTestId(T.disclosure('lender.subUnitConversion'));
        expect(d.dataset.severity).toBe('warn');
        expect(d.textContent).toContain('this transaction can convert it to 0 shares');
        expect(d.textContent).toContain(`Income is vesting in this pool until ${vestEnd}.`);
        expect(d.textContent).toContain(`we advise waiting until ${vestEnd}`);
    });

    it('also on a deposit, and not once the conversion keeps a share', async () => {
        setChain(chainOf({ pool, lender, records: [rebase] }));
        renderPanel('deposit');
        fireEvent.change(screen.getByTestId(T.lendAmountInput), { target: { value: '1' } });
        fireEvent.click(screen.getByTestId(T.disclosureAck('lender.solRepayment')));
        fireEvent.click(submit());
        await waitFor(() => expect(started).toHaveLength(1));
        expect((started[0] as TxPlan).disclosures).toContain('lender.subUnitConversion');
        // 1,600 old shares keep one new share: no warning.
        started = [];
        const keeps = lenderAccount(simpleLender(1_600n, { epoch: 0, pending: 300_000n, real: 300_000n }));
        setChain(chainOf({ pool, lender: keeps, records: [rebase] }));
        renderPanel('claim');
        fireEvent.click(screen.getAllByTestId(T.claimSubmit).at(-1) as HTMLElement);
        await waitFor(() => expect(started).toHaveLength(1));
        expect((started[0] as TxPlan).disclosures).toEqual([]);
    });
});

describe('Lend panel', () => {
    it('syncs a position behind more epochs than a transaction carries', async () => {
        setChain(
            chainOf({
                pool: { tokensFree: 10n, totalShares: 10_000n, shareEpoch: 20 },
                lender: lenderAccount(simpleLender(5n, { epoch: 1 })),
                recordsComplete: false,
            }),
        );
        renderPanel('deposit');
        expect(screen.getByTestId(T.lendBlocked).dataset.code).toBe('LenderBehindEpochs');
        fireEvent.click(screen.getByTestId(T.lendSyncSubmit));
        await waitFor(() => expect(started).toHaveLength(1));
        expect((started[0] as TxPlan).kind).toBe('sync_lender');
        expect(submit().disabled).toBe(true);
    });

    it('switches tabs through the route', () => {
        const { onAction } = renderPanel('deposit');
        fireEvent.click(screen.getByTestId(T.lendTab('withdraw')));
        expect(onAction).toHaveBeenCalledWith('withdraw');
        expect(screen.getByTestId(T.lendTab('deposit')).getAttribute('aria-selected')).toBe('true');
    });
});
