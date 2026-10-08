/**
 * `min_sol` quoting (M1-SEC-2: the quoted payout, vest included) and the choice of other lenders to
 * sync before a claim (R3A-2 pot), FRONTEND.md §3.6, §5.10.
 */
import {
    bigIntToPodI128,
    bigIntToPodU128,
    findLenderPda,
    getLenderPositionEncoder,
    type LenderPosition,
} from '@shortwin/client';
import { type Address, address, createNoopSigner } from '@solana/kit';
import { beforeAll, describe, expect, it, vi } from 'vitest';
import { SHORTWIN_PROGRAM, SYSTEM_PROGRAM } from '../../config/programs.ts';
import { initMathSync, type MathModule } from '../../math/load.ts';
import type { AppRpc } from '../../rpc/client.ts';
import { readRepoBytes } from '../../test/repo.ts';
import type { SimulationReport, TxPlan } from '../../tx/types.ts';
import { MAX_SYNCS_PER_CLAIM } from './plans.ts';
import { FWD, type LenderFields, lessBps } from './previews.ts';
import {
    type OtherLender,
    pickPotSyncs,
    quoteWithdrawSol,
    readOtherLenders,
    readPotRecords,
    type Simulate,
} from './quote.ts';
import { recordOf, simpleLender, simplePool } from './testkit.ts';

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

    it('falls back to the preview’s lower bound (vest included), less the tolerance on a last exit’s vest', async () => {
        const failing = (async () => ({ report: report({ ok: false }), lifetime: null })) as unknown as Simulate;
        const throwing = (async () => {
            throw new Error('rpc down');
        }) as unknown as Simulate;
        const args = { rpc, plan, signer, vault: VAULT, toleranceBps: 50 };
        expect(await quoteWithdrawSol({ ...args, solAtLeast: 100n, vestOnLastExit: 0n, simulate: failing })).toEqual({
            ok: true,
            lamports: 100n,
            source: 'preview',
        });
        const last = { ...args, solAtLeast: 1_000_100n, vestOnLastExit: 1_000_000n, simulate: throwing };
        expect(await quoteWithdrawSol(last)).toEqual({
            ok: true,
            lamports: 100n + lessBps(1_000_000n, 50),
            source: 'preview',
        });
        // Never below zero, whatever the preview says.
        expect(await quoteWithdrawSol({ ...last, solAtLeast: 1n })).toEqual({
            ok: true,
            lamports: 0n,
            source: 'preview',
        });
    });
});

describe('pickPotSyncs', () => {
    const WAD = 10n ** 18n;
    const pool = simplePool({ tokensFree: 1_000n, totalShares: 1_000_000n, lenderSol: 10_000n });
    const ADDRS = [
        '7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE',
        'mSoLzYCxHdYgdzU16g5QSh3i5K3z3KZK7ytfqcJm7So',
        'JUPyiwrYJFskUPiHa7hkeR8VUtAeFoSYbKedZNsDvCN',
        '4k3Dyjzvzp8eMZWUXbBCjEvwSkkk59S5iCNLY3QrkX6R',
        'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v',
        'So11111111111111111111111111111111111111112',
    ];
    const position = (l: LenderFields): LenderPosition => ({
        ...l,
        discriminator: new Uint8Array(8),
        owner: address('11111111111111111111111111111111'),
        market: address('11111111111111111111111111111111'),
        bump: 255,
        reserved: new Uint8Array(10),
    });
    const other = (i: number, real: bigint, o: Partial<LenderFields> = {}): OtherLender => ({
        address: address(ADDRS[i] as string),
        lender: position(simpleLender(100_000n, { pending: 10n, real, ...o })),
    });

    it('picks the largest pot gains first until the shortfall is covered', () => {
        const others = [other(0, 110n), other(1, 510n), other(2, 10n), other(3, 310n)];
        const pick = pickPotSyncs(m, pool, others, [], 0n, 600n);
        expect(pick.syncs.map((s) => s.lender)).toEqual([others[1]?.address, others[3]?.address]);
        expect(pick.syncs.map((s) => s.owner)).toEqual([others[1]?.lender.owner, others[3]?.lender.owner]);
        expect(pick.syncs.map((s) => [s.gain, s.epochs])).toEqual([
            [500n, []],
            [300n, []],
        ]);
        expect(pick.gain).toBe(800n);
        expect(pick.more).toEqual({ count: 0, gain: 0n });
    });

    it('takes at most four, says what the rest would move, and skips lenders without a surplus', () => {
        const others = [0, 1, 2, 3, 4, 5].map((i) => other(i, 1_000n + BigInt(i)));
        const pick = pickPotSyncs(m, pool, others, [], 0n, 10n ** 9n);
        expect(pick.syncs).toHaveLength(MAX_SYNCS_PER_CLAIM);
        expect(pick.more).toEqual({ count: 2, gain: 990n + 991n });
        expect(pickPotSyncs(m, pool, [other(0, 10n)], [], 0n, 10n).syncs).toEqual([]);
        // Covered: nothing more is named.
        expect(pickPotSyncs(m, pool, others, [], 0n, 10n).more).toEqual({ count: 0, gain: 0n });
    });

    // A lender behind a rebase record (KI66-R4): judged with the pot-only touch and its records.
    const rebased = () => {
        const base = simplePool({ shareEpoch: 1, totalShares: 2_000_000n, lenderSol: 10n ** 10n });
        return {
            market: { ...base.market, acc: bigIntToPodI128(3n * WAD), accR: bigIntToPodU128(2n * WAD) },
            vault: base.vault,
        };
    };
    const record = recordOf({
        epoch: 0,
        accEnd: String(2n * WAD),
        accREnd: String(3n * WAD),
        sOld: '2000000000',
        num: 1,
        denLog1000: 1,
        convRem: '0',
    });

    it('judges lenders behind a rebase record with the pot-only touch and the records from where they walk', () => {
        const base = rebased();
        const behind = other(0, 0n, { shares: 1_000_000_001n, pending: 0n });
        const pick = pickPotSyncs(m, base, [behind], [record], 0n, 10n ** 12n);
        expect(pick.syncs).toEqual([
            { lender: behind.address, owner: behind.lender.owner, epochs: [0], gain: 999_000_001n },
        ]);
        // Without its record the sync moves nothing: not picked.
        expect(pickPotSyncs(m, base, [behind], [], 0n, 10n ** 12n).syncs).toEqual([]);
        // A chained lender walks from its fwdEpoch: no record for a chain at the share epoch.
        const chained = other(1, 0n, {
            shares: 1_000_000_001n,
            pending: 0n,
            fwd: FWD.hold,
            fwdEpoch: 1,
            fwdShares: 1_000_000n,
            // The chain's snapshots in epoch 1: credited to 3 SOL per share, realised to 0.
            snap: 3n * WAD,
            snapR: 0n,
        });
        const c = pickPotSyncs(m, base, [chained], [record], 0n, 10n ** 12n);
        expect(c.syncs.map((s) => s.epochs)).toEqual([[]]);
        expect(c.gain).toBe(1_000_000n * 2n);
    });
});

describe('readPotRecords', () => {
    it('reads the union of the epochs every candidate needs, once', async () => {
        const send = vi.fn(async () => ({ value: [] }));
        const getMultipleAccounts = vi.fn(() => ({ send }));
        const rpc = { getMultipleAccounts } as unknown as AppRpc;
        const l = (epoch: number, fwd = 0, fwdEpoch = 0) =>
            ({ address: VAULT, lender: { epoch, fwd, fwdEpoch } }) as unknown as OtherLender;
        await readPotRecords(rpc, VAULT, 12, [l(12), l(3), l(1, 1, 9), l(10)]);
        expect(getMultipleAccounts).toHaveBeenCalledTimes(1);
        // 3..10 (8 records), 9..11 (the chain's), 10..11: one read of 3..11.
        const [addresses] = getMultipleAccounts.mock.calls[0] as unknown as [string[]];
        expect(addresses).toHaveLength(9);
        await readPotRecords(rpc, VAULT, 12, [l(12)]);
        expect(getMultipleAccounts).toHaveBeenCalledTimes(1);
    });
});

describe('readOtherLenders', () => {
    it('drops owners without a position, and a funded system account at a lender PDA alone', async () => {
        const owners = [
            address('JUPyiwrYJFskUPiHa7hkeR8VUtAeFoSYbKedZNsDvCN'),
            address('EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v'),
            address('9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin'),
        ];
        const position = (owner: Address) =>
            new Uint8Array(
                getLenderPositionEncoder().encode({
                    owner,
                    market: VAULT,
                    shares: 7n,
                    snap: 0n,
                    pending: 0n,
                    snapR: 0n,
                    real: 0n,
                    claimed: 0n,
                    epoch: 0,
                    lock: 0n,
                    lockPi: 10n ** 18n,
                    lockEpoch: 4_294_967_295,
                    lockTerm: 0n,
                    lockUntil: 0n,
                    bump: 255,
                    fwdShares: 0n,
                    fwdEpoch: 0,
                    fwd: 0,
                    fwdLag: 0,
                    reserved: new Uint8Array(10),
                }),
            );
        const acct = (owner: Address, data: Uint8Array, lamports: bigint) => ({
            owner,
            lamports,
            executable: false,
            rentEpoch: 0n,
            space: BigInt(data.length),
            data: [Buffer.from(data).toString('base64'), 'base64'] as const,
        });
        const send = vi.fn(async () => ({
            value: [
                acct(SHORTWIN_PROGRAM, position(owners[0] as Address), 2_568_240n),
                // KI-67 closed this one; someone then funded its PDA (system-owned, no data).
                acct(SYSTEM_PROGRAM, new Uint8Array(), 890_880n),
                null,
            ],
        }));
        const rpc = { getMultipleAccounts: vi.fn(() => ({ send })) } as unknown as AppRpc;
        const got = await readOtherLenders(rpc, VAULT, owners);
        expect(got).toHaveLength(1);
        expect(got[0]?.lender.shares).toBe(7n);
        expect(got[0]?.address).toBe((await findLenderPda({ market: VAULT, owner: owners[0] as Address }))[0]);
    });
});
