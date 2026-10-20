/**
 * The glossary (FRONTEND.md §4.3; the owner's thirteenth round, O-3 (b), audit finding D-03): one
 * short definition per term, opened from the ⓘ beside a jargon label (`components/Glossary.tsx`) by
 * a tap, a click or the keyboard — never by hover alone. This file is the single home of the text:
 * `glossary.test.ts` pins every entry, and `scripts/owner-copy.ts` prints them (the owner's file,
 * FRONTEND.md §4.3). Approved as written by the owner on 2026-10-11 (thirteenth round); a change
 * needs the owner's approval, this code and FRONTEND §4.3 together.
 *
 * An entry either has its own `text` or quotes approved disclosure paragraphs (`quote`), which are
 * then rendered from `disclosures.ts` verbatim, never retyped (`glossaryText.ts`). `{name}`
 * placeholders are filled from the live `GlobalConfig` (`glossaryVars`), as the disclosures' are
 * (§4.1 rule 5). Each entry's sources are in `glossarySources.ts` (for the owner and reviewers).
 * Only this file is in the app's first load (the ⓘ sits in the shell's status bar): the sources,
 * and the text resolution with the disclosures, load with the first opened definition.
 *
 * `labels` are the UI labels, exactly as the UI writes them, that the ⓘ attaches to: the shared
 * label renderers (`Kv`, `KvLines`, `Stat`, `Field`, `DataTable` headers, the Short preview rows,
 * the position facts) look a label up with {@link glossaryFor}; a few places name an entry
 * explicitly (the status bar, badges).
 */
import type { DisclosureId } from './disclosures.ts';

export interface GlossaryEntry {
    readonly id: string;
    /** The term in "What is {term}?" (the ⓘ button's accessible name). */
    readonly term: string;
    /** The definition; `{name}` placeholders are filled by `glossaryText` (`glossaryText.ts`). */
    readonly text?: string;
    /** Approved disclosure paragraphs quoted verbatim (0-based indices), in place of `text`. */
    readonly quote?: { readonly disclosure: DisclosureId; readonly paragraphs: readonly number[] };
    /** The UI labels the entry's ⓘ sits beside (exact text). */
    readonly labels: readonly string[];
}

const g = (e: GlossaryEntry): GlossaryEntry => e;

export const GLOSSARY = {
    // ---- prices and the collateral ratio -------------------------------------------------------
    cr: g({
        id: 'cr',
        term: 'CR (the collateral ratio)',
        text: 'Collateral ratio (CR) = vault SOL ÷ the SOL needed to buy the tokens back now, including price impact. If it falls below the position’s liquidation CR on both the live and the smoothed price, anyone can liquidate the position.',
        labels: ['CR', 'CR now', 'CR after', 'CR (spot)', 'CR (EMA)', 'CR at open (spot)', 'CR at open (EMA)'],
    }),
    crOpen: g({
        id: 'crOpen',
        term: 'the opening CR',
        text: 'The CR a new position must have when it opens, checked at both the live and the smoothed price. The tier sets it; your margin is the SOL it takes to reach it.',
        labels: ['Opening CR (D4)'],
    }),
    crLiq: g({
        id: 'crLiq',
        term: 'the liquidation CR',
        text: 'The CR below which the position can be liquidated, on both the live and the smoothed price. It is set by the market’s tier and locked into the position when it opens.',
        labels: ['Liquidation CR'],
    }),
    ema: g({
        id: 'ema',
        term: 'the smoothed price (EMA)',
        text: 'The smoothed price: an average that follows the live price over a few seconds, updated on every trade and by the keeper. A liquidation needs both prices past the line, so a brief spike in the live price cannot by itself liquidate a position whose smoothed price is still healthy.',
        labels: ['Price (EMA)'],
    }),
    spot: g({
        id: 'spot',
        term: 'the spot price',
        text: 'The live price on the token’s venue now, as short.win reads it from the curve or pool.',
        labels: ['Spot'],
    }),
    liqPrice: g({
        id: 'liqPrice',
        term: 'the liquidation price',
        text: 'The token price at which this position’s CR would reach its liquidation CR. Liquidation also needs the smoothed price to cross, and rent accruing over time moves this price closer.',
        labels: ['Liquidation price', 'Liq. price (SOL)', 'Liq. price'],
    }),
    risk: g({
        id: 'risk',
        term: 'the risk grade',
        text: 'How close the CR is to the liquidation CR: Safe, Watch, At risk, At risk (live price below), or Liquidatable.',
        labels: ['Risk'],
    }),

    // ---- shorting ----------------------------------------------------------------------------
    q: g({
        id: 'q',
        term: 'Q (tokens owed)',
        text: 'The number of tokens a short borrowed and must give back to the pool. Closing buys exactly this many back.',
        labels: ['Q', 'Q owed', 'Borrow'],
    }),
    sumQ: g({
        id: 'sumQ',
        term: 'ΣQ (tokens borrowed)',
        text: 'All the tokens currently borrowed by the open shorts in this market.',
        labels: ['Tokens borrowed (ΣQ)'],
    }),
    tokensFree: g({
        id: 'tokensFree',
        term: 'free tokens',
        text: 'The pool’s tokens that are not lent out. New shorts borrow from them and lenders withdraw from them.',
        labels: ['Tokens free (T)', 'Free inventory'],
    }),
    inventory: g({
        id: 'inventory',
        term: 'inventory',
        text: 'All the tokens lenders have in this pool: the free ones plus the ones lent out.',
        labels: ['Inventory', 'Inventory · free', 'Inventory value'],
    }),
    saleProceeds: g({
        id: 'saleProceeds',
        term: 'sale proceeds',
        text: 'The SOL the borrowed tokens sold for. It stays locked in the position’s vault until the position ends.',
        labels: ['Sale proceeds (S)', 'Sale proceeds'],
    }),
    margin: g({
        id: 'margin',
        term: 'margin',
        text: 'The SOL you post when you open, plus any margin you add later. The tier’s opening CR sets how much you need.',
        labels: ['Margin (D4)', 'Margin', 'Margin (incl. added)'],
    }),
    vault: g({
        id: 'vault',
        term: 'the vault',
        text: 'The position’s own SOL account: your margin plus the sale proceeds. Only a close, a liquidation, an expiry or a settlement in SOL can move this SOL out.',
        labels: ['Vault'],
    }),
    notional: g({
        id: 'notional',
        term: 'notional',
        text: 'A position’s size in SOL: what buying its tokens back would cost right after it opened. The open fee is charged on it.',
        labels: ['Notional open'],
    }),
    slippage: g({
        id: 'slippage',
        term: 'slippage',
        text: 'How much worse than the quote a trade may execute before it fails instead. A wider limit fails less often but can cost more.',
        labels: ['Slippage'],
    }),
    openFee: g({
        id: 'openFee',
        term: 'the open fee',
        text: 'A fee to the protocol on the position’s notional, paid when it opens. There is no close fee.',
        labels: ['Open fee'],
    }),
    totalCost: g({
        id: 'totalCost',
        term: 'the total cost',
        text: 'Margin, open fee and refundable deposits for the position’s accounts (Solana’s account rent, not the rent paid to lenders). The deposits come back when the position ends, or when your last position in this market ends. The network fee is shown in the review.',
        labels: ['Total cost'],
    }),
    rentApr: g({
        id: 'rentApr',
        term: 'the rent rate (APR)',
        text: 'The yearly rent rate a short pays on the value of its borrowed tokens. It rises with utilisation along the tier’s curve: gently at first, steeply once most of the pool is lent out.',
        labels: ['Rate (APR)', 'Rent rate (APR)', 'Rate'],
    }),
    rentCurve: g({
        id: 'rentCurve',
        term: 'the rent curve',
        text: 'The three numbers of the rent curve: the rate at zero utilisation, how fast it rises up to a set utilisation (the kink), and how fast it rises above it. A position keeps the curve it opened with; its rate still moves with the market’s utilisation.',
        labels: ['Rent: base / slope 1 / slope 2'],
    }),
    minRent: g({
        id: 'minRent',
        term: 'the minimum rent',
        text: 'The least rent a position pays, counted as a stretch of time at its rate, even if it closes sooner.',
        labels: ['Minimum rent'],
    }),
    rentDue: g({
        id: 'rentDue',
        term: 'rent due',
        text: 'Rent this position owes so far; it is paid when the position ends.',
        labels: ['Rent due'],
    }),
    rentProtocol: g({
        id: 'rentProtocol',
        term: 'the protocol’s rent share',
        text: 'The protocol’s share of the rent; the lenders receive the rest.',
        labels: ['Rent to the protocol'],
    }),
    maxDuration: g({
        id: 'maxDuration',
        term: 'the maximum duration',
        text: 'How long a position may stay open; the tier sets it. After the deadline anyone can close it: if it is still above its liquidation CR, you pay the expiry fee and a small keeper reward instead of the liquidation penalty; if it is below, it is liquidated and pays the penalty.',
        labels: ['Max duration', 'Deadline', 'Time left'],
    }),
    expiryFee: g({
        id: 'expiryFee',
        term: 'the expiry fee',
        text: 'What a position pays when it is closed after its deadline while still above its liquidation CR, in place of the liquidation penalty. It goes to the lenders; the keeper that closes it is also paid a small, capped reward from the position.',
        labels: ['Expiry fee'],
    }),
    penalty: g({
        id: 'penalty',
        term: 'the liquidation penalty',
        text: 'Taken from the vault when a position is liquidated, and split between the lenders, the protocol and the keeper that liquidated.',
        labels: ['Liquidation penalty', 'Penalty'],
    }),
    maxLiqSlippage: g({
        id: 'maxLiqSlippage',
        term: 'the maximum liquidation slippage',
        text: 'The most a liquidation’s buyback may move the price. If buying back would move it more, the position settles in SOL instead (S2).',
        labels: ['Max liquidation slippage (S2)', 'Max liquidation slippage'],
    }),
    buyback: g({
        id: 'buyback',
        term: 'the buyback',
        text: 'The SOL it costs to buy the borrowed tokens back.',
        labels: ['Buyback'],
    }),
    keeper: g({
        id: 'keeper',
        term: 'a keeper',
        text: 'A bot that liquidates, expires and settles positions. Anyone can run one. Small positions can depend on a short.win backstop with operating funds; timely third-party execution is not guaranteed.',
        labels: ['Keeper paid'],
    }),
    keeperReward: g({
        id: 'keeperReward',
        term: 'the settlement keeper reward',
        text: 'What the keeper that settles a position in SOL is paid from the vault: a fixed amount plus a share of the vault, capped.',
        labels: ['Keeper reward (settlement)', 'Settlement keeper reward'],
    }),
    venueFeeHardCap: g({
        id: 'venueFeeHardCap',
        term: 'the venue fee hard cap',
        text: 'The venue trading fee above which short.win treats the venue as unusable for buying back, which can lead to a settlement in SOL (S3). This position’s risk figures count the venue fee at no more than the cap locked when it opened.',
        labels: ['Venue fee hard cap'],
    }),
    settlement: g({
        id: 'settlement',
        term: 'a settlement in SOL (S1, S2, S3)',
        text: 'When the tokens cannot be bought back, the position settles in SOL: the lenders take SOL from its vault instead of tokens. S1: the price jumped past what the vault can pay. S2: buying back would move the price too far. S3: there has been no way to buy back for a while. “How this short can end” has the exact rules.',
        labels: ['Settled in SOL (S1 / S2 / S3)', 'Condition'],
    }),
    armed: g({
        id: 'armed',
        term: 'an armed settlement',
        text: 'The program has started collecting the price observations an S1 or S2 settlement needs. It is not a settlement yet, and it can still be cleared.',
        labels: ['Settlement watch', 'Settlement armed', 'Settlement in SOL being considered (S1 / S2)'],
    }),
    pnl: g({
        id: 'pnl',
        term: 'P&L',
        text: 'Profit or loss in SOL: what you would get back by closing now (or got back), less your margin, any added margin and the open fee.',
        labels: ['P&L', 'P&L if closed now'],
    }),

    // ---- markets, tiers and caps -------------------------------------------------------------
    tier: g({
        id: 'tier',
        term: 'the tier',
        text: 'Tiers A to D set a market’s terms: margin, liquidation CR, penalty, maximum duration, rent curve and size caps. Tiers are set automatically from on-chain liquidity and pool age, and checked again on every trade. A position keeps the terms it opened with.',
        labels: ['Tier'],
    }),
    venue: g({
        id: 'venue',
        term: 'the venue',
        text: 'Where short.win trades the token: the Pump.fun bonding curve, PumpSwap, or Raydium AMM v4 or CPMM.',
        labels: ['Venue'],
    }),
    poolAge: g({
        id: 'poolAge',
        term: 'the pool age',
        text: 'When short.win starts counting the pool’s age for its tier: the graduation it saw, the listing time, or the pool’s own recorded creation time.',
        labels: ['Pool age from'],
    }),
    utilisation: g({
        id: 'utilisation',
        term: 'utilisation',
        text: 'The share of this pool’s tokens that is lent out now. Rent rises with it, and a new short may not take it above the protocol’s utilisation cap.',
        labels: ['Utilisation', 'Util.', 'Utilisation cap'],
    }),
    tvl: g({
        id: 'tvl',
        term: 'TVL (total value locked)',
        text: 'Total value in the market, in SOL: the free tokens at the smoothed price, the SOL held for lenders, and the SOL in open positions’ vaults.',
        labels: ['TVL'],
    }),
    oi: g({
        id: 'oi',
        term: 'open interest',
        text: 'The value of all borrowed tokens in this market at the smoothed price. The market’s open-interest cap keeps the tokens borrowed small enough that buying them all back would move the price by no more than the tier allows.',
        labels: ['Open interest', 'Market open-interest cap'],
    }),
    sizeCap: g({
        id: 'sizeCap',
        term: 'the size cap',
        text: 'The largest short you can open right now. Several caps apply (price impact, the market’s open interest, utilisation, your wallet, free inventory, the curve guard on Tier D, and a new market’s notional cap); the smallest, marked “binding”, sets it.',
        labels: ['Size cap', 'Size caps'],
    }),
    positionImpactCap: g({
        id: 'positionImpactCap',
        term: 'the position impact cap',
        text: 'A single position may not be so large that buying it back would move the price more than the tier allows.',
        labels: ['Position impact cap'],
    }),
    walletCap: g({
        id: 'walletCap',
        term: 'the per-wallet cap',
        text: 'The most one wallet may hold of the market’s open-interest cap. It counts each wallet address separately, so it does not stop one person from using several wallets.',
        labels: ['Per-wallet cap'],
    }),
    notionalCap: g({
        id: 'notionalCap',
        term: 'the market notional cap',
        text: 'The limit on how much can be shorted while a market is new.',
        labels: ['Market notional cap'],
    }),
    curveGuard: g({
        id: 'curveGuard',
        term: 'the curve graduation guard',
        text: 'On a token still on the Pump.fun bonding curve, the open shorts together may borrow only a set share of the tokens the curve still has for sale, so that buying them back never needs more tokens than the curve holds.',
        labels: ['Curve graduation guard'],
    }),
    lockedDepth: g({
        id: 'lockedDepth',
        term: 'locked depth',
        text: 'The part of the pool’s liquidity that cannot be withdrawn, because its LP tokens (the pool’s receipts) are burned or locked. short.win bases its size caps on this part, not on the whole pool.',
        labels: ['Locked depth D'],
    }),
    coinCreator: g({
        id: 'coinCreator',
        term: 'the coin creator',
        text: 'This wallet created the token. It can lend and short its own token, so it can be on both sides of this market.',
        labels: ['Coin creator'],
    }),
    marketCreator: g({
        id: 'marketCreator',
        term: 'the market creator',
        text: 'This wallet created the short.win market. That gives it no special rights.',
        labels: ['Market creator'],
    }),

    // ---- market flags (the badges; the approved banner texts, verbatim) ------------------------
    flagMintUnsafe: g({
        id: 'flagMintUnsafe',
        term: 'Mint unsafe',
        quote: { disclosure: 'market.mintUnsafe', paragraphs: [0] },
        labels: ['Mint unsafe'],
    }),
    flagRouteBroken: g({
        id: 'flagRouteBroken',
        term: 'Route broken',
        quote: { disclosure: 'market.routeBroken', paragraphs: [0] },
        labels: ['Route broken'],
    }),
    flagOpensPaused: g({
        id: 'flagOpensPaused',
        term: 'Opens paused',
        text: 'New shorts are paused (on this market or globally). Exits continue.',
        labels: ['Opens paused'],
    }),
    flagCoveragePaused: g({
        id: 'flagCoveragePaused',
        term: 'Coverage paused',
        quote: { disclosure: 'market.coveragePause', paragraphs: [0, 1, 2] },
        labels: ['Coverage paused', 'Markets in degraded mode'],
    }),
    flagVenueUpgraded: g({
        id: 'flagVenueUpgraded',
        term: 'Venue upgraded',
        quote: { disclosure: 'market.venueUpgraded', paragraphs: [0] },
        labels: ['Venue upgraded'],
    }),
    flagMigrating: g({
        id: 'flagMigrating',
        term: 'Migrating',
        quote: { disclosure: 'market.migrating', paragraphs: [0] },
        labels: ['Migrating'],
    }),
    flagGraduationHalted: g({
        id: 'flagGraduationHalted',
        term: 'Graduation halted',
        quote: { disclosure: 'market.graduationHalted', paragraphs: [0] },
        labels: ['Graduation halted'],
    }),
    flagListOnly: g({
        id: 'flagListOnly',
        term: 'Lend only',
        quote: { disclosure: 'market.listOnly', paragraphs: [0] },
        labels: ['Lend only'],
    }),
    flagWarmingUp: g({
        id: 'flagWarmingUp',
        term: 'Warming up',
        text: 'The market’s prices are being confirmed after a listing, a graduation or a pool change; keeper liquidations and new shorts wait until they are, and you can still close your own position.',
        labels: ['Warming up'],
    }),
    flagPoolWindow: g({
        id: 'flagPoolWindow',
        term: 'Pool switch window',
        quote: { disclosure: 'market.poolSwitch', paragraphs: [0, 1] },
        labels: ['Pool switch window'],
    }),

    // ---- lending -----------------------------------------------------------------------------
    shares: g({
        id: 'shares',
        term: 'pool shares',
        text: 'A deposit receives pool shares; they are your part of the pool’s tokens and SOL.',
        labels: ['Shares', 'Your shares', 'Shares minted'],
    }),
    poolShare: g({
        id: 'poolShare',
        term: 'the pool share',
        text: 'Your shares as a percentage of all of this pool’s shares. Income and losses are shared in this proportion.',
        labels: ['Pool share', 'Your share of the pool', 'Share', 'Its share of the pool'],
    }),
    tokenValue: g({
        id: 'tokenValue',
        term: 'the token value',
        text: 'What your shares are worth in tokens now. On the dashboard, total value is in SOL at each market’s smoothed price and is for display only.',
        labels: ['Token value', 'Value', 'Total value'],
    }),
    minSharesTolerance: g({
        id: 'minSharesTolerance',
        term: 'the tolerance',
        text: 'How many fewer shares (or tokens) than the quote you accept. If the pool changes more than this before the transaction is confirmed, it fails instead.',
        labels: ['Min shares tolerance', 'Min tokens tolerance'],
    }),
    credited: g({
        id: 'credited',
        term: 'credited SOL',
        text: 'Rent and released income credited to your shares that you have not claimed yet. Credited rent is not guaranteed: rent a position never pays is taken back from the lenders who hold shares at that moment. Credited income does not guarantee a particular cash payout from the shared pot.',
        labels: ['Credited to you', 'Rent credited'],
    }),
    claimable: g({
        id: 'claimable',
        term: 'claimable SOL',
        text: 'What a claim would pay you now, from the SOL allocated to your shares and the market’s pot as they stand. Rent is paid in only when borrowers’ positions end, so rent credited to you may not be claimable yet. A claim that first syncs other lenders (see Market pot) can pay more.',
        labels: ['Claimable now', 'SOL claimable', 'Claimable'],
    }),
    vesting: g({
        id: 'vesting',
        term: 'vesting',
        text: 'Income credited to you bit by bit over {vestPeriod} instead of all at once: one-off income, such as your share of liquidation penalties and expiry fees, and rent paid above what accrued (the minimum rent of an early exit). If you withdraw before it has vested, the unvested part stays with the lenders who remain.',
        labels: ['Still vesting (your share)', 'Vesting', 'Vest you give up'],
    }),
    pot: g({
        id: 'pot',
        term: 'the market pot',
        text: 'Spare SOL in this market: SOL allocated to other lenders’ shares beyond their own credit, moved here when they are synced. When the SOL allocated to your shares does not cover what is credited to you, a claim can draw on the pot, and it can first sync such lenders. Claims draw on it in transaction order; an earlier claim can leave less cash for later claims against credited income. Some credited income may never be paid if the shortfall is never covered.',
        labels: ['Market pot'],
    }),
    deficit: g({
        id: 'deficit',
        term: 'a deficit',
        text: 'A negative SOL balance in this pool: rent that borrowers never paid was taken from the lenders, and your part was more than your SOL balance here. You must pay the difference in SOL (Cover deficit, on the Claim tab) before you can withdraw your tokens.',
        labels: ['Deficit'],
    }),
    locked: g({
        id: 'locked',
        term: 'locked shares',
        quote: { disclosure: 'lender.risks', paragraphs: [2] },
        labels: [
            'Locked',
            'Locked shares now',
            'Locked shares after',
            'Lock ends',
            'Current lock ends',
            'Locked until',
            'A new lock would end',
        ],
    }),
    unpaidRent: g({
        id: 'unpaidRent',
        term: 'unpaid rent',
        text: 'Rent the pool has credited that borrowers have not paid yet. If a position ends without enough SOL to pay it, the unpaid part is taken from the lenders who hold shares at that moment. That includes new deposits.',
        labels: ['Unpaid rent in the pool', 'This deposit’s part if it is never paid'],
    }),
    shareEpoch: g({
        id: 'shareEpoch',
        term: 'the share epoch',
        text: 'How many times this pool’s shares have been reissued at a new scale. That happens after settlements in SOL, when each share stands for very few tokens or the pool has none left. Your position converts to the new shares at your next claim, deposit or withdrawal.',
        labels: ['Share epoch'],
    }),
    sync: g({
        id: 'sync',
        term: 'a sync',
        text: 'Brings your position up to date with the pool’s latest share epoch and income. Your own claim, deposit or withdrawal does this too.',
        labels: ['Sync'],
    }),
    poolWiped: g({
        id: 'poolWiped',
        term: 'a wiped-out pool',
        text: 'Every token in this pool was lost to settlements in SOL; its lenders now hold SOL only, which they can claim.',
        labels: ['Pool wiped out'],
    }),
    realisedLoss: g({
        id: 'realisedLoss',
        term: 'the realised loss',
        text: 'What lenders lost in this market’s settlements in SOL: for each settlement that paid them less than the tokens’ value at settlement, the difference, added up.',
        labels: ['Realised loss', 'Loss / token value'],
    }),
    lendersPnl: g({
        id: 'lendersPnl',
        term: 'the lenders’ P&L',
        text: 'The SOL paid to lenders in this market’s settlements in SOL, less the value of the tokens they gave up at settlement; negative when they lost.',
        labels: ['Lenders’ P&L'],
    }),

    // ---- the app itself ------------------------------------------------------------------------
    rpc: g({
        id: 'rpc',
        term: 'the RPC',
        text: 'Your connection to a Solana node, with the latest slot it has seen. Every amount you sign is read from the chain through it.',
        labels: [],
    }),
    indexer: g({
        id: 'indexer',
        term: 'the indexer',
        text: 'short.win’s database of markets and history. Live means at most 10 seconds behind the chain, lagging at most 60, stale beyond; when it is unavailable, lists and history pause and the app reads the chain only.',
        labels: [],
    }),
    liveFeed: g({
        id: 'liveFeed',
        term: 'the live feed',
        text: 'The live feed that pushes market updates from the indexer to this page. “open” means connected, “idle” that this page needs no live updates; pages also refresh on a timer, so they keep updating while it reconnects.',
        labels: [],
    }),
    math: g({
        id: 'math',
        term: 'the math version',
        text: 'The version of the calculation module that runs in your browser. It is the protocol’s own Rust code compiled for the web, so previews use the same calculations as the on-chain program.',
        labels: ['Math'],
    }),
} as const satisfies Record<string, GlossaryEntry>;

export type GlossaryId = keyof typeof GLOSSARY;

const BY_LABEL: ReadonlyMap<string, GlossaryId> = (() => {
    const m = new Map<string, GlossaryId>();
    for (const [id, e] of Object.entries(GLOSSARY) as [GlossaryId, GlossaryEntry][]) {
        for (const l of e.labels) {
            if (m.has(l)) throw new Error(`glossary: label “${l}” in both ${m.get(l)} and ${id}`);
            m.set(l, id);
        }
    }
    return m;
})();

/** The entry a UI label attaches to (exact text), if any. */
export function glossaryFor(label: unknown): GlossaryId | undefined {
    return typeof label === 'string' ? BY_LABEL.get(label.trim()) : undefined;
}

/** The control's strings (FRONTEND.md §4.3). */
export const GLOSSARY_UI = {
    'glossary.trigger.aria': 'What is {term}?',
    'glossary.popover.close': 'Close',
} as const;
