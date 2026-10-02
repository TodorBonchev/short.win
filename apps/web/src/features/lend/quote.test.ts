/**
 * `min_sol` quoting (M1-SEC-2: the quoted payout, vest included) and the choice of other lenders to
 * sync before a claim (R3A-2 pot), FRONTEND.md §3.6, §5.10.
 */
import { address, createNoopSigner } from '@solana/kit';
import { beforeAll, describe, expect, it, vi } from 'vitest';
import { initMathSync, type MathModule } from '../../math/load.ts';
import type { AppRpc } from '../../rpc/client.ts';
import { readRepoBytes } from '../../test/repo.ts';
import type { SimulationReport, TxPlan } from '../../tx/types.ts';
import { lessBps } from './previews.ts';
import { pickSyncs, quoteWithdrawSol, type Simulate } from './quote.ts';
import { simpleLender, simplePool } from './testkit.ts';

let m: MathModule;
beforeAll(() => {
    m = initMathSync(readRepoBytes('packages/math-wasm/pkg/shortwin_math_wasm_bg.wasm'));
});

const VAULT = address('Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB');
const signer = createNoopSigner(address('9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin'));
const plan = { kind: 'withdraw', instructions: [], watch: [], disclosures: [], invalidate: [], title: 'w' } as TxPlan;
const rpc = {} as AppRpc;

const report = (o: Partial<SimulationReport>): SimulationReport => ({
    ok: true,
    unitsConsumed: 1n,
    computeUnitLimit: 1,
    priorityFeeMicroLamports: 0n,
    feeLamports: 5_000n,
    sizeBytes: 1,
    deltas: [],
    logs: [],
    error: null,
    simulatedAt: 0,
    ...o,
});

describe('quoteWithdrawSol', () => {
    it('takes the SOL the simulation moves out of the market vault', async () => {
        const simulate = vi.fn(async () => ({
            report: report({
                deltas: [
                    { key: `${VAULT}:lamports`, label: 'v', before: 10_000n, after: 1_000n, unit: 'SOL', scale: 9 },
                ],
            }),
            lifetime: null,
        })) as unknown as Simulate;
        const q = await quoteWithdrawSol({
            rpc,
            plan,
            signer,
            vault: VAULT,
            solAtLeast: 5n,
            vestOnLastExit: 0n,
            toleranceBps: 50,
            simulate,
        });
        expect(q).toEqual({ ok: true, lamports: 9_000n, source: 'simulation' });
        expect(simulate).toHaveBeenCalledWith(
            expect.objectContaining({ plan, feePayer: signer, priorityFeeMicroLamports: 0n }),
        );
    });

    it('falls back to the preview’s lower bound, plus the vest less the tolerance on a last exit', async () => {
        const failing = (async () => ({ report: report({ ok: false }), lifetime: null })) as unknown as Simulate;
        const throwing = (async () => {
            throw new Error('rpc down');
        }) as unknown as Simulate;
        const args = { rpc, plan, signer, vault: VAULT, solAtLeast: 100n, toleranceBps: 50 };
        expect(await quoteWithdrawSol({ ...args, vestOnLastExit: 0n, simulate: failing })).toEqual({
            ok: true,
            lamports: 100n,
            source: 'preview',
        });
        expect(await quoteWithdrawSol({ ...args, vestOnLastExit: 1_000_000n, simulate: throwing })).toEqual({
            ok: true,
            lamports: 100n + lessBps(1_000_000n, 50),
            source: 'preview',
        });
    });
});

describe('pickSyncs', () => {
    const pool = simplePool({ tokensFree: 1_000n, totalShares: 1_000_000n, lenderSol: 10_000n });
    const other = (i: number, real: bigint, epoch = 0) => ({
        address: address(
            [
                '7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE',
                'mSoLzYCxHdYgdzU16g5QSh3i5K3z3KZK7ytfqcJm7So',
                'JUPyiwrYJFskUPiHa7hkeR8VUtAeFoSYbKedZNsDvCN',
                '4k3Dyjzvzp8eMZWUXbBCjEvwSkkk59S5iCNLY3QrkX6R',
                'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v',
                'So11111111111111111111111111111111111111112',
            ][i] as string,
        ),
        lender: {
            ...simpleLender(100_000n, { pending: 10n, real, epoch }),
            discriminator: new Uint8Array(8),
            owner: address('11111111111111111111111111111111'),
            market: address('11111111111111111111111111111111'),
            bump: 255,
            reserved: new Uint8Array(32),
        },
    });

    it('picks the largest pot gains first until the shortfall is covered', () => {
        const others = [other(0, 110n), other(1, 510n), other(2, 10n), other(3, 310n)];
        const pick = pickSyncs(m, pool, others, 0n, 600n);
        expect(pick.syncs.map((s) => s.lender)).toEqual([others[1]?.address, others[3]?.address]);
        expect(pick.gain).toBe(800n);
    });

    it('takes at most four, and skips lenders behind the share epoch or without a surplus', () => {
        const others = [0, 1, 2, 3, 4, 5].map((i) => other(i, 1_000n));
        expect(pickSyncs(m, pool, others, 0n, 10n ** 9n).syncs).toHaveLength(4);
        expect(pickSyncs(m, pool, [other(0, 1_000n, 1)], 0n, 10n).syncs).toEqual([]);
        expect(pickSyncs(m, pool, [other(0, 10n)], 0n, 10n).syncs).toEqual([]);
    });
});
