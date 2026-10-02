/**
 * The lend previews against pool_model.py v0.4.1 (FRONTEND.md §10.1, LEND): the account-shaped
 * states of `vectors/lend_vectors.json` go through the panel's own conversion (`previews.ts`) into
 * `@shortwin/math-wasm`, which must return exactly the model's deposits, withdrawals (tokens, and
 * the SOL part as a lower bound that is exact when nothing vests), claims, allowances, touches,
 * token values, locks, lock ends and vest releases.
 */
import { beforeAll, describe, expect, it } from 'vitest';
import { initMathSync, type MathModule } from '../../math/load.ts';
import { readRepo, readRepoBytes } from '../../test/repo.ts';
import {
    lessBps,
    MARKET_FLAG,
    potGainOfTouch,
    previewClaimView,
    previewClaimWithPot,
    previewDepositView,
    previewPosition,
    previewTouched,
    previewWithdrawLimits,
    previewWithdrawView,
    proRata,
    refusalOf,
    rewardsRequired,
    standingOf,
    toPoolState,
    wipeLive,
    wipeSweepPending,
} from './previews.ts';
import { simpleLender, simplePool, stateOf, type VectorCase } from './testkit.ts';

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

    it('withdrawals pay the model’s tokens; the SOL preview is a lower bound, exact when nothing vests', () => {
        let n = 0;
        let exact = 0;
        for (const c of cases) {
            const now = BigInt(c.now);
            const pool = toPoolState(m, stateOf(c, null).market, stateOf(c, null).vault);
            const release = m.previewVestRelease(pool, now);
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
                    const sol = BigInt(e.result.sol);
                    expect(r.value.solAtLeast <= sol, label).toBe(true);
                    const flush = BigInt(e.shares) === total && vestBal > 0n;
                    expect(r.value.lastLenderExit, label).toBe(flush);
                    if (release === 0n && !flush) {
                        expect(r.value.solAtLeast, label).toBe(sol);
                        exact++;
                    }
                }
                n++;
            }
        }
        expect(n).toBeGreaterThan(50);
        expect(exact).toBeGreaterThan(10);
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

function toPool(s: ReturnType<typeof simplePool>) {
    return toPoolState(m, s.market, s.vault);
}
