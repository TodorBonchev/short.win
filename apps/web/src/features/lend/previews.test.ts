/**
 * The lend previews against pool_model.py v0.4.1 (FRONTEND.md §10.1, LEND): the account-shaped
 * states of `vectors/lend_vectors.json` go through the panel's own conversion (`previews.ts`) into
 * `@shortwin/math-wasm`, which must return exactly the model's deposits, withdrawals (tokens and
 * the SOL part, `previewWithdrawal`), claims, allowances, touches, token values, locks, lock ends
 * and vest releases.
 */

import { bigIntToPodU128 } from '@shortwin/client';
import { beforeAll, describe, expect, it } from 'vitest';
import { UNPAID_RENT_WARN_BPS, UNPAID_RENT_WARN_LAMPORTS } from '../../config/disclosures.ts';
import { initMathSync, type MathModule } from '../../math/load.ts';
import { readRepo, readRepoBytes } from '../../test/repo.ts';
import {
    lamportsAtPrice,
    lessBps,
    MARKET_FLAG,
    pendingLenderCredit,
    potGainOfTouch,
    previewClaimView,
    previewClaimWithPot,
    previewDepositView,
    previewPosition,
    previewTouched,
    previewUnpaidRent,
    previewWithdrawLimits,
    previewWithdrawView,
    proRata,
    refusalOf,
    rewardsRequired,
    standingOf,
    toPoolState,
    unpaidRentExposure,
    wipeLive,
    wipeSweepPending,
} from './previews.ts';
import { fakeMarket, simpleLender, simplePool, stateOf, type VectorCase } from './testkit.ts';

let m: MathModule;
let cases: VectorCase[];

beforeAll(() => {
    m = initMathSync(readRepoBytes('packages/math-wasm/pkg/shortwin_math_wasm_bg.wasm'));
    const doc = JSON.parse(readRepo('apps/web/src/features/lend/vectors/lend_vectors.json')) as {
        cases: VectorCase[];
        stats: Record<string, number>;
    };
    cases = doc.cases;
});

const resultOf = <T>(r: { ok: true; value: T } | { ok: false; refusal: { name: string } }): T | string =>
    r.ok ? r.value : `!${r.refusal.name}`;

describe('pool_model vectors through the WASM facade', () => {
    it('cover lenders behind share epochs, live wipes, locks and vests', () => {
        expect(cases.length).toBeGreaterThan(30);
        const behind = cases.filter((c) =>
            Object.values(c.lenders).some((l) => Number(l.epoch) < Number(c.market.shareEpoch)),
        );
        expect(behind.length).toBeGreaterThan(5);
        expect(cases.some((c) => (Number(c.market.flags) & MARKET_FLAG.wipeLive) !== 0)).toBe(true);
        expect(cases.some((c) => Object.keys(c.vault.lockCal).length > 0)).toBe(true);
        expect(cases.some((c) => c.market.vestBal !== '0')).toBe(true);
    });

    it('deposits mint what the model mints (or refuse as it does)', () => {
        let n = 0;
        for (const c of cases) {
            for (const e of c.expect) {
                if (e.op !== 'deposit') continue;
                const s = stateOf(c, e.lender);
                const r = previewDepositView(m, s, BigInt(c.now), BigInt(e.amount), 0);
                const got = r.ok ? r.value.shares.toString() : `!${r.refusal.name}`;
                expect(got, `${c.name} deposit ${e.lender} ${e.amount}`).toBe(e.result);
                n++;
            }
        }
        expect(n).toBeGreaterThan(50);
    });

    it('withdrawals pay the model’s tokens and SOL (previewWithdrawal: claim right after, vest flush included)', () => {
        let n = 0;
        for (const c of cases) {
            const now = BigInt(c.now);
            const pool = toPoolState(m, stateOf(c, null).market, stateOf(c, null).vault);
            const total = pool.totalShares;
            const vestBal = pool.vestBal;
            pool.free();
            for (const e of c.expect) {
                if (e.op !== 'withdraw') continue;
                const s = stateOf(c, e.lender);
                const r = previewWithdrawView(m, s, now, c.lockGraceS, BigInt(e.shares), 0);
                const label = `${c.name} withdraw ${e.lender} ${e.shares}`;
                if (typeof e.result === 'string') {
                    expect(r.ok ? 'ok' : `!${r.refusal.name}`, label).toBe(e.result);
                } else {
                    expect(r.ok, label).toBe(true);
                    if (!r.ok) continue;
                    expect(r.value.tokens.toString(), label).toBe(e.result.tokens);
                    // FE-15: the facade's whole withdrawal (`claim` right after the burn): the model's `sol`.
                    expect(r.value.solAtLeast, label).toBe(BigInt(e.result.sol));
                    const flush = BigInt(e.shares) === total && vestBal > 0n;
                    expect(r.value.lastLenderExit, label).toBe(flush);
                }
                n++;
            }
        }
        expect(n).toBeGreaterThan(50);
    });

    it('claims, allowances, touches, token values and locks equal the model', () => {
        let n = 0;
        for (const c of cases) {
            const now = BigInt(c.now);
            for (const e of c.expect) {
                if (
                    e.op !== 'touch' &&
                    e.op !== 'claim' &&
                    e.op !== 'allowance' &&
                    e.op !== 'value' &&
                    e.op !== 'lock'
                ) {
                    continue;
                }
                const s = stateOf(c, e.lender);
                const label = `${c.name} ${e.op} ${e.lender}`;
                if (e.op === 'touch') {
                    const r = previewTouched(m, s, now);
                    if (typeof e.result === 'string') {
                        expect(resultOf(r), label).toBe(e.result);
                    } else {
                        expect(r.ok, label).toBe(true);
                        if (!r.ok) continue;
                        expect(
                            {
                                pending: r.value.pending.toString(),
                                real: r.value.real.toString(),
                                claimed: r.value.claimed.toString(),
                                shares: r.value.shares.toString(),
                                lock: r.value.lock.toString(),
                                epoch: r.value.epoch,
                            },
                            label,
                        ).toEqual(e.result);
                    }
                    n++;
                    continue;
                }
                const p = previewPosition(m, s, now);
                expect(p.ok, label).toBe(true);
                if (!p.ok) continue;
                const v = p.value;
                const got =
                    e.op === 'claim'
                        ? v.claimable
                        : e.op === 'allowance'
                          ? v.allowance
                          : e.op === 'value'
                            ? v.value
                            : v.lock;
                expect(got.toString(), label).toBe(e.result);
                n++;
            }
        }
        expect(n).toBeGreaterThan(300);
    });

    it('lock ends and vest releases equal the model', () => {
        for (const c of cases) {
            const now = BigInt(c.now);
            for (const e of c.expect) {
                if (e.op === 'lockUntil') {
                    const r = previewWithdrawLimits(m, stateOf(c, null), now, c.lockGraceS);
                    expect(r.ok && r.value.lockUntilIfLocked.toString(), c.name).toBe(e.result);
                } else if (e.op === 'release') {
                    const s = stateOf(c, null);
                    const pool = toPoolState(m, s.market, s.vault);
                    expect(m.previewVestRelease(pool, now).toString(), c.name).toBe(e.result);
                    pool.free();
                }
            }
        }
    });

    it('claim views agree with the position view and never pay more than the entitlement', () => {
        for (const c of cases) {
            for (const who of Object.keys(c.lenders)) {
                const s = stateOf(c, who);
                const now = BigInt(c.now);
                const claim = previewClaimView(m, s, now);
                const pos = previewPosition(m, s, now);
                expect(claim.ok && pos.ok, c.name).toBe(true);
                if (!claim.ok || !pos.ok) continue;
                expect(claim.value.claimable).toBe(pos.value.claimable);
                expect(claim.value.claimable <= claim.value.entitled).toBe(true);
                expect(claim.value.deficit > 0n ? pos.value.pending < 0n : true).toBe(true);
            }
        }
    });
});

describe('lend previews on simple pools', () => {
    it('quotes a first deposit with the virtual shares and the tolerance', () => {
        const s = { ...simplePool({ tokensFree: 0n, totalShares: 0n }), lender: null, records: [] };
        const r = previewDepositView(m, s, 100n, 1_000_000_000n, 50);
        expect(r.ok).toBe(true);
        if (!r.ok) return;
        // ⌊a·(S + VS)/(T + B + VA)⌋ = 10⁹ · 1,000 / 1 (ECON §9.2)
        expect(r.value.shares).toBe(1_000_000_000_000n);
        expect(r.value.minShares).toBe(995_000_000_000n);
        expect(r.value.shareBps).toBe(10_000);
        // ⌊10¹²·(10⁹ + 1)/(10¹² + 10³)⌋ = 10⁹: the virtual unit makes the first deposit exact
        expect(r.value.value).toBe(1_000_000_000n);
        expect(lessBps(1_000n, 50)).toBe(995n);
    });

    it('refuses a dust deposit and maps it to ZeroAmount', () => {
        const s = { ...simplePool({ tokensFree: 10n ** 15n, totalShares: 1n }), lender: null, records: [] };
        const r = previewDepositView(m, s, 0n, 1n, 50);
        expect(r.ok).toBe(false);
        if (!r.ok) {
            expect(r.refusal.name).toBe('ZeroAmount');
            expect(r.refusal.code).toBe(6076);
        }
    });

    it('waits for the post-wipe sweep, and previews the deposit once the sweep is bundled', () => {
        const flags = MARKET_FLAG.wipeLive;
        const curve = {
            ...simplePool({ tokensFree: 0n, totalShares: 0n, flags, venue: 1, shareEpoch: 1 }),
            lender: null,
            records: [],
        };
        expect(wipeLive(curve.market)).toBe(true);
        expect(wipeSweepPending(curve.market)).toBe(true);
        const refused = previewDepositView(m, curve, 0n, 10n ** 9n, 50);
        expect(!refused.ok && refused.refusal.name).toBe('WipeSweepPending');
        const bundled = previewDepositView(m, curve, 0n, 10n ** 9n, 50, true);
        expect(bundled.ok && bundled.value.shares).toBe(10n ** 12n);
        // The trader bit is enough off the pump venues; pump venues also need the rewards bit.
        const swept = { ...curve.market, flags: flags | MARKET_FLAG.wipeTraderSwept };
        expect(wipeSweepPending(swept)).toBe(true);
        expect(wipeSweepPending({ ...swept, venue: 6 })).toBe(false);
        expect(wipeSweepPending({ ...swept, flags: swept.flags | MARKET_FLAG.wipeRewardsClaimed })).toBe(false);
        expect([0, 1, 2, 3, 4, 5, 6].map(rewardsRequired)).toEqual([false, true, true, true, false, false, false]);
    });

    it('splits a withdrawal into the allowance, the lock and the forfeited vest', () => {
        // 1,000 tokens lent, 500 borrowed; this lender holds half the shares; 1 SOL vesting half-way.
        const pool = simplePool({
            tokensFree: 500n,
            borrowedQ: 500n,
            totalShares: 1_000_000n,
            vestBal: 1_000_000_000n,
            vestLast: 0n,
            vestEnd: 21_600n,
            lenderSol: 1_000_000_000n,
            maxRelease: 86_400n * 3n,
        });
        const s = { ...pool, lender: simpleLender(500_000n), records: [] };
        const lim = previewWithdrawLimits(m, s, 10_800n, 86_400);
        expect(lim.ok).toBe(true);
        if (!lim.ok) return;
        expect(lim.value.allowance).toBe(250n);
        expect(lim.value.maxShares).toBe(m.sharesForTokens(toPool(s), 250n));
        expect(lim.value.lock).toBe(0n);
        // ⌈(max(now, max_release) + grace)/1 d⌉·1 d
        expect(lim.value.lockUntilIfLocked).toBe(86_400n * 4n);
        const w = previewWithdrawView(m, s, 10_800n, 86_400, lim.value.maxShares, 50);
        expect(w.ok).toBe(true);
        if (!w.ok) return;
        expect(w.value.tokens <= 250n).toBe(true);
        expect(w.value.minTokens).toBe(lessBps(w.value.tokens, 50));
        expect(w.value.lastLenderExit).toBe(false);
        expect(w.value.unreleased).toBe(500_000_000n);
        expect(w.value.vesting).toBe(250_000_000n);
        expect(w.value.vestingForfeit).toBe(proRata(500_000_000n, lim.value.maxShares, 1_000_000n));
        // FE-15: the lock the withdrawal leaves (`previewWithdrawal`'s lender). Burning the whole
        // allowance while half the tokens are borrowed locks every share left, until the new end.
        expect(w.value.lockAfter).toBe(500_000n - lim.value.maxShares);
        expect(w.value.lockUntilAfter).toBe(lim.value.lockUntilIfLocked);
        // Half of it locks ⌈x·(u − b)/b⌉ of the rest (x burnt, u unlocked, b the allowance in shares).
        const x = lim.value.maxShares / 2n;
        const half = previewWithdrawView(m, s, 10_800n, 86_400, x, 50);
        expect(half.ok).toBe(true);
        if (!half.ok) return;
        const b = lim.value.maxShares;
        const add = (x * (500_000n - b) + b - 1n) / b;
        expect(half.value.lockAfter).toBe(add < 500_000n - x ? add : 500_000n - x);
        expect(half.value.lockAfter > 0n && half.value.lockAfter < 500_000n - x).toBe(true);
        expect(half.value.lockUntilAfter).toBe(lim.value.lockUntilIfLocked);
        const over = previewWithdrawView(m, s, 10_800n, 86_400, 500_000n, 50);
        expect(!over.ok && over.refusal.name).toBe('AboveProRataAllowance');
    });

    it('pays the last lender the unreleased vest and forfeits nothing', () => {
        const pool = simplePool({
            tokensFree: 1_000n,
            totalShares: 1_000_000n,
            vestBal: 600n,
            vestEnd: 600n,
            lenderSol: 600n,
        });
        const s = { ...pool, lender: simpleLender(1_000_000n), records: [] };
        const w = previewWithdrawView(m, s, 0n, 86_400, 1_000_000n, 50);
        expect(w.ok && w.value.lastLenderExit).toBe(true);
        expect(w.ok && w.value.vestingForfeit).toBe(0n);
        // Nothing is borrowed: no lock afterwards, and no end to show.
        expect(w.ok && [w.value.lockAfter, w.value.lockUntilAfter]).toEqual([0n, 0n]);
        // FE-15: the SOL preview is the facade's withdrawal (claim right after the burn), the flush
        // included — what the app composed by hand until M10 (previewClaim + the vest).
        const before = previewClaimView(m, s, 0n);
        expect(before.ok && before.value.claimable).toBe(0n);
        expect(w.ok && w.value.solAtLeast).toBe(600n);
        // A partial exit is no flush: the vest stays with the shares.
        const part = previewWithdrawView(m, s, 0n, 86_400, 999_999n, 50);
        expect(part.ok && [part.value.lastLenderExit, part.value.solAtLeast]).toEqual([false, 0n]);
    });

    it('refuses a withdrawal with a negative balance (cover the deficit first)', () => {
        const pool = simplePool({ tokensFree: 1_000n, totalShares: 1_000_000n });
        const s = { ...pool, lender: simpleLender(1_000_000n, { pending: -5n }), records: [] };
        const w = previewWithdrawView(m, s, 0n, 86_400, 10n, 50);
        expect(!w.ok && w.refusal.name).toBe('NegativePending');
        const c = previewClaimView(m, s, 0n);
        expect(c.ok && c.value.deficit).toBe(5n);
    });

    it('measures what another lender’s touch adds to the pot and claims against it', () => {
        const pool = simplePool({ tokensFree: 1_000n, totalShares: 1_000_000n, lenderSol: 1_000n });
        // A lender with 300 realised but only 100 entitled: its touch moves 200 to the pot.
        const rich = { ...pool, lender: simpleLender(500_000n, { pending: 100n, real: 300n }), records: [] };
        expect(potGainOfTouch(m, rich, 0n)).toBe(200n);
        // A lender entitled to 250 with nothing realised: the pot pays it.
        const me = { ...pool, lender: simpleLender(500_000n, { pending: 250n }), records: [] };
        const view = previewClaimView(m, me, 0n);
        expect(view.ok && view.value.claimable).toBe(0n);
        expect(view.ok && view.value.shortfall).toBe(250n);
        expect(previewClaimWithPot(m, me, 0n, 200n)).toEqual({ ok: true, value: 200n });
    });

    it('knows where a lender stands against the share epoch', () => {
        const pool = simplePool({ shareEpoch: 3 });
        expect(standingOf(pool.market, null)).toEqual({ kind: 'none' });
        expect(standingOf(pool.market, simpleLender(1n, { epoch: 3 }))).toEqual({ kind: 'current' });
        expect(standingOf(pool.market, simpleLender(1n, { epoch: 1 }))).toEqual({ kind: 'behind', epochs: 2 });
        const wiped = simplePool({ shareEpoch: 3, totalShares: 0n, flags: MARKET_FLAG.wipeLive });
        expect(standingOf(wiped.market, simpleLender(1n, { epoch: 2 }))).toEqual({ kind: 'parked' });
    });

    it('maps facade refusals to program errors', () => {
        expect(refusalOf(new Error('above pro-rata allowance'))).toMatchObject({ name: 'AboveProRataAllowance' });
        expect(refusalOf(new Error('lender behind share epochs'))).toMatchObject({ name: 'LenderBehindEpochs' });
        expect(refusalOf(new Error('post-wipe sweep pending'))).toMatchObject({ name: 'WipeSweepPending' });
        expect(refusalOf(new Error('something else'))).toMatchObject({ name: 'MathOverflow', code: 6000 });
    });
});

describe('the pool’s unpaid rent at a deposit (ECON §9.4, lender.unpaidRent)', () => {
    const WAD = 10n ** 18n;
    const S = 3_000_000_000_000n;
    const exposure = (receivable: bigint, shares: bigint, amount: bigint, emaPrice = 0n) =>
        unpaidRentExposure(receivable, S, shares, amount, emaPrice);
    // One open position in the aggregate: q = 10⁹ raw at Tier A's curve (10 % / 50 % / 1,000 %),
    // rent_protocol_bps 1,500 — `A_j = q·coef_j·(10⁴ − 1,500)` (`rent::Aggr::add`).
    const A = [1_000n, 5_000n, 100_000n].map((c) => 1_000_000_000n * c * 8_500n);
    const accruing = (o: { receivable?: bigint; lastAccrualTs?: bigint; aggr?: readonly bigint[] } = {}) =>
        fakeMarket(
            { tokensFree: 10n ** 12n, totalShares: S, receivable: o.receivable ?? 0n },
            {
                lastAccrualTs: o.lastAccrualTs ?? 1_000n,
                emaPrice: bigIntToPodU128(WAD),
                rentAggr: (o.aggr ?? A).map((a) => bigIntToPodU128(a)),
            },
        );

    it('is the deposit’s pro-rata part of the receivable at the pool’s size after it, rounded up', () => {
        // 10¹² new shares of 4·10¹²: a quarter of 7 lamports is 1.75 → 2.
        const v = exposure(7n, 1_000_000_000_000n, 10n ** 9n);
        expect(v.exposure).toBe(2n);
        expect(v.shareBps).toBe(2_500);
        // Exact multiples are not rounded.
        expect(exposure(8n, 1_000_000_000_000n, 10n ** 9n).exposure).toBe(2n);
        // No shares minted, nothing carried; a pool with nothing unpaid shows nothing.
        expect(exposure(8n, 0n, 0n)).toMatchObject({ exposure: 0n, level: 'info' });
        expect(exposure(0n, 1_000_000_000_000n, 10n ** 9n)).toMatchObject({ exposure: 0n, level: 'none' });
    });

    it('values the deposit at P_ema and warns from 1 % of that value or from 1 SOL', () => {
        expect(UNPAID_RENT_WARN_BPS).toBe(100);
        expect(UNPAID_RENT_WARN_LAMPORTS).toBe(1_000_000_000n);
        // 1 lamport per raw unit: 10⁹ raw tokens are worth 1 SOL.
        expect(lamportsAtPrice(10n ** 9n, WAD)).toBe(10n ** 9n);
        expect(lamportsAtPrice(10n ** 9n, 0n)).toBeNull();
        const shares = 1_000_000_000_000n; // a quarter of the pool after the deposit
        // 1 % of 1 SOL is 10⁷ lamports: a receivable of 4·10⁷ puts exactly that on the deposit.
        const at = exposure(40_000_000n, shares, 10n ** 9n, WAD);
        expect(at).toMatchObject({ exposure: 10_000_000n, value: 10n ** 9n, level: 'warn' });
        const below = exposure(39_999_996n, shares, 10n ** 9n, WAD);
        expect(below).toMatchObject({ exposure: 9_999_999n, level: 'info' });
        // Without a price only the absolute threshold applies.
        expect(exposure(40_000_000n, shares, 10n ** 9n)).toMatchObject({ value: null, level: 'info' });
        expect(exposure(4_000_000_000n, shares, 10n ** 9n)).toMatchObject({
            exposure: 1_000_000_000n,
            level: 'warn',
        });
        expect(exposure(3_999_999_996n, shares, 10n ** 9n).level).toBe('info');
        // A large deposit at a high price: 1 SOL carried warns even below 1 % of its value.
        const whale = exposure(4_000_000_000n, shares, 10n ** 9n, 1_000n * WAD);
        expect(whale).toMatchObject({ value: 10n ** 12n, level: 'warn' });
    });

    it('adds the rent the deposit’s own prologue credits first (engine::prologue::accrue)', () => {
        // U = 0: only J0 moves, P_ema·dt; the lenders get 85 % of the 10 % APR on 1 SOL of tokens.
        expect(pendingLenderCredit(m, accruing(), 1_000n + 31_536_000n)).toBe(85_000_000n);
        // A day: ⌊8.5·10⁷·86,400/31,536,000⌋.
        expect(pendingLenderCredit(m, accruing(), 1_000n + 86_400n)).toBe(232_876n);
        // Nothing elapsed, or no open position: nothing pending.
        expect(pendingLenderCredit(m, accruing(), 1_000n)).toBe(0n);
        expect(pendingLenderCredit(m, accruing({ aggr: [0n, 0n, 0n] }), 1_000n + 86_400n)).toBe(0n);

        // A pool whose rent nothing has credited yet (stored receivable 0, keepers quiet): the rows show.
        const shares = 1_000_000_000_000n;
        const v = previewUnpaidRent(m, accruing(), 1_000n + 86_400n, shares, 10n ** 9n);
        expect(v).toMatchObject({ stored: 0n, pending: 232_876n, receivable: 232_876n, level: 'info' });
        expect(v.exposure).toBe((232_876n * shares + S + shares - 1n) / (S + shares));
        // On top of a stored receivable; the same second as the last accrual adds nothing.
        const w = previewUnpaidRent(m, accruing({ receivable: 7n }), 1_000n + 86_400n, shares, 10n ** 9n);
        expect(w.receivable).toBe(232_883n);
        expect(previewUnpaidRent(m, accruing(), 1_000n, shares, 10n ** 9n)).toMatchObject({
            receivable: 0n,
            exposure: 0n,
            level: 'none',
        });
    });

    it('reads the shares the deposit preview mints (the WASM)', () => {
        const market = fakeMarket({ receivable: 1_000_000n });
        const s = { market, vault: simplePool().vault, lender: null, records: [] };
        const r = previewDepositView(m, s, 100n, 1_000_000_000n, 50);
        expect(r.ok).toBe(true);
        if (!r.ok) return;
        const total = 1_000_000_000_000n + r.value.shares;
        const v = previewUnpaidRent(m, market, 100n, r.value.shares, 1_000_000_000n);
        expect(v).toMatchObject({ stored: 1_000_000n, pending: 0n, receivable: 1_000_000n });
        expect(v.exposure).toBe((1_000_000n * r.value.shares + total - 1n) / total);
        expect(v.shareBps).toBe(r.value.shareBps);
    });
});

function toPool(s: ReturnType<typeof simplePool>) {
    return toPoolState(m, s.market, s.vault);
}
