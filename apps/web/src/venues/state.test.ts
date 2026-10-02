/**
 * The venue state of each real venue on the dumped mainnet fixtures (`tests/fixtures`): the venue
 * predicate, the pricing reserves and real SOL, the resolved fee, the locked depth, and quotes that
 * behave (FRONTEND.md §10.1 SHORT: venue-state readers against the fixture accounts). The markets
 * are written as the program's listing writes them (the pool caches, the SOL side, the derived keys).
 */
import type { Address } from '@solana/kit';
import { beforeAll, describe, expect, it } from 'vitest';
import { initMathSync, type MathModule } from '../math/load.ts';
import { readRepoBytes } from '../test/repo.ts';
import { MARKET_FLAG, PAMM_CACHE, RAY_CACHE } from './constants.ts';
import { detectFrom } from './detect.ts';
import type { MarketKeys } from './lists.ts';
import { canonicalPool, pammBoostVault, pammLpMint, poolAuthority, wsolAta } from './pda.ts';
import type { RawAccount } from './readers.ts';
import {
    afterSale,
    capFeePpm,
    liquidityOf,
    lockedDepth,
    quoteBuy,
    quoteBuyAt,
    quoteSell,
    readVenueState,
    spotPrice,
} from './state.ts';
import { fixture, fixtureAccount, fixtureAddress, snapshotTime } from './testing.ts';

let m: MathModule;
beforeAll(() => {
    m = initMathSync(readRepoBytes('packages/math-wasm/pkg/shortwin_math_wasm_bg.wasm'));
});

const D = '11111111111111111111111111111111' as Address;
const cache = () => Array.from({ length: 10 }, () => D);
const dec = (n: string) => fixture(n).decoded as Record<string, unknown>;
const big = (v: unknown) => BigInt(v as string | number);

function market(fields: Partial<MarketKeys>): MarketKeys {
    return {
        mint: D,
        tokenProgram: D,
        trader: D,
        traderWsol: D,
        traderBaseAta: D,
        bondingCurve: D,
        canonicalPool: D,
        curveBaseAta: D,
        curveQuoteAta: D,
        sharingConfig: D,
        pumpUva: D,
        pumpUvaQuoteAta: D,
        cachedCreator: D,
        cachedCreatorVault: D,
        cachedCreatorVaultAta: D,
        poolCache: cache(),
        flags: 0,
        traderWsolAta: D,
        pumpPoolAuthority: D,
        ...fields,
    } as MarketKeys;
}

const acc = (n: string) => fixtureAccount(n);

describe('pump curve', () => {
    for (const role of ['fresh', 'near']) {
        it(`${role}: executable on (vT, vQ), the FeeConfig fee, the live leg's X_p`, async () => {
            const mint = fixtureAddress(`pump_${role}_mint`);
            const authority = await poolAuthority(mint);
            const mk = market({
                mint,
                tokenProgram: fixture(`pump_${role}_mint`).owner as Address,
                bondingCurve: fixtureAddress(`pump_${role}_curve`),
                canonicalPool: await canonicalPool(mint, authority),
                curveBaseAta: fixtureAddress(`pump_${role}_curve_base_ata`),
                pumpPoolAuthority: authority,
            });
            const read: (RawAccount | null)[] = [
                acc(`pump_${role}_curve`),
                acc('pump_global'),
                acc('pump_fee_config'),
                null,
                acc(`pump_${role}_curve_base_ata`),
                acc('pamm_fee_config'),
                acc('pamm_global_config'),
            ];
            const c = dec(`pump_${role}_curve`);
            const s = readVenueState(m, 'pump_curve', mk, read, { now: 0n, supply: null, pEma: 0n });
            expect(s.executable).toBe(true);
            expect(s.x).toBe(big(c.virtual_token_reserves));
            expect(s.y).toBe(big(c.virtual_quote_reserves));
            expect(s.r).toBe(big(c.real_quote_reserves));
            // pump-fees tier 0: protocol 95 + creator 30 bps (the creator is set) = 1.25 %.
            expect(s.feePpm).toBe(12_500);
            expect(s.curve?.pricing?.livePoolTokens).toBe(
                big(dec(`pump_${role}_curve_base_ata`).amount) - big(c.real_token_reserves),
            );
            expect(s.curve?.pricing?.offsetToken).toBe(big(c.virtual_token_reserves) - big(c.real_token_reserves));
            // A sale and a buyback of 1 % of the real reserve, with the round trip losing the fees. A
            // sale is paid out of the real SOL: the `fresh` curve holds 1 lamport, so it has no quote.
            const q = big(c.real_token_reserves) / 100n;
            const sale = quoteSell(m, s, q);
            const buy = quoteBuy(m, s, q);
            expect(buy).not.toBeNull();
            if (big(c.real_quote_reserves) < 1_000_000n) {
                expect(sale).toBeNull();
                return;
            }
            expect(sale && buy && buy.cost > sale.proceeds).toBe(true);
            // Selling moves the curve: more tokens, less SOL, a lower spot.
            if (!sale) return;
            const post = afterSale(m, s, q, sale);
            expect(post.x).toBe(s.x + q);
            const before = spotPrice(m, s);
            const after = spotPrice(m, post);
            expect(before !== null && after !== null && after < before).toBe(true);
            // The EMA leg on the live state's own (x, y) quotes the same curve (deterministic X_p).
            expect(quoteBuyAt(m, s, q, s.x, s.y)).not.toBeNull();
            expect(capFeePpm(s)).toBe(12_500);
            // The risk view at a 1 % cap is cheaper than the live 1.25 % fee.
            const risk = quoteBuy(m, s, q, 100);
            expect(risk && buy && risk.cost < buy.cost).toBe(true);
        });
    }

    it('a complete curve is not executable (migrating when vT > 0)', async () => {
        const mint = fixtureAddress('pump_graduated_mint');
        const authority = await poolAuthority(mint);
        const mk = market({ mint, bondingCurve: fixtureAddress('pump_graduated_curve'), pumpPoolAuthority: authority });
        const read = [
            acc('pump_graduated_curve'),
            acc('pump_global'),
            acc('pump_fee_config'),
            acc('pump_graduated_pool'),
            null,
            null,
            null,
        ];
        const s = readVenueState(m, 'pump_curve', mk, read, { now: 0n, supply: null, pEma: 0n });
        expect(s.executable).toBe(false);
        expect(s.curve?.live.complete).toBe(true);
        expect(s.curve?.canonicalPool).toBe('valid');
    });
});

describe('PumpSwap', () => {
    for (const role of ['fresh', 'mature', 'cashback']) {
        it(`${role}: vault + VQR, the tier at the live supply, the burned-LP depth`, async () => {
            const mint = fixtureAddress(`pamm_${role}_mint`);
            const pool = fixtureAddress(`pamm_${role}_pool`);
            const authority = await poolAuthority(mint);
            const pc = cache();
            pc[PAMM_CACHE.poolBaseVault] = fixtureAddress(`pamm_${role}_pool_base_vault`);
            pc[PAMM_CACHE.poolQuoteVault] = fixtureAddress(`pamm_${role}_pool_quote_vault`);
            const boost = await pammBoostVault(pool);
            pc[PAMM_CACHE.boostVault] = boost;
            pc[PAMM_CACHE.boostVaultAta] = await wsolAta(boost);
            const mk = market({
                mint,
                tokenProgram: fixture(`pamm_${role}_mint`).owner as Address,
                canonicalPool: pool,
                poolCache: pc,
                pumpPoolAuthority: authority,
            });
            expect(await pammLpMint(pool)).toBe(fixtureAddress(`pamm_${role}_lp_mint`));
            const read = [
                acc(`pamm_${role}_pool`),
                acc(`pamm_${role}_pool_base_vault`),
                acc(`pamm_${role}_pool_quote_vault`),
                acc('pamm_state_global_config'),
                acc('pamm_state_fee_config'),
                acc(`pamm_${role}_boost_vault_ata`),
                acc(`pamm_${role}_lp_mint`),
            ];
            const supply = big(dec(`pamm_${role}_mint`).supply);
            const s = readVenueState(m, 'pump_swap', mk, read, { now: 0n, supply, pEma: 0n });
            const p = dec(`pamm_${role}_pool`);
            const base = big(dec(`pamm_${role}_pool_base_vault`).amount);
            const quote = big(dec(`pamm_${role}_pool_quote_vault`).amount);
            const vqr = big(p.virtual_quote_reserves ?? 0);
            expect(s.executable).toBe(true);
            expect(s.x).toBe(base);
            expect(s.y).toBe(quote + vqr);
            expect(s.r).toBe(quote < quote + vqr ? quote : quote + vqr);
            expect(s.pool?.isCashback).toBe(p.is_cashback_coin === 1);
            expect(s.feePpm).toBeGreaterThan(0);
            const lp = big(dec(`pamm_${role}_lp_mint`).supply);
            expect(s.lockedDepth).toBe(lockedDepth(m, big(p.lp_supply), lp, base, quote + vqr));
            const q = base / 1_000n;
            const sale = quoteSell(m, s, q);
            const buy = quoteBuy(m, s, q);
            expect(sale && buy && buy.cost > sale.proceeds).toBe(true);
            // Without the supply the fee tier cannot resolve: not executable (A63).
            expect(readVenueState(m, 'pump_swap', mk, read, { now: 0n, supply: null, pEma: 0n }).executable).toBe(
                false,
            );
        });
    }
});

describe('Raydium', () => {
    it('AMM v4 Fartcoin: SOL is the coin, fee 25 bps, D = ⌊9,978·√(X·Y)/10⁴⌋', () => {
        const pc = cache();
        pc[RAY_CACHE.pool] = fixtureAddress('ray_v4_fartcoin_pool');
        pc[RAY_CACHE.vaultSol] = fixtureAddress('ray_v4_fartcoin_coin_vault');
        pc[RAY_CACHE.vaultTok] = fixtureAddress('ray_v4_fartcoin_pc_vault');
        pc[RAY_CACHE.lpMint] = fixtureAddress('ray_v4_fartcoin_lp_mint');
        const mk = market({
            mint: fixtureAddress('mint_fartcoin'),
            tokenProgram: 'TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA' as Address,
            poolCache: pc,
            flags: MARKET_FLAG.solIsToken0OrCoin,
        });
        const read = [
            acc('ray_v4_fartcoin_pool'),
            acc('ray_v4_fartcoin_coin_vault'),
            acc('ray_v4_fartcoin_pc_vault'),
            acc('ray_v4_fartcoin_lp_mint'),
        ];
        const now = snapshotTime('raydium_state');
        const s = readVenueState(m, 'raydium_v4', mk, read, { now, supply: null, pEma: 0n });
        expect(s.executable).toBe(true);
        expect(s.x).toBe(big(dec('ray_v4_fartcoin_pc_vault').amount));
        expect(s.y).toBe(big(dec('ray_v4_fartcoin_coin_vault').amount));
        expect(s.feePpm).toBe(2_500);
        expect(s.lockedDepth).toBe((9_978n * m.liquidity(s.x, s.y)) / 10_000n);
        expect(liquidityOf(m, s)).toBe(m.liquidity(s.x, s.y));
        // The market's recorded SOL side must match the pool's: the other side is not executable.
        const wrong = readVenueState(m, 'raydium_v4', { ...mk, flags: 0 }, read, { now, supply: null, pEma: 0n });
        expect(wrong.executable).toBe(false);
    });

    it('CPMM USELESS: WSOL token 0, vault − counters, the config fee', () => {
        const p = dec('cpmm_useless_pool');
        const pc = cache();
        pc[RAY_CACHE.pool] = fixtureAddress('cpmm_useless_pool');
        pc[RAY_CACHE.ammConfig] = p.amm_config as Address;
        pc[RAY_CACHE.vaultSol] = fixtureAddress('cpmm_useless_token_0_vault');
        pc[RAY_CACHE.vaultTok] = fixtureAddress('cpmm_useless_token_1_vault');
        pc[RAY_CACHE.observation] = p.observation_key as Address;
        pc[RAY_CACHE.lpMint] = fixtureAddress('cpmm_useless_lp_mint');
        const config = ['00', '01', '02', '03', '04', '05', '06', '07', '08', '09', '10']
            .map((i) => `cpmm_config_${i}`)
            .find((n) => fixture(n).address === p.amm_config);
        if (!config) throw new Error('config');
        const mk = market({
            mint: fixtureAddress('cpmm_useless_token_mint'),
            tokenProgram: 'TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA' as Address,
            poolCache: pc,
            flags: MARKET_FLAG.solIsToken0OrCoin,
        });
        const read = [
            acc('cpmm_useless_pool'),
            acc(config),
            acc('cpmm_useless_token_0_vault'),
            acc('cpmm_useless_token_1_vault'),
            acc('cpmm_useless_lp_mint'),
        ];
        const s = readVenueState(m, 'raydium_cpmm', mk, read, {
            now: snapshotTime('raydium_state'),
            supply: null,
            pEma: 0n,
        });
        expect(s.executable).toBe(true);
        const counters0 = big(p.protocol_fees_token_0) + big(p.fund_fees_token_0) + big(p.creator_fees_token_0 ?? 0);
        const counters1 = big(p.protocol_fees_token_1) + big(p.fund_fees_token_1) + big(p.creator_fees_token_1 ?? 0);
        expect(s.y).toBe(big(dec('cpmm_useless_token_0_vault').amount) - counters0);
        expect(s.x).toBe(big(dec('cpmm_useless_token_1_vault').amount) - counters1);
        expect(s.feePpm).toBe(Number(big(dec(config).trade_fee_rate)));
        const q = s.x / 1_000n;
        expect(quoteSell(m, s, q)?.proceeds).toBeGreaterThan(0n);
    });
});

describe('detection (classify_pump)', () => {
    async function detect(role: string) {
        const mint = fixtureAddress(`pump_${role}_mint`);
        const authority = await poolAuthority(mint);
        const cp = await canonicalPool(mint, authority);
        const pool = (() => {
            try {
                return acc(`pump_${role}_pool`);
            } catch {
                return null;
            }
        })();
        return detectFrom(
            mint,
            { bondingCurve: fixtureAddress(`pump_${role}_curve`), canonicalPool: cp, authority },
            acc(`pump_${role}_curve`),
            pool,
        );
    }

    it('lists a live curve with the pump init set', async () => {
        const d = await detect('fresh');
        expect(d.kind).toBe('pump_curve');
        expect(d.refusal).toBeNull();
        expect(d.remainingAccounts.map((a) => a.role)).toEqual([0, 0, 0, 1, 0]);
        expect(d.remainingAccounts[2]?.address).toBe('6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P');
    });

    it('lists a graduated coin on its canonical pool with the PumpSwap init set', async () => {
        const d = await detect('graduated');
        expect(d.kind).toBe('pump_swap');
        expect(d.remainingAccounts[2]?.address).toBe('pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA');
    });

    it('refuses mayhem and USDC curves, and a short curve without a payer to extend it', async () => {
        expect((await detect('mayhem')).refusal).toBe('CurveMayhemUnsupported');
        expect((await detect('usdc')).refusal).toBe('CurveQuoteNotSol');
        const legacy = await detect('legacy49');
        expect(legacy.refusal).toBe('CurveNeedsExtend');
    });

    it('a curve that graduated elsewhere lists as list-only', async () => {
        const d = await detect('fartcoin');
        expect(d.kind).toBe('none');
        expect(d.refusal).toBeNull();
        expect(d.remainingAccounts).toHaveLength(2);
    });
});
