import type { GlobalConfig, VenueGuard } from '@shortwin/client';
import { describe, expect, it } from 'vitest';
import type { MarketFlags, MarketSummary, VenueStatus } from '../../../api/types.ts';
import { VENUE_BIT } from '../shared/marketFlags.ts';
import {
    brokenVenues,
    degradedMarkets,
    routeBrokenMarkets,
    venueProgramRows,
    venueProgramRowsFromChain,
} from './statusModel.ts';

const NO_FLAGS: MarketFlags = {
    paused_opens: false,
    paused_coverage: false,
    mint_unsafe: false,
    route_broken: false,
    wipe_live: false,
};

function m(mint: string, venue: MarketSummary['venue'], flags: Partial<MarketFlags> = {}): MarketSummary {
    return {
        market: mint,
        mint,
        symbol: null,
        name: null,
        decimals: 6,
        venue,
        tier: 'C',
        created_ts: 0,
        flags: { ...NO_FLAGS, ...flags },
        tokens_free: '0',
        borrowed_q: '0',
        inventory_tokens: '0',
        util_bps: 0,
        open_positions: 0,
        ema_price: null,
        price_ts: null,
        oi_lamports: '0',
        tvl_lamports: '0',
        tvl_usd: null,
    };
}

const venues: VenueStatus = {
    venue_enabled: VENUE_BIT.pump_curve | VENUE_BIT.pump_swap | VENUE_BIT.raydium_cpmm,
    route_broken_bits: VENUE_BIT.raydium_cpmm,
    paused_opens: false,
    guards: [
        {
            venue: 0,
            name: 'pump',
            upgraded: false,
            guard_slot: '100',
            programdata_slot: '100',
            flagged_at: null,
            last_ts: 1,
        },
        {
            venue: 2,
            name: 'pump_fees',
            upgraded: true,
            guard_slot: '90',
            programdata_slot: '120',
            flagged_at: 1_700,
            last_ts: 2,
        },
    ],
    extras: [],
    admin_changes: [],
};

describe('venue program rows', () => {
    it('joins the latch with the venue bits each program serves', () => {
        const rows = venueProgramRows(venues);
        expect(rows.map((r) => r.name)).toEqual([
            'pump',
            'pump_amm',
            'pump_fees',
            'raydium_v4',
            'raydium_cpmm',
            'jupiter',
        ]);
        const fees = rows.find((r) => r.name === 'pump_fees');
        expect(fees).toMatchObject({
            upgraded: true,
            guardSlot: '90',
            deploySlot: '120',
            flaggedAt: 1_700,
            enabled: true,
        });
        expect(rows.find((r) => r.name === 'raydium_v4')).toMatchObject({ enabled: false, routeBroken: false });
        expect(rows.find((r) => r.name === 'raydium_cpmm')).toMatchObject({ enabled: true, routeBroken: true });
        expect(venueProgramRows({ ...venues, venue_enabled: null })[0]?.enabled).toBeNull();
    });

    it('reads the same from the chain while the indexer is down', () => {
        const guard = {
            upgraded: 1 << 3,
            guardSlot: [1n, 2n, 3n, 4n, 5n, 6n],
            flaggedSlot: [0n, 0n, 0n, 44n, 0n, 0n],
            flaggedAt: [0n, 0n, 0n, 1_800n, 0n, 0n],
        } as unknown as VenueGuard;
        const config = { venueEnabled: VENUE_BIT.raydium_v4, venueRouteBroken: VENUE_BIT.pump_swap } as GlobalConfig;
        const rows = venueProgramRowsFromChain(guard, config);
        expect(rows.find((r) => r.name === 'raydium_v4')).toMatchObject({
            upgraded: true,
            guardSlot: '4',
            deploySlot: '44',
            flaggedAt: 1_800,
            enabled: true,
        });
        expect(rows.find((r) => r.name === 'pump_amm')).toMatchObject({ routeBroken: true, enabled: false });
        expect(rows.find((r) => r.name === 'pump_fees')?.routeBroken).toBe(true);
    });
});

describe('degraded and route-broken markets (FE-5)', () => {
    const rows = [
        m('A', 'pump_swap', { paused_coverage: true }),
        m('B', 'raydium_cpmm'),
        m('C', 'raydium_v4', { route_broken: true }),
        m('D', 'none', { paused_coverage: true, route_broken: true }),
    ];
    it('filters coverage-paused markets', () => {
        expect(degradedMarkets(rows).map((x) => x.mint)).toEqual(['A', 'D']);
    });
    it('includes markets on a route-broken venue', () => {
        expect(routeBrokenMarkets(rows, VENUE_BIT.raydium_cpmm).map((x) => x.mint)).toEqual(['B', 'C', 'D']);
        expect(brokenVenues(VENUE_BIT.raydium_cpmm | VENUE_BIT.pump_curve).map((v) => v.key)).toEqual([
            'pump_curve',
            'raydium_cpmm',
        ]);
    });
});
