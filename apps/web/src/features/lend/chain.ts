/**
 * The chain reads behind the lend panel and the lender dashboard (FRONTEND.md §6.5, FE-D1): every
 * number a lender transaction depends on comes from these accounts at `confirmed`, decoded with
 * `@shortwin/client` — `Market`, `MarketVault`, `GlobalConfig`, the owner's `LenderPosition`, the
 * `ShareEpoch` records it is behind on, the mint, the owner's token accounts of the mint (CPI Guard
 * and memo flags parsed per account), the wallet's SOL and the cluster's `Clock` — in three RPC
 * round trips (`getMultipleAccounts`, the records, `getTokenAccountsByOwner`).
 *
 * Time (M9-UX-5): the lender previews (claimable, credited, vesting, the vest a withdrawal gives
 * up, locks, the allowance) use the cluster clock the program reads — `Clock::unix_timestamp` at
 * the read plus the wall time elapsed since (`chainNowSeconds`) — never the device's clock alone.
 */
import {
    decodeGlobalConfig,
    decodeLenderPosition,
    decodeMarket,
    decodeMarketVault,
    findConfigPda,
    findLenderPda,
    findMarketPda,
    findMarketVaultPda,
    findShareEpochPda,
    type GlobalConfig,
    getShareEpochDecoder,
    type LenderPosition,
    type Market,
    type MarketVault,
    type ShareEpoch,
} from '@shortwin/client';
import { type Address, type Base58EncodedBytes, getBase64Encoder, parseBase64RpcAccount } from '@solana/kit';
import { findAssociatedTokenPda } from '@solana-program/token-2022';
import { qk } from '../../api/keys.ts';
import { SHORTWIN_PROGRAM } from '../../config/programs.ts';
import type { AppRpc } from '../../rpc/client.ts';
import { type MintInfo, parseMint } from '../../rpc/mint.ts';
import { readOwnerTokenAccounts } from '../../rpc/tokens.ts';
import type { TokenAccountInfo } from '../../tx/tokenAccount.ts';
import { CLOCK_SYSVAR } from '../../venues/fetch.ts';
import { wipeLive } from './previews.ts';

/**
 * The most `ShareEpoch` records one lender transaction carries (32 bytes each in the account
 * list, outside the ALT). A lender further behind is advanced with `sync_lender` first, in steps
 * of this size (ARCH §3.3). 8 since KI-65: a conversion through a rebase record also writes the
 * record's `conv_rem`, and with the record's SOL credit accrued each record costs ≈ 4.7–5.2k CU
 * (`tests/bench/m1_cu`, records that credit the lender). `claim_sol` / `cover_deficit` /
 * `sync_lender` stay within ARCH §6.1's 60,000 CU up to 8 records (≤ 54,549); 9 leaves 306 CU
 * (`cover_deficit` 59,694) and 10 exceeds it (≈ 64.8k).
 */
export const MAX_RECORDS = 8;

/**
 * The most records an owner's own transaction carries while its position has a forward chain
 * (`LenderPosition.fwd ≠ 0`, KI-66 full): settling the chain at a lag of 1 at every record costs
 * `lag_part`'s product twice per record, 64,355 CU for `cover_deficit` over 8 records and 58,229
 * over 7 (ARCH §6.1, `tests/bench/m1_cu` `behind_lag1`), so a chained owner settles 7 at a time.
 */
export const MAX_RECORDS_CHAINED = 7;

/** The records the owner's own transaction carries for `lender` (`MAX_RECORDS`, 7 with a forward chain). */
export function recordLimitOf(lender: Pick<LenderPosition, 'fwd'> | null): number {
    return lender && lender.fwd !== 0 ? MAX_RECORDS_CHAINED : MAX_RECORDS;
}

const COMMITMENT = { commitment: 'confirmed' } as const;
const b64 = getBase64Encoder();

/** Query keys of the LEND track, under the scaffold's `chain` roots (so `qk.marketAccount(mint)` invalidates them). */
export const lendKeys = {
    state: (mint: string, owner: string | null) => [...qk.marketAccount(mint), 'lend', owner ?? 'none'] as const,
    lenders: (mint: string) => [...qk.marketAccount(mint), 'lend', 'lenders'] as const,
    onChainPositions: (owner: string) => [...qk.chain, 'lender-positions', owner] as const,
};

export interface LendChain {
    readonly mint: Address;
    readonly marketAddress: Address;
    readonly vaultAddress: Address;
    readonly market: Market | null;
    readonly vault: MarketVault | null;
    readonly config: GlobalConfig | null;
    readonly mintInfo: MintInfo | null;
    readonly owner: Address | null;
    readonly lenderAddress: Address | null;
    readonly lender: LenderPosition | null;
    /** The records from the lender's epoch on (at most `recordLimitOf(lender)`), in order. */
    readonly records: readonly ShareEpoch[];
    /** Whether `records` reaches the market's current epoch (else `sync_lender` first). */
    readonly recordsComplete: boolean;
    /** The owner's token accounts of the mint, ATA first (empty without an owner). */
    readonly tokenAccounts: readonly TokenAccountInfo[];
    /** `ATA(owner, mint)` under the market's token program. */
    readonly ata: Address | null;
    readonly walletLamports: bigint;
    /** The RPC context slot of the account reads. */
    readonly slot: bigint;
    /** Wall clock of the read (ms). */
    readonly readAt: number;
    /** The cluster's `Clock::unix_timestamp` at the read (`null`: not readable). */
    readonly clockUnix?: bigint | null;
}

const SYSVAR_OWNER = 'Sysvar1111111111111111111111111111111111111';

/** `Clock::unix_timestamp` (i64 LE at offset 32) of a base64 `Clock` sysvar account. */
export function clockUnixOf(acc: { owner: string; data: readonly [string, string] | string[] } | null): bigint | null {
    if (!acc || acc.owner !== SYSVAR_OWNER) return null;
    const bytes = b64.encode(String(acc.data[0] ?? ''));
    if (bytes.length < 40) return null;
    return new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).getBigInt64(32, true);
}

/**
 * Seconds of cluster time now: the chain's clock at the read plus the wall time elapsed since,
 * the same estimate the Short side uses (`useChainNow`). Without a readable clock, the wall clock.
 */
export function chainNowSeconds(chain: Pick<LendChain, 'clockUnix' | 'readAt'>, wallMs: number): number {
    if (chain.clockUnix === undefined || chain.clockUnix === null) return Math.floor(wallMs / 1000);
    const elapsed = Math.max(0, Math.floor((wallMs - chain.readAt) / 1000));
    return Number(chain.clockUnix) + elapsed;
}

export interface ReadOptions {
    /** Read the owner's token accounts (the panel; the dashboard does not need them). */
    readonly tokenAccounts?: boolean;
}

/** The epochs whose records a lender at `fromEpoch` needs in a market at `shareEpoch` (live wipe included). */
export function neededEpochs(fromEpoch: number, shareEpoch: number, limit = MAX_RECORDS): number[] {
    const out: number[] = [];
    for (let e = fromEpoch; e < shareEpoch && out.length < limit; e++) out.push(e);
    return out;
}

/**
 * The records a third party's `sync_lender` of `l` carries (KI-66 full; ARCH §3.3, ops item 13): from
 * its `fwdEpoch` once it has a forward chain (the pot-only touch continues the chain there), else
 * from its `epoch`, at most `MAX_RECORDS` (the pot-only touch writes no record, so the owner's lag-1
 * cost does not apply). Empty for a current lender or a chain that reached the share epoch: the
 * touch still accrues the current epoch.
 */
export function potEpochs(l: Pick<LenderPosition, 'fwd' | 'fwdEpoch' | 'epoch'>, shareEpoch: number): number[] {
    return neededEpochs(l.fwd !== 0 ? l.fwdEpoch : l.epoch, shareEpoch);
}

/** Reads the `ShareEpoch` records of `epochs` (missing or foreign accounts are dropped). */
export async function readRecords(rpc: AppRpc, market: Address, epochs: readonly number[]): Promise<ShareEpoch[]> {
    if (!epochs.length) return [];
    const addresses = await Promise.all(epochs.map(async (epoch) => (await findShareEpochPda({ market, epoch }))[0]));
    const { value } = await rpc.getMultipleAccounts(addresses, { encoding: 'base64', ...COMMITMENT }).send();
    const dec = getShareEpochDecoder();
    const out: ShareEpoch[] = [];
    value.forEach((acc) => {
        if (!acc || acc.owner !== SHORTWIN_PROGRAM) return;
        try {
            out.push(dec.decode(new Uint8Array(b64.encode(acc.data[0]))));
        } catch {
            // not a record: the conversion will refuse with LenderBehindEpochs
        }
    });
    return out;
}

/** Everything the lend panel shows for `mint`, as the chain has it now. */
export async function readLendChain(
    rpc: AppRpc,
    mint: Address,
    owner: Address | null,
    opts: ReadOptions = {},
): Promise<LendChain> {
    const [marketAddress] = await findMarketPda({ mint });
    const [vaultAddress] = await findMarketVaultPda({ market: marketAddress });
    const [configAddress] = await findConfigPda();
    const lenderAddress = owner ? (await findLenderPda({ market: marketAddress, owner }))[0] : null;
    const keys: Address[] = [marketAddress, vaultAddress, configAddress, mint];
    if (lenderAddress && owner) keys.push(lenderAddress, owner);
    const clockIndex = keys.push(CLOCK_SYSVAR) - 1;
    const { context, value } = await rpc.getMultipleAccounts(keys, { encoding: 'base64', ...COMMITMENT }).send();
    const readAt = Date.now();
    const clockUnix = clockUnixOf((value[clockIndex] ?? null) as Parameters<typeof clockUnixOf>[0]);
    const at = (i: number) => parseBase64RpcAccount(keys[i] as Address, value[i] ?? null);
    const marketAcc = decodeMarket(at(0));
    const vaultAcc = decodeMarketVault(at(1));
    const configAcc = decodeGlobalConfig(at(2));
    const mintRaw = value[3];
    const mintInfo = mintRaw ? parseMint(mint, mintRaw.owner, new Uint8Array(b64.encode(mintRaw.data[0]))) : null;
    const lenderAcc = lenderAddress ? decodeLenderPosition(at(4)) : null;
    const walletLamports = owner ? BigInt(value[5]?.lamports ?? 0n) : 0n;

    const market = marketAcc.exists ? marketAcc.data : null;
    const lender = lenderAcc?.exists ? lenderAcc.data : null;
    let records: ShareEpoch[] = [];
    let recordsComplete = true;
    if (market && lender && lender.epoch < market.shareEpoch) {
        const epochs = neededEpochs(lender.epoch, market.shareEpoch, recordLimitOf(lender));
        records = await readRecords(rpc, marketAddress, epochs);
        recordsComplete = records.length === epochs.length && epochs[epochs.length - 1] === market.shareEpoch - 1;
    }

    let tokenAccounts: TokenAccountInfo[] = [];
    let ata: Address | null = null;
    if (owner && market) {
        [ata] = await findAssociatedTokenPda({ owner, mint, tokenProgram: market.tokenProgram });
        if (opts.tokenAccounts) tokenAccounts = await readOwnerTokenAccounts(rpc, owner, mint, ata);
    }
    return {
        mint,
        marketAddress,
        vaultAddress,
        market,
        vault: vaultAcc.exists ? vaultAcc.data : null,
        config: configAcc.exists ? configAcc.data : null,
        mintInfo,
        owner,
        lenderAddress,
        lender,
        records,
        recordsComplete,
        tokenAccounts,
        ata,
        walletLamports,
        slot: context.slot,
        readAt,
        clockUnix,
    };
}

/** The program's `market_now` for `deposit` / `withdraw`: `max(clock, last_accrual_ts)`. */
export function marketNow(market: Pick<Market, 'lastAccrualTs'> | null, nowS: number): bigint {
    const t = BigInt(nowS);
    return market && market.lastAccrualTs > t ? market.lastAccrualTs : t;
}

/** Whether the latest record is a live full-wipe record (`sweep_trader` names it). */
export function liveWipeRecordEpoch(market: Market): number | null {
    return wipeLive(market) && market.shareEpoch > 0 ? market.shareEpoch - 1 : null;
}

/** The markets (and their mints) `owner` lends in, from the chain alone (dashboard fallback). */
export async function readOnChainLenderMarkets(
    rpc: AppRpc,
    owner: Address,
    size: number,
): Promise<{ market: Address; mint: Address }[]> {
    const found = await findLenderPositionsOnChain(rpc, owner, size);
    if (!found.length) return [];
    const markets = found.map((f) => f.lender.market);
    const { value } = await rpc.getMultipleAccounts(markets, { encoding: 'base64', ...COMMITMENT }).send();
    const out: { market: Address; mint: Address }[] = [];
    markets.forEach((market, i) => {
        const d = decodeMarket(parseBase64RpcAccount(market, value[i] ?? null));
        if (d.exists) out.push({ market, mint: d.data.mint });
    });
    return out;
}

/**
 * The `LenderPosition`s of `owner` found by `getProgramAccounts` (the dashboard's fallback while the
 * indexer is unavailable; FRONTEND.md §6.5): a `dataSize` filter and a `memcmp` on the owner field
 * (offset 8, after the discriminator).
 */
export async function findLenderPositionsOnChain(
    rpc: AppRpc,
    owner: Address,
    size: number,
): Promise<{ address: Address; lender: LenderPosition }[]> {
    const res = await rpc
        .getProgramAccounts(SHORTWIN_PROGRAM, {
            encoding: 'base64',
            ...COMMITMENT,
            filters: [
                { dataSize: BigInt(size) },
                { memcmp: { offset: 8n, bytes: owner as string as Base58EncodedBytes, encoding: 'base58' } },
            ],
        })
        .send();
    const list = Array.isArray(res) ? res : (res as { value: typeof res }).value;
    const out: { address: Address; lender: LenderPosition }[] = [];
    for (const { pubkey, account } of list as readonly {
        pubkey: Address;
        account: Parameters<typeof parseBase64RpcAccount>[1];
    }[]) {
        const d = decodeLenderPosition(parseBase64RpcAccount(pubkey, account));
        if (d.exists) out.push({ address: pubkey, lender: d.data });
    }
    return out;
}
