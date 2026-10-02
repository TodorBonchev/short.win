/**
 * The venue layer's constants against the program's sources (`programs/shortwin/src/constants.rs`,
 * `state/market.rs`, `state/venue_extras.rs`, the adapters): the static venue accounts, the venue
 * kinds, flags, bits and masks, the pool-cache slots and the extras codes.
 */
import { describe, expect, it } from 'vitest';
import { readRepo } from '../test/repo.ts';
import {
    COVERAGE_CLEAR_DELAY_S,
    CURVE_MIN_LEN,
    ENTRY,
    EXTRAS_IX,
    GUARD_MASK,
    KIND,
    MARKET_FLAG,
    MOCK_CPAMM_AUTHORITY,
    PAMM_CACHE,
    PLACEHOLDER,
    PUMP_AMM_EVENT_AUTHORITY,
    PUMP_AMM_FEE_CONFIG,
    PUMP_AMM_GLOBAL_CONFIG,
    PUMP_AMM_GLOBAL_VOLUME_ACCUMULATOR,
    PUMP_EVENT_AUTHORITY,
    PUMP_FEE_CONFIG,
    PUMP_GLOBAL,
    PUMP_GLOBAL_VOLUME_ACCUMULATOR,
    RAY_CACHE,
    RAYDIUM_CPMM_AUTHORITY,
    RAYDIUM_V4_AUTHORITY,
    SEED,
    STANDARD_POOL_TOKENS,
    VENUE_BIT,
} from './constants.ts';

const constants = readRepo('programs/shortwin/src/constants.rs');
const market = readRepo('programs/shortwin/src/state/market.rs');
const extras = readRepo('programs/shortwin/src/state/venue_extras.rs');

function pubkey(name: string): string {
    const m = new RegExp(`pub const ${name}: Pubkey =\\s*pubkey!\\("(\\w+)"\\)`).exec(constants);
    if (!m?.[1]) throw new Error(`${name} not in constants.rs`);
    return m[1];
}

function num(src: string, name: string): number {
    const m = new RegExp(`pub const ${name}: \\w+ = ([^;]+);`).exec(src);
    if (!m?.[1]) throw new Error(`${name} not found`);
    const e = m[1].replace(/_/g, '').trim();
    const shift = /^1 << (\d+)$/.exec(e);
    return shift ? 1 << Number(shift[1]) : Number(e);
}

describe('venue constants = the program', () => {
    it('static venue accounts', () => {
        expect(PUMP_GLOBAL).toBe(pubkey('PUMP_GLOBAL'));
        expect(PUMP_EVENT_AUTHORITY).toBe(pubkey('PUMP_EVENT_AUTHORITY'));
        expect(PUMP_GLOBAL_VOLUME_ACCUMULATOR).toBe(pubkey('PUMP_GLOBAL_VOLUME_ACCUMULATOR'));
        expect(PUMP_FEE_CONFIG).toBe(pubkey('PUMP_FEE_CONFIG'));
        expect(PUMP_AMM_GLOBAL_CONFIG).toBe(pubkey('PUMP_AMM_GLOBAL_CONFIG'));
        expect(PUMP_AMM_EVENT_AUTHORITY).toBe(pubkey('PUMP_AMM_EVENT_AUTHORITY'));
        expect(PUMP_AMM_GLOBAL_VOLUME_ACCUMULATOR).toBe(pubkey('PUMP_AMM_GLOBAL_VOLUME_ACCUMULATOR'));
        expect(PUMP_AMM_FEE_CONFIG).toBe(pubkey('PUMP_AMM_FEE_CONFIG'));
        expect(RAYDIUM_V4_AUTHORITY).toBe(pubkey('RAYDIUM_V4_AUTHORITY'));
        expect(RAYDIUM_CPMM_AUTHORITY).toBe(pubkey('RAYDIUM_CPMM_AUTHORITY'));
        expect(MOCK_CPAMM_AUTHORITY).toBe(pubkey('MOCK_CPAMM_AUTHORITY'));
    });

    it('venue kinds, bits and guard masks', () => {
        const kinds = /pub enum VenueKind \{([\s\S]*?)\n\}/.exec(market)?.[1] ?? '';
        for (const [name, code] of [
            ['None', KIND.none],
            ['PumpCurve', KIND.pump_curve],
            ['PumpMigrating', KIND.pump_migrating],
            ['PumpSwap', KIND.pump_swap],
            ['RaydiumV4', KIND.raydium_v4],
            ['RaydiumCpmm', KIND.raydium_cpmm],
            ['Mock', KIND.mock],
        ] as const) {
            expect(kinds).toMatch(new RegExp(`${name} = ${code},`));
        }
        expect(VENUE_BIT.pump_curve).toBe(num(constants, 'VENUE_BIT_PUMP_CURVE'));
        expect(VENUE_BIT.pump_swap).toBe(num(constants, 'VENUE_BIT_PUMP_SWAP'));
        expect(VENUE_BIT.raydium_v4).toBe(num(constants, 'VENUE_BIT_RAYDIUM_V4'));
        expect(VENUE_BIT.raydium_cpmm).toBe(num(constants, 'VENUE_BIT_RAYDIUM_CPMM'));
        expect(VENUE_BIT.mock).toBe(num(constants, 'VENUE_BIT_MOCK'));
        const g = (n: string) => 1 << num(constants, n);
        expect(GUARD_MASK.pump_curve).toBe(g('GUARD_PUMP') | g('GUARD_PUMP_FEES') | g('GUARD_PUMP_AMM'));
        expect(GUARD_MASK.pump_swap).toBe(g('GUARD_PUMP_AMM') | g('GUARD_PUMP_FEES'));
        expect(GUARD_MASK.raydium_v4).toBe(g('GUARD_RAYDIUM_V4') | g('GUARD_RAYDIUM_CPMM'));
        expect(GUARD_MASK.mock).toBe(GUARD_MASK.raydium_v4);
        expect(COVERAGE_CLEAR_DELAY_S).toBe(BigInt(num(constants, 'COVERAGE_CLEAR_DELAY_S')));
    });

    it('market flags and pool-cache slots', () => {
        expect(MARKET_FLAG.pausedOpens).toBe(num(market, 'MARKET_FLAG_PAUSED_OPENS'));
        expect(MARKET_FLAG.mintUnsafe).toBe(num(market, 'MARKET_FLAG_MINT_UNSAFE'));
        expect(MARKET_FLAG.routeBroken).toBe(num(market, 'MARKET_FLAG_ROUTE_BROKEN'));
        expect(MARKET_FLAG.solIsToken0OrCoin).toBe(num(market, 'MARKET_FLAG_SOL_IS_TOKEN0_OR_COIN'));
        expect(MARKET_FLAG.pausedCoverage).toBe(num(market, 'MARKET_FLAG_PAUSED_COVERAGE'));
        expect(RAY_CACHE.pool).toBe(num(market, 'RAY_POOL'));
        expect(RAY_CACHE.vaultSol).toBe(num(market, 'RAY_VAULT_SOL'));
        expect(RAY_CACHE.vaultTok).toBe(num(market, 'RAY_VAULT_TOK'));
        expect(RAY_CACHE.ammConfig).toBe(num(market, 'RAY_AMM_CONFIG'));
        expect(RAY_CACHE.observation).toBe(num(market, 'RAY_OBSERVATION'));
        expect(RAY_CACHE.lpMint).toBe(num(market, 'RAY_LP_MINT'));
        expect(PAMM_CACHE.poolBaseVault).toBe(num(market, 'POOL_BASE_VAULT'));
        expect(PAMM_CACHE.poolQuoteVault).toBe(num(market, 'POOL_QUOTE_VAULT'));
        expect(PAMM_CACHE.poolV2Marker).toBe(num(market, 'POOL_V2_MARKER'));
        expect(PAMM_CACHE.boostVaultAta).toBe(num(market, 'BOOST_VAULT_ATA'));
        expect(PAMM_CACHE.uva).toBe(num(market, 'PAMM_UVA'));
        expect(PAMM_CACHE.uvaQuoteAta).toBe(num(market, 'PAMM_UVA_QUOTE_ATA'));
        expect(PAMM_CACHE.cachedCoinCreator).toBe(num(market, 'CACHED_COIN_CREATOR'));
        expect(PAMM_CACHE.cachedCoinCreatorVaultAuth).toBe(num(market, 'CACHED_COIN_CREATOR_VAULT_AUTH'));
        expect(PAMM_CACHE.cachedCoinCreatorVaultAta).toBe(num(market, 'CACHED_COIN_CREATOR_VAULT_ATA'));
        expect(PAMM_CACHE.boostVault).toBe(num(market, 'BOOST_VAULT'));
    });

    it('extras codes and the curve constants', () => {
        expect(EXTRAS_IX.sell).toBe(num(extras, 'EXTRAS_IX_SELL'));
        expect(EXTRAS_IX.buy).toBe(num(extras, 'EXTRAS_IX_BUY'));
        expect(ENTRY.fixed).toBe(num(extras, 'ENTRY_FIXED'));
        expect(ENTRY.template).toBe(num(extras, 'ENTRY_TEMPLATE'));
        expect(SEED.const).toBe(num(extras, 'SEED_CONST'));
        expect(SEED.placeholder).toBe(num(extras, 'SEED_PLACEHOLDER'));
        expect(PLACEHOLDER.mint).toBe(num(extras, 'PLACEHOLDER_MINT'));
        expect(PLACEHOLDER.pool).toBe(num(extras, 'PLACEHOLDER_POOL'));
        expect(PLACEHOLDER.trader).toBe(num(extras, 'PLACEHOLDER_TRADER'));
        expect(PLACEHOLDER.bondingCurve).toBe(num(extras, 'PLACEHOLDER_BONDING_CURVE'));
        expect(PLACEHOLDER.coinCreator).toBe(num(extras, 'PLACEHOLDER_COIN_CREATOR'));
        expect(readRepo('programs/shortwin/src/venues/mod.rs')).toContain(
            `pub const STANDARD_POOL_TOKENS: u128 = ${STANDARD_POOL_TOKENS.toLocaleString('en-US').replace(/,/g, '_')};`,
        );
        expect(readRepo('crates/shortwin-venue-state/src/pump.rs')).toContain(
            `pub const CURVE_MIN_LEN: usize = ${CURVE_MIN_LEN};`,
        );
    });
});
