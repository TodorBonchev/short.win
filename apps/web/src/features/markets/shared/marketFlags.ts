/**
 * A market's state flags and banners (FRONTEND.md §3.4, §3.9; testids `market-flag-<flag>`), derived
 * from the market's own flag bits, the protocol's venue bits (`GlobalConfig.venue_enabled` /
 * `venue_route_broken`, ARCH §2.3.2), the venue-upgrade latch (`VenueGuard.upgraded`, ARCH §3.1)
 * and venue state read from the chain (the EMA warm-up, PumpSwap's `disable_flags`).
 *
 * The bit layouts are the program's (programs/shortwin/src/constants.rs, state/market.rs);
 * `marketFlags.test.ts` holds them to the Rust source.
 */
import type { MarketFlags, VenueName } from '../../../api/types.ts';
import type { DisclosureId } from '../../../config/disclosures.ts';

/** `GlobalConfig.venue_enabled` / `venue_route_broken` bits (constants.rs `VENUE_BIT_*`). */
export const VENUE_BIT = {
    pump_curve: 1 << 0,
    pump_swap: 1 << 1,
    raydium_v4: 1 << 2,
    raydium_cpmm: 1 << 3,
    jupiter: 1 << 4,
    /** TEST-ONLY (`test-venue` builds). */
    mock: 1 << 5,
} as const;

/** `VenueGuard` indices (constants.rs `GUARD_*`), the order of `/v1/venues.guards[].venue`. */
export const GUARD = { pump: 0, pump_amm: 1, pump_fees: 2, raydium_v4: 3, raydium_cpmm: 4, jupiter: 5 } as const;

/** `Market.flags` bits (state/market.rs `MARKET_FLAG_*`). */
export const MARKET_FLAG = {
    pausedOpens: 1 << 0,
    mintUnsafe: 1 << 1,
    routeBroken: 1 << 2,
    solIsToken0OrCoin: 1 << 3,
    graduatedElsewhere: 1 << 4,
    wipeLive: 1 << 5,
    pausedCoverage: 1 << 6,
} as const;

/** `GlobalConfig.flags` bit 0 (`CONFIG_FLAG_PAUSED_OPENS`). */
export const CONFIG_FLAG_PAUSED_OPENS = 1 << 0;

/** The venue bit a market's opens and S3 evidence use (`VenueKind::venue_bit`); 0 for none. */
export function venueBit(venue: VenueName): number {
    switch (venue) {
        case 'pump_curve':
            return VENUE_BIT.pump_curve;
        case 'pump_swap':
            return VENUE_BIT.pump_swap;
        case 'raydium_v4':
            return VENUE_BIT.raydium_v4;
        case 'raydium_cpmm':
            return VENUE_BIT.raydium_cpmm;
        case 'mock':
            return VENUE_BIT.mock;
        default:
            return 0;
    }
}

/**
 * The `VenueGuard.upgraded` mask that blocks opens on a venue (`VenueKind::guard_mask`): the curve
 * also watches pAMM and pump fees, PumpSwap pAMM and pump fees, both Raydium kinds (and the
 * test-only mock) both Raydium programs; none for list-only and migrating markets.
 */
export function guardMask(venue: VenueName): number {
    switch (venue) {
        case 'pump_curve':
            return (1 << GUARD.pump) | (1 << GUARD.pump_fees) | (1 << GUARD.pump_amm);
        case 'pump_swap':
            return (1 << GUARD.pump_amm) | (1 << GUARD.pump_fees);
        case 'raydium_v4':
        case 'raydium_cpmm':
        case 'mock':
            return (1 << GUARD.raydium_v4) | (1 << GUARD.raydium_cpmm);
        default:
            return 0;
    }
}

/** `Market.venue` codes (state/market.rs `VenueKind`; INDEXER §6.2 `venue_name`). */
export function venueName(code: number): VenueName {
    const names: readonly VenueName[] = [
        'none',
        'pump_curve',
        'pump_migrating',
        'pump_swap',
        'raydium_v4',
        'raydium_cpmm',
        'mock',
    ];
    return names[code] ?? 'unknown';
}

/** Flags from `Market.flags` (the same bits the indexer exposes as `flags.*`). */
export function flagsFromBits(bits: number): MarketFlags {
    return {
        paused_opens: (bits & MARKET_FLAG.pausedOpens) !== 0,
        paused_coverage: (bits & MARKET_FLAG.pausedCoverage) !== 0,
        mint_unsafe: (bits & MARKET_FLAG.mintUnsafe) !== 0,
        route_broken: (bits & MARKET_FLAG.routeBroken) !== 0,
        wipe_live: (bits & MARKET_FLAG.wipeLive) !== 0,
    };
}

export type MarketFlagKey =
    | 'paused-opens'
    | 'coverage-paused'
    | 'route-broken'
    | 'venue-upgraded'
    | 'migrating'
    | 'graduation-halted'
    | 'list-only'
    | 'mint-unsafe'
    | 'warming-up'
    | 'pool-window';

/** Badge text and tone per flag (text always: colour never carries meaning alone, §8). */
export const FLAG_META: Readonly<Record<MarketFlagKey, { label: string; tone: 'info' | 'warn' | 'danger' }>> = {
    'mint-unsafe': { label: 'Mint unsafe', tone: 'danger' },
    'route-broken': { label: 'Route broken', tone: 'danger' },
    'paused-opens': { label: 'Opens paused', tone: 'warn' },
    'coverage-paused': { label: 'Coverage paused', tone: 'warn' },
    'venue-upgraded': { label: 'Venue upgraded', tone: 'warn' },
    migrating: { label: 'Migrating', tone: 'warn' },
    'graduation-halted': { label: 'Graduation halted', tone: 'warn' },
    'list-only': { label: 'Lend only', tone: 'info' },
    'warming-up': { label: 'Warming up', tone: 'info' },
    'pool-window': { label: 'Pool switch window', tone: 'info' },
};

/** Everything the flags are derived from; `undefined` = not known (e.g. the indexer is down). */
export interface MarketStateInput {
    readonly venue: VenueName;
    readonly flags: MarketFlags;
    /** `GlobalConfig` paused opens (all markets). */
    readonly globalPausedOpens?: boolean | undefined;
    /** `GlobalConfig.venue_route_broken`. */
    readonly routeBrokenBits?: number | undefined;
    /** `VenueGuard.upgraded` as a bit set over the guard indices. */
    readonly upgradedBits?: number | undefined;
    /** `Market.ema_warm` (chain). */
    readonly emaWarm?: number | undefined;
    /** PumpSwap `GlobalConfig.disable_flags` bit 0 (only meaningful on a curve market). */
    readonly pammCreatePoolDisabled?: boolean | undefined;
    /** `/pools.window` is open. */
    readonly poolWindowOpen?: boolean | undefined;
}

/** The flags that are true, in severity order (the order the badges and banners use). */
export function marketFlags(s: MarketStateInput): MarketFlagKey[] {
    const out: MarketFlagKey[] = [];
    if (s.flags.mint_unsafe) out.push('mint-unsafe');
    if (s.flags.route_broken || ((s.routeBrokenBits ?? 0) & venueBit(s.venue)) !== 0) out.push('route-broken');
    if (s.flags.paused_opens || s.globalPausedOpens) out.push('paused-opens');
    if (s.flags.paused_coverage) out.push('coverage-paused');
    if (((s.upgradedBits ?? 0) & guardMask(s.venue)) !== 0) out.push('venue-upgraded');
    if (s.venue === 'pump_migrating') out.push('migrating');
    if (s.venue === 'pump_curve' && s.pammCreatePoolDisabled) out.push('graduation-halted');
    if (s.venue === 'none') out.push('list-only');
    if ((s.emaWarm ?? 0) > 0) out.push('warming-up');
    if (s.poolWindowOpen) out.push('pool-window');
    return out;
}

export interface BannerSpec {
    readonly key: MarketFlagKey;
    readonly severity: 'info' | 'warn' | 'danger';
    /** The disclosure shown verbatim, or `null` for a program-error message (paused opens). */
    readonly disclosure: DisclosureId | null;
}

const BANNER_DISCLOSURE: Readonly<Partial<Record<MarketFlagKey, DisclosureId>>> = {
    'mint-unsafe': 'market.mintUnsafe',
    'route-broken': 'market.routeBroken',
    'coverage-paused': 'market.coveragePause',
    'venue-upgraded': 'market.venueUpgraded',
    migrating: 'market.migrating',
    'graduation-halted': 'market.graduationHalted',
    'list-only': 'market.listOnly',
    'warming-up': 'market.poolSwitch',
    'pool-window': 'market.poolSwitch',
};

/**
 * The market page's banners in FRONTEND.md §3.4's severity order: `market.mintUnsafe`,
 * `market.routeBroken`, (paused opens), `market.coveragePause`, `market.venueUpgraded`,
 * `market.migrating`, `market.graduationHalted`, `market.listOnly`; then `market.poolSwitch` once
 * for a collection window or a warm-up.
 */
export function marketBanners(flags: readonly MarketFlagKey[]): BannerSpec[] {
    const out: BannerSpec[] = [];
    let poolSwitch = false;
    for (const key of flags) {
        const disclosure = BANNER_DISCLOSURE[key] ?? null;
        if (disclosure === 'market.poolSwitch') {
            if (poolSwitch) continue;
            poolSwitch = true;
        }
        out.push({ key, severity: FLAG_META[key].tone, disclosure });
    }
    return out;
}

/** Guard indices set in `upgradedBits` → the venue programs' names (status page). */
export function upgradedFromGuards(guards: readonly { venue: number; upgraded: boolean }[]): number {
    let bits = 0;
    for (const g of guards) if (g.upgraded && g.venue >= 0 && g.venue < 31) bits |= 1 << g.venue;
    return bits;
}
