/**
 * Where each glossary entry's statements come from (`glossary.ts`; FRONTEND.md §4.3), for the
 * owner's word-for-word approval (`scripts/owner-copy.ts`) and reviewers. Not imported by the app.
 */
import type { GlossaryId } from './glossary.ts';

export const GLOSSARY_SOURCES: Readonly<Record<GlossaryId, string>> = {
    cr: 'Sentence 1 verbatim from D:borrower.liquidation ¶1. Sentence 2: D:borrower.liquidation ¶2; SPEC §5.1; ECON §5.5; the threshold is the position’s own (ECON §15 cr_liq_bps scope T→P; SettlementRisk.tsx, A26).',
    crOpen: 'SPEC §4 “Margin rule”; ECON §4; features/short/ui.tsx TierTerms; FE §3.7 cr-open, cr-ema.',
    crLiq: 'SPEC §4 table, §5.1; ECON §15 cr_liq_bps (scope T→P).',
    ema: 'SPEC §5.1 (both legs), §9 (“EMA price updated on every interaction and by keeper crank”); ECON §15 EMA_TAU_S = 5 s (compile-time); PositionsPage RISK_GRADES “below” grade.',
    spot: 'SPEC §9 “Pricing”; FE §3.4.',
    liqPrice:
        'FE §3.7 liq-price (liquidationPrice(q, v_eff, …)); both legs: SPEC §5.1; rent lowers the effective vault: SPEC §4 note “ignoring rent accrual”, FE §3.8 vEff(v, R_due).',
    risk: 'features/short/PositionsPage.tsx RISK_GRADES; components/Badge.tsx RISK_TEXT; FE §3.8 “Risk levels”.',
    q: 'SPEC §2.1 step 1, §2.2.',
    sumQ: 'SPEC §10 “total borrowed Q”; ECON §9.1 B_Q; features/markets/market/OverviewTab.tsx.',
    tokensFree: 'ECON §9.1 T; SPEC §2.4 step 3.',
    inventory: 'ECON §9.2 T + B_Q; features/markets/market/StatStrip.tsx; MarketsPage.tsx column “Inventory · free”.',
    saleProceeds: 'SPEC §2.1 step 4.',
    margin: 'SPEC §2.1 step 2, §4 “Margin rule”; ECON §4; add margin: FE §3.8.',
    vault: 'SPEC §2.1 step 4, §10 invariant “A position’s vault SOL can only leave via close, liquidation, or SOL settlement”; expiry is its own exit (instructions/expire.rs, the machinery of liquidate).',
    notional: 'FE §3.7 open-fee (N_open = B̂_spot post-open); ECON §8.7; SPEC §6.2.',
    slippage: 'FE-D6; FE §5.10; SPEC §2.1 step 1, §12 R4.',
    openFee: 'SPEC §6.2; ECON §15 open_fee_bps.',
    totalCost:
        'FE §3.7 total-cost; ShortPanel.tsx (totalCost = margin + openFee + account rents: the position’s, plus the WalletExposure’s on a wallet’s first position in the market; sub-line “network fee in the review”); the position account’s rent returns to the borrower on every exit (liquidate.rs, expire.rs, sol_settle.rs close = borrower); the WalletExposure closes only when q_open == 0 (liquidate.rs close_exposure).',
    rentApr: 'SPEC §6.1; ECON §7; features/markets/market/StatStrip.tsx (“for a new short at … utilisation”).',
    rentCurve:
        'SPEC §6.1 formula; ECON §7 (“The position locks its rate curve”; the market-level indices follow utilisation).',
    minRent: 'SPEC §6.1 “Minimum rent”; D:borrower.liquidation last ¶ (“at least {minRent} of rent is charged”).',
    rentDue: 'ECON §9.4 (“rent is paid only at exit”); SPEC §2.2 step 3; FE §3.8 R_due.',
    rentProtocol: 'SPEC §6.3; ECON §15 rent_protocol_bps.',
    maxDuration:
        'SPEC §4 “Duration”, §5.3 (the expiry fee only when “not below its liquidation CR”); D:borrower.liquidation ¶3; expire.rs (precondition: **not** CR-liquidatable, else `UseLiquidate`, the penalty through liquidate, ECON §8.4; K_e from the borrower’s remainder); ECON G15.',
    expiryFee:
        'SPEC §4 “Duration”, §5.3, §6.3 (expiry fee 100% lenders); expire.rs:1-6, 14-15 (not CR-liquidatable), 40; ECON §15 expiry_keeper_fixed / _bps / _cap; ECON G15.',
    penalty: 'SPEC §5.2 step 3, §6.3.',
    maxLiqSlippage: 'SPEC §5.3 table row S2; D:borrower.liquidation (S2 line).',
    buyback: 'SPEC §2.2 step 2; INDEXER §8.4 buyback_cost.',
    keeper: 'SPEC §5.1 (“Anyone can call liquidate … short.win runs its own keeper as a backstop”), §6.3; AIR2-13 operating-funding and third-party incentive limitations (owner, 2026-10-21).',
    keeperReward:
        'SPEC §6.3 (SOL settlement row); features/short/ui.tsx TierTerms; disclosureVars.ts keeperRewardText.',
    venueFeeHardCap:
        'ECON §10.3 (“Continuous S3 chain … with the global hard cap”); ECON §5 “Two fee views” (risk quotes use the position’s locked venue_fee_hard_cap_bps); ECON §14 N10; ARCH §2.3.3.',
    settlement: 'SPEC §5.3; D:borrower.liquidation (S1–S3 lines).',
    armed: 'ECON §5.6 (settle_persist_n, arming; disarms on refuted or no-longer-liquidatable evidence); features/short/positions/SettlementRisk.tsx; PositionDetailPage.tsx “Settlement watch”; HistoryTabs.tsx badge “Settlement armed”.',
    pnl: 'FE §3.8 (P&L formula).',
    tier: 'Sentence 1: SPEC §4 tables, §6.1. Sentence 2 verbatim from D:market.tierAge ¶1. Sentence 3: ECON §15 scope T→P; features/short/ui.tsx (“terms locked at open”).',
    venue: 'SPEC §3 “Supported venues”.',
    poolAge: 'D:market.tierAge ¶2; SPEC §4 “Pool age”.',
    utilisation: 'ECON §9.1; utilizationBps(T, ΣB_Q) (StatStrip.tsx); SPEC §4 “Utilization cap”, §6.1.',
    tvl: 'services/indexer/src/analytics/formulas.rs tvl_lamports (INDEXER §8.1).',
    oi: 'formulas.rs oi_lamports; SPEC §4 “Per market (total open shorts)”.',
    sizeCap: 'SPEC §4 “Size caps”; features/short/ui.tsx CAP_TEXT (7 caps); FE §3.7 caps.',
    positionImpactCap: 'SPEC §4 “Per position”.',
    walletCap: 'SPEC §4 “Per wallet”, §12 R1 (M7-SEC-9); D:market.creatorSelfDealing ¶2.',
    notionalCap:
        'D:lender.residuals (“the market’s notional cap — the limit on how much can be shorted while a market is new”); ECON §15 market_max_notional.',
    curveGuard:
        'ECON §6.1 caps table (“Tier D graduation guard”); ECON §10.2 (“q > rt on a live curve: pump rejects the buy (6021). Prevented at open (γ = 50%)”); ECON §15 curve_oi_gamma_bps.',
    lockedDepth:
        'SPEC §14 M6R-1, M6D-1; ECON §15 pool_lock_min_bps (the sizing bound D/λ_min), PUMPSWAP_LOCK_MIN_BPS; ECON N20 (Raydium sizes on D/λ_min, up to 2× D; PumpSwap on D), so the text says “bases its size caps on this part”, not “on this part only”.',
    coinCreator:
        'features/markets/market/CreatorDeposits.tsx (“This wallet created the token (SPEC R1)”); SPEC §7, §12 R1; D:market.creatorSelfDealing ¶1.',
    marketCreator: 'D:market.create ¶1 (“no approval and no special status for the creator”).',
    flagMintUnsafe: 'D:market.mintUnsafe ¶1, verbatim.',
    flagRouteBroken: 'D:market.routeBroken ¶1, verbatim (¶2 needs {s3Wait}; the market page’s banner shows it).',
    flagOpensPaused:
        'Verbatim the message of the program error 6050 OpensPaused (config/errors.ts), which the market page’s banner shows; SPEC §11 (“Pause new opens globally or per market (keeps closes and liquidations running)”).',
    flagCoveragePaused: 'D:market.coveragePause ¶1–¶3, verbatim.',
    flagVenueUpgraded: 'D:market.venueUpgraded ¶1, verbatim.',
    flagMigrating: 'D:market.migrating ¶1, verbatim.',
    flagGraduationHalted: 'D:market.graduationHalted ¶1, verbatim.',
    flagListOnly: 'D:market.listOnly ¶1, verbatim.',
    flagWarmingUp:
        'ARCH changelog 55 (the PumpSwap transition restarts ema_warm = 8; “the warm-up gates keeper exits only”); ARCH §3.6 precondition 1b; D:market.poolSwitch ¶3; ECON §15 EMA_WARMUP_BUCKETS.',
    flagPoolWindow: 'D:market.poolSwitch ¶1–¶2, verbatim.',
    shares: 'SPEC §2.4 step 1; ECON §9.2.',
    poolShare: 'SPEC §5.3 “pro-rata to all lenders”, §5.4; D:lender.risks ¶2.',
    tokenValue:
        'ECON §9.2 (share value); FE §3.8 (dashboard KPI “≈ SOL at each market’s indexer EMA price, display only”).',
    minSharesTolerance: 'FE-D6 (min_shares, min_tokens); FE §3.6.',
    credited:
        'ECON §9.4 (entitlement pending; claim_sol pending −= pay; a shortfall is debited pro-rata); LenderDashboardPage.tsx (Rent credited = Σ pending); previews.ts entitled = max(pending, 0); SPEC §2.4 step 5; D:lender.unpaidRent ¶1; AIR2-P2-02 cash-first claim policy (owner, 2026-10-21).',
    claimable:
        'programs/shortwin/src/instructions/claim_sol.rs (pay = min(pending, real − claimed, lender_sol)); ECON §9.4 (“rent is paid only at exit”); ClaimForm.tsx previewClaim (no syncs); build.ts claimPlanFrom (syncs when shortfall > 0). Paid-in rent can still wait on other lenders’ syncs (claim.note.shortfall), so the text does not say “claimable once paid”.',
    vesting:
        'D:lender.risks ¶4 (one-off income vests over {vestPeriod}); D:lender.exitForfeitsVest (“the unvested part stays with the lenders who remain”), KI-09; ClaimForm.tsx note (rent paid above what accrued vests, M10-8C-1); ECON §9.3; ECON §15 lump_vest_s.',
    pot: 'ECON §9.4 touch (surplus → MarketVault.unmatched), “Why the pot (R3A-2)”; claim_sol.rs (draw = min(unmatched, max(0, pending − own))); D:lender.potSyncs ¶1; AIR2-P2-02 cash-first claim policy (owner, 2026-10-21).',
    deficit: 'SPEC §2.4 step 5; ECON §9.4; D:lender.unpaidRent ¶3 (sentence 2 verbatim).',
    locked: 'D:lender.risks ¶3, verbatim ({lockGrace} from GlobalConfig lock_grace_s).',
    unpaidRent: 'D:lender.unpaidRent ¶1; SPEC §2.4 step 5; ECON §9.4.',
    shareEpoch:
        'ECON §9.5 (roll(0, 1) on a full wipe; roll(1, 1,000^k) when S + VS > R_MAX·(T + B_Q + VA); “lender touch: for each epoch behind”); D:lender.residuals ¶10.',
    sync: 'FE §3.6, §3.8 (sync_lender); ECON §9.4 (touch).',
    poolWiped:
        'ECON §9.5 (full-wipe record; “the wiped lenders keep their unreleased lumps”); claim_sol.rs header (“A lender parked on a live full-wipe record claims from that record”).',
    realisedLoss: 'INDEXER §8.4 (realised_lender_loss = Σ max(0, −lender_pnl) per settlement); SPEC §13.',
    lendersPnl: 'INDEXER §8.4 (lender_pnl signed, per settlement); OverviewTab.tsx LenderLoss.',
    rpc: 'FE §3.1 status bar; FE-D1.',
    indexer: 'lib/staleness.ts (LIVE_LAG_S 10, STALE_LAG_S 60); FE §6.6; the indexer-down banner (shell/AppShell.tsx).',
    liveFeed:
        'api/ws.ts (WsState; “idle” when the page subscribes to nothing, StatusBar.tsx “Live {ws}”); FE §6.4; refetchInterval is unconditional (api/queries.ts, features/short/hooks.ts, features/lend/hooks.ts, rpc/queries.ts).',
    math: 'crates/shortwin-math-wasm/README.md; FE §6.7; FE-15.',
};
