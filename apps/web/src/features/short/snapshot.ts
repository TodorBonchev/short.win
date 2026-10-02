/**
 * SHORT track — one market as the chain holds it at one slot (FRONTEND.md §6.5, FE-D1): the
 * accounts every preview and transaction of the Short panel depends on, read through RPC at
 * `confirmed` in two batched `getMultipleAccounts` calls (the protocol accounts, the mint and the
 * `Clock` sysvar; then the venue's ReadSet and its `VenueExtras`), decoded with the Codama client
 * and the venue readers. Pure assembly (`assembleSnapshot`) is separate from the RPC so the tests
 * feed it the program's own account dumps.
 */
import {
    findConfigPda,
    findMarketPda,
    findOracleCachePda,
    findVenueGuardPda,
    getGlobalConfigDecoder,
    getMarketDecoder,
    getOracleCacheDecoder,
    getShortPositionDecoder,
    getVenueExtrasDecoder,
    getVenueGuardDecoder,
    getWalletExposureDecoder,
    type Market,
    podU128ToBigInt,
    type ShortPosition,
    type VenueExtras,
} from '@shortwin/client';
import type { Address } from '@solana/kit';
import { config as appConfig } from '../../config/env.ts';
import type { MathModule } from '../../math/load.ts';
import type { AppRpc } from '../../rpc/client.ts';
import { DEFAULT_KEY, i64At, u64At } from '../../venues/bytes.ts';
import { type KindName, kindName, SHORTWIN_PROGRAM } from '../../venues/constants.ts';
import { CLOCK_SYSVAR, fetchRaw } from '../../venues/fetch.ts';
import { extrasSeed, type MarketKeys, readSet } from '../../venues/lists.ts';
import { poolAuthority, venueExtras as venueExtrasPda } from '../../venues/pda.ts';
import { type RawAccount, readMintBase, value } from '../../venues/readers.ts';
import { notExecutable, readVenueState } from '../../venues/state.ts';
import type { MarketSnapshot } from './previews.ts';

export { CLOCK_SYSVAR };

/** A market snapshot with what a transaction needs besides the previews. */
export interface ChainSnapshot extends MarketSnapshot {
    readonly mint: Address;
    readonly marketAddress: Address;
    readonly tokenProgram: Address;
    readonly supply: bigint;
    /** The market with its derived-key caches filled (`pump_pool_authority` on old markets). */
    readonly keys: MarketKeys;
    /** The venue's ReadSet keys and accounts, in the adapter's order. */
    readonly readKeys: readonly Address[];
    readonly read: readonly (RawAccount | null)[];
    readonly extras: VenueExtras | null;
    /** `Date.now()` when the read returned (ticks displays between reads). */
    readonly fetchedAt: number;
}

export type AccountSource = (address: Address) => RawAccount | null;

/** Decodes `bytes` with `decoder`, `null` if the account is absent, foreign or does not decode. */
export function decodeOwned<T>(acc: RawAccount | null, decoder: { decode(b: Uint8Array): T }): T | null {
    if (!acc || acc.owner !== SHORTWIN_PROGRAM || acc.data.length === 0) return null;
    try {
        return decoder.decode(acc.data);
    } catch {
        return null;
    }
}

export function decodeMarket(acc: RawAccount | null): Market | null {
    return decodeOwned(acc, getMarketDecoder());
}

export function decodePosition(acc: RawAccount | null): ShortPosition | null {
    return decodeOwned(acc, getShortPositionDecoder());
}

/** `Clock`: `slot` @0, `unix_timestamp` @32. */
export function decodeClock(acc: RawAccount | null): { slot: bigint; unix: bigint } | null {
    if (!acc || acc.data.length < 40) return null;
    return { slot: u64At(acc.data, 0), unix: i64At(acc.data, 32) };
}

export interface AssembleInput {
    readonly mint: Address;
    readonly now: bigint;
    readonly slot: bigint;
    readonly get: AccountSource;
    readonly fetchedAt?: number;
    /**
     * FE-D7: the TEST-ONLY mock venue is priced only in builds with `VITE_ENABLE_TEST_VENUE`
     * (localnet); elsewhere a mock market reads as not executable. Defaults to the configuration.
     */
    readonly allowTestVenue?: boolean;
}

/** The protocol accounts of a market (first batch). */
export async function protocolKeys(mint: Address): Promise<Address[]> {
    const [[config], [guard], [oracle], [market]] = await Promise.all([
        findConfigPda(),
        findVenueGuardPda(),
        findOracleCachePda(),
        findMarketPda({ mint }),
    ]);
    return [config, guard, oracle, market, mint, CLOCK_SYSVAR];
}

/** The venue accounts of a decoded market (second batch): its ReadSet, then its `VenueExtras`. */
export async function venueKeys(
    market: MarketKeys,
    kind: KindName,
): Promise<{ read: Address[]; extras: Address | null }> {
    const seed = extrasSeed(kind);
    const [read, extras] = await Promise.all([readSet(kind, market), seed === null ? null : venueExtrasPda(seed)]);
    return { read, extras };
}

/** Fills the market's derived-key caches a market listed before them may lack. */
export async function withDerivedKeys(m: Market): Promise<Market> {
    if (m.pumpPoolAuthority !== DEFAULT_KEY) return m;
    return { ...m, pumpPoolAuthority: await poolAuthority(m.mint) };
}

/**
 * Assembles the snapshot from accounts already read (`get`); `null` without a config or a
 * market. The venue accounts must be in `get` too (`venueKeys`).
 */
export async function assembleSnapshot(math: MathModule, inp: AssembleInput): Promise<ChainSnapshot | null> {
    const [config, guard, oracle, marketKey] = await protocolKeys(inp.mint);
    const cfg = decodeOwned(inp.get(config as Address), getGlobalConfigDecoder());
    const raw = decodeMarket(inp.get(marketKey as Address));
    if (!cfg || !raw) return null;
    const market = await withDerivedKeys(raw);
    const kind = kindName(market.venue);
    if (!kind) return null;
    const mintAcc = inp.get(inp.mint);
    const mintBase = mintAcc ? value(readMintBase(mintAcc.owner, mintAcc.data)) : null;
    const { read: readKeys, extras: extrasKey } = await venueKeys(market, kind);
    const read = readKeys.map((k) => inp.get(k));
    const pEma = podU128ToBigInt(market.emaPrice);
    const testVenueOff = kind === 'mock' && !(inp.allowTestVenue ?? appConfig.enableTestVenue);
    const venue = testVenueOff
        ? notExecutable('mock')
        : readVenueState(math, kind, market, read, {
              now: inp.now,
              supply: mintBase?.supply ?? null,
              pEma,
          });
    return {
        now: inp.now,
        slot: inp.slot,
        config: cfg,
        guard: decodeOwned(inp.get(guard as Address), getVenueGuardDecoder()),
        oracle: decodeOwned(inp.get(oracle as Address), getOracleCacheDecoder()),
        market,
        kind,
        venue,
        decimals: mintBase?.decimals ?? market.decimals,
        mint: inp.mint,
        marketAddress: marketKey as Address,
        tokenProgram: market.tokenProgram,
        supply: mintBase?.supply ?? 0n,
        keys: market,
        readKeys,
        read,
        extras: extrasKey ? decodeOwned(inp.get(extrasKey), getVenueExtrasDecoder()) : null,
        fetchedAt: inp.fetchedAt ?? Date.now(),
    };
}

// ---- RPC ----------------------------------------------------------------------------------------

export { fetchRaw };

/** Reads and assembles the snapshot of `mint`'s market (two RPC round trips). */
export async function loadSnapshot(
    rpc: AppRpc,
    math: MathModule,
    mint: Address,
    extra: readonly Address[] = [],
    signal?: AbortSignal,
    opts: { readonly allowTestVenue?: boolean } = {},
): Promise<{ snapshot: ChainSnapshot | null; get: AccountSource }> {
    const keys = await protocolKeys(mint);
    const first = await fetchRaw(rpc, [...keys, ...extra], signal);
    const marketKey = keys[3] as Address;
    const raw = decodeMarket(first.accounts.get(marketKey) ?? null);
    const all = new Map(first.accounts);
    if (raw) {
        const market = await withDerivedKeys(raw);
        const kind = kindName(market.venue);
        if (kind) {
            const { read, extras } = await venueKeys(market, kind);
            const second = await fetchRaw(rpc, extras ? [...read, extras] : read, signal);
            for (const [k, v] of second.accounts) all.set(k, v);
        }
    }
    const clock = decodeClock(all.get(CLOCK_SYSVAR) ?? null);
    const get: AccountSource = (a) => all.get(a) ?? null;
    const snapshot = await assembleSnapshot(math, {
        mint,
        now: clock?.unix ?? BigInt(Math.floor(Date.now() / 1000)),
        slot: clock?.slot ?? first.slot,
        get,
        fetchedAt: Date.now(),
        ...(opts.allowTestVenue === undefined ? {} : { allowTestVenue: opts.allowTestVenue }),
    });
    return { snapshot, get };
}

/** The wallet's open quantity in a market (`WalletExposure.q_open`; 0 without a record). */
export function exposureQ(acc: RawAccount | null): bigint {
    return decodeOwned(acc, getWalletExposureDecoder())?.qOpen ?? 0n;
}
