/**
 * Raydium candidates of a mint for create-market (FRONTEND.md §3.5; ARCH §2.6): discovery as the
 * keeper does it (`services/keeper/src/pool_monitor.rs`: `getProgramAccounts` byte filters on the
 * two mint fields, both orientations, at the one account size the program verifies), then each
 * pool judged as `create_market(venue_hint = Raydium…)` would: `raydium_v4::verify` /
 * `raydium_cpmm::verify` (owner, length, status, nonce, the SOL and token sides, vaults, config,
 * LP mint), the venue predicate, and `eligible()` at the candidate's own spot (a new market has no
 * price): the fee gate and the locked-depth floor (the facade's `lockedFloorOk` =
 * `pool_select::locked_floor_ok`, M4-SEC-1 / -3; λ by its `lambdaBps`), plus the venue-upgrade
 * latch of a listing (M3-SEC-2).
 *
 * The pure part (`judgeCandidate`) takes the accounts as read, so it is tested on the dumped
 * Raydium fixtures.
 */
import { type AccountMeta, type Address, type Base58EncodedBytes, getBase64Encoder } from '@solana/kit';
import type { MathModule } from '../math/load.ts';
import type { AppRpc } from '../rpc/client.ts';
import {
    GUARD_MASK,
    MARKET_FLAG,
    RAY_CACHE,
    RAYDIUM_CPMM_AUTHORITY,
    RAYDIUM_CPMM_PROGRAM,
    RAYDIUM_V4_AUTHORITY,
    RAYDIUM_V4_PROGRAM,
    TOKEN_PROGRAM,
    WSOL_MINT,
} from './constants.ts';
import { fetchRaw } from './fetch.ts';
import { readMetas } from './lists.ts';
import {
    AMM_V4_LEN,
    CPMM_POOL_LEN,
    cpmmSolIsToken0,
    type RawAccount,
    readAmmInfo,
    readCpmmConfig,
    readCpmmPool,
    readMintBase,
    tokenOf,
    v4SolIsCoin,
    value,
} from './readers.ts';
import { readVenueState, spotPrice, type VenueState } from './state.ts';
import type { PoolCandidate } from './types.ts';

export interface EligibilityParams {
    readonly venueFeeCapBps: number;
    readonly poolMinRealSol: bigint;
    readonly poolLockMinBps: number;
}

/** The candidate's ReadSet keys from its pool account (the adapter's order). */
export function candidateReadKeys(kind: PoolCandidate['kind'], pool: RawAccount): Address[] | null {
    if (kind === 'raydium_v4') {
        const i = value(readAmmInfo(pool.owner, pool.data));
        return i ? [pool.address, i.coinVault, i.pcVault, i.lpMint] : null;
    }
    const p = value(readCpmmPool(pool.owner, pool.data));
    return p ? [pool.address, p.ammConfig, p.token0Vault, p.token1Vault, p.lpMint] : null;
}

const NO_ADOPTION = {
    state: null as VenueState | null,
    solFirst: false,
};

/**
 * Judges one candidate from its ReadSet accounts (`read`, the adapter's order) as `verify` and
 * `check_eligible` would for a listing at `now` (`p_ref` = its own spot). `upgradedMask` is
 * `VenueGuard.upgraded`.
 */
export function judgeCandidate(
    m: MathModule,
    kind: PoolCandidate['kind'],
    mint: Address,
    tokenProgram: Address,
    read: readonly (RawAccount | null)[],
    now: bigint,
    params: EligibilityParams,
    upgradedMask: number,
): Omit<PoolCandidate, 'remainingAccounts'> & { readonly state: VenueState | null } {
    const pool = read[0] ?? null;
    const reasons: string[] = [];
    const out = (s: Partial<{ state: VenueState | null; solFirst: boolean }> = NO_ADOPTION) => {
        const st = s.state ?? null;
        const lam = lambdaOf(m, kind, read);
        return {
            pool: (pool?.address ?? '') as Address,
            kind,
            solReserve: st?.y ?? 0n,
            lockedDepth: st?.lockedDepth ?? 0n,
            lambdaBps: lam,
            feePpm: st?.feePpm ?? 0,
            eligible: reasons.length === 0,
            reasons,
            state: st,
        };
    };
    if ((upgradedMask & (GUARD_MASK.raydium_v4 ?? 0)) !== 0) reasons.push('VenueUpgraded');
    const verified = kind === 'raydium_v4' ? verifyV4(read, mint, tokenProgram) : verifyCpmm(read, mint, tokenProgram);
    if (verified === null) {
        reasons.push('PoolIneligible');
        return out();
    }
    const market = candidateMarket(kind, mint, tokenProgram, read, verified.solFirst);
    const state = readVenueState(m, kind, market, read, { now, supply: null, pEma: 0n });
    if (!state.executable) {
        reasons.push('PoolIneligible');
        return out({ state });
    }
    if (m.exceedsCap(state.feePpm, params.venueFeeCapBps)) reasons.push('PoolIneligible');
    const pRef = spotPrice(m, state);
    if (pRef === null) reasons.push('PoolIneligible');
    else if (!m.lockedFloorOk(state.lockedDepth, pRef, params.poolLockMinBps, params.poolMinRealSol)) {
        reasons.push('PoolTooThin');
    }
    return out({ state });
}

function lambdaOf(m: MathModule, kind: PoolCandidate['kind'], read: readonly (RawAccount | null)[]): number {
    const pool = read[0];
    const lp = read[kind === 'raydium_v4' ? 3 : 4];
    if (!pool || !lp) return 0;
    const supply = value(readMintBase(lp.owner, lp.data))?.supply;
    if (supply === undefined) return 0;
    const counter =
        kind === 'raydium_v4'
            ? value(readAmmInfo(pool.owner, pool.data))?.lpAmount
            : value(readCpmmPool(pool.owner, pool.data))?.lpSupply;
    return counter === undefined ? 0 : m.lambdaBps(counter, supply);
}

/** A market as the listing would write it for this candidate (the pool cache, the SOL side). */
function candidateMarket(
    kind: PoolCandidate['kind'],
    mint: Address,
    tokenProgram: Address,
    read: readonly (RawAccount | null)[],
    solFirst: boolean,
) {
    const cache = Array.from({ length: 10 }, () => '11111111111111111111111111111111' as Address);
    const key = (i: number) => read[i]?.address ?? cache[0];
    cache[RAY_CACHE.pool] = key(0) as Address;
    if (kind === 'raydium_v4') {
        const [coin, pc] = [key(1), key(2)] as Address[];
        cache[RAY_CACHE.vaultSol] = (solFirst ? coin : pc) as Address;
        cache[RAY_CACHE.vaultTok] = (solFirst ? pc : coin) as Address;
        cache[RAY_CACHE.lpMint] = key(3) as Address;
    } else {
        const p = read[0] ? value(readCpmmPool(read[0].owner, read[0].data)) : null;
        const [v0, v1] = [key(2), key(3)] as Address[];
        cache[RAY_CACHE.ammConfig] = key(1) as Address;
        cache[RAY_CACHE.vaultSol] = (solFirst ? v0 : v1) as Address;
        cache[RAY_CACHE.vaultTok] = (solFirst ? v1 : v0) as Address;
        cache[RAY_CACHE.lpMint] = key(4) as Address;
        if (p) cache[RAY_CACHE.observation] = p.observationKey;
    }
    return {
        mint,
        tokenProgram,
        poolCache: cache,
        flags: solFirst ? MARKET_FLAG.solIsToken0OrCoin : 0,
    } as unknown as Parameters<typeof readVenueState>[2];
}

function verifyV4(
    read: readonly (RawAccount | null)[],
    mint: Address,
    tokenProgram: Address,
): { solFirst: boolean } | null {
    const [pool, coinV, pcV, lp] = read;
    if (!pool || !coinV || !pcV || !lp || read.length !== 4) return null;
    if (pool.owner !== RAYDIUM_V4_PROGRAM || pool.data.length !== AMM_V4_LEN) return null;
    const info = value(readAmmInfo(pool.owner, pool.data));
    if (!info || ![1n, 6n, 7n].includes(info.status) || info.nonce !== 254n) return null;
    const solFirst = v4SolIsCoin(info);
    if (solFirst === null) return null;
    const tokMint = solFirst ? info.pcVaultMint : info.coinVaultMint;
    if (tokMint !== mint || tokenProgram !== TOKEN_PROGRAM) return null;
    if (coinV.address !== info.coinVault || pcV.address !== info.pcVault || lp.address !== info.lpMint) return null;
    for (const [v, want] of [
        [coinV, info.coinVaultMint],
        [pcV, info.pcVaultMint],
    ] as const) {
        const t = tokenOf(v, TOKEN_PROGRAM);
        if (!t || t.mint !== want || t.owner !== RAYDIUM_V4_AUTHORITY) return null;
    }
    return { solFirst };
}

function verifyCpmm(
    read: readonly (RawAccount | null)[],
    mint: Address,
    tokenProgram: Address,
): { solFirst: boolean } | null {
    const [pool, cfg, v0, v1, lp] = read;
    if (!pool || !cfg || !v0 || !v1 || !lp || read.length !== 5) return null;
    if (pool.owner !== RAYDIUM_CPMM_PROGRAM || pool.data.length !== CPMM_POOL_LEN) return null;
    const p = value(readCpmmPool(pool.owner, pool.data));
    if (!p) return null;
    if (
        cfg.address !== p.ammConfig ||
        v0.address !== p.token0Vault ||
        v1.address !== p.token1Vault ||
        lp.address !== p.lpMint
    ) {
        return null;
    }
    if (cfg.owner !== RAYDIUM_CPMM_PROGRAM || !value(readCpmmConfig(cfg.owner, cfg.data))) return null;
    const sol0 = cpmmSolIsToken0(p);
    if (sol0 === null) return null;
    const [tokMint, solProg, tokProg] = sol0
        ? [p.token1Mint, p.token0Program, p.token1Program]
        : [p.token0Mint, p.token1Program, p.token0Program];
    if (tokMint !== mint || solProg !== TOKEN_PROGRAM || tokProg !== tokenProgram) return null;
    for (const [v, want, prog] of [
        [v0, p.token0Mint, p.token0Program],
        [v1, p.token1Mint, p.token1Program],
    ] as const) {
        const t = tokenOf(v, prog);
        if (!t || t.mint !== want || t.owner !== RAYDIUM_CPMM_AUTHORITY) return null;
    }
    return { solFirst: sol0 };
}

// ---- discovery (RPC) ---------------------------------------------------------------------------

/** The four discovery queries of a mint (`pool_monitor::discovery_queries`). */
export function discoveryQueries(mint: Address) {
    const q = (program: Address, size: number, a: number, b: number, x: Address, y: Address) => ({
        program,
        size,
        filters: [
            { offset: a, bytes: x },
            { offset: b, bytes: y },
        ],
    });
    return [
        { kind: 'raydium_v4' as const, ...q(RAYDIUM_V4_PROGRAM, AMM_V4_LEN, 400, 432, mint, WSOL_MINT) },
        { kind: 'raydium_v4' as const, ...q(RAYDIUM_V4_PROGRAM, AMM_V4_LEN, 400, 432, WSOL_MINT, mint) },
        { kind: 'raydium_cpmm' as const, ...q(RAYDIUM_CPMM_PROGRAM, CPMM_POOL_LEN, 168, 200, mint, WSOL_MINT) },
        { kind: 'raydium_cpmm' as const, ...q(RAYDIUM_CPMM_PROGRAM, CPMM_POOL_LEN, 168, 200, WSOL_MINT, mint) },
    ];
}

/** Discovers and judges the mint's Raydium SOL pools; the deepest eligible first. */
export async function discoverCandidates(
    rpc: AppRpc,
    m: MathModule,
    mint: Address,
    tokenProgram: Address,
    params: EligibilityParams,
    upgradedMask: number,
    now: bigint,
    detectionSet: readonly AccountMeta[],
): Promise<PoolCandidate[]> {
    const pools: { kind: PoolCandidate['kind']; acc: RawAccount }[] = [];
    for (const q of discoveryQueries(mint)) {
        const res = await rpc
            .getProgramAccounts(q.program, {
                encoding: 'base64',
                commitment: 'confirmed',
                filters: [
                    { dataSize: BigInt(q.size) },
                    ...q.filters.map((f) => ({
                        memcmp: {
                            offset: BigInt(f.offset),
                            bytes: f.bytes as unknown as Base58EncodedBytes,
                            encoding: 'base58' as const,
                        },
                    })),
                ],
            })
            .send();
        const b64 = getBase64Encoder();
        for (const r of res as unknown as readonly {
            pubkey: Address;
            account: { owner: Address; lamports: bigint; executable: boolean; data: readonly [string, string] };
        }[]) {
            pools.push({
                kind: q.kind,
                acc: {
                    address: r.pubkey,
                    owner: r.account.owner,
                    lamports: BigInt(r.account.lamports),
                    executable: r.account.executable,
                    data: new Uint8Array(b64.encode(r.account.data[0])),
                },
            });
        }
    }
    const reads = pools.map((p) => ({ ...p, keys: candidateReadKeys(p.kind, p.acc) }));
    const { accounts } = await fetchRaw(rpc, [...new Set(reads.flatMap((r) => (r.keys ?? []).slice(1)))]);
    const out: PoolCandidate[] = [];
    for (const r of reads) {
        if (!r.keys) continue;
        const read = [r.acc, ...r.keys.slice(1).map((k) => accounts.get(k) ?? null)];
        const judged = judgeCandidate(m, r.kind, mint, tokenProgram, read, now, params, upgradedMask);
        out.push({
            pool: judged.pool,
            kind: judged.kind,
            solReserve: judged.solReserve,
            lockedDepth: judged.lockedDepth,
            lambdaBps: judged.lambdaBps,
            feePpm: judged.feePpm,
            eligible: judged.eligible,
            reasons: judged.reasons,
            remainingAccounts: [...detectionSet, ...readMetas(r.kind, r.keys, false)],
        });
    }
    return out.sort((a, b) =>
        a.eligible !== b.eligible
            ? a.eligible
                ? -1
                : 1
            : b.lockedDepth > a.lockedDepth
              ? 1
              : b.lockedDepth < a.lockedDepth
                ? -1
                : 0,
    );
}
