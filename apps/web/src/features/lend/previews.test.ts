/**
 * The lend previews against pool_model.py v0.4.2 with the AI-review fixes (FRONTEND.md §10.1, LEND;
 * AIR-02, -03, -07, -08, -12: `gen_lend_vectors.py` regenerates the vectors): the account-shaped
 * states of `vectors/lend_vectors.json` go through the panel's own conversion (`previews.ts`) into
 * `@shortwin/math-wasm`, which must return exactly the model's deposits, withdrawals (tokens and
 * the SOL part, `previewWithdrawal`), claims, allowances, touches, token values, locks, lock ends
 * and vest releases; a third party's sync of each lender (`previewTouchPot`: the pot-only touch and
 * its forward chain, KI-66 full, on the records from where its walk starts) and whether its claim
 * closes the position (`previewClaimOutcome`, KI-67).
 */

import { bigIntToPodI128, bigIntToPodU128 } from '@shortwin/client';
import { beforeAll, describe, expect, it } from 'vitest';
import { UNPAID_RENT_WARN_BPS, UNPAID_RENT_WARN_LAMPORTS } from '../../config/disclosures.ts';
import { initMathSync, type MathModule } from '../../math/load.ts';
import { readRepo, readRepoBytes } from '../../test/repo.ts';
import { potEpochs } from './chain.ts';
import {
    CLAIM_DUST_LAMPORTS,
    claimShortfall,
    closable,
    FWD,
    type LendState,
    lamportsAtPrice,
    lessBps,
    MARKET_FLAG,
    pendingLenderCredit,
    potGainOfSync,
    previewClaimOutcome,
    previewClaimView,
    previewClaimWithPot,
    previewDepositHold,
    previewDepositView,
    previewPosition,
    previewSubUnitConversion,
    previewTouched,
    previewTouchPot,
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
import {
    fakeMarket,
    NO_UVA,
    PUMP_UVA,
    recordOf,
    simpleLender,
    simplePool,
    stateOf,
    type VectorCase,
} from './testkit.ts';

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

describe('deposits after a halted-venue settlement', () => {
    const state = (o: Parameters<typeof simplePool>[0] = {}): LendState => ({
        ...simplePool({ vestBal: 600n, vestLast: 0n, vestEnd: 600n, depositHoldUntil: 600n, ...o }),
        lender: null,
        records: [],
    });

    it('refuses new deposits until the actual vest releases, including its final second', () => {
        for (const now of [0n, 300n, 599n]) {
            expect(resultOf(previewDepositHold(m, state(), now))).toBe(600n);
            expect(resultOf(previewDepositView(m, state(), now, 1_000n, 50))).toBe('!PoolDrained');
        }
        expect(resultOf(previewDepositHold(m, state(), 600n))).toBeNull();
        expect(previewDepositView(m, state(), 600n, 1_000n, 50).ok).toBe(true);
    });

    it('follows a later sweep’s extended vest instead of the saved marker deadline', () => {
        const s = state({ vestEnd: 1_200n });
        expect(resultOf(previewDepositHold(m, s, 900n))).toBe(1_200n);
        expect(resultOf(previewDepositView(m, s, 900n, 1_000n, 50))).toBe('!PoolDrained');
        expect(previewDepositView(m, s, 1_200n, 1_000n, 50).ok).toBe(true);
    });

    it('allows ordinary vesting income without a hold and a hold whose vest was already flushed', () => {
        for (const s of [state({ depositHoldUntil: -1n }), state({ vestBal: 0n })]) {
            expect(resultOf(previewDepositHold(m, s, 300n))).toBeNull();
            expect(previewDepositView(m, s, 300n, 1_000n, 50).ok).toBe(true);
        }
    });

    it('protects a legacy market’s existing vest until it releases', () => {
        const s = state({ depositHoldUntil: 0n });
        expect(resultOf(previewDepositHold(m, s, 300n))).toBe(600n);
        expect(resultOf(previewDepositView(m, s, 300n, 1_000n, 50))).toBe('!PoolDrained');
        expect(resultOf(previewDepositHold(m, s, 600n))).toBeNull();
        expect(resultOf(previewDepositView(m, s, 600n, 1_000n, 50))).toBe('!WipeSweepPending');
        expect(previewDepositView(m, s, 600n, 1_000n, 50, true).ok).toBe(true);
    });
});

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
        let drained = 0;
        let emptied = 0;
        for (const c of cases) {
            for (const e of c.expect) {
                if (e.op !== 'deposit') continue;
                const s = stateOf(c, e.lender);
                const r = previewDepositView(m, s, BigInt(c.now), BigInt(e.amount), 0);
                const got = r.ok ? r.value.shares.toString() : `!${r.refusal.name}`;
                expect(got, `${c.name} deposit ${e.lender} ${e.amount}`).toBe(e.result);
                n++;
                drained += got === '!PoolDrained' ? 1 : 0;
                // AIR-08: a pool every lender left (no wipe record) waits for the sweep too.
                emptied += got === '!WipeSweepPending' && !wipeLive(s.market) ? 1 : 0;
            }
        }
        expect(n).toBeGreaterThan(50);
        // The deposit gates of the AI review are covered: a drained pool (AIR-07) and an emptied one.
        expect(drained).toBeGreaterThan(5);
        expect(emptied).toBeGreaterThan(0);
    });

    it('withdrawals pay the model’s tokens and SOL (previewWithdrawal: claim right after, vest flush included)', () => {
        let n = 0;
        let lastShares = 0;
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
                    // M1-SEC-3: the program's own refusal of a burn that leaves S = 0 with tokens lent.
                    lastShares += e.result === '!LastSharesBorrowed' ? 1 : 0;
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
        // The burn of the last shares while a token is lent (the generator's `last_share_state`).
        expect(lastShares).toBeGreaterThan(0);
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

    it('a third party’s sync (the pot-only touch, its forward chain) and a claim’s close equal the model (KI-66 full, KI-67)', () => {
        let pot = 0;
        let chained = 0;
        let open = 0;
        let closes = 0;
        for (const c of cases) {
            const now = BigInt(c.now);
            const shareEpoch = Number(c.market.shareEpoch);
            for (const e of c.expect) {
                if (e.op !== 'touchPot' && e.op !== 'claimOutcome') continue;
                const s = stateOf(c, e.lender);
                const label = `${c.name} ${e.op} ${e.lender}`;
                if (e.op === 'claimOutcome') {
                    const r = previewClaimOutcome(m, s, now);
                    const got = r.ok ? { sol: r.value.sol.toString(), closes: r.value.closes } : `!${r.refusal.name}`;
                    expect(got, label).toEqual(e.result);
                    closes += r.ok && r.value.closes ? 1 : 0;
                    continue;
                }
                // The records the claim's sync carries: from fwdEpoch (chained) or epoch, at most 8.
                const l = s.lender;
                if (!l) continue;
                const window = new Set(potEpochs(l, shareEpoch));
                const r = previewTouchPot(m, { ...s, records: s.records.filter((x) => window.has(x.epoch)) }, now);
                if (typeof e.result === 'string') {
                    expect(resultOf(r), label).toBe(e.result);
                    continue;
                }
                expect(r.ok, label).toBe(true);
                if (!r.ok) continue;
                const t = r.value;
                expect(
                    {
                        potGain: t.potGain.toString(),
                        standing: t.standing,
                        pending: t.lender.pending.toString(),
                        real: t.lender.real.toString(),
                        shares: t.lender.shares.toString(),
                        epoch: t.lender.epoch,
                        snap: t.lender.snap.toString(),
                        snapR: t.lender.snapR.toString(),
                        fwd: t.lender.fwd,
                        fwdEpoch: t.lender.fwdEpoch,
                        fwdShares: t.lender.fwdShares.toString(),
                        fwdLag: t.lender.fwdLag,
                    },
                    label,
                ).toEqual(e.result);
                pot += t.potGain > 0n ? 1 : 0;
                chained += l.fwd !== FWD.none ? 1 : 0;
                open += l.fwd === FWD.open ? 1 : 0;
            }
        }
        // Coverage: syncs that move SOL, chains continued (owners stopped inside them too), closes.
        expect(pot).toBeGreaterThan(5);
        expect(chained).toBeGreaterThan(20);
        expect(open).toBeGreaterThan(5);
        expect(closes).toBeGreaterThan(30);
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
        // A new market starts with both sweep bits set (AIR-08: an emptied pool clears them).
        const flags = MARKET_FLAG.wipeTraderSwept | MARKET_FLAG.wipeRewardsClaimed;
        const s = { ...simplePool({ tokensFree: 0n, totalShares: 0n, flags }), lender: null, records: [] };
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

    it('refuses old empty-pool carries even when sweeps are complete or bundled', () => {
        for (const flags of [0, MARKET_FLAG.wipeTraderSwept | MARKET_FLAG.wipeRewardsClaimed]) {
            const empty = {
                ...simplePool({ tokensFree: 0n, totalShares: 0n, flags, lenderSol: 1n }),
                lender: null,
                records: [],
            };
            for (const [carry, carryR] of [
                [1n, 0n],
                [-1n, 0n],
                [0n, 1n],
                [1n, 1n],
            ]) {
                const s = {
                    ...empty,
                    market: {
                        ...empty.market,
                        carry: bigIntToPodI128(carry),
                        carryR: bigIntToPodU128(carryR),
                    },
                };
                for (const sweepBundled of [false, true]) {
                    const r = previewDepositView(m, s, 100n, 1_000_000_000n, 50, sweepBundled);
                    expect(r.ok).toBe(false);
                    if (!r.ok) {
                        expect(r.refusal).toMatchObject({ name: 'LegacyPoolCarry', code: 6110 });
                        expect(r.refusal.message).toContain('pending a reviewed migration');
                    }
                }
            }
            // The same empty pool without old carries can take deposits once sweeps are included.
            expect(previewDepositView(m, empty, 100n, 1_000_000_000n, 50, true).ok).toBe(true);
        }
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
        // The trader bit is enough off the pump venues on a market without a pump UVA; pump venues
        // also need the rewards bit.
        const swept = { ...curve.market, flags: flags | MARKET_FLAG.wipeTraderSwept };
        expect(wipeSweepPending(swept)).toBe(true);
        expect(wipeSweepPending({ ...swept, venue: 6, pumpUva: NO_UVA })).toBe(false);
        expect(wipeSweepPending({ ...swept, flags: swept.flags | MARKET_FLAG.wipeRewardsClaimed })).toBe(false);
        // AIR-05: a curve market that left pump (None, then Raydium) keeps its pump UVA, and with it the gate.
        expect(wipeSweepPending({ ...swept, venue: 0 })).toBe(true);
        expect(wipeSweepPending({ ...swept, venue: 4 })).toBe(true);
        expect(wipeSweepPending({ ...swept, venue: 4, flags: swept.flags | MARKET_FLAG.wipeRewardsClaimed })).toBe(
            false,
        );
    });

    it('requires the venue-reward sweep on the pump venues and on every market with a pump UVA (AIR-05)', () => {
        const venues = [0, 1, 2, 3, 4, 5, 6];
        // engine::lender::rewards_required: pump kinds, or Market.pump_uva set.
        expect(venues.map((venue) => rewardsRequired({ venue, pumpUva: NO_UVA }))).toEqual([
            false,
            true,
            true,
            true,
            false,
            false,
            false,
        ]);
        expect(venues.map((venue) => rewardsRequired({ venue, pumpUva: PUMP_UVA }))).toEqual(venues.map(() => true));
        // The deposit preview passes it to the facade: off pump with a UVA the trader bit alone is not enough.
        const left = {
            ...simplePool({
                tokensFree: 0n,
                totalShares: 0n,
                flags: MARKET_FLAG.wipeTraderSwept,
                venue: 4,
                pumpUva: PUMP_UVA,
            }),
            lender: null,
            records: [],
        };
        const refused = previewDepositView(m, left, 0n, 10n ** 9n, 50);
        expect(!refused.ok && refused.refusal.name).toBe('WipeSweepPending');
        const raydium = { ...left, market: { ...left.market, pumpUva: NO_UVA } };
        expect(previewDepositView(m, raydium, 0n, 10n ** 9n, 50).ok).toBe(true);
    });

    it('waits for the sweep after the last lender’s exit too (AIR-08: the gate is S = 0, not the wipe record)', () => {
        // Every lender withdrew: S = 0, no wipe record, `withdraw` cleared both bits.
        const emptied = {
            ...simplePool({ tokensFree: 0n, totalShares: 0n, flags: 0, shareEpoch: 3 }),
            lender: null,
            records: [],
        };
        expect(wipeLive(emptied.market)).toBe(false);
        expect(wipeSweepPending(emptied.market)).toBe(true);
        const refused = previewDepositView(m, emptied, 0n, 10n ** 9n, 50);
        expect(!refused.ok && refused.refusal.name).toBe('WipeSweepPending');
        const bundled = previewDepositView(m, emptied, 0n, 10n ** 9n, 50, true);
        expect(bundled.ok && bundled.value.shares).toBe(10n ** 12n);
        // The trader bit is enough off the pump venues, as after a wipe; a pump venue needs both.
        const swept = { ...emptied.market, flags: MARKET_FLAG.wipeTraderSwept };
        expect(wipeSweepPending(swept)).toBe(false);
        expect(wipeSweepPending({ ...swept, venue: 3 })).toBe(true);
        expect(
            wipeSweepPending({
                ...swept,
                venue: 3,
                flags: MARKET_FLAG.wipeTraderSwept | MARKET_FLAG.wipeRewardsClaimed,
            }),
        ).toBe(false);
        // A new market starts with both bits set. A dust flush can leave shares and clear both.
        const fresh = { ...emptied.market, flags: MARKET_FLAG.wipeTraderSwept | MARKET_FLAG.wipeRewardsClaimed };
        expect(wipeSweepPending({ ...fresh, venue: 1 })).toBe(false);
        expect(wipeSweepPending(simplePool({ flags: 0, venue: 1 }).market)).toBe(true);
    });

    it('requires the pending sweep after a dust flush that retains shares', () => {
        const s: LendState = { ...simplePool({ flags: 0, venue: 1 }), lender: null, records: [] };
        expect(wipeLive(s.market)).toBe(false);
        expect(wipeSweepPending(s.market)).toBe(true);
        expect(resultOf(previewDepositView(m, s, 0n, 1_000n, 50))).toBe('!WipeSweepPending');
        expect(previewDepositView(m, s, 0n, 1_000n, 50, true).ok).toBe(true);
    });

    it('rechecks legacy source bits even with surviving shares and no vest', () => {
        const s: LendState = { ...simplePool({ depositHoldUntil: 0n }), lender: null, records: [] };
        expect(wipeSweepPending(s.market)).toBe(true);
        expect(resultOf(previewDepositView(m, s, 0n, 1_000n, 50))).toBe('!WipeSweepPending');
        expect(previewDepositView(m, s, 0n, 1_000n, 50, true).ok).toBe(true);
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
        const pool = simplePool({ tokensFree: 1_000n, totalShares: 1_000_000n, lenderSol: 1_000_000n });
        // A lender with 300,000 realised but only 100,000 entitled: its touch moves 200,000 to the pot.
        const rich = { ...pool, lender: simpleLender(500_000n, { pending: 100_000n, real: 300_000n }), records: [] };
        expect(potGainOfSync(m, rich, 0n)).toBe(200_000n);
        // A lender entitled to 250,000 with nothing realised: the pot pays it.
        const me = { ...pool, lender: simpleLender(500_000n, { pending: 250_000n }), records: [] };
        const view = previewClaimView(m, me, 0n);
        expect(view.ok && view.value.claimable).toBe(0n);
        expect(view.ok && view.value.shortfall).toBe(250_000n);
        expect(previewClaimWithPot(m, me, 0n, 200_000n)).toEqual({ ok: true, value: 200_000n });
    });

    it('counts a shortfall under 10,000 lamports as none (rounding dust; the owner’s decision of 2026-10-11)', () => {
        expect(CLAIM_DUST_LAMPORTS).toBe(10_000n);
        expect([0n, 1n, 9_999n, 10_000n, 10_001n].map(claimShortfall)).toEqual([0n, 0n, 0n, 10_000n, 10_001n]);
        const pool = simplePool({ tokensFree: 1_000n, totalShares: 1_000_000n, lenderSol: 1_000_000n });
        const view = (pending: bigint, real: bigint) => {
            const v = previewClaimView(
                m,
                { ...pool, lender: simpleLender(500_000n, { pending, real }), records: [] },
                0n,
            );
            if (!v.ok) throw new Error('refused');
            return [v.value.claimable, v.value.shortfall, v.value.entitled];
        };
        // Claimable 0: nothing short, dust, the threshold.
        expect(view(0n, 0n)).toEqual([0n, 0n, 0n]);
        expect(view(9_999n, 0n)).toEqual([0n, 0n, 9_999n]);
        expect(view(10_000n, 0n)).toEqual([0n, 10_000n, 10_000n]);
        // Claimable > 0: the same edges above it.
        expect(view(300_000n, 300_000n)).toEqual([300_000n, 0n, 300_000n]);
        expect(view(309_999n, 300_000n)).toEqual([300_000n, 0n, 309_999n]);
        expect(view(310_000n, 300_000n)).toEqual([300_000n, 10_000n, 310_000n]);
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
        expect(refusalOf(new Error('pool drained'))).toMatchObject({ name: 'PoolDrained', code: 6108 });
        expect(refusalOf(new Error('legacy empty-pool carry requires migration'))).toMatchObject({
            name: 'LegacyPoolCarry',
            code: 6110,
        });
        expect(refusalOf(new Error('something else'))).toMatchObject({ name: 'MathOverflow', code: 6000 });
    });

    it('maps every MathError text as the program converts it (errors.rs From<MathError>)', () => {
        // `MathError::as_str()`, the message the facade throws (crates/shortwin-math/src/error.rs).
        const math = readRepo('crates/shortwin-math/src/error.rs');
        const at = math.indexOf('fn as_str');
        const asStr = math.slice(at, math.indexOf('\n    }', at));
        const texts = new Map(
            [...asStr.matchAll(/Self::(\w+) => "([^"]+)"/g)].map(([, v = '', text = '']) => [v, text] as const),
        );
        // The program's `impl From<MathError> for ShortwinError` (programs/shortwin/src/errors.rs).
        const prog = readRepo('programs/shortwin/src/errors.rs');
        const from = prog.slice(prog.indexOf('impl From<MathError> for ShortwinError'));
        const to = new Map<string, string>();
        const arms = from.slice(0, from.indexOf('\n}')).matchAll(/((?:\s*\|?\s*MathError::\w+)+)\s*=> Self::(\w+)/g);
        for (const [, lhs = '', target = ''] of arms) {
            for (const [, variant = ''] of lhs.matchAll(/MathError::(\w+)/g)) to.set(variant, target);
        }
        expect(texts.size).toBeGreaterThanOrEqual(14);
        expect([...texts.keys()].sort()).toEqual([...to.keys()].sort());
        for (const [variant, text] of texts) {
            const r = refusalOf(new Error(text));
            expect(r.name, variant).toBe(to.get(variant));
            expect(r.code, variant).not.toBeNull();
        }
    });

    it('refuses a deposit into a drained pool (AIR-07: at most 1/1,000 of the claims unborrowed)', () => {
        const at = (tokensFree: bigint, borrowedQ: bigint, totalShares: bigint) => ({
            ...simplePool({ tokensFree, borrowedQ, totalShares }),
            lender: null,
            records: [],
        });
        // 1,000·T = 10,000 ≤ T + ΣB_Q = 10,000, shares held, tokens lent out: PoolDrained (6108).
        const r = previewDepositView(m, at(10n, 9_990n, 10_000_000n), 0n, 10n ** 9n, 50);
        expect(!r.ok && [r.refusal.name, r.refusal.code]).toEqual(['PoolDrained', 6108]);
        // One more unborrowed token (11,000 > 10,001): deposits are taken again.
        expect(previewDepositView(m, at(11n, 9_990n, 10_000_000n), 0n, 10n ** 9n, 50).ok).toBe(true);
        // Nothing lent out: never drained.
        expect(previewDepositView(m, at(1n, 0n, 1_000n), 0n, 10n ** 9n, 50).ok).toBe(true);
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
    const accruing = (
        o: { receivable?: bigint; lastAccrualTs?: bigint; aggr?: readonly bigint[]; rem?: bigint } = {},
    ) =>
        fakeMarket(
            { tokensFree: 10n ** 12n, totalShares: S, receivable: o.receivable ?? 0n },
            {
                lastAccrualTs: o.lastAccrualTs ?? 1_000n,
                emaPrice: bigIntToPodU128(WAD),
                rentAggr: (o.aggr ?? A).map((a) => bigIntToPodU128(a)),
                lenderCreditRem: bigIntToPodU128(o.rem ?? 0n),
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
        // AIR-03: the remainder the market carries (`lender_credit_rem`) joins the floor, as the
        // prologue's `rent::lender_credit_carried`. The day is 232,876 + 260/365 lamports, so 105/365
        // of the denominator completes the lamport and one unit less does not. A remainder at the
        // denominator would refuse the prologue (`InvalidInput`): 0 here.
        const DEN = 10_000n * 10_000n * 31_536_000n * WAD;
        const edge = (105n * DEN) / 365n;
        expect(pendingLenderCredit(m, accruing({ rem: edge - 1n }), 1_000n + 86_400n)).toBe(232_876n);
        expect(pendingLenderCredit(m, accruing({ rem: edge }), 1_000n + 86_400n)).toBe(232_877n);
        expect(pendingLenderCredit(m, accruing({ rem: DEN }), 1_000n + 86_400n)).toBe(0n);
        expect(previewUnpaidRent(m, accruing({ rem: edge }), 1_000n + 86_400n, 1n, 1n).pending).toBe(232_877n);

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

describe('a sub-unit conversion while income vests (lender.subUnitConversion; KI-65, the owner’s decision of 2026-10-07)', () => {
    /** The rebase record of epoch 0 (1/1,000), `convRem` as given. */
    const rebase = (convRem = 0n, num = 1) => ({
        epoch: 0,
        accEnd: 0n,
        accREnd: 0n,
        sOld: 7_600n,
        carry: 0n,
        carryR: 0n,
        num,
        denLog1000: num,
        convRem,
    });
    /** A 3 SOL vest released linearly over [0, 21,600 s]; the lender holds `shares` old shares at epoch 0. */
    const state = (shares: bigint, o: { convRem?: bigint; num?: number; vestBal?: bigint; epoch?: number } = {}) => ({
        ...simplePool({ shareEpoch: 1, totalShares: 7n, vestBal: o.vestBal ?? 3_000_000_000n, vestEnd: 21_600n }),
        lender: simpleLender(shares, { epoch: o.epoch ?? 0 }),
        records: [rebase(o.convRem, o.num)],
    });

    it('flags a position below one new share converting to 0 during the vest, with the vest’s end', () => {
        const x = previewSubUnitConversion(m, state(600n), 1_000n);
        expect(x).not.toBeNull();
        expect(x?.vestEnd).toBe(21_600n);
        expect(x?.shares).toBe(600n);
        // The unreleased part at now: 3 SOL less the linear release over the first 1,000 of 21,600 s.
        const released = previewTouched(m, state(600n), 1_000n);
        expect(released.ok && released.value.shares).toBe(0n);
        expect(x?.unreleased).toBe(3_000_000_000n - (3_000_000_000n * 1_000n) / 21_600n);
    });

    it('flags the worst case, not the remainder at review time (KI-66 review)', () => {
        // With conv_rem 400 the conversion now gives ⌊(400 + 600)/1,000⌋ = 1 share, but another
        // lender's conversion landing first can move the remainder below 400: the floor is 0.
        const now = previewTouched(m, state(600n, { convRem: 400n }), 1_000n);
        expect(now.ok && now.value.shares).toBe(1n);
        expect(previewSubUnitConversion(m, state(600n, { convRem: 400n }), 1_000n)?.shares).toBe(600n);
        // Two rebase records (1/1,000 each): 999,999 → 999 → 0, whatever the remainders; 1,000,000 keeps 1.
        const two = (shares: bigint) => ({
            ...simplePool({ shareEpoch: 2, totalShares: 7n, vestBal: 3_000_000_000n, vestEnd: 21_600n }),
            lender: simpleLender(shares, { epoch: 0 }),
            records: [
                { ...rebase(), sOld: 7_600_000n },
                { ...rebase(), epoch: 1 },
            ],
        });
        expect(previewSubUnitConversion(m, two(999_999n), 1_000n)?.shares).toBe(999_999n);
        expect(previewSubUnitConversion(m, two(1_000_000n), 1_000n)).toBeNull();
    });

    it('stays quiet where the conversion keeps a share, the vest has ended, or nothing vests', () => {
        // ⌊1,600/1,000⌋ = 1 whatever the remainder.
        expect(previewSubUnitConversion(m, state(1_600n), 1_000n)).toBeNull();
        expect(previewSubUnitConversion(m, state(1_600n, { convRem: 999n }), 1_000n)).toBeNull();
        expect(previewSubUnitConversion(m, state(600n), 21_600n)).toBeNull();
        expect(previewSubUnitConversion(m, state(600n, { vestBal: 0n }), 1_000n)).toBeNull();
    });

    it('stays quiet for a full wipe, a current or empty lender, and records that do not reach the epoch', () => {
        expect(previewSubUnitConversion(m, state(600n, { num: 0 }), 1_000n)).toBeNull();
        expect(previewSubUnitConversion(m, state(600n, { epoch: 1 }), 1_000n)).toBeNull();
        expect(previewSubUnitConversion(m, state(0n), 1_000n)).toBeNull();
        expect(previewSubUnitConversion(m, { ...state(600n), records: [] }, 1_000n)).toBeNull();
        expect(previewSubUnitConversion(m, { ...state(600n), lender: null }, 1_000n)).toBeNull();
    });
});

const WAD = 10n ** 18n;

/**
 * A pool one rebase (`1/1,000`) past epoch 0 whose record froze 2 SOL of entitlement and 3 of
 * realised SOL per 10⁹ shares; the current epoch has credited 3 and realised 2 per share so far.
 * Lender C never converted: 1,000,000,001 shares of epoch 0 (the chain's floor is 1,000,000).
 */
function rebased(): LendState {
    const base = simplePool({ shareEpoch: 1, totalShares: 2_000_000n, lenderSol: 10_000_000_000n, unmatched: 7n });
    return {
        market: { ...base.market, acc: bigIntToPodI128(3n * WAD), accR: bigIntToPodU128(2n * WAD) },
        vault: base.vault,
        lender: simpleLender(1_000_000_001n),
        records: [
            recordOf({
                epoch: 0,
                accEnd: String(2n * WAD),
                accREnd: String(3n * WAD),
                sOld: '2000000000',
                carry: '0',
                carryR: '0',
                num: 1,
                denLog1000: 1,
                convRem: '0',
            }),
        ],
    };
}

describe('a third party’s sync of another lender: the pot-only touch and its forward chain (KI-66 full)', () => {
    it('holds a lender behind a rebase record there, walks its chain and moves the surplus to the pot', () => {
        const s = rebased();
        const t = previewTouchPot(m, s, 0n);
        expect(t.ok).toBe(true);
        if (!t.ok) return;
        expect(t.value.standing).toBe('behind');
        const l = t.value.lender;
        // Not converted: shares and epoch stay; the chain holds ⌊s/1,000⌋ in epoch 1.
        expect([l.shares, l.epoch, l.fwd, l.fwdEpoch, l.fwdShares, l.fwdLag]).toEqual([
            1_000_000_001n,
            0,
            FWD.hold,
            1,
            1_000_000n,
            0,
        ]);
        // Held: 2,000,000,002 credited and 3,000,000,003 realised at the record; the chain's
        // 10⁶ shares then 3,000,000 and 2,000,000 in epoch 1. The excess realised moves.
        expect(l.pending).toBe(2_003_000_002n);
        expect(t.value.potGain).toBe(3_002_000_003n - 2_003_000_002n);
        expect(l.real).toBe(2_003_000_002n);
        expect(potGainOfSync(m, s, 0n)).toBe(t.value.potGain);
        // Idempotent: a second sync of the result moves nothing.
        const again = previewTouchPot(m, { ...s, lender: l }, 0n);
        expect(again.ok && again.value.potGain).toBe(0n);
        expect(again.ok && again.value.lender).toEqual(l);
    });

    it('reads the chain: the owner’s own touch afterwards ends as if it had converted first (no double count)', () => {
        const s = rebased();
        const t = previewTouchPot(m, s, 0n);
        if (!t.ok) throw new Error('refused');
        const after = previewTouched(m, { ...s, lender: t.value.lender }, 0n);
        const alone = previewTouched(m, s, 0n);
        expect(after.ok && alone.ok).toBe(true);
        if (!after.ok || !alone.ok) return;
        expect(after.value.pending).toBe(alone.value.pending);
        expect([after.value.shares, after.value.epoch]).toEqual([alone.value.shares, alone.value.epoch]);
        // What the sync moved to the pot is what the owner's own touch moves.
        expect(t.value.lender.real).toBe(alone.value.real);
        // Without the fwd fields the chain's snapshots would read as the lender's own: wrong.
        const blind = previewTouched(
            m,
            { ...s, lender: { ...t.value.lender, fwd: 0, fwdEpoch: 0, fwdShares: 0n } },
            0n,
        );
        expect(blind.ok && blind.value.pending).not.toBe(alone.value.pending);
    });

    it('continues a chain from fwdEpoch, and judges a current lender as a plain touch', () => {
        const s = rebased();
        const t = previewTouchPot(m, s, 0n);
        if (!t.ok) throw new Error('refused');
        // The chain reached the share epoch: no record needed any more, and nothing more moves.
        const chained = previewTouchPot(m, { ...s, lender: t.value.lender, records: [] }, 0n);
        expect(chained.ok && chained.value.potGain).toBe(0n);
        expect(chained.ok && chained.value.standing).toBe('behind');
        // More credit and realised SOL in epoch 1 later: the chain alone (no record) moves its part.
        const later = {
            ...s,
            market: { ...s.market, acc: bigIntToPodI128(3n * WAD), accR: bigIntToPodU128(5n * WAD) },
            lender: t.value.lender,
            records: [],
        };
        expect(potGainOfSync(m, later, 0n)).toBe(3_000_000n);
        const cur = { ...simplePool(), lender: simpleLender(500_000n, { pending: 100n, real: 300n }), records: [] };
        const c = previewTouchPot(m, cur, 0n);
        expect(c.ok && [c.value.standing, c.value.potGain]).toEqual(['current', 200n]);
    });

    it('moves nothing without the record (a lender behind is judged on the records at hand)', () => {
        const s = rebased();
        expect(potGainOfSync(m, { ...s, records: [] }, 0n)).toBe(0n);
        expect(potGainOfSync(m, { ...s, lender: null }, 0n)).toBe(0n);
    });
});

describe('a claim that empties the position closes it (KI-67)', () => {
    it('says so when the claim leaves no shares, nothing owed, no lock and no chain', () => {
        const pool = simplePool({ lenderSol: 1_000n, totalShares: 0n, tokensFree: 0n });
        const wiped = { ...pool, lender: simpleLender(0n, { pending: 400n, real: 400n }), records: [] };
        expect(previewClaimOutcome(m, wiped, 0n)).toEqual({ ok: true, value: { sol: 400n, closes: true } });
        expect(closable(m, wiped)).toBe(false);
        expect(closable(m, { ...wiped, lender: simpleLender(0n) })).toBe(true);
        // A dead lock term (no lock) does not keep it open.
        expect(closable(m, { ...wiped, lender: simpleLender(0n, { lockTerm: 5n }) })).toBe(true);
        // Short of SOL: part paid, the rest owed — open; with the pot raised by syncs first — closes.
        const short = { ...wiped, lender: simpleLender(0n, { pending: 400n, real: 100n }) };
        expect(previewClaimOutcome(m, short, 0n)).toEqual({ ok: true, value: { sol: 100n, closes: false } });
        expect(previewClaimOutcome(m, { ...short, vault: { ...short.vault, unmatched: 300n } }, 0n)).toEqual({
            ok: true,
            value: { sol: 400n, closes: true },
        });
        expect(previewClaimOutcome(m, short, 0n, 300n)).toEqual({ ok: true, value: { sol: 400n, closes: true } });
    });

    it('stays open with shares, a lock, a deficit, a forward chain, or behind the share epoch', () => {
        const pool = simplePool({ lenderSol: 1_000n, shareEpoch: 2 });
        const at = (o: Parameters<typeof simpleLender>[1]) => ({
            ...pool,
            lender: simpleLender(0n, { epoch: 2, ...o }),
            records: [],
        });
        expect(closable(m, at({}))).toBe(true);
        expect(closable(m, at({ shares: 1n }))).toBe(false);
        expect(closable(m, at({ lock: 1n }))).toBe(false);
        expect(closable(m, at({ pending: -1n }))).toBe(false);
        expect(closable(m, at({ pending: 1n }))).toBe(false);
        expect(closable(m, at({ fwd: FWD.open, fwdEpoch: 2 }))).toBe(false);
        expect(closable(m, at({ epoch: 1 }))).toBe(false);
        expect(closable(m, { ...pool, lender: null })).toBe(false);
    });
});
