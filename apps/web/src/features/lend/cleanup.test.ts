/** Standalone pending-source cleanup: no deposit, token transfer or retry that adds one. */
import { identifyShortwinInstruction, type MarketVault, ShortwinInstruction } from '@shortwin/client';
import { type Address, address, createNoopSigner } from '@solana/kit';
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { PUMP_PROGRAM, SHORTWIN_PROGRAM } from '../../config/programs.ts';
import { initMathSync, type MathModule } from '../../math/load.ts';
import type { AppRpc } from '../../rpc/client.ts';
import { readRepoBytes } from '../../test/repo.ts';
import { assertAllowedInstructions } from '../../tx/integrity.ts';
import type { TxPlan } from '../../tx/types.ts';
import { cleanupPlanFrom, depositPlanFrom, type LendEnv } from './build.ts';
import { readLendChain } from './chain.ts';
import type { LoadedChain } from './ctx.ts';
import { MARKET_FLAG } from './previews.ts';
import { IX_CLAIM_CASHBACK } from './sweep.ts';
import { fakeMarket, simplePool } from './testkit.ts';

vi.mock('./chain.ts', async (orig) => ({
    ...(await orig<typeof import('./chain.ts')>()),
    readLendChain: vi.fn(),
}));

const MINT = address('EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v');
const OWNER = address('9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin');
const MARKET = address('7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE');
const VAULT = address('Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB');
const LENDER = address('mSoLzYCxHdYgdzU16g5QSh3i5K3z3KZK7ytfqcJm7So');
const SOURCE = address('8sLbNZoA1cfnvMJLPfp98ZLAnFSYCFApfJKMbiXNLwxj');
let math: MathModule;
beforeAll(() => {
    math = initMathSync(readRepoBytes('packages/math-wasm/pkg/shortwin_math_wasm_bg.wasm'));
});
beforeEach(() => {
    vi.mocked(readLendChain).mockReset();
});

function chainOf(pool: Parameters<typeof simplePool>[0] = {}): LoadedChain {
    return {
        mint: MINT,
        marketAddress: MARKET,
        vaultAddress: VAULT,
        market: fakeMarket(pool),
        vault: simplePool(pool).vault as MarketVault,
        config: null,
        mintInfo: null,
        owner: OWNER,
        lenderAddress: LENDER,
        lender: null,
        records: [],
        recordsComplete: true,
        tokenAccounts: [
            {
                address: SOURCE,
                program: 'token',
                mint: MINT,
                owner: OWNER,
                amount: 5_000_000n,
                cpiGuard: false,
                memoRequired: false,
            },
        ],
        ata: SOURCE,
        walletLamports: 1_000_000_000n,
        slot: 1n,
        readAt: Date.now(),
        clockUnix: 1_000n,
    };
}

const env = (rpc = {} as AppRpc): LendEnv => ({
    rpc,
    math,
    mint: MINT,
    signer: createNoopSigner(OWNER),
    symbol: 'TKN',
});
const input = { source: SOURCE, amount: 1_000_000n, toleranceBps: 50, bracket: false };
const kinds = (plan: TxPlan) =>
    plan.instructions.map((ix) =>
        ix.programAddress === SHORTWIN_PROGRAM
            ? ShortwinInstruction[identifyShortwinInstruction(ix.data as Uint8Array)]
            : ix.programAddress,
    );

describe('surviving-lender cleanup plans', () => {
    it.each([{ flags: 0 }, { depositHoldUntil: 0n }, { venue: 1, flags: MARKET_FLAG.wipeTraderSwept }])(
        'refuses a bundled deposit while cleanup is pending (case %#)',
        async (pool) => {
            await expect(depositPlanFrom(env(), chainOf(pool), 1_000, input)).rejects.toThrow(
                'Clean up pending lender income first',
            );
        },
    );

    it('refuses a previously valid deposit on retry when surviving-share cleanup becomes pending', async () => {
        const plan = await depositPlanFrom(env(), chainOf(), 1_000, input);
        expect(kinds(plan)).toEqual(['Deposit']);
        vi.mocked(readLendChain).mockResolvedValue(chainOf({ flags: 0 }));
        await expect(plan.rebuild?.()).rejects.toThrow('Clean up pending lender income first');
        expect(readLendChain).toHaveBeenCalledOnce();
    });

    it('needs no token account or complete lender records and watches the wallet, vault and actual vest', async () => {
        const chain = { ...chainOf({ flags: 0 }), tokenAccounts: [], recordsComplete: false };
        const plan = await cleanupPlanFrom(env(), chain);
        expect(plan.kind).toBe('lender_cleanup');
        expect(kinds(plan)).toEqual(['SweepTrader']);
        expect(plan.disclosures).toEqual(['lender.cleanup']);
        expect(plan.instructions.flatMap((ix) => ix.accounts?.map((a) => a.address))).not.toContain(SOURCE);
        expect(plan.watch.map((w) => w.address)).toEqual([OWNER, VAULT, MARKET]);
        expect(plan.watch.map((w) => w.kind)).not.toContain('token');
        expect(() => assertAllowedInstructions(plan.instructions)).not.toThrow();
        vi.mocked(readLendChain).mockResolvedValue(chain);
        const rebuilt = await plan.rebuild?.();
        expect(rebuilt?.kind).toBe('lender_cleanup');
        expect(kinds(rebuilt as TxPlan)).toEqual(['SweepTrader']);
    });

    it('claims delayed pump cashback before the standalone sweep, with no wallet token leg', async () => {
        const chain = chainOf({ venue: 1, flags: MARKET_FLAG.wipeTraderSwept });
        const rpc = {
            getMultipleAccounts: (keys: readonly Address[]) => ({
                send: async () => ({
                    value: keys.map((key) =>
                        key === chain.market.pumpUva
                            ? { owner: PUMP_PROGRAM, data: ['', 'base64'], lamports: 2_000_000n }
                            : null,
                    ),
                }),
            }),
            getMinimumBalanceForRentExemption: () => ({ send: async () => 1_000_000n }),
        } as unknown as AppRpc;
        const plan = await cleanupPlanFrom(env(rpc), chain);
        expect(kinds(plan)).toEqual([PUMP_PROGRAM, 'SweepTrader']);
        expect(plan.instructions[0]?.data).toEqual(IX_CLAIM_CASHBACK);
        expect(plan.instructions.flatMap((ix) => ix.accounts?.map((a) => a.address))).not.toContain(SOURCE);
        expect(() => assertAllowedInstructions(plan.instructions)).not.toThrow();
    });

    it.each([{}, { flags: 0, totalShares: 0n }])(
        'never converts a cleanup retry to a deposit (case %#)',
        async (pool) => {
            const plan = await cleanupPlanFrom(env(), chainOf({ flags: 0 }));
            vi.mocked(readLendChain).mockResolvedValue(chainOf(pool));
            await expect(plan.rebuild?.()).rejects.toThrow('No separate pool cleanup is needed now');
        },
    );

    it('blocks the later deposit on a vest created by cleanup until its actual release', async () => {
        const vest = { depositHoldUntil: 900n, vestBal: 600n, vestLast: 1_000n, vestEnd: 1_200n };
        await expect(depositPlanFrom(env(), chainOf(vest), 1_000, input)).rejects.toThrow('vesting');
        const released = await depositPlanFrom(env(), chainOf(vest), 1_200, input);
        expect(kinds(released)).toEqual(['Deposit']);
    });
});
