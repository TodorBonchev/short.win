/**
 * The lend panel's chain reads (FRONTEND.md §6.5, FE-D1) over a scripted RPC: the accounts are
 * decoded with `@shortwin/client`, the `ShareEpoch` records a lender is behind on are read in
 * order (and their absence noticed), the owner's token accounts come ATA first with their CPI
 * Guard / memo flags, and `market_now` is `max(clock, last_accrual_ts)`.
 */
import {
    findLenderPda,
    findMarketPda,
    findShareEpochPda,
    getLenderPositionEncoder,
    getMarketDecoder,
    getMarketEncoder,
    getMarketSize,
    getMarketVaultDecoder,
    getMarketVaultEncoder,
    getMarketVaultSize,
    getShareEpochEncoder,
    MARKET_DISCRIMINATOR,
    MARKET_VAULT_DISCRIMINATOR,
} from '@shortwin/client';
import { type Address, address, getAddressEncoder, parseBase64RpcAccount } from '@solana/kit';
import { findAssociatedTokenPda } from '@solana-program/token-2022';
import { describe, expect, it } from 'vitest';
import { SHORTWIN_PROGRAM, SYSTEM_PROGRAM, TOKEN_2022_PROGRAM } from '../../config/programs.ts';
import type { AppRpc } from '../../rpc/client.ts';
import {
    lenderPositionOf,
    lendKeys,
    liveWipeRecordEpoch,
    MAX_RECORDS,
    marketNow,
    neededEpochs,
    readLendChain,
} from './chain.ts';
import { MARKET_FLAG } from './previews.ts';

const MINT = address('EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v');
const OWNER = address('9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin');
const OTHER_ACCT = address('JUPyiwrYJFskUPiHa7hkeR8VUtAeFoSYbKedZNsDvCN');

const b64 = (bytes: Uint8Array | readonly number[]) => Buffer.from(bytes as Uint8Array).toString('base64');
const rpcAccount = (owner: Address, data: Uint8Array | readonly number[], lamports = 1_000_000n) => ({
    owner,
    lamports,
    executable: false,
    rentEpoch: 0n,
    space: BigInt(data.length),
    data: [b64(data), 'base64'] as const,
});

function marketBytes(shareEpoch: number, flags = 0): Uint8Array {
    const zero = new Uint8Array(getMarketSize());
    zero.set(MARKET_DISCRIMINATOR, 0);
    const m = getMarketDecoder().decode(zero);
    return new Uint8Array(
        getMarketEncoder().encode({
            ...m,
            shareEpoch,
            flags,
            decimals: 6,
            tokenProgram: TOKEN_2022_PROGRAM,
            mint: MINT,
        }),
    );
}

function vaultBytes(): Uint8Array {
    const zero = new Uint8Array(getMarketVaultSize());
    zero.set(MARKET_VAULT_DISCRIMINATOR, 0);
    return new Uint8Array(getMarketVaultEncoder().encode(getMarketVaultDecoder().decode(zero)));
}

function lenderBytes(market: Address, epoch: number): Uint8Array {
    return new Uint8Array(
        getLenderPositionEncoder().encode({
            owner: OWNER,
            market,
            shares: 5n,
            snap: 0n,
            pending: 0n,
            snapR: 0n,
            real: 0n,
            claimed: 0n,
            epoch,
            lock: 0n,
            lockPi: 10n ** 18n,
            lockEpoch: 4_294_967_295,
            lockTerm: 0n,
            lockUntil: 0n,
            bump: 254,
            fwdShares: 0n,
            fwdEpoch: 0,
            fwd: 0,
            fwdLag: 0,
            reserved: new Uint8Array(10),
        }),
    );
}

/** A Token-2022 account of MINT owned by OWNER; `ext` = TLV entries `[type, value]`. */
function tokenBytes(amount: bigint, ext: [number, number][] = []): Uint8Array {
    const base = new Uint8Array(165);
    base.set(getAddressEncoder().encode(MINT), 0);
    base.set(getAddressEncoder().encode(OWNER), 32);
    new DataView(base.buffer).setBigUint64(64, amount, true);
    base[108] = 1; // initialized
    if (!ext.length) return base;
    const tlv = ext.flatMap(([type, v]) => [type, 0, 1, 0, v]);
    return new Uint8Array([...base, 2, ...tlv]);
}

interface Script {
    readonly lenderEpoch?: number;
    /** A system-owned, empty account at the lender PDA (someone sent it the rent-exempt minimum). */
    readonly fundedPda?: boolean;
    readonly shareEpoch?: number;
    readonly flags?: number;
    readonly missingRecord?: number;
}

async function fakeRpc(s: Script) {
    const [market] = await findMarketPda({ mint: MINT });
    const [ata] = await findAssociatedTokenPda({ owner: OWNER, mint: MINT, tokenProgram: TOKEN_2022_PROGRAM });
    const records = new Map<string, Uint8Array>();
    for (let e = 0; e < 40; e++) {
        if (e === s.missingRecord) continue;
        const [pda, bump] = await findShareEpochPda({ market, epoch: e });
        records.set(
            pda,
            new Uint8Array(
                getShareEpochEncoder().encode({
                    market,
                    epoch: e,
                    accEnd: BigInt(e),
                    accREnd: 0n,
                    sOld: 10n,
                    carry: 0n,
                    carryR: 0n,
                    num: 1,
                    denLog1000: 1,
                    bump,
                    convRem: 0n,
                }),
            ),
        );
    }
    const calls: string[][] = [];
    const rpc = {
        getMultipleAccounts: (keys: Address[]) => ({
            send: async () => {
                calls.push(keys);
                if (keys[0] === market) {
                    return {
                        context: { slot: 77n },
                        value: [
                            rpcAccount(SHORTWIN_PROGRAM, marketBytes(s.shareEpoch ?? 0, s.flags ?? 0)),
                            rpcAccount(SHORTWIN_PROGRAM, vaultBytes()),
                            null,
                            rpcAccount(TOKEN_2022_PROGRAM, [...new Uint8Array(44), 6, 1, ...new Uint8Array(36)]),
                            s.fundedPda
                                ? rpcAccount(SYSTEM_PROGRAM, [], 890_880n)
                                : s.lenderEpoch === undefined
                                  ? null
                                  : rpcAccount(SHORTWIN_PROGRAM, lenderBytes(market, s.lenderEpoch)),
                            rpcAccount(SYSTEM_PROGRAM, [], 3_000_000_000n),
                        ],
                    };
                }
                return {
                    context: { slot: 77n },
                    value: keys.map((k) => {
                        const r = records.get(k);
                        return r ? rpcAccount(SHORTWIN_PROGRAM, r) : null;
                    }),
                };
            },
        }),
        getTokenAccountsByOwner: () => ({
            send: async () => ({
                value: [
                    { pubkey: OTHER_ACCT, account: rpcAccount(TOKEN_2022_PROGRAM, tokenBytes(900n, [[11, 1]])) },
                    { pubkey: ata, account: rpcAccount(TOKEN_2022_PROGRAM, tokenBytes(5n, [[8, 1]])) },
                ],
            }),
        }),
    } as unknown as AppRpc;
    return { rpc, calls, market, ata };
}

describe('lend chain reads', () => {
    it('reads the market, the vault, the lender, the wallet and the token accounts (ATA first)', async () => {
        const { rpc, market, ata } = await fakeRpc({ lenderEpoch: 0 });
        const c = await readLendChain(rpc, MINT, OWNER, { tokenAccounts: true });
        expect(c.marketAddress).toBe(market);
        expect(c.market?.decimals).toBe(6);
        expect(c.market?.tokenProgram).toBe(TOKEN_2022_PROGRAM);
        expect(c.vault).not.toBeNull();
        expect(c.config).toBeNull();
        expect(c.mintInfo?.decimals).toBe(6);
        expect(c.lenderAddress).toBe((await findLenderPda({ market, owner: OWNER }))[0]);
        expect(c.lender?.shares).toBe(5n);
        expect(c.records).toEqual([]);
        expect(c.recordsComplete).toBe(true);
        expect(c.walletLamports).toBe(3_000_000_000n);
        expect(c.slot).toBe(77n);
        expect(c.ata).toBe(ata);
        expect(c.tokenAccounts.map((a) => [a.address, a.amount, a.cpiGuard, a.memoRequired])).toEqual([
            [ata, 5n, false, true],
            [OTHER_ACCT, 900n, true, false],
        ]);
    });

    it('reads the records a lender is behind on, in order, and notices a missing one', async () => {
        const behind = await fakeRpc({ lenderEpoch: 1, shareEpoch: 4 });
        const c = await readLendChain(behind.rpc, MINT, OWNER);
        expect(c.records.map((r) => r.epoch)).toEqual([1, 2, 3]);
        expect(c.recordsComplete).toBe(true);
        expect(c.tokenAccounts).toEqual([]);
        const gap = await fakeRpc({ lenderEpoch: 1, shareEpoch: 4, missingRecord: 2 });
        expect((await readLendChain(gap.rpc, MINT, OWNER)).recordsComplete).toBe(false);
        const far = await fakeRpc({ lenderEpoch: 0, shareEpoch: 30 });
        const f = await readLendChain(far.rpc, MINT, OWNER);
        expect(f.records).toHaveLength(MAX_RECORDS);
        expect(f.recordsComplete).toBe(false);
    });

    it('reads a system account someone funded at the lender PDA as no position', async () => {
        // Anyone can send 890,880 lamports to a lender PDA that does not exist (never created, or
        // closed by a claim or withdrawal, KI-67); `deposit` takes it over, so the panel must load.
        const { rpc } = await fakeRpc({ fundedPda: true });
        const c = await readLendChain(rpc, MINT, OWNER);
        expect(c.market).not.toBeNull();
        expect(c.lender).toBeNull();
        expect(c.records).toEqual([]);
        const at = (owner: Address, data: Uint8Array) =>
            parseBase64RpcAccount(
                OWNER,
                rpcAccount(owner, data) as unknown as Parameters<typeof parseBase64RpcAccount>[1],
            );
        const [market] = await findMarketPda({ mint: MINT });
        const good = lenderBytes(market, 0);
        expect(lenderPositionOf(at(SHORTWIN_PROGRAM, good))?.shares).toBe(5n);
        expect(lenderPositionOf(at(SYSTEM_PROGRAM, new Uint8Array()))).toBeNull();
        expect(lenderPositionOf(at(SYSTEM_PROGRAM, good))).toBeNull();
        expect(lenderPositionOf(at(SHORTWIN_PROGRAM, good.slice(0, 200)))).toBeNull();
        const foreign = good.slice();
        foreign[0] ^= 1;
        expect(lenderPositionOf(at(SHORTWIN_PROGRAM, foreign))).toBeNull();
        expect(lenderPositionOf(parseBase64RpcAccount(OWNER, null))).toBeNull();
    });

    it('reads no lender and no token accounts without a wallet', async () => {
        const { rpc, calls } = await fakeRpc({});
        const c = await readLendChain(rpc, MINT, null, { tokenAccounts: true });
        expect(c.lender).toBeNull();
        expect(c.lenderAddress).toBeNull();
        expect(c.ata).toBeNull();
        expect(c.tokenAccounts).toEqual([]);
        // market, vault, config, mint and the Clock sysvar (the previews run on cluster time, M9-UX-5).
        expect(calls[0]).toHaveLength(5);
        expect(calls[0]?.[4]).toBe('SysvarC1ock11111111111111111111111111111111');
    });

    it('computes the epochs, market_now, the live wipe record and the query keys', async () => {
        expect(neededEpochs(2, 5)).toEqual([2, 3, 4]);
        expect(neededEpochs(5, 5)).toEqual([]);
        expect(neededEpochs(0, 40)).toHaveLength(MAX_RECORDS);
        expect(marketNow({ lastAccrualTs: 2_000n }, 1_000)).toBe(2_000n);
        expect(marketNow({ lastAccrualTs: 2_000n }, 3_000)).toBe(3_000n);
        expect(marketNow(null, 5)).toBe(5n);
        const { rpc } = await fakeRpc({ shareEpoch: 3, flags: MARKET_FLAG.wipeLive });
        const c = await readLendChain(rpc, MINT, null);
        if (!c.market) throw new Error('no market');
        expect(liveWipeRecordEpoch(c.market)).toBe(2);
        expect(lendKeys.state(MINT, OWNER).slice(0, 3)).toEqual(['chain', 'market', MINT]);
    });
});
