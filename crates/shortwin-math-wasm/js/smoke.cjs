// Smoke test of the wasm-bindgen facade through Node: BigInt u64/u128 arguments and
// results, `undefined` for undefined quotes, thrown MathError texts, and the numbers of
// the research worked examples (R-PUMP §4.2, R-VEN §2.4, R-RAY §3.5).
//
// cargo build -p shortwin-math-wasm --target wasm32-unknown-unknown --release
// wasm-bindgen --target nodejs --out-dir target/wasm-pkg target/wasm32-unknown-unknown/release/shortwin_math_wasm.wasm
// node crates/shortwin-math-wasm/js/smoke.cjs target/wasm-pkg
const assert = require("node:assert/strict");
const path = require("node:path");
const m = require(path.resolve(process.argv[2] ?? "target/wasm-pkg", "shortwin_math_wasm.js"));
const WAD = 10n ** 18n;

assert.equal(typeof m.version(), "string");

// R-PUMP §4.2 curve worked example.
const pamm = new m.PammSchedule(10n ** 15n, true, 0, false);
pamm.addTier(0n, 2, 93, 30);
pamm.addTier(420_000_000_000n, 20, 5, 95);
const curve = m.Venue.pumpCurve(new m.Fees(0, 95, 30), 279_900_000_000_000n, 30_000_000_000n, 15_000_001n, 206_900_000_000_000n, pamm);
const s = new m.CurveState(824_130_602_106_172n, 39_059_343_535n, 544_230_602_106_172n, 9_059_343_535n, false);
const q = 10_000_000_000_000n;
const b = curve.buyCurve(q, s);
assert.equal(b.cost, 485_764_635n); // ECON §3.2: the quote rounds up (the program charges 485,764,634)
assert.equal(b.net, 479_767_540n);
assert.equal(curve.sellCurve(q, s).proceeds, 462_410_820n);
assert.equal(curve.onewayBps(), 125);
assert.equal(curve.onewayPpm(), 12_500);
assert.equal(m.exceedsCap(curve.onewayPpm(), 124), true); // fee gates compare ppm with cap·100
assert.equal(m.exceedsCap(curve.onewayPpm(), 125), false);
// A curve venue has no (x, y) quote: the thrown Error carries the MathError text.
assert.throws(() => curve.buy(q, 1n, 1n), /invalid input/);

// Raydium AMM v4 (R-RAY §3.5 (A)); u128 reserves round-trip through BigInt.
const v4 = m.Venue.rayV4(25n, 10_000n);
const bq = v4.buy(235_932_179_000n, 37_639_277_914_829_371n, 17_595_123_332_536n);
assert.equal(bq.cost, 110_567_639n);
assert.equal(bq.tokenAfter, 37_639_277_914_829_371n - 235_932_179_000n);
assert.throws(() => m.Venue.rayV4(10n, 10n), /invalid input/);
const big = (1n << 127n) + 12345n;
const r = m.reprice(big, WAD);
assert.equal(r.x, big);
assert.equal(m.reprice(0n, WAD), undefined);
assert.equal(m.liquidity(1n << 60n, 1n << 60n), 1n << 60n);
assert.throws(() => m.liquidity(1n << 64n, 1n << 64n), /overflow/); // X·Y ≥ 2¹²⁸ (ECON §2.2)

// PumpSwap (R-VEN §2.4 TX-BUY) and the liquidation price.
const ps = m.Venue.pumpSwap(pamm.resolve(18_917_931_618n + 17_584_505_333n, 482_553_257_944_361n));
const y = 18_917_931_618n + 17_584_505_333n;
const bb = ps.buy(9_155_477_586_733n, 482_553_257_944_361n, y);
assert.equal(bb.cost, 714_778_826n);
const L = m.liquidity(482_553_257_944_361n, y);
const P = m.priceWad(482_553_257_944_361n, y);
const risk = ps.capped(100);
const bHat = risk.buyRepriced(q, L, P).cost;
const vEff = 2n * bHat;
const pLiq = risk.liquidationPrice(q, vEff, 12_500, L, P);
assert.ok(pLiq > P && pLiq < 2n * P);
assert.equal(m.crBelow(vEff, risk.buyRepriced(q, L, pLiq).cost, 12_500), true);
assert.equal(m.crBelow(vEff, risk.buyRepriced(q, L, pLiq - 1n).cost, 12_500), false);
assert.equal(m.crBps(vEff, undefined), 0n);
assert.equal(m.marginD4(100n, 100n, 120n, 20_000), 140n);
assert.throws(() => m.marginD4(100n, 100n, undefined, 20_000), /undefined/);

// Rent due and waterfall previews.
const open = new m.RentBasis(0n, 0n, 0n);
const later = open.accrue(WAD, 0, 8_000, 31_536_000n);
const pos = new m.PositionRent(1_000_000_000n, open, 1_000, 5_000, 100_000, 1_500, 7n);
assert.equal(pos.due(later), 100_000_000n);
// Liquidation with the keeper floor at K_s (N22): 15 + a top-up of 5 from the borrower.
const liq = m.previewLiquidation(1_000n, 600n, 100n, 500n, 550n, 1_500, 1_000, 5_000, 2_000, 3_000, 20n);
assert.equal(liq.buyback + liq.keeper + liq.protocol + liq.lenders + liq.borrower, 1_000n);
assert.equal(liq.keeper, 20n);
assert.equal(liq.keeperTopup, 5n);
assert.equal(liq.lump, 25n);
assert.throws(() => m.previewClose(100n, 90n, 20n, 1_500), /underflow/);
assert.equal(m.previewSettlement(1_000n, 1, 20n, 5n, 500n, WAD, 50n).lenders, 980n);
// S2: the part above ⌊Q·P_ema/WAD⌋ + R_due vests.
assert.equal(m.previewSettlement(1_000n, 2, 20n, 700n, 500n, WAD, 50n).lump, 150n);

// Exit bounds (v0.4.1): no liquidation reserve; expiry keeps rent and fee senior.
assert.equal(m.maxInLiq(1_000n, 200, 10_000n, 100n, 0n), 1_020n);
assert.equal(m.maxInLiq(1_000n, 200, 100n, 100n, 1n), undefined);
const piRef = m.piRef(1_000n, 200);
assert.equal(piRef, 20n);
assert.equal(m.maxInExp(1_000n, 100, 10, 1_100n, 0n, 1_000n, piRef, 0n), 1_001n);

// Settlement box: medians of the bucket extremes widened to the EMA point.
const o = new m.OracleState(2n * WAD, 1_000n, 10n);
assert.throws(() => o.settleBox(10n), /invalid input/);
for (const [ts, lo, hi] of [[8n, 1n, 3n], [9n, 2n, 5n], [10n, 1n, 4n]]) {
  o.push(ts, 2n * WAD, 1_000n, 0n, 0n, lo * WAD, hi * WAD, 900n, 1_100n);
}
const box = o.settleBox(11n);
assert.deepEqual([box.pLo, box.pHi, box.lLo, box.lHi], [WAD, 4n * WAD, 900n, 1_100n]);
assert.equal(box.isPoint, false);
assert.equal(o.settleBox(10n + BigInt(m.ringLen()) + 1n).isPoint, true);
assert.equal(m.pprime(true, 1_250n, 1_000n, 12_500, 500, 200), 200);
assert.equal(m.amountOk(10_000n, 10_101n, 100), false);

// Parameter checks (ECON §15 = shortwin_sim.validate_params).
const params = new m.ParamSet();
assert.equal(params.validate(), undefined);
params.set("kappa_liq_bps", undefined, 2_000n);
params.set("cr_liq_bps", 1, 13_000n);
assert.deepEqual(params.violations(), ["kappa_liq_bps", "B:tier_spec_cr"]);
assert.equal(params.get("cr_liq_bps", 1), 13_000n);
assert.throws(() => params.set("nope", undefined, 1n), /invalid input/);

// Lender previews, incl. a pool-rule refusal.
const pool = new m.PoolState();
const lender = m.LenderState.newIn(pool);
const none = new m.ShareEpochs();
const now = 1_000n;
assert.equal(m.previewDeposit(pool, lender, none, now, 1n, false), 1_000n);
assert.throws(() => m.previewDeposit(pool, lender, none, now, 0n, false), /deposit too small/);
assert.throws(() => m.previewWithdraw(pool, lender, none, now, 86_400, 1n), /insufficient shares/);
pool.totalShares = 1_000_000_000_000n; pool.tokensFree = 1_000_000_000n; pool.lenderSol = 5n;
lender.shares = 1_000_000_000_000n; lender.pending = -1n;
assert.throws(() => m.previewWithdraw(pool, lender, none, now, 86_400, 1n), /negative pending/);
lender.pending = 0n;
assert.equal(m.previewAllowance(pool, lender, none, now), 1_000_000_000n);
// A live wipe record waits for its post-wipe sweep before the next deposit (R4A-7).
const wiped = new m.PoolState();
wiped.wipeLive = true; wiped.wipeSwept = 0; // Market.flags bits 7, 8 cleared at the full wipe
const parked = m.LenderState.newIn(wiped);
assert.throws(() => m.previewDeposit(wiped, parked, none, now, 1n, false), /post-wipe sweep pending/);
wiped.wipeSwept = 1; // sweep_trader ran; the venue-reward claim is required on pump venues only
assert.equal(m.previewDeposit(wiped, parked, none, now, 1n, false), 1_000n);
assert.throws(() => m.previewDeposit(wiped, parked, none, now, 1n, true), /post-wipe sweep pending/);
// Locks end at a day boundary after max_release + lock_grace_s; lumps vest linearly.
pool.maxRelease = 1_000_000n;
assert.equal(m.previewLockUntil(pool, now, 86_400), 13n * 86_400n);
pool.vestBal = 100n; pool.vestLast = 0n; pool.vestEnd = 100n;
assert.equal(m.previewVestRelease(pool, 50n), 50n);
assert.equal(lender.lockUntil, 0n);

// Caps and tiers.
assert.equal(m.maxQPosition(1_000_000_000_000n, 500), 24_695_076_595n);
const t = new m.TierInputs();
t.venue = 0;
assert.equal(t.tier(), 3);
assert.equal(t.liquidityLamports(), undefined);
t.venue = 9;
assert.throws(() => t.tier(), /invalid input/);
assert.throws(() => t.liquidityLamports(), /invalid input/);
assert.equal(m.tierParams(3).maxDurationS, 259_200);
console.log("facade smoke test: ok");
