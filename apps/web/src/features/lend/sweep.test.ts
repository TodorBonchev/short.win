/**
 * The post-wipe sweep bundle (FRONTEND.md §3.6, §10.1 "the sync / sweep prepends") against the
 * keeper's `services/keeper/src/sweep.rs`: the same instructions, accounts, roles and order per
 * venue and per cashback state, and the venue discriminators as Anchor derives them.
 */
import { createHash } from 'node:crypto';
import { findShareEpochPda, identifyShortwinInstruction, ShortwinInstruction } from '@shortwin/client';
import { AccountRole, address, createNoopSigner, getProgramDerivedAddress } from '@solana/kit';
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
import { fakeMarket } from './testkit.ts';

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
