/**
 * The venue-state readers and the venue PDAs against the dumped mainnet fixtures
 * (`tests/fixtures/accounts`) and the values `tests/fixtures/manifest.json` decodes for them
 * (FRONTEND.md §10.1 SHORT: "venue-state readers against the fixture accounts").
 */
import type { Address } from '@solana/kit';
import { describe, expect, it } from 'vitest';
import { PUMP_PROGRAM, TOKEN_2022_PROGRAM, TOKEN_PROGRAM } from './constants.ts';
import {
    ata,
    bondingCurve,
    canonicalPool,
    pammBoostVault,
    pammCoinCreatorVaultAuthority,
    pammLpMint,
    poolAuthority,
    pumpCreatorVault,
    wsolAta,
} from './pda.ts';
import {
    lambdaBps,
    pammReserves,
    readAmmInfo,
    readCpmmConfig,
    readCpmmPool,
    readCurve,
    readFeeConfig,
    readMintBase,
    readPammGlobalConfig,
    readPammPool,
    readPumpGlobal,
    readTokenAccount,
    v4SolIsCoin,
    value,
} from './readers.ts';
import { fixture, fixtureAccount, fixtureAddress, fixtureManifest } from './testing.ts';

const dec = (name: string) => fixture(name).decoded as Record<string, unknown>;
const big = (v: unknown) => BigInt(v as number | string);
const nullKey = (v: unknown) => (v === null ? null : (v as string));

function acc(name: string) {
    const a = fixtureAccount(name);
    if (!a) throw new Error(`fixture ${name} is absent`);
    return a;
}

describe('token accounts and mints', () => {
    const tokenNames = fixtureManifest()
        .accounts.filter((a) => a.decoded && 'amount' in a.decoded && a.file)
        .map((a) => a.name);
    const mintNames = fixtureManifest()
        .accounts.filter((a) => a.decoded && 'supply' in a.decoded && a.file && 'program' in a.decoded)
        .map((a) => a.name);

    it('covers the fixtures', () => {
        expect(tokenNames.length).toBeGreaterThan(20);
        expect(mintNames.length).toBeGreaterThan(10);
    });

    for (const n of tokenNames) {
        it(`token account ${n}`, () => {
            const a = acc(n);
            const t = value(readTokenAccount(a.owner, a.data));
            const d = dec(n);
            expect(t).not.toBeNull();
            expect(t?.mint).toBe(d.mint);
            expect(t?.owner).toBe(d.owner);
            expect(t?.amount).toBe(big(d.amount));
            expect(t?.delegate ?? null).toBe(nullKey(d.delegate));
            expect(t?.isNative ?? null).toBe(d.is_native === null ? null : big(d.is_native));
        });
    }

    for (const n of mintNames) {
        it(`mint ${n}`, () => {
            const a = acc(n);
            const m = value(readMintBase(a.owner, a.data));
            const d = dec(n);
            expect(m?.supply).toBe(big(d.supply));
            expect(m?.decimals).toBe(d.decimals);
            expect(m?.mintAuthority ?? null).toBe(nullKey(d.mint_authority));
            expect(m?.freezeAuthority ?? null).toBe(nullKey(d.freeze_authority));
        });
    }

    it('refuses a foreign owner and a short account', () => {
        const a = acc('pump_fresh_curve_base_ata');
        expect(readTokenAccount(PUMP_PROGRAM, a.data)).toEqual({ ok: false, error: 'WrongOwner' });
        expect(readTokenAccount(TOKEN_PROGRAM, a.data.subarray(0, 100))).toEqual({ ok: false, error: 'TooShort' });
        // A Token-2022-length account under SPL Token is not a token account of it.
        expect(readTokenAccount(TOKEN_PROGRAM, a.data)).toEqual({ ok: false, error: 'WrongTag' });
        expect(value(readTokenAccount(TOKEN_2022_PROGRAM, a.data))?.amount).toBe(
            big(dec('pump_fresh_curve_base_ata').amount),
        );
    });
});

describe('pump', () => {
    const curves = fixtureManifest()
        .accounts.filter((a) => a.name.startsWith('pump_') && a.name.endsWith('_curve') && a.file && a.decoded)
        .map((a) => a.name);

    for (const n of curves) {
        it(`bonding curve ${n}`, () => {
            const a = acc(n);
            const c = value(readCurve(a.owner, a.data));
            const d = dec(n);
            expect(c).not.toBeNull();
            expect(c?.vt).toBe(big(d.virtual_token_reserves));
            expect(c?.vq).toBe(big(d.virtual_quote_reserves));
            expect(c?.rt).toBe(big(d.real_token_reserves));
            expect(c?.rq).toBe(big(d.real_quote_reserves));
            expect(c?.complete).toBe(d.complete === 1);
            expect(c?.isMayhemMode).toBe(d.is_mayhem_mode === 1);
            expect(c?.isCashbackCoin).toBe(d.is_cashback_coin === 1);
            expect(c?.creatorFeeBps).toBe(big(d.creator_fee_bps));
            if (typeof d.creator === 'string') expect(c?.creator).toBe(d.creator);
            if (typeof d.quote_mint === 'string') expect(c?.quoteMint).toBe(d.quote_mint);
        });
    }

    it('Global: the recipient lists, the migration fee, creator_fee_configurable', () => {
        const a = acc('pump_global');
        const g = value(readPumpGlobal(a.owner, a.data));
        const d = dec('pump_global');
        expect(g?.feeRecipients).toEqual(d.fee_recipients);
        expect(g?.buybackFeeRecipients).toEqual(d.buyback_fee_recipients);
        expect(g?.poolMigrationFee).toBe(big(d.pool_migration_fee));
        expect(g?.creatorFeeConfigurable).toBe(d.creator_fee_configurable === 1 || d.creator_fee_configurable === true);
        expect(readPumpGlobal(a.owner, a.data.subarray(0, 1000))).toEqual({ ok: false, error: 'TooShort' });
    });

    for (const n of ['pump_fee_config', 'pamm_fee_config']) {
        it(`FeeConfig ${n}`, () => {
            const a = acc(n);
            const f = value(readFeeConfig(a.owner, a.data));
            const d = dec(n) as { fee_tiers: { threshold: number; lp: number; protocol: number; creator: number }[] };
            expect(f?.feeTiers).toEqual(
                d.fee_tiers.map((t) => ({
                    threshold: BigInt(t.threshold),
                    lp: t.lp,
                    protocol: t.protocol,
                    creator: t.creator,
                })),
            );
        });
    }

    it('PDAs: the bonding curve, the curve base ATA and the creator vault of every coin', async () => {
        const coins = (
            fixtureManifest().snapshots.find((s) => s.group === 'pump_state') as unknown as {
                coins: Record<string, string>;
            }
        ).coins;
        for (const [role, mint] of Object.entries(coins)) {
            const curve = fixture(`pump_${role}_curve`);
            expect(await bondingCurve(mint as Address)).toBe(curve.address);
            const base = fixture(`pump_${role}_curve_base_ata`);
            const mintAcc = fixture(`pump_${role}_mint`);
            if (base.address && mintAcc.owner) {
                expect(await ata(curve.address as Address, mint as Address, mintAcc.owner as Address)).toBe(
                    base.address,
                );
            }
            const vault = fixtureManifest().accounts.find((a) => a.name === `pump_${role}_creator_vault`);
            const creator = (curve.decoded as { creator?: string } | null)?.creator;
            if (vault?.address && creator) expect(await pumpCreatorVault(creator as Address)).toBe(vault.address);
        }
    });
});

describe('PumpSwap', () => {
    const roles = ['fresh', 'mature', 'cashback', 'legacy211', 'legacy261'];

    for (const role of roles) {
        it(`pool ${role}: fields, reserves and its PDAs`, async () => {
            const n = `pamm_${role}_pool`;
            const a = acc(n);
            const p = value(readPammPool(a.owner, a.data));
            const d = dec(n);
            expect(p?.index).toBe(d.index);
            expect(p?.creator).toBe(d.creator);
            expect(p?.baseMint).toBe(d.base_mint);
            expect(p?.lpMint).toBe(d.lp_mint);
            expect(p?.poolBaseTokenAccount).toBe(d.pool_base_token_account);
            expect(p?.poolQuoteTokenAccount).toBe(d.pool_quote_token_account);
            expect(p?.lpSupply).toBe(big(d.lp_supply));
            expect(p?.virtualQuoteReserves).toBe(big(d.virtual_quote_reserves ?? 0));
            if (!p) return;
            const mint = p.baseMint;
            expect(await poolAuthority(mint)).toBe(fixtureAddress(`pamm_${role}_pool_authority`));
            expect(await canonicalPool(mint)).toBe(a.address);
            expect(await pammLpMint(a.address)).toBe(p.lpMint);
            const boost = await pammBoostVault(a.address);
            expect(boost).toBe(fixtureAddress(`pamm_${role}_boost_vault`));
            expect(await wsolAta(boost)).toBe(fixtureAddress(`pamm_${role}_boost_vault_ata`));
            const auth = await pammCoinCreatorVaultAuthority(p.coinCreator);
            expect(auth).toBe(fixtureAddress(`pamm_${role}_coin_creator_vault_authority`));
            expect(await wsolAta(auth)).toBe(fixtureAddress(`pamm_${role}_coin_creator_vault_ata`));
        });
    }

    it('a negative VQR prices on vault + VQR and the real SOL is the smaller', () => {
        const a = acc('pamm_mature_pool_negative_vqr');
        const p = value(readPammPool(a.owner, a.data));
        if (!p) throw new Error('pool');
        expect(p.virtualQuoteReserves < 0n).toBe(true);
        const r = pammReserves(p, 1_000n, 1_000_000_000_000n);
        expect(r?.effectiveQuote).toBe(1_000_000_000_000n + p.virtualQuoteReserves);
        expect(pammReserves(p, 1n, 1n)).toBeNull();
    });

    it('GlobalConfig: disable flags and the recipient lists', () => {
        const a = acc('pamm_global_config');
        const g = value(readPammGlobalConfig(a.owner, a.data));
        const d = dec('pamm_global_config');
        expect(g?.disableFlags).toBe(d.disable_flags);
        expect(g?.protocolFeeRecipients).toEqual(d.protocol_fee_recipients);
        expect(g?.buybackFeeRecipients).toEqual(d.buyback_fee_recipients);
    });
});

describe('Raydium', () => {
    for (const n of ['ray_v4_fartcoin_pool', 'ray_v4_fartcoin_rev_pool']) {
        it(`AMM v4 ${n}`, () => {
            const a = acc(n);
            const i = value(readAmmInfo(a.owner, a.data));
            const d = dec(n);
            expect(i?.status).toBe(big(d.status));
            expect(i?.nonce).toBe(254n);
            expect(i?.swapFeeNumerator).toBe(big(d.swap_fee_numerator));
            expect(i?.swapFeeDenominator).toBe(big(d.swap_fee_denominator));
            expect(i?.coinVault).toBe(d.coin_vault);
            expect(i?.pcVault).toBe(d.pc_vault);
            expect(i?.lpMint).toBe(d.lp_mint);
            expect(i?.lpAmount).toBe(big(d.lp_amount));
            expect(readAmmInfo(a.owner, a.data.subarray(0, 700))).toEqual({ ok: false, error: 'TooShort' });
        });
    }

    it('the locked LP share λ of the Fartcoin pool (pool_select tests: 9,978 bps)', () => {
        const i = value(readAmmInfo(acc('ray_v4_fartcoin_pool').owner, acc('ray_v4_fartcoin_pool').data));
        if (!i) throw new Error('pool');
        const lp = value(readMintBase(TOKEN_PROGRAM, acc('ray_v4_fartcoin_lp_mint').data));
        expect(lambdaBps(i.lpAmount, lp?.supply ?? 0n)).toBe(9_978);
        expect(v4SolIsCoin(i)).toBe(true);
        expect(lambdaBps(0n, 0n)).toBe(0);
        expect(lambdaBps(5n, 7n)).toBe(0);
    });

    for (const role of ['useless', 'creator_fee', 'token2022', 'wsol_token1', 'creator_both']) {
        it(`CPMM pool ${role}`, () => {
            const n = `cpmm_${role}_pool`;
            const a = acc(n);
            const p = value(readCpmmPool(a.owner, a.data));
            const d = dec(n);
            expect(p?.ammConfig).toBe(d.amm_config);
            expect(p?.token0Mint).toBe(d.token_0_mint);
            expect(p?.token1Mint).toBe(d.token_1_mint);
            expect(p?.token0Program).toBe(d.token_0_program);
            expect(p?.token1Program).toBe(d.token_1_program);
            expect(p?.observationKey).toBe(d.observation_key);
            expect(p?.lpSupply).toBe(big(d.lp_supply));
            expect(p?.protocolFeesToken0).toBe(big(d.protocol_fees_token_0));
            expect(p?.fundFeesToken1).toBe(big(d.fund_fees_token_1));
            expect(p?.openTime).toBe(big(d.open_time));
            expect(p?.creatorFeeOn).toBe(d.creator_fee_on);
            expect(p?.enableCreatorFee).toBe(d.enable_creator_fee === 1 || d.enable_creator_fee === true);
        });
    }

    for (let i = 0; i <= 20; i++) {
        const n = `cpmm_config_${String(i).padStart(2, '0')}`;
        it(`CPMM config ${n}`, () => {
            const a = acc(n);
            const c = value(readCpmmConfig(a.owner, a.data));
            const d = dec(n);
            expect(c?.tradeFeeRate).toBe(big(d.trade_fee_rate));
            expect(c?.protocolFeeRate).toBe(big(d.protocol_fee_rate));
            expect(c?.fundFeeRate).toBe(big(d.fund_fee_rate));
            expect(c?.creatorFeeRate).toBe(big(d.creator_fee_rate));
        });
    }
});
