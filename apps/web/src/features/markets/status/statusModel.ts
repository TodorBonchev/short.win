/**
 * Protocol status aggregation (FRONTEND.md §3.9): the venue programs with their upgrade latch,
 * enabled and route-broken bits, and the degraded / route-broken markets filtered client-side from
 * the first pages of `/v1/markets` (FE-5: the list has no flag filter yet). Pure, so the
 * aggregation is unit-tested; the page renders it.
 */
import type { GlobalConfig, VenueGuard } from '@shortwin/client';
import type { MarketSummary, VenueGuardRow, VenueStatus } from '../../../api/types.ts';
import { GUARD, VENUE_BIT, venueBit } from '../shared/marketFlags.ts';

export type VenueProgramName = VenueGuardRow['name'];

/** Which venue bits each watched program serves (ARCH §2.3.2, §3.1). */
const PROGRAMS: readonly {
    readonly name: Exclude<VenueProgramName, 'unknown'>;
    readonly guard: number;
    readonly label: string;
    readonly venues: readonly { readonly key: VenueBitKey; readonly label: string }[];
}[] = [
    { name: 'pump', guard: GUARD.pump, label: 'Pump.fun', venues: [{ key: 'pump_curve', label: 'Pump curve' }] },
    { name: 'pump_amm', guard: GUARD.pump_amm, label: 'PumpSwap', venues: [{ key: 'pump_swap', label: 'PumpSwap' }] },
    {
        name: 'pump_fees',
        guard: GUARD.pump_fees,
        label: 'Pump fees',
        venues: [
            { key: 'pump_curve', label: 'Pump curve' },
            { key: 'pump_swap', label: 'PumpSwap' },
        ],
    },
    {
        name: 'raydium_v4',
        guard: GUARD.raydium_v4,
        label: 'Raydium AMM v4',
        venues: [{ key: 'raydium_v4', label: 'Raydium v4' }],
    },
    {
        name: 'raydium_cpmm',
        guard: GUARD.raydium_cpmm,
        label: 'Raydium CPMM',
        venues: [{ key: 'raydium_cpmm', label: 'Raydium CPMM' }],
    },
    { name: 'jupiter', guard: GUARD.jupiter, label: 'Jupiter (v1.1)', venues: [{ key: 'jupiter', label: 'Jupiter' }] },
];

export type VenueBitKey = keyof typeof VENUE_BIT;

export interface VenueProgramRow {
    readonly name: VenueProgramName;
    readonly label: string;
    readonly serves: readonly string[];
    readonly upgraded: boolean;
    readonly guardSlot: string;
    /** The ProgramData deploy slot (indexer) or the latest one seen while flagged (chain). */
    readonly deploySlot: string;
    readonly flaggedAt: number | null;
    /** `true` if any venue it serves takes opens; `null` when `venue_enabled` is unknown. */
    readonly enabled: boolean | null;
    readonly routeBroken: boolean;
}

function bitsOf(venues: readonly { key: VenueBitKey }[]): number {
    return venues.reduce((a, v) => a | VENUE_BIT[v.key], 0);
}

/** Rows from `/v1/venues` (the guards in the program's index order, joined with the venue bits). */
export function venueProgramRows(v: VenueStatus): VenueProgramRow[] {
    return PROGRAMS.map((p) => {
        const g = v.guards.find((x) => x.venue === p.guard || x.name === p.name);
        const bits = bitsOf(p.venues);
        return {
            name: p.name,
            label: p.label,
            serves: p.venues.map((x) => x.label),
            upgraded: g?.upgraded ?? false,
            guardSlot: g?.guard_slot ?? '—',
            deploySlot: g?.programdata_slot ?? '—',
            flaggedAt: g?.flagged_at ?? null,
            enabled: v.venue_enabled === null ? null : (v.venue_enabled & bits) !== 0,
            routeBroken: (v.route_broken_bits & bits) !== 0,
        };
    });
}

/** The same rows from the chain (`VenueGuard`, `GlobalConfig`) while the indexer is unavailable. */
export function venueProgramRowsFromChain(guard: VenueGuard | null, config: GlobalConfig | null): VenueProgramRow[] {
    return PROGRAMS.map((p) => {
        const bits = bitsOf(p.venues);
        const flaggedAt = guard ? Number(guard.flaggedAt[p.guard] ?? 0n) : 0;
        return {
            name: p.name,
            label: p.label,
            serves: p.venues.map((x) => x.label),
            upgraded: guard ? (guard.upgraded & (1 << p.guard)) !== 0 : false,
            guardSlot: guard ? String(guard.guardSlot[p.guard] ?? '—') : '—',
            deploySlot: guard ? String(guard.flaggedSlot[p.guard] ?? '—') : '—',
            flaggedAt: flaggedAt > 0 ? flaggedAt : null,
            enabled: config ? (config.venueEnabled & bits) !== 0 : null,
            routeBroken: config ? (config.venueRouteBroken & bits) !== 0 : false,
        };
    });
}

/** Venues whose route-broken bit is set (`GlobalConfig.venue_route_broken`). */
export function brokenVenues(bits: number): { key: VenueBitKey; bit: number }[] {
    return (Object.entries(VENUE_BIT) as [VenueBitKey, number][])
        .filter(([, bit]) => (bits & bit) !== 0)
        .map(([key, bit]) => ({ key, bit }));
}

/** Markets in degraded mode: new shorts paused for keeper coverage (`flags.paused_coverage`). */
export function degradedMarkets(rows: readonly MarketSummary[]): MarketSummary[] {
    return rows.filter((m) => m.flags.paused_coverage);
}

/** Markets whose route is broken: their own bit, or their venue's. */
export function routeBrokenMarkets(rows: readonly MarketSummary[], venueBits: number): MarketSummary[] {
    return rows.filter((m) => m.flags.route_broken || (venueBits & venueBit(m.venue)) !== 0);
}
