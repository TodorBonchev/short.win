/**
 * SHORT track — the venue layer (FRONTEND.md §5.1), behind the frozen interface of `types.ts`:
 * `detectVenue` and `raydiumCandidates` (create market, MARKETS) and `tradeContext` (the trading
 * instructions' remaining accounts). The building blocks live beside it: `readers.ts` (the
 * venue-state readers), `lists.ts` (the keeper's account lists), `state.ts` (the venue predicate,
 * reserves, fees and quotes), `detect.ts` (pump detection), `candidates.ts` (Raydium pools).
 *
 * Each function takes an optional trailing `payer` (the connected wallet's signer): the
 * preparations a venue needs before a listing or a trade (pump `extend_account`, PumpSwap's
 * `create_idempotent(T11)`) are signed by it, so without it they are left out and the refusal says
 * why. The frozen three-argument calls stay valid.
 */
import { findConfigPda, findVenueGuardPda, getGlobalConfigDecoder, getVenueGuardDecoder } from '@shortwin/client';
import { type AccountMeta, AccountRole, type Address, type TransactionSigner } from '@solana/kit';
import { config } from '../config/env.ts';
import { loadSnapshot } from '../features/short/snapshot.ts';
import { loadMath } from '../math/load.ts';
import type { AppRpc } from '../rpc/client.ts';
import { discoverCandidates } from './candidates.ts';
import { EXTRAS_IX, SHORTWIN_PROGRAM } from './constants.ts';
import { detectFrom } from './detect.ts';
import { CLOCK_SYSVAR, fetchRaw } from './fetch.ts';
import { extrasCandidates, NO_EXTRAS, pammCreatorVaultPrep, readMetas, tradeExtras } from './lists.ts';
import { bondingCurve, canonicalPool, mockPool, poolAuthority } from './pda.ts';
import { type RawAccount, readMintBase, readMockPool, value } from './readers.ts';
import type { PoolCandidate, TradeContext, VenueDetection } from './types.ts';

export * from './types.ts';

/** What an `auto` listing of `mint` would detect (pump curve / migrating / PumpSwap / none). */
export async function detectVenue(rpc: AppRpc, mint: Address, payer?: TransactionSigner): Promise<VenueDetection> {
    const authority = await poolAuthority(mint);
    const [bc, cp] = await Promise.all([bondingCurve(mint), canonicalPool(mint, authority)]);
    const { accounts } = await fetchRaw(rpc, [bc, cp]);
    return detectFrom(
        mint,
        { bondingCurve: bc, canonicalPool: cp, authority },
        accounts.get(bc) ?? null,
        accounts.get(cp) ?? null,
        payer,
    );
}

/** The mint's Raydium AMM v4 / CPMM SOL pools with their eligibility (create market); the best first. */
export async function raydiumCandidates(rpc: AppRpc, mint: Address): Promise<PoolCandidate[]> {
    const math = await loadMath();
    const [[config], [guard], authority] = await Promise.all([
        findConfigPda(),
        findVenueGuardPda(),
        poolAuthority(mint),
    ]);
    const [bc, cp] = await Promise.all([bondingCurve(mint), canonicalPool(mint, authority)]);
    const { accounts } = await fetchRaw(rpc, [config, guard, mint, CLOCK_SYSVAR]);
    const cfgAcc = accounts.get(config) ?? null;
    const guardAcc = accounts.get(guard) ?? null;
    if (!cfgAcc || cfgAcc.owner !== SHORTWIN_PROGRAM) throw new Error('short.win is not initialised on this cluster');
    const cfg = getGlobalConfigDecoder().decode(cfgAcc.data);
    const g = guardAcc && guardAcc.owner === SHORTWIN_PROGRAM ? getVenueGuardDecoder().decode(guardAcc.data) : null;
    const mintAcc = accounts.get(mint) ?? null;
    const base = mintAcc ? value(readMintBase(mintAcc.owner, mintAcc.data)) : null;
    if (!mintAcc || !base) return [];
    const clock = accounts.get(CLOCK_SYSVAR);
    const now =
        clock && clock.data.length >= 40
            ? new DataView(clock.data.buffer, clock.data.byteOffset).getBigInt64(32, true)
            : BigInt(Math.floor(Date.now() / 1000));
    return discoverCandidates(
        rpc,
        math,
        mint,
        mintAcc.owner,
        {
            venueFeeCapBps: cfg.params.venueFeeCapBps,
            poolMinRealSol: cfg.params.poolMinRealSol,
            poolLockMinBps: cfg.params.poolLockMinBps,
        },
        g?.upgraded ?? 0,
        now,
        [
            { address: bc, role: AccountRole.READONLY },
            { address: cp, role: AccountRole.READONLY },
        ],
    );
}

/**
 * The venue accounts and reserves for a trade on the market of `mint` (`sell` for an open, `buy`
 * for an exit): `ReadSet ++ TradeExtras ++ Extras(current)`, with the slots the program checks
 * against live state read at the same slot.
 */
export async function tradeContext(
    rpc: AppRpc,
    mint: Address,
    side: 'sell' | 'buy',
    payer?: TransactionSigner,
): Promise<TradeContext> {
    const math = await loadMath();
    const { snapshot: s } = await loadSnapshot(rpc, math, mint);
    if (!s) throw new Error(`no market for ${mint}`);
    const candidates = await extrasCandidates(
        s.kind,
        side === 'sell' ? EXTRAS_IX.sell : EXTRAS_IX.buy,
        s.keys,
        s.extras,
    );
    const extras = candidates[0] ?? NO_EXTRAS;
    const remainingAccounts = [
        ...readMetas(s.kind, s.readKeys, true),
        ...(await tradeExtras(s.kind, s.keys, s.read)),
        ...extras.metas,
    ];
    const prep = payer ? await pammCreatorVaultPrep(payer, s.keys, s.kind, s.read) : null;
    return {
        venue: s.kind,
        remainingAccounts,
        preInstructions: prep ? [prep] : [],
        reserves: { x: s.venue.x, y: s.venue.y },
        feePpm: s.venue.feePpm,
        slot: s.slot,
        extras,
        extrasCandidates: candidates,
    };
}

/**
 * TEST-ONLY (FE-D7; `VITE_ENABLE_TEST_VENUE` on localnet): the remaining accounts of
 * `create_market(venue_hint = 6)` for `mint`'s mock pool — the detection set, then the mock
 * ReadSet `[pool, base vault, quote vault]` read-only (`venues::mock::adopt`). `null` when the mock
 * venue is disabled in this build or the mint has no mock pool.
 */
export async function mockListing(
    rpc: AppRpc,
    mint: Address,
): Promise<{ readonly pool: Address; readonly remainingAccounts: readonly AccountMeta[] } | null> {
    if (!config.enableTestVenue) return null;
    const authority = await poolAuthority(mint);
    const [bc, cp, pool] = await Promise.all([bondingCurve(mint), canonicalPool(mint, authority), mockPool(mint)]);
    const { accounts } = await fetchRaw(rpc, [pool]);
    return mockListingFrom(bc, cp, accounts.get(pool) ?? null);
}

/** `mockListing` on the pool account as read. */
export function mockListingFrom(
    bondingCurveKey: Address,
    canonicalPoolKey: Address,
    pool: RawAccount | null,
): { readonly pool: Address; readonly remainingAccounts: readonly AccountMeta[] } | null {
    const p = pool ? value(readMockPool(pool.owner, pool.data)) : null;
    if (!pool || !p) return null;
    const ro = (address: Address): AccountMeta => ({ address, role: AccountRole.READONLY });
    return {
        pool: pool.address,
        remainingAccounts: [
            ro(bondingCurveKey),
            ro(canonicalPoolKey),
            ro(pool.address),
            ro(p.baseVault),
            ro(p.quoteVault),
        ],
    };
}
