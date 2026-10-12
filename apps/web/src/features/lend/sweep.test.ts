/**
 * The post-wipe sweep bundle (FRONTEND.md §3.6, §10.1 "the sync / sweep prepends") against the
 * keeper's `services/keeper/src/sweep.rs`: the same instructions, accounts, roles and order per
 * venue and per cashback state, and the venue discriminators as Anchor derives them.
 */
import { createHash } from 'node:crypto';
import { findShareEpochPda, identifyShortwinInstruction, ShortwinInstruction } from '@shortwin/client';
import { AccountRole, type Address, address, createNoopSigner, getProgramDerivedAddress } from '@solana/kit';
import { describe, expect, it } from 'vitest';
import {
    ASSOCIATED_TOKEN_PROGRAM,
    PUMP_AMM_PROGRAM,
    PUMP_PROGRAM,
    SHORTWIN_PROGRAM,
    SYSTEM_PROGRAM,
    TOKEN_PROGRAM,
    WSOL_MINT,
} from '../../config/programs.ts';
import type { AppRpc } from '../../rpc/client.ts';
import { eventAuthority } from '../../rpc/pda.ts';
import { MARKET_FLAG } from './previews.ts';
import {
    cashbackClaimIxs,
    IX_CLAIM_CASHBACK,
    IX_INIT_USER_VOLUME_ACCUMULATOR,
    NO_CASHBACK,
    PUMP_AMM_EVENT_AUTHORITY,
    PUMP_EVENT_AUTHORITY,
    pammCashbackAta,
    pammUva,
    postWipeSweepIxs,
    readSweepFacts,
    sweepTraderIx,
    traderWsolAta,
    userVolumeAccumulator,
    wsolAta,
} from './sweep.ts';
import { fakeMarket, NO_UVA } from './testkit.ts';

const MARKET = address('8sLbNZoA1cfnvMJLPfp98ZLAnFSYCFApfJKMbiXNLwxj');
const PAYER = createNoopSigner(address('9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin'));
const anchor = (name: string) => [...createHash('sha256').update(`global:${name}`).digest().subarray(0, 8)];

const wiped = (venue: number, flags = MARKET_FLAG.wipeLive) =>
    fakeMarket({ tokensFree: 0n, totalShares: 0n, flags, venue, shareEpoch: 3 });

describe('post-wipe sweep bundle', () => {
    it('uses the Anchor discriminators of claim_cashback and init_user_volume_accumulator', () => {
        expect([...IX_CLAIM_CASHBACK]).toEqual(anchor('claim_cashback'));
        expect([...IX_INIT_USER_VOLUME_ACCUMULATOR]).toEqual(anchor('init_user_volume_accumulator'));
    });

    it('derives the program and venue PDAs as the program does', async () => {
        const [ea] = await getProgramDerivedAddress({ programAddress: SHORTWIN_PROGRAM, seeds: ['__event_authority'] });
        expect(await eventAuthority()).toBe(ea);
        const [pumpEa] = await getProgramDerivedAddress({ programAddress: PUMP_PROGRAM, seeds: ['__event_authority'] });
        expect(pumpEa).toBe(PUMP_EVENT_AUTHORITY);
        const [pammEa] = await getProgramDerivedAddress({
            programAddress: PUMP_AMM_PROGRAM,
            seeds: ['__event_authority'],
        });
        expect(pammEa).toBe(PUMP_AMM_EVENT_AUTHORITY);
        const m = wiped(3);
        // Nothing cached: derived from the trader.
        expect(await traderWsolAta(m)).toBe(await wsolAta(m.trader));
        expect(await pammUva(m)).toBe(await userVolumeAccumulator(PUMP_AMM_PROGRAM, m.trader));
        expect(await pammCashbackAta(m)).toBe(await wsolAta(await pammUva(m)));
        // Cached keys win.
        const cache = [...m.poolCache];
        cache[4] = MARKET;
        expect(await pammUva({ ...m, poolCache: cache })).toBe(MARKET);
    });

    it('sweeps a mock / Raydium market with the 10 fixed accounts and the live wipe record', async () => {
        const m = wiped(6);
        const [record] = await findShareEpochPda({ market: MARKET, epoch: 2 });
        const ixs = await postWipeSweepIxs(PAYER, MARKET, m, NO_CASHBACK, record);
        expect(ixs).toHaveLength(1);
        const ix = ixs[0];
        expect(identifyShortwinInstruction(ix?.data as Uint8Array)).toBe(ShortwinInstruction.SweepTrader);
        expect(ix?.accounts?.map((a) => [a.address, a.role])).toEqual([
            [expect.any(String), AccountRole.READONLY], // config
            [MARKET, AccountRole.WRITABLE],
            [m.marketVault, AccountRole.WRITABLE],
            [m.trader, AccountRole.WRITABLE],
            [m.traderWsol, AccountRole.WRITABLE],
            [await wsolAta(m.trader), AccountRole.WRITABLE],
            [TOKEN_PROGRAM, AccountRole.READONLY],
            [SYSTEM_PROGRAM, AccountRole.READONLY],
            [await eventAuthority(), AccountRole.READONLY],
            [SHORTWIN_PROGRAM, AccountRole.READONLY],
            [record, AccountRole.WRITABLE],
        ]);
    });

    it('adds the pump token leg (and the PumpSwap cashback ATA) after the fixed accounts', async () => {
        const curve = await sweepTraderIx(MARKET, wiped(1), null);
        const m = wiped(1);
        expect(curve.accounts?.slice(10).map((a) => [a.address, a.role])).toEqual([
            [m.traderBaseAta, AccountRole.WRITABLE],
            [m.poolVault, AccountRole.WRITABLE],
            [m.mint, AccountRole.READONLY],
            [m.tokenProgram, AccountRole.READONLY],
            [m.pumpUva, AccountRole.READONLY],
        ]);
        const pamm = await sweepTraderIx(MARKET, wiped(3), null);
        expect(pamm.accounts).toHaveLength(16);
        expect(pamm.accounts?.[15]).toEqual({ address: await pammCashbackAta(wiped(3)), role: AccountRole.READONLY });
    });

    it('claims pump cashback before the sweep (user unsigned)', async () => {
        const m = wiped(1);
        const ixs = await postWipeSweepIxs(PAYER, MARKET, m, { ...NO_CASHBACK, pumpClaimable: 5n }, null);
        expect(ixs.map((i) => i.programAddress)).toEqual([PUMP_PROGRAM, SHORTWIN_PROGRAM]);
        const claim = ixs[0];
        expect([...((claim?.data as Uint8Array) ?? [])]).toEqual(anchor('claim_cashback'));
        expect(claim?.accounts?.map((a) => [a.address, a.role])).toEqual([
            [m.trader, AccountRole.WRITABLE],
            [await userVolumeAccumulator(PUMP_PROGRAM, m.trader), AccountRole.WRITABLE],
            [SYSTEM_PROGRAM, AccountRole.READONLY],
            [PUMP_EVENT_AUTHORITY, AccountRole.READONLY],
            [PUMP_PROGRAM, AccountRole.READONLY],
        ]);
    });

    it('creates the pAMM UVA when missing, then ATA(trader, WSOL), then claims into it', async () => {
        const m = wiped(3);
        const ixs = await cashbackClaimIxs(PAYER, m, { pumpClaimable: 0n, pammClaimable: 9n, pammUvaExists: false });
        expect(ixs.map((i) => i.programAddress)).toEqual([
            PUMP_AMM_PROGRAM,
            ASSOCIATED_TOKEN_PROGRAM,
            PUMP_AMM_PROGRAM,
        ]);
        const [init, ata, claim] = ixs;
        expect(init?.accounts?.[0]).toMatchObject({ address: PAYER.address, role: AccountRole.WRITABLE_SIGNER });
        expect(init?.accounts?.[1]).toEqual({ address: m.trader, role: AccountRole.READONLY });
        expect(ata?.accounts?.[1]?.address).toBe(await wsolAta(m.trader));
        expect(ata?.accounts?.[3]?.address).toBe(WSOL_MINT);
        const uva = await pammUva(m);
        expect(claim?.accounts?.map((a) => a.address)).toEqual([
            m.trader,
            uva,
            WSOL_MINT,
            TOKEN_PROGRAM,
            await wsolAta(uva),
            await traderWsolAta(m),
            SYSTEM_PROGRAM,
            PUMP_AMM_EVENT_AUTHORITY,
            PUMP_AMM_PROGRAM,
        ]);
        const existing = await cashbackClaimIxs(PAYER, m, {
            pumpClaimable: 0n,
            pammClaimable: 9n,
            pammUvaExists: true,
        });
        expect(existing).toHaveLength(2);
    });

    it('sweeps a pool every lender left, which has no wipe record to name (AIR-08)', async () => {
        // S = 0 after the last exit: `withdraw` cleared both bits and no record is live.
        const ixs = await postWipeSweepIxs(PAYER, MARKET, wiped(6, 0), NO_CASHBACK, null);
        expect(ixs).toHaveLength(1);
        expect(identifyShortwinInstruction(ixs[0]?.data as Uint8Array)).toBe(ShortwinInstruction.SweepTrader);
        expect(ixs[0]?.accounts).toHaveLength(10);
        // On the curve: the pump claim while cashback is outstanding, then the sweep with its token leg.
        const curve = await postWipeSweepIxs(PAYER, MARKET, wiped(1, 0), { ...NO_CASHBACK, pumpClaimable: 5n }, null);
        expect(curve.map((i) => i.programAddress)).toEqual([PUMP_PROGRAM, SHORTWIN_PROGRAM]);
        expect(curve[1]?.accounts).toHaveLength(15);
        // The trader bit is enough off the pump venues; the curve also waits for the rewards bit.
        expect(await postWipeSweepIxs(PAYER, MARKET, wiped(6, MARKET_FLAG.wipeTraderSwept), NO_CASHBACK, null)).toEqual(
            [],
        );
        expect(
            await postWipeSweepIxs(PAYER, MARKET, wiped(1, MARKET_FLAG.wipeTraderSwept), NO_CASHBACK, null),
        ).toHaveLength(1);
    });

    it('passes the pump UVA alone as the token leg off the pump venues where the market kept one (AIR-05)', async () => {
        // The program's TOKEN_LEG_LEN_UVA; the keeper's sweep_trader_names_the_token_leg_and_the_wipe_record.
        const [record] = await findShareEpochPda({ market: MARKET, epoch: 2 });
        const uva = await userVolumeAccumulator(PUMP_PROGRAM, fakeMarket().trader);
        for (const venue of [0, 4, 5, 6]) {
            const left = fakeMarket({ tokensFree: 0n, totalShares: 0n, flags: 0, venue, shareEpoch: 3, pumpUva: uva });
            const ix = await sweepTraderIx(MARKET, left, null);
            expect(ix.accounts, `venue ${venue}`).toHaveLength(11);
            expect(ix.accounts?.[10]).toEqual({ address: uva, role: AccountRole.READONLY });
            const live = await sweepTraderIx(MARKET, { ...left, flags: MARKET_FLAG.wipeLive }, record);
            expect(live.accounts?.slice(10)).toEqual([
                { address: uva, role: AccountRole.READONLY },
                { address: record, role: AccountRole.WRITABLE },
            ]);
            // Listed off pump (no pump UVA): no token leg.
            expect((await sweepTraderIx(MARKET, { ...left, pumpUva: NO_UVA }, null)).accounts).toHaveLength(10);
        }
    });

    it('claims the curve-era pump cashback before the sweep of a market that left pump (AIR-05)', async () => {
        const uva = await userVolumeAccumulator(PUMP_PROGRAM, fakeMarket().trader);
        // A full wipe on Raydium after the curve: the trader bit alone does not let a deposit in.
        const flags = MARKET_FLAG.wipeLive | MARKET_FLAG.wipeTraderSwept;
        const left = fakeMarket({ tokensFree: 0n, totalShares: 0n, flags, venue: 4, shareEpoch: 3, pumpUva: uva });
        const [record] = await findShareEpochPda({ market: MARKET, epoch: 2 });
        const ixs = await postWipeSweepIxs(PAYER, MARKET, left, { ...NO_CASHBACK, pumpClaimable: 7n }, record);
        expect(ixs.map((i) => i.programAddress)).toEqual([PUMP_PROGRAM, SHORTWIN_PROGRAM]);
        // The claim drains the UVA the sweep then reads (pump's claim is venue-independent).
        expect(ixs[0]?.accounts?.[1]).toEqual({ address: uva, role: AccountRole.WRITABLE });
        expect(ixs[1]?.accounts?.slice(10).map((a) => a.address)).toEqual([uva, record]);
        // The facts: off the pump venues only the pump UVA is read, its lamports above rent.
        const reads: Address[][] = [];
        const b64 = (bytes: number[]) => Buffer.from(bytes).toString('base64');
        const rpc = {
            getMultipleAccounts: (keys: Address[]) => {
                reads.push(keys);
                return {
                    send: async () => ({
                        value: [
                            { owner: PUMP_PROGRAM, lamports: 2_000_000n, data: [b64(new Array(40).fill(1)), 'base64'] },
                        ],
                    }),
                };
            },
            getMinimumBalanceForRentExemption: (len: bigint) => ({ send: async () => (len === 40n ? 1_169_280n : 0n) }),
        } as unknown as AppRpc;
        expect(await readSweepFacts(rpc, left)).toEqual({
            pumpClaimable: 830_720n,
            pammClaimable: 0n,
            pammUvaExists: true,
        });
        expect(reads).toEqual([[uva]]);
        // A market without a pump UVA off the pump venues has nothing to read.
        expect(await readSweepFacts(rpc, { ...left, pumpUva: NO_UVA })).toEqual(NO_CASHBACK);
        expect(reads).toHaveLength(1);
    });

    it('builds nothing when no sweep is pending', async () => {
        const swept = wiped(6, MARKET_FLAG.wipeLive | MARKET_FLAG.wipeTraderSwept);
        expect(await postWipeSweepIxs(PAYER, MARKET, swept, NO_CASHBACK, null)).toEqual([]);
        expect(await postWipeSweepIxs(PAYER, MARKET, fakeMarket(), NO_CASHBACK, null)).toEqual([]);
    });

    it('reads the cashback facts: pump UVA above rent, pAMM WSOL, pAMM UVA presence', async () => {
        const m = wiped(3);
        const b64 = (bytes: number[]) => Buffer.from(bytes).toString('base64');
        const token = new Array<number>(165).fill(0);
        token[64] = 77;
        const rpc = {
            getMultipleAccounts: () => ({
                send: async () => ({
                    value: [
                        { owner: PUMP_PROGRAM, lamports: 3_000_000n, data: [b64(new Array(40).fill(1)), 'base64'] },
                        { owner: PUMP_AMM_PROGRAM, lamports: 1n, data: [b64([1, 2, 3]), 'base64'] },
                        { owner: TOKEN_PROGRAM, lamports: 1n, data: [b64(token), 'base64'] },
                    ],
                }),
            }),
            getMinimumBalanceForRentExemption: (len: bigint) => ({ send: async () => (len === 40n ? 1_169_280n : 0n) }),
        } as unknown as AppRpc;
        expect(await readSweepFacts(rpc, m)).toEqual({
            pumpClaimable: 1_830_720n,
            pammClaimable: 77n,
            pammUvaExists: true,
        });
        expect(await readSweepFacts(rpc, wiped(6))).toEqual(NO_CASHBACK);
    });
});
