/**
 * The claim builder (`claimPlanFrom`; FRONTEND.md §3.6 Claim, KI66-R4, KI-67): which other lenders'
 * unsigned `sync_lender` it prepends — current, held at a rebase record or walking a forward chain,
 * each judged with `previewTouchPot` on its records — and how: largest gain first, at most 4, the
 * records read-only from where each walk starts (one read for all), trimmed to 1,232 bytes, a sync
 * that fails in the simulation dropped, nothing without the indexer; the expected payout with the
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
import { AccountRole, type Address, address, createNoopSigner, getBase64Decoder, type Instruction } from '@solana/kit';
import { beforeAll, describe, expect, it, vi } from 'vitest';
import { SHORTWIN_PROGRAM } from '../../config/programs.ts';
import { initMathSync, type MathModule } from '../../math/load.ts';
import type { AppRpc } from '../../rpc/client.ts';
import { readRepoBytes } from '../../test/repo.ts';
import { MAX_TRANSACTION_BYTES, wireSizeOf } from '../../tx/message.ts';
import type { SimulationReport, TxError } from '../../tx/types.ts';
import { claimPlanFrom, LENDER_POSITION_RENT, type LendEnv } from './build.ts';
import type { LoadedChain } from './ctx.ts';
import { MAX_SYNCS_PER_CLAIM } from './plans.ts';
import { FWD, type LenderFields } from './previews.ts';
import type { Simulate } from './quote.ts';
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
        const plan = await claimPlanFrom(env(rpc), chain, 0, async () => [OWNER, ...OTHERS.slice(0, 3)]);
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
        // Reads: the other lenders, then one read of the records 1..2 for all of them.
        expect(getMultipleAccounts).toHaveBeenCalledTimes(2);
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
        const plan = await claimPlanFrom(env(rpc), chain, 0, async () => OTHERS);
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
        const plan = await claimPlanFrom(env(rpc, simulate), claimant({ shareEpoch: 3 }), 0, async () => OTHERS);
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
        const kept = await claimPlanFrom(env(rpc, claimFails), claimant({ shareEpoch: 3 }), 0, async () => OTHERS);
        expect(kinds(kept.instructions)).toEqual(['SyncLender', 'SyncLender', 'ClaimSol']);
        const down: Simulate = async () => {
            throw new Error('rpc down');
        };
        const still = await claimPlanFrom(env(rpc, down), claimant({ shareEpoch: 3 }), 0, async () => OTHERS);
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
        const none = await claimPlanFrom(env(rpc, simulate), paid, 0, async () => OTHERS);
        expect(kinds(none.instructions)).toEqual(['ClaimSol']);
        expect(getMultipleAccounts).not.toHaveBeenCalled();
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
