/**
 * The claim builder (`claimPlanFrom`; FRONTEND.md §3.6 Claim, KI66-R4, KI-67): which other lenders'
 * unsigned `sync_lender` it prepends — current, held at a rebase record or walking a forward chain,
 * each judged with `previewTouchPot` on its records — and how: largest gain first, at most 4, the
 * records read-only from where each walk starts (one read for all), trimmed to 1,232 bytes, a sync
 * that fails in the simulation dropped, nothing without the indexer; the candidates from the
 * indexer's lenders behind the share epoch first, paged further while the shortfall is not covered
 * (FE-30); the expected payout with the
 * pot they move; the close disclosure; a closed position refused. The RPC is a map of accounts; the
 * math is the real WASM.
 */
import {
    findLenderPda,
    findShareEpochPda,
    getLenderPositionEncoder,
    getShareEpochEncoder,
    getSyncLenderInstructionDataDecoder,
    identifyShortwinInstruction,
    type LenderPosition,
    type MarketVault,
    ShortwinInstruction,
} from '@shortwin/client';
import {
    AccountRole,
    type Address,
    address,
    createNoopSigner,
    getAddressDecoder,
    getBase64Decoder,
    type Instruction,
} from '@solana/kit';
import { beforeAll, describe, expect, it, vi } from 'vitest';
import { SHORTWIN_PROGRAM } from '../../config/programs.ts';
import { initMathSync, type MathModule } from '../../math/load.ts';
import type { AppRpc } from '../../rpc/client.ts';
import { readRepoBytes } from '../../test/repo.ts';
import { MAX_TRANSACTION_BYTES, wireSizeOf } from '../../tx/message.ts';
import type { SimulationReport, TxError } from '../../tx/types.ts';
import { claimPlanFrom, LENDER_POSITION_RENT, type LendEnv, NO_SYNCS, searchPotSyncs } from './build.ts';
import type { LoadedChain } from './ctx.ts';
import { MAX_SYNCS_PER_CLAIM } from './plans.ts';
import { FWD, type LenderFields } from './previews.ts';
import {
    type LenderPage,
    MAX_SYNC_CANDIDATE_PAGES,
    type OtherLenders,
    type SearchProgress,
    type Simulate,
    SYNC_CANDIDATE_PAGE,
} from './quote.ts';
import { fakeMarket, simpleLender, simplePool } from './testkit.ts';

let m: MathModule;
beforeAll(() => {
    m = initMathSync(readRepoBytes('packages/math-wasm/pkg/shortwin_math_wasm_bg.wasm'));
});

const WAD = 10n ** 18n;
const MINT = address('EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v');
const OWNER = address('9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin');
const MARKET = address('7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE');
const VAULT = address('Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB');
const LENDER = address('mSoLzYCxHdYgdzU16g5QSh3i5K3z3KZK7ytfqcJm7So');
const OTHERS = [
    'JUPyiwrYJFskUPiHa7hkeR8VUtAeFoSYbKedZNsDvCN',
    '4k3Dyjzvzp8eMZWUXbBCjEvwSkkk59S5iCNLY3QrkX6R',
    'So11111111111111111111111111111111111111112',
    '8sLbNZoA1cfnvMJLPfp98ZLAnFSYCFApfJKMbiXNLwxj',
    'TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA',
    'ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL',
].map((a) => address(a));
const signer = createNoopSigner(OWNER);
const b64 = getBase64Decoder();

type Accounts = Map<string, Uint8Array>;

/** An RPC whose `getMultipleAccounts` serves `accounts` (owned by short.win). */
function rpcOf(accounts: Accounts) {
    const getMultipleAccounts = vi.fn((addresses: readonly Address[]) => ({
        send: async () => ({
            context: { slot: 1n },
            value: addresses.map((a) => {
                const data = accounts.get(a);
                return data
                    ? {
                          data: [b64.decode(data), 'base64'],
                          executable: false,
                          lamports: 1_000_000n,
                          owner: SHORTWIN_PROGRAM,
                          rentEpoch: 0n,
                          space: BigInt(data.length),
                      }
                    : null;
            }),
        }),
    }));
    return { rpc: { getMultipleAccounts } as unknown as AppRpc, getMultipleAccounts };
}

const position = (owner: Address, l: LenderFields): LenderPosition => ({
    ...l,
    discriminator: new Uint8Array(8),
    owner,
    market: MARKET,
    bump: 255,
    reserved: new Uint8Array(10),
});

async function putLender(accounts: Accounts, owner: Address, l: LenderFields): Promise<Address> {
    const [pda] = await findLenderPda({ market: MARKET, owner });
    accounts.set(pda, new Uint8Array(getLenderPositionEncoder().encode(position(owner, l))));
    return pda;
}

/** Records `from..to−1`: rebases (`1/1,000`) whose epochs realised 2 SOL per share and credited none. */
async function putRecords(accounts: Accounts, from: number, to: number): Promise<void> {
    for (let epoch = from; epoch < to; epoch++) {
        const [pda] = await findShareEpochPda({ market: MARKET, epoch });
        const bytes = getShareEpochEncoder().encode({
            market: MARKET,
            epoch,
            accEnd: 0n,
            accREnd: 2n * WAD,
            sOld: 10n ** 20n,
            carry: 0n,
            carryR: 0n,
            num: 1,
            denLog1000: 1,
            bump: 255,
            convRem: 0n,
        });
        accounts.set(pda, new Uint8Array(bytes));
    }
}

/** The claimant: current, 10 SOL credited, nothing realised, an empty pot (a 10 SOL shortfall). */
function claimant(o: { shareEpoch: number; lender?: Partial<LenderFields> | null; unmatched?: bigint }): LoadedChain {
    const pool = { shareEpoch: o.shareEpoch, lenderSol: 100n * 10n ** 9n, unmatched: o.unmatched ?? 0n };
    const lender =
        o.lender === null
            ? null
            : position(
                  OWNER,
                  simpleLender(1_000n, { epoch: o.shareEpoch, pending: 10n * 10n ** 9n, ...(o.lender ?? {}) }),
              );
    return {
        mint: MINT,
        marketAddress: MARKET,
        vaultAddress: VAULT,
        market: fakeMarket(pool, { mint: MINT }),
        vault: simplePool(pool).vault as unknown as MarketVault,
        config: null,
        mintInfo: null,
        owner: OWNER,
        lenderAddress: LENDER,
        lender,
        records: [],
        recordsComplete: true,
        tokenAccounts: [],
        ata: null,
        walletLamports: 10n ** 10n,
        slot: 1n,
        readAt: 0,
    } as LoadedChain;
}

const report = (error: TxError | null): SimulationReport => ({
    ok: error === null,
    unitsConsumed: 1n,
    computeUnitLimit: 1,
    priorityFeeMicroLamports: 0n,
    feeLamports: 5_000n,
    sizeBytes: 1,
    deltas: [],
    logs: [],
    error,
    simulatedAt: 0,
});

const okSim: Simulate = async () => ({ report: report(null), lifetime: null });

const env = (rpc: AppRpc, simulate: Simulate = okSim): LendEnv => ({
    rpc,
    math: m,
    mint: MINT,
    signer,
    symbol: 'TKN',
    simulate,
});

const kinds = (ixs: readonly Instruction[]) =>
    ixs.map((ix) =>
        ix.programAddress === SHORTWIN_PROGRAM
            ? ShortwinInstruction[identifyShortwinInstruction(ix.data as Uint8Array)]
            : ix.programAddress,
    );
const maxEpochs = (ix: Instruction | undefined) =>
    getSyncLenderInstructionDataDecoder().decode((ix?.data ?? new Uint8Array()) as Uint8Array).maxEpochs;

/**
 * A lender held at the record of `fwdEpoch − 1`, walking its forward chain from `fwdEpoch` with
 * `q` shares there (its snapshots there 0): the walk realises 2 lamports per chain share and record.
 */
const chained = (fwdEpoch: number, q = 10n ** 9n): LenderFields =>
    simpleLender(q * 1_000n, { epoch: fwdEpoch - 1, fwd: FWD.hold, fwdEpoch, fwdShares: q });

type Row = readonly [owner: Address, epoch: number];

/**
 * The indexer's lender lists of a market at `shareEpoch` (FE-30): `rows` in its order, each list
 * the rows of its `epoch` class (all of them when `filters` is false: an indexer without the
 * parameter), `limit` a page, an offset cursor; every request recorded.
 */
function indexer(rows: readonly Row[], shareEpoch: number, o: { filters?: boolean; failAt?: number } = {}) {
    const calls: { epoch: string; cursor: string | null }[] = [];
    const source: OtherLenders = async (q) => {
        calls.push({ epoch: q.epoch, cursor: q.cursor });
        if (o.failAt !== undefined && calls.length >= o.failAt) throw new Error('indexer down');
        const list = o.filters === false ? rows : rows.filter(([, e]) => e < shareEpoch === (q.epoch === 'behind'));
        const start = q.cursor === null ? 0 : Number(q.cursor);
        const end = start + q.limit;
        return {
            lenders: list.slice(start, end).map(([owner, epoch]) => ({ owner, epoch })),
            next: end < list.length ? String(end) : null,
        } satisfies LenderPage;
    };
    return { source, calls };
}

/** A distinct owner per `i` (no keys needed: the claimant signs, the owners do not). */
const ownerAt = (i: number) =>
    getAddressDecoder().decode(new Uint8Array(32).fill(0).map((_, j) => (j < 4 ? (i >> (8 * j)) & 255 : 7)));

describe('claimPlanFrom: the claim-time pot-only syncs (KI66-R4)', () => {
    it('judges current, held and chained lenders, largest gain first, with their records read once', async () => {
        const accounts: Accounts = new Map();
        await putRecords(accounts, 0, 3);
        // A current lender with 1 SOL realised beyond its credit.
        await putLender(accounts, OTHERS[0] as Address, simpleLender(1n, { epoch: 3, real: 10n ** 9n }));
        // Held at record 1 on its first sync: 3·10⁹ shares realise 6 SOL there, then its chain's
        // 3·10⁶ shares 0.006 SOL at record 2: 6.006 SOL.
        await putLender(accounts, OTHERS[1] as Address, simpleLender(3n * 10n ** 9n, { epoch: 1 }));
        // Chained from epoch 2 with 2·10⁹ shares: 4 SOL at record 2.
        await putLender(accounts, OTHERS[2] as Address, chained(2, 2n * 10n ** 9n));
        const { rpc, getMultipleAccounts } = rpcOf(accounts);
        const chain = claimant({ shareEpoch: 3 });
        const { source, calls } = indexer(
            [
                [OWNER, 3],
                [OTHERS[0] as Address, 3],
                [OTHERS[1] as Address, 1],
                [OTHERS[2] as Address, 1],
            ],
            3,
        );
        const plan = await claimPlanFrom(env(rpc), chain, 0, source);
        expect(kinds(plan.instructions)).toEqual(['SyncLender', 'SyncLender', 'ClaimSol']);
        // 6.006 SOL, then 4: together above the 10 SOL short.
        const [held, ch] = plan.instructions;
        expect(held?.accounts?.[2]?.address).toBe(
            (await findLenderPda({ market: MARKET, owner: OTHERS[1] as Address }))[0],
        );
        expect(maxEpochs(held)).toBe(2);
        expect(ch?.accounts?.[2]?.address).toBe(
            (await findLenderPda({ market: MARKET, owner: OTHERS[2] as Address }))[0],
        );
        expect(maxEpochs(ch)).toBe(1);
        expect((ch?.accounts ?? []).slice(-1)).toEqual([
            { address: (await findShareEpochPda({ market: MARKET, epoch: 2 }))[0], role: AccountRole.READONLY },
        ]);
        // Covered by the two largest: the current lender's 1 SOL is not needed.
        expect(plan.title).toContain('2 lender syncs first');
        expect(plan.title).toContain('Claim 10 SOL');
        expect(plan.disclosures).toEqual(['lender.potSyncs']);
        // FE-30: the lenders behind the share epoch first; they cover the shortfall, so the current
        // list is never asked for. Reads: their positions, then one read of the records 1..2.
        expect(calls).toEqual([{ epoch: 'behind', cursor: null }]);
        expect(getMultipleAccounts).toHaveBeenCalledTimes(2);
        expect((getMultipleAccounts.mock.calls[0] as unknown as [Address[]])[0]).toHaveLength(2);
        expect((getMultipleAccounts.mock.calls[1] as unknown as [Address[]])[0]).toHaveLength(2);
    });

    it('takes at most four, trims to 1,232 bytes, and names the syncs left for the next claim', async () => {
        const accounts: Accounts = new Map();
        await putRecords(accounts, 0, 49);
        // Six chains, each 8 distinct records behind: four of them alone exceed the byte limit.
        for (const [i, from] of [1, 9, 17, 25, 33, 41].entries()) {
            await putLender(accounts, OTHERS[i] as Address, chained(from));
        }
        const { rpc } = rpcOf(accounts);
        // A shortfall above what all of them move together.
        const chain = claimant({ shareEpoch: 49, lender: { pending: 10n ** 18n } });
        const rows = [1, 9, 17, 25, 33, 41].map((from, i): Row => [OTHERS[i] as Address, from - 1]);
        const plan = await claimPlanFrom(env(rpc), chain, 0, indexer(rows, 49).source);
        const n = plan.instructions.length - 1;
        expect(n).toBeGreaterThan(0);
        expect(n).toBeLessThan(MAX_SYNCS_PER_CLAIM);
        expect(wireSizeOf(plan.instructions, signer)).toBeLessThanOrEqual(MAX_TRANSACTION_BYTES);
        for (const ix of plan.instructions.slice(0, n)) expect(maxEpochs(ix)).toBe(8);
        // Six candidates: the cap leaves two, the size the rest of the four.
        expect(plan.disclosures).toEqual(['lender.potSyncs', 'lender.potSyncsDeferred']);
        expect(plan.disclosureVars?.['lender.potSyncsDeferred']?.count).toBe(String(6 - n));
        expect(plan.disclosureVars?.['lender.potSyncs']?.count).toBe(String(n));
    });

    it('drops a sync the simulation fails on and rebuilds without it', async () => {
        const accounts: Accounts = new Map();
        await putRecords(accounts, 0, 3);
        await putLender(accounts, OTHERS[0] as Address, simpleLender(1n, { epoch: 3, real: 3n * 10n ** 9n }));
        await putLender(accounts, OTHERS[1] as Address, simpleLender(1n, { epoch: 3, real: 2n * 10n ** 9n }));
        const { rpc } = rpcOf(accounts);
        // The first sync's position was closed since the read (3012 at message index 2: after the
        // two compute-budget instructions).
        const calls: number[] = [];
        const simulate: Simulate = async ({ plan }) => {
            calls.push(plan.instructions.length);
            return {
                report: report(
                    plan.instructions.length === 3
                        ? { kind: 'unknown', action: 'report', message: 'closed', instructionIndex: 2 }
                        : null,
                ),
                lifetime: null,
            };
        };
        const current = indexer(
            OTHERS.map((o): Row => [o, 3]),
            3,
        ).source;
        const plan = await claimPlanFrom(env(rpc, simulate), claimant({ shareEpoch: 3 }), 0, current);
        expect(calls).toEqual([3, 2]);
        expect(kinds(plan.instructions)).toEqual(['SyncLender', 'ClaimSol']);
        expect(plan.instructions[0]?.accounts?.[2]?.address).toBe(
            (await findLenderPda({ market: MARKET, owner: OTHERS[1] as Address }))[0],
        );
        // The pot the remaining sync moves: 2 SOL.
        expect(plan.title).toContain('Claim 2 SOL');
        // The failed sync is dropped as invalid, not deferred to the next claim: the review names
        // no deferred syncs (it was neither capped nor trimmed for size).
        expect(plan.disclosures).toEqual(['lender.potSyncs']);
        expect(plan.disclosureVars?.['lender.potSyncsDeferred']).toBeUndefined();
        // A failure in the claim itself (or no simulation) keeps the syncs: the review shows it.
        const claimFails: Simulate = async () => ({
            report: report({ kind: 'program', action: 'report', message: 'x', instructionIndex: 4 }),
            lifetime: null,
        });
        const kept = await claimPlanFrom(env(rpc, claimFails), claimant({ shareEpoch: 3 }), 0, current);
        expect(kinds(kept.instructions)).toEqual(['SyncLender', 'SyncLender', 'ClaimSol']);
        const down: Simulate = async () => {
            throw new Error('rpc down');
        };
        const still = await claimPlanFrom(env(rpc, down), claimant({ shareEpoch: 3 }), 0, current);
        expect(still.instructions).toHaveLength(3);
    });

    it('claims what the pot pays now without the indexer, and needs no sync when nothing is short', async () => {
        const accounts: Accounts = new Map();
        await putLender(accounts, OTHERS[0] as Address, simpleLender(1n, { epoch: 3, real: 10n ** 9n }));
        const { rpc, getMultipleAccounts } = rpcOf(accounts);
        const simulate = vi.fn(okSim);
        const down = await claimPlanFrom(env(rpc, simulate), claimant({ shareEpoch: 3 }), 0, async () => {
            throw new Error('indexer down');
        });
        expect(kinds(down.instructions)).toEqual(['ClaimSol']);
        expect(simulate).not.toHaveBeenCalled();
        const paid = claimant({ shareEpoch: 3, lender: { pending: 5n, real: 5n } });
        const listed = indexer(
            OTHERS.map((o): Row => [o, 3]),
            3,
        );
        const none = await claimPlanFrom(env(rpc, simulate), paid, 0, listed.source);
        expect(kinds(none.instructions)).toEqual(['ClaimSol']);
        expect(getMultipleAccounts).not.toHaveBeenCalled();
        expect(listed.calls).toEqual([]);
    });

    it('looks for no syncs under the 10,000-lamport dust threshold, and does from it (owner, 2026-10-11)', async () => {
        const accounts: Accounts = new Map();
        // Another lender with 1 SOL realised beyond its credit: a sync would move it to the pot.
        await putLender(accounts, OTHERS[0] as Address, simpleLender(1n, { epoch: 3, real: 10n ** 9n }));
        const { rpc, getMultipleAccounts } = rpcOf(accounts);
        const rows = OTHERS.slice(0, 1).map((o): Row => [o, 3]);
        // 9,999 lamports credited, nothing realised, an empty pot: dust (ECON §9.4), so no syncs.
        for (const pending of [9_999n, 9_999n + 300_000n]) {
            const dust = indexer(rows, 3);
            const real = pending - 9_999n;
            const plan = await claimPlanFrom(
                env(rpc),
                claimant({ shareEpoch: 3, lender: { pending, real } }),
                0,
                dust.source,
            );
            expect(kinds(plan.instructions), String(pending)).toEqual(['ClaimSol']);
            expect(dust.calls, String(pending)).toEqual([]);
            expect(plan.disclosures).toEqual([]);
        }
        expect(getMultipleAccounts).not.toHaveBeenCalled();
        // 10,000 lamports: the claim syncs the lender first.
        const at = indexer(rows, 3);
        const plan = await claimPlanFrom(
            env(rpc),
            claimant({ shareEpoch: 3, lender: { pending: 10_000n } }),
            0,
            at.source,
        );
        expect(kinds(plan.instructions)).toEqual(['SyncLender', 'ClaimSol']);
        expect(at.calls.length).toBeGreaterThan(0);
    });

    it('finds a lender behind the share epoch past the first page of raw share counts (FE-30)', async () => {
        const accounts: Accounts = new Map();
        await putRecords(accounts, 0, 3);
        // 60 current lenders, each 0.01 SOL realised beyond its credit, listed first by raw shares.
        const rows: Row[] = [];
        for (let i = 0; i < 60; i++) {
            await putLender(accounts, ownerAt(i), simpleLender(10n ** 12n, { epoch: 3, real: 10n ** 7n }));
            rows.push([ownerAt(i), 3]);
        }
        // The absent surplus holder, held at record 1 on its first sync (6.006 SOL), listed last.
        await putLender(accounts, OTHERS[1] as Address, simpleLender(3n * 10n ** 9n, { epoch: 1 }));
        rows.push([OTHERS[1] as Address, 1]);
        const chain = claimant({ shareEpoch: 3, lender: { pending: 6n * 10n ** 9n } });
        for (const filters of [true, false]) {
            const { rpc, getMultipleAccounts } = rpcOf(accounts);
            const { source, calls } = indexer(rows, 3, { filters });
            const plan = await claimPlanFrom(env(rpc), chain, 0, source);
            expect(kinds(plan.instructions)).toEqual(['SyncLender', 'ClaimSol']);
            expect(plan.instructions[0]?.accounts?.[2]?.address).toBe(
                (await findLenderPda({ market: MARKET, owner: OTHERS[1] as Address }))[0],
            );
            expect(plan.title).toContain('Claim 6 SOL');
            // The behind list alone; the current list is never needed. With the filter: one
            // position read, then the records. Without it the behind pass serves the whole list in
            // the indexer's order, and every row is read: two pages of positions, then the records.
            expect(calls.every((c) => c.epoch === 'behind')).toBe(true);
            expect(calls).toHaveLength(filters ? 1 : 2);
            const sizes = getMultipleAccounts.mock.calls.map((c) => (c as unknown as [Address[]])[0].length);
            expect(sizes).toEqual(filters ? [1, 2] : [SYNC_CANDIDATE_PAGE, 11, 2]);
        }
    });

    it("reads every row of either list whatever its epoch: an indexer behind or ahead of the RPC's share epoch", async () => {
        const accounts: Accounts = new Map();
        await putRecords(accounts, 0, 3);
        // Held at record 1 on its first sync: 6.006 SOL.
        await putLender(accounts, OTHERS[1] as Address, simpleLender(3n * 10n ** 9n, { epoch: 1 }));
        const rows: Row[] = [[OTHERS[1] as Address, 1]];
        const chain = claimant({ shareEpoch: 3, lender: { pending: 6n * 10n ** 9n } });
        const { rpc } = rpcOf(accounts);
        // Fresh: the behind list has it.
        const fresh = indexer(rows, 3);
        expect(kinds((await claimPlanFrom(env(rpc), chain, 0, fresh.source)).instructions)).toEqual([
            'SyncLender',
            'ClaimSol',
        ]);
        expect(fresh.calls.map((c) => c.epoch)).toEqual(['behind']);
        // The indexer has not ingested the rebases (its share epoch 1): the newly behind lender comes
        // in its current list, and is read there.
        const lagging = indexer(rows, 1);
        const late = await claimPlanFrom(env(rpc), chain, 0, lagging.source);
        expect(kinds(late.instructions)).toEqual(['SyncLender', 'ClaimSol']);
        expect(late.instructions[0]?.accounts?.[2]?.address).toBe(
            (await findLenderPda({ market: MARKET, owner: OTHERS[1] as Address }))[0],
        );
        expect(lagging.calls.map((c) => c.epoch)).toEqual(['behind', 'current']);
        // The RPC behind the indexer (the chain read at share epoch 3, the indexer at 4): a lender
        // current on the chain read comes in the indexer's behind list, and is read there.
        await putLender(accounts, OTHERS[0] as Address, simpleLender(1n, { epoch: 3, real: 10n ** 9n }));
        const ahead = indexer([[OTHERS[0] as Address, 3]], 4);
        const first = await claimPlanFrom(
            env(rpc),
            claimant({ shareEpoch: 3, lender: { pending: 10n ** 9n } }),
            0,
            ahead.source,
        );
        expect(kinds(first.instructions)).toEqual(['SyncLender', 'ClaimSol']);
        expect(first.instructions[0]?.accounts?.[2]?.address).toBe(
            (await findLenderPda({ market: MARKET, owner: OTHERS[0] as Address }))[0],
        );
        expect(ahead.calls.map((c) => c.epoch)).toEqual(['behind']);
    });

    it('judges a lender once when the indexer repeats it on a page', async () => {
        const accounts: Accounts = new Map();
        await putRecords(accounts, 0, 3);
        // Held at record 1 on its first sync (6.006 SOL); the indexer lists it three times.
        const absent = OTHERS[1] as Address;
        await putLender(accounts, absent, simpleLender(3n * 10n ** 9n, { epoch: 1 }));
        const source: OtherLenders = async (q) => ({
            lenders: q.epoch === 'behind' ? [absent, absent, absent].map((owner) => ({ owner, epoch: 1 })) : [],
            next: null,
        });
        const { rpc, getMultipleAccounts } = rpcOf(accounts);
        const chain = claimant({ shareEpoch: 3, lender: { pending: 6n * 10n ** 9n } });
        const plan = await claimPlanFrom(env(rpc), chain, 0, source);
        // One sync, its gain counted once: no repeated sync and no deferred one.
        expect(kinds(plan.instructions)).toEqual(['SyncLender', 'ClaimSol']);
        expect(plan.title).toContain('Claim 6 SOL');
        expect(plan.disclosureVars?.['lender.potSyncsDeferred']).toBeUndefined();
        // One position read (the repeats skipped), then the records.
        const sizes = getMultipleAccounts.mock.calls.map((c) => (c as unknown as [Address[]])[0].length);
        expect(sizes).toEqual([1, 2]);
    });

    it('reads at most a page of each indexer response, whatever the indexer sends', async () => {
        const accounts: Accounts = new Map();
        // 120 current lenders on one oversized page, each 0.001 SOL beyond its credit.
        const rows: Row[] = [];
        for (let i = 0; i < 120; i++) {
            await putLender(accounts, ownerAt(i), simpleLender(1n, { epoch: 3, real: 10n ** 6n }));
            rows.push([ownerAt(i), 3]);
        }
        const { rpc, getMultipleAccounts } = rpcOf(accounts);
        const calls: string[] = [];
        // An indexer that ignores `limit`: every row on one page, no cursor.
        const source: OtherLenders = async (q) => {
            calls.push(q.epoch);
            return {
                lenders: q.epoch === 'current' ? rows.map(([owner, epoch]) => ({ owner, epoch })) : [],
                next: null,
            };
        };
        const plan = await claimPlanFrom(env(rpc), claimant({ shareEpoch: 3 }), 0, source);
        expect(calls).toEqual(['behind', 'current']);
        // One position read of the page's first 50; the rest of the response is not read.
        const sizes = getMultipleAccounts.mock.calls.map((c) => (c as unknown as [Address[]])[0].length);
        expect(sizes).toEqual([SYNC_CANDIDATE_PAGE]);
        expect(kinds(plan.instructions)).toHaveLength(5);
        expect(plan.disclosureVars?.['lender.potSyncsDeferred']?.count).toBe(String(SYNC_CANDIDATE_PAGE - 4));
    });

    it('pages further while the shortfall is not covered, within the bound, and keeps what it found', async () => {
        const accounts: Accounts = new Map();
        // 260 current lenders, the i-th with (i + 1)·0.001 SOL realised beyond its credit.
        const rows: Row[] = [];
        for (let i = 0; i < 260; i++) {
            await putLender(accounts, ownerAt(i), simpleLender(1n, { epoch: 3, real: BigInt(i + 1) * 10n ** 6n }));
            rows.push([ownerAt(i), 3]);
        }
        // A shortfall above what all of them move.
        const chain = claimant({ shareEpoch: 3, lender: { pending: 10n ** 18n } });
        const { rpc, getMultipleAccounts } = rpcOf(accounts);
        const { source, calls } = indexer(rows, 3);
        const plan = await claimPlanFrom(env(rpc), chain, 0, source);
        // ≤ 4 pages in all: the (empty) behind list's, then three of the current list's 50.
        expect(calls).toHaveLength(MAX_SYNC_CANDIDATE_PAGES);
        expect(calls.map((c) => c.epoch)).toEqual(['behind', 'current', 'current', 'current']);
        const read = (MAX_SYNC_CANDIDATE_PAGES - 1) * SYNC_CANDIDATE_PAGE;
        // One position read per page; current lenders need no records.
        expect(getMultipleAccounts).toHaveBeenCalledTimes(MAX_SYNC_CANDIDATE_PAGES - 1);
        // The four largest of those read (rows 149, 148, 147, 146), the rest named as deferred.
        expect(kinds(plan.instructions)).toEqual(['SyncLender', 'SyncLender', 'SyncLender', 'SyncLender', 'ClaimSol']);
        expect(plan.instructions[0]?.accounts?.[2]?.address).toBe(
            (await findLenderPda({ market: MARKET, owner: ownerAt(read - 1) }))[0],
        );
        expect(plan.disclosureVars?.['lender.potSyncsDeferred']?.count).toBe(String(read - 4));
        // A page that fails ends the search with what the pages before it found.
        const failing = indexer(rows, 3, { failAt: 3 });
        const partial = await claimPlanFrom(env(rpcOf(accounts).rpc), chain, 0, failing.source);
        expect(failing.calls).toHaveLength(3);
        expect(partial.instructions).toHaveLength(5);
        expect(partial.instructions[0]?.accounts?.[2]?.address).toBe(
            (await findLenderPda({ market: MARKET, owner: ownerAt(SYNC_CANDIDATE_PAGE - 1) }))[0],
        );
        expect(partial.disclosureVars?.['lender.potSyncsDeferred']?.count).toBe(String(SYNC_CANDIDATE_PAGE - 4));
    });
});

describe('claimPlanFrom: a claim that empties the position (KI-67)', () => {
    it('discloses the close with the rent returned', async () => {
        const { rpc } = rpcOf(new Map());
        const wiped = claimant({ shareEpoch: 3, lender: { shares: 0n, pending: 7n, real: 7n } });
        const plan = await claimPlanFrom(env(rpc), wiped, 0, null);
        expect(plan.disclosures).toEqual(['lender.claimCloses']);
        expect(LENDER_POSITION_RENT).toBe(2_568_240n);
        expect(plan.disclosureVars?.['lender.claimCloses']).toEqual({ rent: '0.00256824' });
        // Shares left: no close.
        const kept = await claimPlanFrom(
            env(rpc),
            claimant({ shareEpoch: 3, lender: { pending: 7n, real: 7n } }),
            0,
            null,
        );
        expect(kept.disclosures).toEqual([]);
    });

    it('refuses a position that is gone (closed by an earlier claim or withdrawal)', async () => {
        const { rpc } = rpcOf(new Map());
        await expect(claimPlanFrom(env(rpc), claimant({ shareEpoch: 3, lender: null }), 0, null)).rejects.toThrow(
            /no lender position/,
        );
    });
});

describe('the Claim button’s search and the review (the owner’s decision of 2026-10-11, “only when syncs can pay”)', () => {
    it('searchPotSyncs is the review’s search: the same pick for a shortfall, none without one, a source or on a failure', async () => {
        const accounts: Accounts = new Map();
        // Another lender with 1 SOL realised beyond its credit: its sync moves it to the pot.
        await putLender(accounts, OTHERS[0] as Address, simpleLender(1n, { epoch: 3, real: 10n ** 9n }));
        const { rpc } = rpcOf(accounts);
        const rows = OTHERS.slice(0, 1).map((o): Row => [o, 3]);
        const chain = claimant({ shareEpoch: 3 });
        const a = { rpc, math: m, chain, claimant: OWNER, nowS: 0 };
        const pick = await searchPotSyncs({ ...a, source: indexer(rows, 3).source });
        expect(pick.syncs.map((x) => x.owner)).toEqual([OTHERS[0]]);
        expect(pick.gain).toBe(10n ** 9n);
        // The review's own search finds the same sync.
        const plan = await claimPlanFrom(env(rpc), chain, 0, indexer(rows, 3).source);
        expect(kinds(plan.instructions)).toEqual(['SyncLender', 'ClaimSol']);
        // No source (no indexer), a failing one, a dust shortfall, no shortfall, no position: none.
        expect(await searchPotSyncs({ ...a, source: null })).toEqual(NO_SYNCS);
        expect(await searchPotSyncs({ ...a, source: indexer(rows, 3, { failAt: 1 }).source })).toEqual(NO_SYNCS);
        const dust = indexer(rows, 3);
        const dustChain = claimant({ shareEpoch: 3, lender: { pending: 9_999n } });
        expect(await searchPotSyncs({ ...a, chain: dustChain, source: dust.source })).toEqual(NO_SYNCS);
        expect(dust.calls).toEqual([]);
        const paid = claimant({ shareEpoch: 3, lender: { pending: 5n, real: 5n } });
        expect(await searchPotSyncs({ ...a, chain: paid, source: indexer(rows, 3).source })).toEqual(NO_SYNCS);
        const gone = claimant({ shareEpoch: 3, lender: null });
        expect(await searchPotSyncs({ ...a, chain: gone, source: indexer(rows, 3).source })).toEqual(NO_SYNCS);
    });

    it('reports its pick as it goes, and whether a failed page or an abort cut it short (the button’s timeout keeps it)', async () => {
        const accounts: Accounts = new Map();
        const rows: Row[] = [];
        for (let i = 0; i < 120; i++) {
            await putLender(accounts, ownerAt(i), simpleLender(1n, { epoch: 3, real: BigInt(i + 1) * 10n ** 6n }));
            rows.push([ownerAt(i), 3]);
        }
        // A shortfall no page covers: the search pages on.
        const chain = claimant({ shareEpoch: 3, lender: { pending: 10n ** 18n } });
        const a = { rpc: rpcOf(accounts).rpc, math: m, chain, claimant: OWNER, nowS: 0 };
        // Run to its end: whole, and the progress holds the pick returned.
        const whole: SearchProgress = { pick: NO_SYNCS, cut: false };
        const all = await searchPotSyncs({ ...a, source: indexer(rows, 3).source, progress: whole });
        expect(whole).toEqual({ pick: all, cut: false });
        expect(all.gain).toBeGreaterThan(0n);
        // A failing page: what the pages before found, cut short.
        const failed: SearchProgress = { pick: NO_SYNCS, cut: false };
        const some = await searchPotSyncs({ ...a, source: indexer(rows, 3, { failAt: 3 }).source, progress: failed });
        expect(failed.cut).toBe(true);
        expect(failed.pick).toBe(some);
        expect(some.gain).toBeGreaterThan(0n);
        // A page that never answers (a slow indexer): the progress already holds the first pages'
        // pick for the caller that stops waiting (the button's timeout); the abort cuts it short.
        const slow = indexer(rows, 3);
        const stop = new AbortController();
        const hung: SearchProgress = { pick: NO_SYNCS, cut: false };
        let reached!: () => void;
        const third = new Promise<void>((r) => {
            reached = r;
        });
        const source: OtherLenders = async (q) => {
            if (slow.calls.length >= 2) {
                reached();
                await new Promise((r) => setTimeout(r, 20));
            }
            return slow.source(q);
        };
        const out = searchPotSyncs({ ...a, source, signal: stop.signal, progress: hung });
        await third;
        expect(hung.pick.gain).toBe(some.gain);
        expect(hung.cut).toBe(false);
        stop.abort();
        await out;
        expect(hung.cut).toBe(true);
    });

    it('the review takes the button’s fresh pick instead of searching again', async () => {
        const accounts: Accounts = new Map();
        await putLender(accounts, OTHERS[0] as Address, simpleLender(1n, { epoch: 3, real: 10n ** 9n }));
        const { rpc } = rpcOf(accounts);
        const rows = OTHERS.slice(0, 1).map((o): Row => [o, 3]);
        const chain = claimant({ shareEpoch: 3 });
        const pick = await searchPotSyncs({
            rpc,
            math: m,
            chain,
            claimant: OWNER,
            nowS: 0,
            source: indexer(rows, 3).source,
        });
        const again = indexer(rows, 3);
        const plan = await claimPlanFrom(env(rpc), chain, 0, again.source, pick);
        expect(again.calls).toEqual([]);
        expect(kinds(plan.instructions)).toEqual(['SyncLender', 'ClaimSol']);
        // A pick of none: the claim alone, still without a search.
        const none = indexer(rows, 3);
        expect(kinds((await claimPlanFrom(env(rpc), chain, 0, none.source, NO_SYNCS)).instructions)).toEqual([
            'ClaimSol',
        ]);
        expect(none.calls).toEqual([]);
    });

    it('a claim whose simulation pays 0 lamports is not offered for signing: “Nothing to claim yet”', async () => {
        const { rpc } = rpcOf(new Map());
        const plan = await claimPlanFrom(env(rpc), claimant({ shareEpoch: 3 }), 0, null);
        const hold = plan.holdSign;
        expect(hold).toBeDefined();
        const vault = (before: bigint, after: bigint): SimulationReport => ({
            ...report(null),
            deltas: [{ key: `${VAULT}:lamports`, label: 'Market SOL vault', before, after, unit: 'SOL', scale: 9 }],
        });
        expect(hold?.(vault(5_000_000n, 5_000_000n))).toBe('Nothing to claim yet');
        expect(hold?.(vault(5_000_000n, 4_999_999n))).toBeNull();
        // Without the vault's delta the review's own figures decide (no hold).
        expect(hold?.(report(null))).toBeNull();
    });
});
