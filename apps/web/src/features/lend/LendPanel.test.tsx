/**
 * The lend panel in the DOM (FRONTEND.md §3.6, §4.1, §10.1, §10.3): the SPEC §5.3 sentence verbatim
 * with its required checkbox gating the review; the CPI Guard notice, the plain deposit disabled
 * and the opt-in bracket; the memo-required destination and `memo_program`; the quoted `min_sol`;
 * the vest-forfeit acknowledgement; claims, the deficit cover and the epoch sync; blocked states.
 * Chain reads, the wallet and the transaction flow are stubbed; the previews run the real WASM.
 */
import {
    bigIntToPodI128,
    bigIntToPodU128,
    findShareEpochPda,
    getCoverDeficitInstructionDataDecoder,
    getWithdrawInstructionDataDecoder,
    identifyShortwinInstruction,
    type LenderPosition,
    type MarketVault,
    podI128ToBigInt,
    podU128ToBigInt,
    ShortwinInstruction,
} from '@shortwin/client';
import { AccountRole, address, createNoopSigner } from '@solana/kit';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { ToastProvider } from '../../components/Toast.tsx';
import { LENDER_SOL_REPAYMENT } from '../../config/disclosures.ts';
import { config } from '../../config/env.ts';
import { MEMO_PROGRAM, TOKEN_2022_PROGRAM } from '../../config/programs.ts';
import { T } from '../../config/testids.ts';
import { formatBps, formatSol, formatTokens } from '../../lib/format.ts';
import { formatUtc } from '../../lib/time.ts';
import { initMathSync, type MathModule } from '../../math/load.ts';
import { createAppClient } from '../../rpc/client.ts';
import { AppClientProvider } from '../../rpc/context.tsx';
import { readRepoBytes } from '../../test/repo.ts';
import { TxReviewModal } from '../../tx/TxReviewModal.tsx';
import type { TokenAccountInfo } from '../../tx/tokenAccount.ts';
import type { TxPlan } from '../../tx/types.ts';
import type { TxFlow } from '../../tx/useTxFlow.ts';
import { NO_SYNCS } from './build.ts';
import type { LendChain } from './chain.ts';
import { LendPanelView } from './LendPanel.tsx';
import { MARKET_FLAG, previewClaimView, previewDepositView, unpaidRentExposure } from './previews.ts';
import type { SyncPick } from './quote.ts';
import { fakeMarket, simpleLender, simplePool } from './testkit.ts';

const h = vi.hoisted(() => ({
    wallet: { current: null as unknown },
    lend: { current: null as unknown },
    math: { current: undefined as unknown },
    flow: { current: null as unknown },
    simulate: { current: null as unknown },
    /** The Claim button's search for syncs (`searchPotSyncs`); the review's own search is the real one. */
    search: { current: null as unknown },
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
vi.mock('./build.ts', async (orig) => ({
    ...(await orig<typeof import('./build.ts')>()),
    searchPotSyncs: (...args: unknown[]) => (h.search.current as (...a: unknown[]) => unknown)(...args),
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
    // By default the search finds no lender whose sync funds the pot.
    h.search.current = async () => NO_SYNCS;
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
    const view = () => wrap(<LendPanelView mint={MINT} action={action} onAction={onAction} />);
    const r = render(view());
    return {
        ...r,
        onAction,
        panel: () => screen.getByTestId(T.lendPanel),
        /** Renders again on the chain read `setChain` set (a refetched read). */
        reread: () => r.rerender(view()),
    };
}

const submit = () => screen.getByTestId(T.lendSubmit) as HTMLButtonElement;

/** A search result whose one sync (another lender) moves `gain` lamports to the pot. */
const pickOf = (gain: bigint): SyncPick =>
    gain === 0n
        ? NO_SYNCS
        : {
              syncs: [{ lender: GUARDED, owner: MEMO_DEST, epochs: [], gain }],
              gain,
              more: { count: 0, gain: 0n },
          };

describe('Lend → Deposit', () => {
    it('explains a settlement vest hold and enables review when the current vest has released', () => {
        const pool = { vestBal: 600n, vestLast: 0n, vestEnd: 1_200n, depositHoldUntil: 600n };
        setChain(chainOf({ pool }));
        const view = renderPanel('deposit');
        // The old marker has expired at now=1,000, but a sweep extended the shared vest to 1,200.
        expect(screen.getByTestId(T.lendDepositReason).textContent).toContain(formatUtc(1_200n));
        expect(screen.getByTestId(T.lendDepositReason).textContent).toContain('Further donations can extend the hold');
        expect(screen.getByTestId(T.lendDepositReason).textContent).toContain('no guaranteed reopening time');
        fireEvent.change(screen.getByTestId(T.lendAmountInput), { target: { value: '1' } });
        fireEvent.click(screen.getByTestId(T.disclosureAck('lender.solRepayment')));
        expect(submit().disabled).toBe(true);
        expect(started).toEqual([]);
        // A fresh read after full release still carries the marker until the next deposit clears it.
        setChain(chainOf({ pool: { ...pool, vestBal: 0n } }));
        view.reread();
        expect(submit().disabled).toBe(false);
        expect(screen.queryByText(/Deposits resume at/)).toBeNull();
    });

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

    it('follows the Short form’s order: account, amount, tolerance, preview, risks, acknowledgement, submit (O-1)', () => {
        // A wallet's first deposit (nothing remembered from the tests before).
        globalThis.localStorage.clear();
        renderPanel('deposit');
        fireEvent.change(screen.getByTestId(T.lendAmountInput), { target: { value: '1.5' } });
        const order = [
            screen.getByTestId(T.lendTokenAccount),
            screen.getByTestId(T.lendAmountInput),
            screen.getByTestId(T.lendSlippageInput),
            screen.getByTestId(T.lendPreview('shares')),
            screen.getByTestId(T.disclosure('lender.risks')),
            screen.getByTestId(T.disclosure('lender.residuals')),
            screen.getByTestId(T.disclosure('lender.solRepayment')),
            screen.getByTestId(T.disclosureAck('lender.solRepayment')),
            screen.getByTestId(T.lendDepositReason),
            submit(),
        ];
        for (let i = 1; i < order.length; i++) {
            const [a, b] = [order[i - 1] as HTMLElement, order[i] as HTMLElement];
            expect(
                a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING,
                `${a.dataset.testid} before ${b.dataset.testid}`,
            ).toBeTruthy();
        }
        // The texts are the approved ones, verbatim; only their place moved.
        expect(screen.getByTestId(T.disclosure('lender.solRepayment')).textContent).toBe(
            `You may be repaid in SOL${LENDER_SOL_REPAYMENT}`,
        );
        // The risks are expanded on a wallet's first deposit.
        expect(screen.getByTestId(T.disclosure('lender.risks')).tagName).toBe('SECTION');
        // The reason still points up at the box, and focuses it.
        expect(screen.getByTestId(T.lendDepositReason).textContent).toBe('Tick the acknowledgement above to continue');
    });

    it('shows a review that could not be built at the submit: after the acknowledgement, before the button (O-1)', async () => {
        globalThis.localStorage.clear();
        h.flow.current = {
            ...flowStub(),
            start: () => {
                throw new Error('The deposit could not be built.');
            },
        };
        renderPanel('deposit');
        fireEvent.change(screen.getByTestId(T.lendAmountInput), { target: { value: '1.5' } });
        fireEvent.click(screen.getByTestId(T.disclosureAck('lender.solRepayment')));
        fireEvent.click(submit());
        const alert = await screen.findByText('The deposit could not be built.');
        expect(alert.getAttribute('role')).toBe('alert');
        const ack = screen.getByTestId(T.disclosureAck('lender.solRepayment'));
        expect(ack.compareDocumentPosition(alert) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
        expect(alert.compareDocumentPosition(submit()) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
        // In the sticky foot, beside the submit.
        expect(alert.closest('.form-foot')).toBe(submit().closest('.form-foot'));
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
        expect(screen.getByTestId(T.lendSweepNotice).dataset.kind).toBe('wipe');
        expect(screen.queryByTestId(T.lendCleanupSubmit)).toBeNull();
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

    it('prepends the sweep to the first deposit after the last lender’s exit (AIR-08: no wipe record)', async () => {
        // S = 0 without a wipe record: `withdraw` cleared both sweep bits when the last lender left.
        setChain(chainOf({ pool: { tokensFree: 0n, totalShares: 0n, flags: 0, shareEpoch: 2 } }));
        renderPanel('deposit');
        expect(screen.getByTestId(T.lendSweepNotice).dataset.kind).toBe('emptied');
        expect(screen.queryByTestId(T.lendCleanupSubmit)).toBeNull();
        fireEvent.change(screen.getByTestId(T.lendAmountInput), { target: { value: '1' } });
        expect(screen.getByTestId(T.lendPreview('shares')).dataset.value).toBe('1000000000');
        fireEvent.click(screen.getByTestId(T.disclosureAck('lender.solRepayment')));
        fireEvent.click(submit());
        await waitFor(() => expect(started).toHaveLength(1));
        const plan = started[0] as TxPlan;
        expect(plan.instructions).toHaveLength(2);
        const [sweep, deposit] = plan.instructions;
        expect(identifyShortwinInstruction(sweep?.data as Uint8Array)).toBe(ShortwinInstruction.SweepTrader);
        // No live record: the sweep names none (the program would refuse it, BadRemainingAccounts).
        expect(sweep?.accounts).toHaveLength(10);
        expect(identifyShortwinInstruction(deposit?.data as Uint8Array)).toBe(ShortwinInstruction.Deposit);
        // The review says which sweep it is: no wipe here.
        expect(plan.title).toContain('· sweep after the last exit');
        expect(plan.title).not.toContain('post-wipe');
    });

    it('reviews surviving-lender cleanup separately, preserves inputs and waits for its vest before deposit', async () => {
        setChain(chainOf({ pool: { flags: 0 } }));
        const view = renderPanel('deposit');
        const notice = screen.getByTestId(T.lendSweepNotice);
        expect(notice.dataset.kind).toBe('lenders');
        expect(notice.textContent).toContain('existing lenders');
        expect(notice.textContent).toContain('network fee');
        expect(notice.textContent).toContain('account rent');
        fireEvent.change(screen.getByTestId(T.lendTokenAccount), { target: { value: GUARDED } });
        fireEvent.change(screen.getByTestId(T.lendAmountInput), { target: { value: '1' } });
        fireEvent.click(screen.getByTestId(T.lendCpiGuardBracket));
        fireEvent.click(screen.getByTestId(T.disclosureAck('lender.solRepayment')));
        expect(submit().disabled).toBe(true);
        expect(screen.getByTestId(T.lendDepositReason).textContent).toContain('Clean up pending lender income first');
        fireEvent.click(screen.getByTestId(T.lendCleanupSubmit));
        await waitFor(() => expect(started).toHaveLength(1));
        const plan = started[0] as TxPlan;
        expect(plan.kind).toBe('lender_cleanup');
        expect(plan.instructions).toHaveLength(1);
        expect(identifyShortwinInstruction(plan.instructions[0]?.data as Uint8Array)).toBe(
            ShortwinInstruction.SweepTrader,
        );
        expect(plan.title).toBe('Clean up pending lender income');
        expect(plan.disclosures).toEqual(['lender.cleanup']);
        expect(plan.instructions.flatMap((i) => i.accounts?.map((a) => a.address))).not.toContain(GUARDED);

        // Cleanup lands and a delayed claim has started a held vest. No deposit went with it.
        const flow = flowStub();
        h.flow.current = { ...flow, state: { ...flow.state, stage: 'confirmed', plan } };
        const vest = { vestBal: 600n, vestLast: 1_000n, vestEnd: 1_200n, depositHoldUntil: 1_000n };
        setChain(chainOf({ pool: vest }));
        view.reread();
        expect(screen.queryByTestId(T.lendCleanupSubmit)).toBeNull();
        expect((screen.getByTestId(T.lendAmountInput) as HTMLInputElement).value).toBe('1');
        expect((screen.getByTestId(T.lendTokenAccount) as HTMLSelectElement).value).toBe(GUARDED);
        expect((screen.getByTestId(T.lendCpiGuardBracket) as HTMLInputElement).checked).toBe(true);
        expect((screen.getByTestId(T.disclosureAck('lender.solRepayment')) as HTMLInputElement).checked).toBe(true);
        expect(submit().disabled).toBe(true);
        expect(screen.getByTestId(T.lendDepositReason).textContent).toContain('Settlement income is vesting');

        // Actual release, even with the positive marker still stored, enables a separate deposit.
        h.flow.current = flowStub();
        setChain(chainOf({ pool: { ...vest, vestBal: 0n } }));
        view.reread();
        expect(submit().disabled).toBe(false);
        fireEvent.click(submit());
        await waitFor(() => expect(started).toHaveLength(2));
        const deposit = started[1] as TxPlan;
        expect(deposit.kind).toBe('deposit');
        expect(deposit.instructions).toHaveLength(3); // CPI Guard off → Deposit → on, no cleanup.
        expect(identifyShortwinInstruction(deposit.instructions[1]?.data as Uint8Array)).toBe(
            ShortwinInstruction.Deposit,
        );
    });

    it('offers legacy cleanup without tokens, an amount, a deposit acknowledgement or complete lender records', async () => {
        setChain(chainOf({ pool: { depositHoldUntil: 0n }, accounts: [], recordsComplete: false }));
        renderPanel('deposit');
        expect(submit().disabled).toBe(true);
        const cleanup = screen.getByTestId(T.lendCleanupSubmit) as HTMLButtonElement;
        expect(cleanup.disabled).toBe(false);
        fireEvent.click(cleanup);
        await waitFor(() => expect(started).toHaveLength(1));
        expect(started[0]?.kind).toBe('lender_cleanup');
        expect(started[0]?.instructions).toHaveLength(1);
    });

    it('requires a connected wallet and a fresh read for separate cleanup', () => {
        setChain({ ...chainOf({ pool: { flags: 0 } }), readAt: 0 });
        const view = renderPanel('deposit');
        expect((screen.getByTestId(T.lendCleanupSubmit) as HTMLButtonElement).disabled).toBe(true);
        expect(screen.getByTestId(T.lendSweepNotice).textContent).toContain('Waiting for a fresh chain read');
        setChain(chainOf({ pool: { flags: 0 } }));
        h.wallet.current = { ready: true, status: 'disconnected', connected: null };
        view.reread();
        expect((screen.getByTestId(T.lendCleanupSubmit) as HTMLButtonElement).disabled).toBe(true);
        expect(screen.getByTestId(T.lendSweepNotice).textContent).toContain('Connect a wallet first');
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
        // The label carries a glossary ⓘ after its last word (its text is split around it).
        expect(
            screen.getByText(
                (_, el) => el?.tagName === 'DT' && el.textContent === 'This deposit’s part if it is never paid',
            ),
        ).toBeTruthy();
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
        expect(screen.getByText(/earlier claims can leave less for later claimants/)).toBeTruthy();
        expect(
            screen.getByText(/Some credited income may never be paid if the shortfall is never covered/),
        ).toBeTruthy();
        expect(screen.queryByTestId(T.coverDeficitSubmit)).toBeNull();
        fireEvent.click(screen.getByTestId(T.claimSubmit));
        await waitFor(() => expect(started).toHaveLength(1));
        expect((started[0] as TxPlan).kind).toBe('claim_sol');
    });

    it('says what the claim does: claimable now, credited only, both, neither (O-5)', async () => {
        const label = () => screen.getByTestId(T.claimSubmit) as HTMLButtonElement;
        const at = (pending: bigint, real: bigint) => {
            setChain(chainOf({ lender: lenderAccount(simpleLender(500_000_000_000n, { pending, real })) }));
            return renderPanel('claim');
        };
        // The syncs found move 1 SOL to the pot: more than any shortfall below.
        h.search.current = async () => pickOf(10n ** 9n);
        // Claimable now, nothing out of reach: one line, as before; no search.
        let r = at(300_000n, 300_000n);
        expect(label().textContent).toBe('Review claim · 0.0003 SOL');
        expect(screen.queryByTestId(T.claimSubmitSyncs)).toBeNull();
        expect(label().dataset.lines).toBeUndefined();
        r.unmount();
        // Credited only (nothing paid in for it yet): 0 now, the syncs add up to the credit.
        r = at(25_000n, 0n);
        await waitFor(() =>
            expect(label().textContent).toBe('Review claim · 0 SOL now · syncs may add up to 0.000025 SOL'),
        );
        expect(screen.getByTestId(T.claimSubmitSyncs).textContent).toBe('syncs may add up to 0.000025 SOL');
        expect(label().dataset.lines).toBe('2');
        expect(label().disabled).toBe(false);
        expect(screen.getByText(/of what is credited to you cannot be claimed now/).textContent).toContain(
            '0.000025 SOL of what is credited to you cannot be claimed now.',
        );
        r.unmount();
        // Both: part now, the rest through syncs.
        r = at(300_000n, 100_000n);
        await waitFor(() =>
            expect(label().textContent).toBe('Review claim · 0.0001 SOL now · syncs may add up to 0.0002 SOL'),
        );
        r.unmount();
        // Neither: nothing to claim, the button disabled with its reason.
        r = at(0n, 0n);
        expect(label().textContent).toBe('Review claim · 0 SOL');
        expect(label().disabled).toBe(true);
        expect(screen.getByTestId(T.lendClaimReason).textContent).toBe('Nothing to claim yet');
        r.unmount();
    });

    it('counts a shortfall under 10,000 lamports as none (the owner’s decision of 2026-10-11)', async () => {
        const label = () => screen.getByTestId(T.claimSubmit) as HTMLButtonElement;
        const note = () => screen.queryByText(/of what is credited to you cannot be claimed now/);
        const searched: bigint[] = [];
        h.search.current = async () => {
            searched.push(1n);
            return pickOf(10n ** 9n);
        };
        const at = (pending: bigint, real: bigint) => {
            setChain(chainOf({ lender: lenderAccount(simpleLender(500_000_000_000n, { pending, real })) }));
            return renderPanel('claim');
        };
        // Claimable 0, a 9,999-lamport shortfall (rounding dust): nothing to claim yet, no search.
        let r = at(9_999n, 0n);
        expect(label().textContent).toBe('Review claim · 0 SOL');
        expect(label().disabled).toBe(true);
        expect(screen.getByTestId(T.lendClaimReason).textContent).toBe('Nothing to claim yet');
        expect(screen.queryByTestId(T.claimSubmitSyncs)).toBeNull();
        expect(note()).toBeNull();
        expect(searched).toEqual([]);
        r.unmount();
        // Claimable 0, 10,000 lamports short, syncs that cover it: the syncs may add it.
        r = at(10_000n, 0n);
        await waitFor(() =>
            expect(label().textContent).toBe('Review claim · 0 SOL now · syncs may add up to 0.00001 SOL'),
        );
        expect(label().disabled).toBe(false);
        expect(note()).not.toBeNull();
        r.unmount();
        // Claimable > 0 with a dust shortfall: the full label, no note, no second line.
        r = at(309_999n, 300_000n);
        expect(label().textContent).toBe('Review claim · 0.0003 SOL');
        expect(label().dataset.lines).toBeUndefined();
        expect(label().disabled).toBe(false);
        expect(note()).toBeNull();
        r.unmount();
        // Claimable > 0 and 10,000 lamports short: two lines.
        at(310_000n, 300_000n);
        await waitFor(() =>
            expect(label().textContent).toBe('Review claim · 0.0003 SOL now · syncs may add up to 0.00001 SOL'),
        );
        expect(label().dataset.lines).toBe('2');
    });

    it('only when syncs can pay: {g} is min(shortfall, their gain), none under 10,000 lamports (owner, 2026-10-11)', async () => {
        const label = () => screen.getByTestId(T.claimSubmit) as HTMLButtonElement;
        const reason = () => screen.getByTestId(T.lendClaimReason).textContent;
        const note = () => screen.queryByText(/of what is credited to you cannot be claimed now/)?.textContent;
        // Nothing claimable now, 25,000 lamports credited and not paid in (the only lender: no sync).
        const only = () => {
            setChain(chainOf({ lender: lenderAccount(simpleLender(500_000_000_000n, { pending: 25_000n })) }));
            return renderPanel('claim');
        };
        for (const [gain, want] of [
            // No lender whose sync funds the pot, or less than the dust threshold: disabled.
            [0n, null],
            [9_999n, null],
            // From 10,000 lamports: {g} is the gain while it is below the shortfall …
            [10_000n, '0.00001'],
            [12_000n, '0.000012'],
            // … and the shortfall when the syncs could move more (a claim pays at most what is credited).
            [50_000n, '0.000025'],
        ] as const) {
            h.search.current = async () => pickOf(gain);
            const r = only();
            if (want === null) {
                await waitFor(() => expect(reason()).toBe('Nothing to claim yet'));
                expect(label().textContent).toBe('Review claim · 0 SOL');
                expect(label().disabled).toBe(true);
                expect(label().getAttribute('aria-busy')).toBeNull();
                expect(label().dataset.lines).toBeUndefined();
            } else {
                await waitFor(() =>
                    expect(label().textContent).toBe(`Review claim · 0 SOL now · syncs may add up to ${want} SOL`),
                );
                expect(label().disabled).toBe(false);
                expect(label().dataset.lines).toBe('2');
                expect(reason()).toBe('');
            }
            // The note keeps the whole shortfall: what is out of reach, and why.
            expect(note()).toContain('0.000025 SOL of what is credited to you cannot be claimed now.');
            r.unmount();
        }
    });

    it('while the search runs: never more than is known, and no reason that then flips', async () => {
        let release: (p: SyncPick) => void = () => {};
        h.search.current = () =>
            new Promise<SyncPick>((r) => {
                release = r;
            });
        const label = () => screen.getByTestId(T.claimSubmit) as HTMLButtonElement;
        // Nothing claimable now: disabled and busy, the full label, no reason yet.
        setChain(chainOf({ lender: lenderAccount(simpleLender(500_000_000_000n, { pending: 25_000n })) }));
        let r = renderPanel('claim');
        expect(label().textContent).toBe('Review claim · 0 SOL');
        expect(label().disabled).toBe(true);
        expect(label().getAttribute('aria-busy')).toBe('true');
        expect(screen.getByTestId(T.lendClaimReason).textContent).toBe('');
        expect(screen.queryByTestId(T.claimSubmitSyncs)).toBeNull();
        await act(async () => release(pickOf(30_000n)));
        await waitFor(() => expect(label().disabled).toBe(false));
        expect(label().getAttribute('aria-busy')).toBeNull();
        expect(label().textContent).toBe('Review claim · 0 SOL now · syncs may add up to 0.000025 SOL');
        r.unmount();
        // Something claimable now: enabled with the full label; the second line once the gain is known.
        setChain(
            chainOf({ lender: lenderAccount(simpleLender(500_000_000_000n, { pending: 125_000n, real: 100_000n })) }),
        );
        r = renderPanel('claim');
        expect(label().textContent).toBe('Review claim · 0.0001 SOL');
        expect(label().disabled).toBe(false);
        expect(label().getAttribute('aria-busy')).toBeNull();
        await act(async () => release(pickOf(30_000n)));
        await waitFor(() =>
            expect(label().textContent).toBe('Review claim · 0.0001 SOL now · syncs may add up to 0.000025 SOL'),
        );
        r.unmount();
    });

    it('a price crank (the realised-rent index alone) neither searches again nor flips the button', async () => {
        let searches = 0;
        let release: (p: SyncPick) => void = () => {};
        h.search.current = () => {
            searches++;
            if (searches === 1) return Promise.resolve(pickOf(30_000n));
            return new Promise<SyncPick>((r) => {
                release = r;
            });
        };
        const label = () => screen.getByTestId(T.claimSubmit) as HTMLButtonElement;
        const lender = lenderAccount(simpleLender(500_000_000_000n, { pending: 25_000n }));
        const base = chainOf({ lender });
        setChain(base);
        const r = renderPanel('claim');
        const want = 'Review claim · 0 SOL now · syncs may add up to 0.000025 SOL';
        await waitFor(() => expect(label().textContent).toBe(want));
        expect(searches).toBe(1);
        // A vest's release on each crank moves `acc` and `acc_r` together (here by 1 unit: no
        // lamport accrues to this lender, the claim view stays); six cranks in a row.
        const market = base.market as NonNullable<LendChain['market']>;
        for (let k = 1n; k <= 6n; k++) {
            const acc = bigIntToPodI128(podI128ToBigInt(market.acc) + k);
            const accR = bigIntToPodU128(podU128ToBigInt(market.accR) + k);
            setChain({ ...base, market: { ...market, acc, accR }, readAt: Date.now() });
            r.reread();
            expect(label().textContent).toBe(want);
            expect(label().disabled).toBe(false);
            expect(label().getAttribute('aria-busy')).toBeNull();
        }
        await act(async () => {});
        expect(searches).toBe(1);
        // An input of the pick that does change (here the pool's shares: a deposit) searches again,
        // and the last result stays while it runs (no flicker; the review would search again).
        const totalShares = bigIntToPodU128(podU128ToBigInt(market.totalShares) + 1n);
        setChain({ ...base, market: { ...market, totalShares }, readAt: Date.now() });
        r.reread();
        await waitFor(() => expect(searches).toBe(2));
        expect(label().textContent).toBe(want);
        expect(label().disabled).toBe(false);
        expect(label().getAttribute('aria-busy')).toBeNull();
        await act(async () => release(NO_SYNCS));
        await waitFor(() => expect(screen.getByTestId(T.lendClaimReason).textContent).toBe('Nothing to claim yet'));
        expect(label().textContent).toBe('Review claim · 0 SOL');
    });

    it('a failed search counts as no syncs', async () => {
        h.search.current = async () => {
            throw new Error('indexer down');
        };
        setChain(chainOf({ lender: lenderAccount(simpleLender(500_000_000_000n, { pending: 25_000n })) }));
        renderPanel('claim');
        await waitFor(() => expect(screen.getByTestId(T.lendClaimReason).textContent).toBe('Nothing to claim yet'));
        expect((screen.getByTestId(T.claimSubmit) as HTMLButtonElement).disabled).toBe(true);
    });

    it('the review takes the button’s fresh pick (no second search) and holds a claim that pays 0', async () => {
        h.search.current = async () => pickOf(30_000n);
        setChain(chainOf({ lender: lenderAccount(simpleLender(500_000_000_000n, { pending: 25_000n })) }));
        renderPanel('claim');
        const submit = screen.getByTestId(T.claimSubmit) as HTMLButtonElement;
        await waitFor(() => expect(submit.disabled).toBe(false));
        // The review's own search would wait on the (stubbed, never answering) indexer: the plan
        // comes from the button's pick.
        fireEvent.click(submit);
        await waitFor(() => expect(started).toHaveLength(1));
        const plan = started[0] as TxPlan;
        expect(plan.instructions.map((ix) => identifyShortwinInstruction(ix.data as Uint8Array))).toEqual([
            ShortwinInstruction.SyncLender,
            ShortwinInstruction.ClaimSol,
        ]);
        // Its simulation decides: one that pays 0 out of the vault is held.
        const paying = (after: bigint) => ({
            ok: true,
            unitsConsumed: 1n,
            computeUnitLimit: 1,
            priorityFeeMicroLamports: 0n,
            feeLamports: 5_000n,
            sizeBytes: 1,
            deltas: [{ key: `${VAULT}:lamports`, label: 'Market SOL vault', before: 9n, after, unit: 'SOL', scale: 9 }],
            logs: [],
            error: null,
            simulatedAt: 0,
        });
        expect(plan.holdSign?.(paying(9n))).toBe('Nothing to claim yet');
        expect(plan.holdSign?.(paying(8n))).toBeNull();
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

describe('Lend panel on any width (M10-RESP, FRONTEND.md §7.6)', () => {
    const reasonText = (id: string) => screen.getByTestId(id).textContent;

    it('deposit: the form foot says what is missing, first unmet first; its reason focuses the acknowledgement', () => {
        renderPanel('deposit');
        expect(reasonText(T.lendDepositReason)).toBe('Enter an amount');
        fireEvent.change(screen.getByTestId(T.lendAmountInput), { target: { value: '1.5' } });
        expect(reasonText(T.lendDepositReason)).toBe('Tick the acknowledgement above to continue');
        // The reason is a button that hands the focus to the checkbox (FormFoot reasonTarget).
        const ack = screen.getByTestId(T.disclosureAck('lender.solRepayment'));
        fireEvent.click(within(screen.getByTestId(T.lendDepositReason)).getByRole('button'));
        expect(document.activeElement).toBe(ack);
        fireEvent.click(ack);
        expect(reasonText(T.lendDepositReason)).toBe('');
        expect(submit().disabled).toBe(false);
        // Reason first, the submit last: the submit is the form foot's last child.
        const foot = submit().closest('.form-foot');
        expect(foot?.lastElementChild).toBe(submit());
    });

    it('deposit: a CPI-Guarded source points at the bracket box', () => {
        setChain(chainOf({ accounts: [account(GUARDED, 5_000_000n, { cpiGuard: true })] }));
        renderPanel('deposit');
        fireEvent.change(screen.getByTestId(T.lendAmountInput), { target: { value: '1' } });
        expect(reasonText(T.lendDepositReason)).toBe('Turn CPI Guard off for this deposit (above)');
        fireEvent.click(within(screen.getByTestId(T.lendDepositReason)).getByRole('button'));
        expect(document.activeElement).toBe(screen.getByTestId(T.lendCpiGuardBracket));
    });

    it('without a wallet: Connect wallet in the form foot above the disabled submit, on every tab', () => {
        h.wallet.current = { ready: true, status: 'disconnected', connected: null };
        const { unmount } = renderPanel('deposit');
        expect(reasonText(T.lendDepositReason)).toBe('Connect a wallet first');
        expect(screen.getByTestId(T.connectCta('lend'))).toBeTruthy();
        expect(submit().disabled).toBe(true);
        unmount();
        const w = renderPanel('withdraw');
        expect(screen.getByTestId(T.connectCta('lend'))).toBeTruthy();
        expect(screen.queryByText('You have no shares in this market.')).toBeNull();
        w.unmount();
        renderPanel('claim');
        expect(screen.getByTestId(T.connectCta('lend'))).toBeTruthy();
    });

    it('withdraw: the 25 / 50 / 75 chips fill a share of the holding, pressed while it is the input', () => {
        setChain(chainOf({ lender: lenderAccount(simpleLender(500_000_000_000n)), accounts: [account(ATA, 0n)] }));
        renderPanel('withdraw');
        expect(reasonText(T.lendWithdrawReason)).toBe('Enter the shares to withdraw');
        expect(screen.queryByTestId(T.withdrawApprox)).toBeNull();
        const c75 = screen.getByTestId(T.withdrawPercent('75'));
        fireEvent.click(c75);
        expect(c75.getAttribute('aria-pressed')).toBe('true');
        expect(screen.getByTestId(T.withdrawPercent('25')).getAttribute('aria-pressed')).toBe('false');
        const input = screen.getByTestId(T.withdrawSharesInput) as HTMLInputElement;
        // 75 % of 500,000,000,000 raw shares at 6 decimals.
        expect(input.value).toBe('375000');
        // "≈ N TOKEN" follows the preview's tokens.
        const tokens = BigInt(screen.getByTestId(T.withdrawPreview('tokens')).dataset.value ?? '0');
        expect(tokens > 0n).toBe(true);
        expect(screen.getByTestId(T.withdrawApprox).textContent).toBe(`≈ ${formatTokens(tokens, 6)}EPjF…Dt1v`);
        fireEvent.change(input, { target: { value: '1' } });
        expect(c75.getAttribute('aria-pressed')).toBe('false');
    });

    it('claim: nothing to claim says so in the form foot', () => {
        setChain(chainOf({ lender: lenderAccount(simpleLender(500_000_000_000n)) }));
        renderPanel('claim');
        expect(reasonText(T.lendClaimReason)).toBe('Nothing to claim yet');
        expect((screen.getByTestId(T.claimSubmit) as HTMLButtonElement).disabled).toBe(true);
    });
});
