/**
 * Regressions of the M9 review — refusal and disclosure UX: Retry after a refusal a fresh plan
 * fixes rebuilds the plan (or closes the review so the form builds a new one), and the copy says
 * so (M9-UX-1); the lender residuals of O19 are disclosed to lenders (M9-UX-2); the lender
 * previews run on the cluster clock (M9-UX-5).
 */
import type { LenderPosition, MarketVault } from '@shortwin/client';
import { address, createNoopSigner } from '@solana/kit';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook } from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { DISCLOSURES } from '../config/disclosures.ts';
import { PROGRAM_ERRORS } from '../config/errors.ts';
import { MEMO_PROGRAM, TOKEN_2022_PROGRAM } from '../config/programs.ts';
import type { LendChain } from '../features/lend/chain.ts';
import type { LoadedChain } from '../features/lend/ctx.ts';
import { previewWithdrawView } from '../features/lend/previews.ts';
import { fakeMarket, simpleLender, simplePool } from '../features/lend/testkit.ts';
import { initMathSync, type MathModule } from '../math/load.ts';
import { readRepoBytes } from '../test/repo.ts';
import type { TokenAccountInfo } from '../tx/tokenAccount.ts';
import type { SimulationReport, TxError, TxPlan } from '../tx/types.ts';

const WALLET = address('9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin');
const MINT = address('EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v');
const MARKET = address('7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE');
const VAULT = address('Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB');
const LENDER = address('mSoLzYCxHdYgdzU16g5QSh3i5K3z3KZK7ytfqcJm7So');
const ATA = address('4k3Dyjzvzp8eMZWUXbBCjEvwSkkk59S5iCNLY3QrkX6R');

const h = vi.hoisted(() => ({
    calls: [] as { plan: TxPlan }[],
    outcome: [] as (TxError | null)[],
    chain: { current: null as unknown },
}));

vi.mock('../rpc/context.tsx', () => ({ useAppClient: () => ({ rpc: {}, rpcSubscriptions: {} }) }));
vi.mock('../wallet/useWallet.ts', () => ({
    useWallet: () => ({
        ready: true,
        status: 'connected',
        connected: { address: WALLET, signer: createNoopSigner(WALLET), walletName: 'Test', supportsV0: true },
    }),
}));
vi.mock('../tx/priorityFee.ts', () => ({ estimatePriorityFee: async () => 0n }));
vi.mock('../tx/simulate.ts', () => ({
    simulatePlan: vi.fn(async (a: { plan: TxPlan }) => {
        h.calls.push(a);
        const error = h.outcome.length ? (h.outcome.shift() ?? null) : null;
        const report: SimulationReport = {
            ok: error === null,
            unitsConsumed: 100_000n,
            computeUnitLimit: 125_000,
            priorityFeeMicroLamports: 0n,
            feeLamports: 5_000n,
            sizeBytes: 0,
            deltas: [],
            logs: [],
            simulatedAt: Date.now(),
            error,
        };
        return { report, lifetime: null };
    }),
}));
// A fresh chain read for the review's Retry (`build.ts` `freshChain`).
vi.mock('../features/lend/chain.ts', async (orig) => ({
    ...(await orig<typeof import('../features/lend/chain.ts')>()),
    readLendChain: async () => h.chain.current,
}));

const { useTxFlow } = await import('../tx/useTxFlow.ts');
const { withdrawPlanFrom } = await import('../features/lend/build.ts');
const { useTradeFlow } = await import('../features/short/useTradeFlow.ts');

const refusal = (code: number): TxError => {
    const info = PROGRAM_ERRORS.get(code);
    return { kind: 'program', code, name: info?.name, action: info?.action ?? 'retry', message: info?.message ?? '' };
};

function harness() {
    const qc = new QueryClient();
    const invalidate = vi.spyOn(qc, 'invalidateQueries');
    const wrapper = ({ children }: { children: ReactNode }) => (
        <QueryClientProvider client={qc}>{children}</QueryClientProvider>
    );
    return { wrapper, invalidate };
}

const basePlan = (o: Partial<TxPlan> = {}): TxPlan => ({
    kind: 'open_short',
    title: 'Open short',
    instructions: [],
    watch: [],
    disclosures: [],
    invalidate: [['chain', 'market', MINT]],
    ...o,
});

beforeEach(() => {
    h.calls = [];
    h.outcome = [];
});

describe('M9-UX-1: Retry after a refusal a fresh plan fixes builds a new plan', () => {
    it('ProceedsBelowMin → Retry rebuilds (new quote) and simulates the NEW plan; the review says so', async () => {
        const { wrapper } = harness();
        const { result } = renderHook(() => useTxFlow(), { wrapper });
        const fresh = basePlan({ title: 'Open short · new quote' });
        const rebuild = vi.fn(async () => fresh);
        h.outcome = [refusal(6063), null];
        await act(async () => {
            result.current.start(basePlan({ rebuild }));
        });
        expect(result.current.state.stage).toBe('failed');
        await act(async () => {
            result.current.refresh();
        });
        expect(rebuild).toHaveBeenCalledTimes(1);
        expect(h.calls).toHaveLength(2);
        expect(h.calls[1]?.plan).toBe(fresh);
        expect(result.current.state.stage).toBe('review');
        expect(result.current.state.rebuilt).toBe(true);
        expect(result.current.state.plan).toBe(fresh);
    });

    it('a plan without `rebuild` closes the review and re-reads what it was built from', async () => {
        const { wrapper, invalidate } = harness();
        const { result } = renderHook(() => useTxFlow(), { wrapper });
        h.outcome = [refusal(6095)];
        await act(async () => {
            result.current.start(basePlan());
        });
        expect(result.current.state.stage).toBe('failed');
        await act(async () => {
            result.current.refresh();
        });
        expect(h.calls).toHaveLength(1); // no blind re-simulation of the stale plan
        expect(result.current.state.stage).toBe('idle');
        expect(invalidate).toHaveBeenCalledWith({ queryKey: ['chain', 'market', MINT] });
    });

    it('a transient failure is re-simulated as it is', async () => {
        const { wrapper } = harness();
        const { result } = renderHook(() => useTxFlow(), { wrapper });
        const rebuild = vi.fn(async () => basePlan());
        const plan = basePlan({ rebuild });
        h.outcome = [{ kind: 'rpc', action: 'retry', message: 'unreachable' }, null];
        await act(async () => {
            result.current.start(plan);
        });
        await act(async () => {
            result.current.refresh();
        });
        expect(rebuild).not.toHaveBeenCalled();
        expect(h.calls[1]?.plan).toBe(plan);
        expect(result.current.state.stage).toBe('review');
    });

    it('the copy no longer promises a step a Retry never adds', () => {
        for (const e of PROGRAM_ERRORS.values()) {
            expect(e.message, e.name).not.toMatch(/Try again: the app adds|Refresh the quote|Refresh and try again/);
        }
        for (const code of [6083, 6084, 6095]) expect(PROGRAM_ERRORS.get(code)?.message).toMatch(/A new review adds/);
    });

    it('a withdrawal’s Retry re-reads the destination: a memo now required is added (6095)', async () => {
        const m = initMathSync(readRepoBytes('packages/math-wasm/pkg/shortwin_math_wasm_bg.wasm'));
        const env = { rpc: {} as never, math: m, mint: MINT, signer: createNoopSigner(WALLET), symbol: 'USDC' };
        const before = lendChain([account(ATA, 0n)]);
        const plan = await withdrawPlanFrom(env, before, 1_000, {
            shares: 1_000_000n,
            destination: ATA,
            toleranceBps: 50,
        });
        expect(plan.title).not.toContain('with memo');
        expect(plan.rebuild).toBeDefined();
        // Memos became required on the destination after the review (the race of §10.3).
        h.chain.current = lendChain([account(ATA, 0n, { memoRequired: true })]);
        const rebuilt = await plan.rebuild?.();
        expect(rebuilt?.title).toContain('with memo');
        expect(rebuilt?.disclosures).toContain('token.memoRequired');
        const withdraw = rebuilt?.instructions.at(-1);
        expect(withdraw?.accounts?.some((a) => a.address === MEMO_PROGRAM)).toBe(true);
        expect(rebuilt?.rebuild).toBeDefined();
    });

    it('a trade’s plans carry `rebuild`, which re-reads and restarts the extras sets', async () => {
        const { result } = renderHook(() => useTradeFlow(), { wrapper: harness().wrapper });
        const extras = { label: 'current', sel: { set: 0, bumps: [] }, metas: [] } as never;
        const refresh = vi.fn(async () => ({
            build: async () => basePlan({ title: 'fresh' }),
            candidates: [extras],
        }));
        await act(async () => {
            result.current.start(async () => basePlan({ title: 'first' }), [extras], refresh);
        });
        // The plan useTradeFlow started (and the flow simulated) carries `rebuild`.
        const first = h.calls[0]?.plan;
        expect(first?.title).toBe('first');
        expect(first?.rebuild).toBeDefined();
        const again = await first?.rebuild?.();
        expect(refresh).toHaveBeenCalledTimes(1);
        expect(again?.title).toBe('fresh');
        expect(again?.rebuild).toBeDefined();
    });
});

describe('M9-UX-2: lenders are told the O19 residuals that fall on them', () => {
    const text = DISCLOSURES['lender.residuals'].body.join(' ');
    it('the liquidation that reserves nothing: the penalty and the aged rent, with the model figures', () => {
        expect(text).toMatch(/sets nothing aside for the lenders/);
        expect(text).toMatch(/penalty/);
        expect(text).toMatch(/\brent\b/);
        expect(text).toContain('2.06 SOL');
        expect(text).toContain('8.02 SOL');
    });
    it('Raydium pool pinning and the notional-cap exceedance, in a lender’s framing', () => {
        expect(text).toMatch(/thinner pool/);
        expect(text).toMatch(/notional cap .* can be exceeded by up to about 11%/);
        expect(text).not.toMatch(/Your margin is not affected/);
    });
});

describe('M9-UX-5: lender previews run on the cluster clock, not the device’s', () => {
    let m: MathModule;
    beforeAll(() => {
        m = initMathSync(readRepoBytes('packages/math-wasm/pkg/shortwin_math_wasm_bg.wasm'));
    });
    it('a device clock one hour ahead changes nothing: now = Clock at the read + time since', async () => {
        const { chainNowSeconds, clockUnixOf } = await import('../features/lend/chain.ts');
        // The read happened at device time D (an hour ahead of the cluster) with Clock = 10,800.
        const D = 1_700_000_000_000 + 3_600_000;
        const read = { clockUnix: 10_800n, readAt: D };
        expect(chainNowSeconds(read, D + 2_000)).toBe(10_802);
        expect(chainNowSeconds({ clockUnix: null, readAt: D }, D)).toBe(Math.floor(D / 1000));
        const pool = simplePool({
            tokensFree: 500n,
            borrowedQ: 500n,
            totalShares: 1_000_000n,
            vestBal: 1_000_000_000n,
            vestLast: 0n,
            vestEnd: 21_600n,
            lenderSol: 1_000_000_000n,
            maxRelease: 86_400n * 3n,
        });
        const s = { ...pool, lender: simpleLender(500_000n), records: [] };
        const atChain = previewWithdrawView(m, s, BigInt(chainNowSeconds(read, D)), 86_400, 100_000n, 50);
        expect(atChain.ok && atChain.value.vesting).toBe(250_000_000n);
        // The Clock sysvar decodes (unix_timestamp, i64 LE at 32).
        const bytes = new Uint8Array(40);
        new DataView(bytes.buffer).setBigInt64(32, 10_800n, true);
        const b64 = btoa(String.fromCharCode(...bytes));
        expect(clockUnixOf({ owner: 'Sysvar1111111111111111111111111111111111111', data: [b64, 'base64'] })).toBe(
            10_800n,
        );
        expect(clockUnixOf({ owner: TOKEN_2022_PROGRAM, data: [b64, 'base64'] })).toBeNull();
    });
});

// ---- fixtures -------------------------------------------------------------------------------

function account(a: string, amount: bigint, o: Partial<TokenAccountInfo> = {}): TokenAccountInfo {
    return {
        address: address(a),
        program: 'token-2022',
        mint: MINT,
        owner: WALLET,
        amount,
        cpiGuard: false,
        memoRequired: false,
        ...o,
    };
}

function lendChain(accounts: TokenAccountInfo[]): LoadedChain {
    const pool = { tokensFree: 1_000_000_000n, totalShares: 1_000_000_000_000n, lenderSol: 1_000_000n };
    const lender: LenderPosition = {
        ...simpleLender(10_000_000n),
        discriminator: new Uint8Array(8),
        owner: WALLET,
        market: MARKET,
        bump: 255,
        reserved: new Uint8Array(32),
    } as LenderPosition;
    const c: LendChain = {
        mint: MINT,
        marketAddress: MARKET,
        vaultAddress: VAULT,
        market: fakeMarket(pool, { tokenProgram: TOKEN_2022_PROGRAM, mint: MINT }),
        vault: simplePool(pool).vault as unknown as MarketVault,
        config: { params: { lockGraceS: 86_400 } } as unknown as LendChain['config'],
        mintInfo: null,
        owner: WALLET,
        lenderAddress: LENDER,
        lender,
        records: [],
        recordsComplete: true,
        tokenAccounts: accounts,
        ata: ATA,
        walletLamports: 10n ** 10n,
        slot: 1n,
        readAt: Date.now(),
        clockUnix: 1_000n,
    };
    return c as LoadedChain;
}
