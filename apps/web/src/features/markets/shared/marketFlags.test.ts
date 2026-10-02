import { describe, expect, it } from 'vitest';
import type { MarketFlags } from '../../../api/types.ts';
import { readRepo } from '../../../test/repo.ts';
import {
    flagsFromBits,
    GUARD,
    guardMask,
    MARKET_FLAG,
    marketBanners,
    marketFlags,
    upgradedFromGuards,
    VENUE_BIT,
    venueBit,
    venueName,
} from './marketFlags.ts';

const NO_FLAGS: MarketFlags = {
    paused_opens: false,
    paused_coverage: false,
    mint_unsafe: false,
    route_broken: false,
    wipe_live: false,
};

function rustConst(src: string, name: string): number {
    const m = new RegExp(`pub const ${name}: u\\d+ = (?:1 << (\\d+)|(\\d+));`).exec(src);
    if (!m) throw new Error(`${name} not found`);
    return m[1] !== undefined ? 1 << Number(m[1]) : Number(m[2]);
}

describe('bit layouts match the program', () => {
    const constants = readRepo('programs/shortwin/src/constants.rs');
    const market = readRepo('programs/shortwin/src/state/market.rs');

    it('venue bits and guard indices (constants.rs)', () => {
        expect(VENUE_BIT.pump_curve).toBe(rustConst(constants, 'VENUE_BIT_PUMP_CURVE'));
        expect(VENUE_BIT.pump_swap).toBe(rustConst(constants, 'VENUE_BIT_PUMP_SWAP'));
        expect(VENUE_BIT.raydium_v4).toBe(rustConst(constants, 'VENUE_BIT_RAYDIUM_V4'));
        expect(VENUE_BIT.raydium_cpmm).toBe(rustConst(constants, 'VENUE_BIT_RAYDIUM_CPMM'));
        expect(VENUE_BIT.jupiter).toBe(rustConst(constants, 'VENUE_BIT_JUPITER'));
        expect(VENUE_BIT.mock).toBe(rustConst(constants, 'VENUE_BIT_MOCK'));
        expect(GUARD.pump).toBe(rustConst(constants, 'GUARD_PUMP'));
        expect(GUARD.pump_amm).toBe(rustConst(constants, 'GUARD_PUMP_AMM'));
        expect(GUARD.pump_fees).toBe(rustConst(constants, 'GUARD_PUMP_FEES'));
        expect(GUARD.raydium_v4).toBe(rustConst(constants, 'GUARD_RAYDIUM_V4'));
        expect(GUARD.raydium_cpmm).toBe(rustConst(constants, 'GUARD_RAYDIUM_CPMM'));
        expect(GUARD.jupiter).toBe(rustConst(constants, 'GUARD_JUPITER'));
        // The masks' own unit test in constants.rs.
        expect(guardMask('pump_curve')).toBe(0b00111);
        expect(guardMask('pump_swap')).toBe(0b00110);
        expect(guardMask('raydium_cpmm')).toBe(0b11000);
        expect(guardMask('none')).toBe(0);
        expect(guardMask('pump_migrating')).toBe(0);
    });

    it('market flag bits (state/market.rs)', () => {
        expect(MARKET_FLAG.pausedOpens).toBe(rustConst(market, 'MARKET_FLAG_PAUSED_OPENS'));
        expect(MARKET_FLAG.mintUnsafe).toBe(rustConst(market, 'MARKET_FLAG_MINT_UNSAFE'));
        expect(MARKET_FLAG.routeBroken).toBe(rustConst(market, 'MARKET_FLAG_ROUTE_BROKEN'));
        expect(MARKET_FLAG.wipeLive).toBe(rustConst(market, 'MARKET_FLAG_WIPE_LIVE'));
        expect(MARKET_FLAG.pausedCoverage).toBe(rustConst(market, 'MARKET_FLAG_PAUSED_COVERAGE'));
        expect(flagsFromBits(MARKET_FLAG.pausedCoverage | MARKET_FLAG.mintUnsafe)).toEqual({
            ...NO_FLAGS,
            paused_coverage: true,
            mint_unsafe: true,
        });
    });

    it('venue codes follow VenueKind', () => {
        expect([0, 1, 2, 3, 4, 5, 6, 9].map(venueName)).toEqual([
            'none',
            'pump_curve',
            'pump_migrating',
            'pump_swap',
            'raydium_v4',
            'raydium_cpmm',
            'mock',
            'unknown',
        ]);
        expect(venueBit('pump_migrating')).toBe(0);
    });
});

describe('marketFlags', () => {
    it('is empty for a healthy tradable market', () => {
        expect(marketFlags({ venue: 'pump_swap', flags: NO_FLAGS, routeBrokenBits: 0, upgradedBits: 0 })).toEqual([]);
    });

    it('derives each flag from its source, in severity order', () => {
        expect(
            marketFlags({
                venue: 'pump_curve',
                flags: { ...NO_FLAGS, mint_unsafe: true, paused_coverage: true },
                globalPausedOpens: true,
                routeBrokenBits: VENUE_BIT.pump_curve,
                upgradedBits: 1 << GUARD.pump_fees,
                emaWarm: 3,
                pammCreatePoolDisabled: true,
                poolWindowOpen: true,
            }),
        ).toEqual([
            'mint-unsafe',
            'route-broken',
            'paused-opens',
            'coverage-paused',
            'venue-upgraded',
            'graduation-halted',
            'warming-up',
            'pool-window',
        ]);
    });

    it('applies venue bits and the latch only to the market’s own venue', () => {
        const raydium = { venue: 'raydium_v4' as const, flags: NO_FLAGS };
        expect(marketFlags({ ...raydium, routeBrokenBits: VENUE_BIT.pump_swap })).toEqual([]);
        expect(marketFlags({ ...raydium, routeBrokenBits: VENUE_BIT.raydium_v4 })).toEqual(['route-broken']);
        expect(marketFlags({ ...raydium, upgradedBits: 1 << GUARD.pump })).toEqual([]);
        // Both Raydium programs latch either Raydium kind (a fast path can move the market).
        expect(marketFlags({ ...raydium, upgradedBits: 1 << GUARD.raydium_cpmm })).toEqual(['venue-upgraded']);
        // The curve's graduation halt only matters on a curve.
        expect(marketFlags({ ...raydium, pammCreatePoolDisabled: true })).toEqual([]);
        expect(marketFlags({ venue: 'none', flags: NO_FLAGS })).toEqual(['list-only']);
        expect(marketFlags({ venue: 'pump_migrating', flags: NO_FLAGS })).toEqual(['migrating']);
        expect(marketFlags({ ...raydium, flags: { ...NO_FLAGS, route_broken: true } })).toEqual(['route-broken']);
    });

    it('builds banners with their disclosures, pool switch once', () => {
        const banners = marketBanners(['mint-unsafe', 'paused-opens', 'warming-up', 'pool-window']);
        expect(banners.map((b) => [b.key, b.disclosure])).toEqual([
            ['mint-unsafe', 'market.mintUnsafe'],
            ['paused-opens', null],
            ['warming-up', 'market.poolSwitch'],
        ]);
        expect(banners[0]?.severity).toBe('danger');
    });

    it('collects upgraded guard bits', () => {
        expect(
            upgradedFromGuards([
                { venue: 1, upgraded: true },
                { venue: 3, upgraded: false },
                { venue: 4, upgraded: true },
            ]),
        ).toBe(0b10010);
    });
});
