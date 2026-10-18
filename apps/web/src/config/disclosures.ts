/**
 * Every user-facing disclosure, with its exact copy (FRONTEND.md §4). This file is the single
 * source of the text: components render it through `<Disclosure id=… />` and never paraphrase it.
 * `disclosures.test.ts` holds `LENDER_SOL_REPAYMENT` to SPEC §5.3 verbatim and checks every
 * placeholder.
 *
 * Changing a text is a contract change (FRONTEND.md §4.1): update FRONTEND.md in the same change.
 */

/** SPEC §5.3, verbatim: shown on the deposit screen with a required checkbox, before signing. */
export const LENDER_SOL_REPAYMENT =
    "If a borrower can't be bought back, you will be repaid in SOL, which may be worth less than the tokens you lent.";

export type DisclosureId =
    | 'lender.solRepayment'
    | 'lender.cleanup'
    | 'lender.risks'
    | 'lender.lossHistory'
    | 'lender.exitForfeitsVest'
    | 'lender.residuals'
    | 'lender.unpaidRent'
    | 'lender.subUnitConversion'
    | 'lender.potSyncs'
    | 'lender.potSyncsDeferred'
    | 'lender.claimCloses'
    | 'borrower.liquidation'
    | 'borrower.residuals'
    | 'market.creatorSelfDealing'
    | 'market.tierAge'
    | 'market.coveragePause'
    | 'market.poolSwitch'
    | 'market.routeBroken'
    | 'market.venueUpgraded'
    | 'market.migrating'
    | 'market.graduationHalted'
    | 'market.listOnly'
    | 'market.venueTrust'
    | 'market.mintUnsafe'
    | 'market.create'
    | 'market.priceRefresh'
    | 'token.cpiGuard'
    | 'token.memoRequired'
    | 'tx.simulation';

/**
 * `{breakEvenDays}` of `lender.lossHistory`: the break-even jump frequency of lending, in days, per
 * tier (ECON §11.6, α = 2 column; FRONTEND.md §4.2).
 */
export const BREAK_EVEN_DAYS: Readonly<Record<'A' | 'B' | 'C' | 'D', number>> = { A: 535, B: 205, C: 84, D: 28 };

/**
 * When the deposit review warns about the pool's unpaid rent (`lender.unpaidRent`; FRONTEND.md §3.6,
 * ECON §9.4; the owner's decision of 2026-10-03): the deposit's pro-rata part of `Market.receivable`
 * is material once it reaches {@link UNPAID_RENT_WARN_BPS} of the deposit's value at `P_ema`, or
 * {@link UNPAID_RENT_WARN_LAMPORTS} whatever the deposit's size (also on a market without a price).
 * Below both, the review shows the figures and the disclosure collapsed.
 */
export const UNPAID_RENT_WARN_BPS = 100;
/** 1 SOL: see {@link UNPAID_RENT_WARN_BPS}. */
export const UNPAID_RENT_WARN_LAMPORTS = 1_000_000_000n;

export type Severity = 'info' | 'warn' | 'danger';

export interface Disclosure {
    readonly id: DisclosureId;
    readonly title: string;
    /** Paragraphs or list items, rendered in order. `{name}` placeholders are filled by `renderDisclosure`. */
    readonly body: readonly string[];
    /** The user must tick a checkbox with this label before the wallet is asked to sign. */
    readonly ack: string | null;
    readonly severity: Severity;
    /** Where the rule comes from (for reviewers; not rendered). */
    readonly source: string;
}

const d = (x: Disclosure): Disclosure => x;

export const DISCLOSURES: Readonly<Record<DisclosureId, Disclosure>> = {
    'lender.cleanup': d({
        id: 'lender.cleanup',
        title: 'Clean up pending lender income',
        body: [
            'This transaction collects pending income for the pool’s existing lenders. You pay its network fee and any account rent needed to claim venue cashback.',
            'Your tokens stay in your wallet. Once cleanup confirms, deposits wait until any income still vesting has released. You review and sign your deposit separately.',
        ],
        ack: null,
        severity: 'info',
        source: 'AIR2-05; ARCH §3.3, §3.9; FRONTEND §3.6',
    }),
    'market.priceRefresh': d({
        id: 'market.priceRefresh',
        title: 'Refresh the market price',
        body: [
            'An idle market may need two transactions: record a price observation, then update the smoothed price after the chain clock advances. You review and sign each separately and pay its network fee.',
            'Opening a short is a separate action. Refreshing the price does not guarantee that new shorts are available.',
        ],
        ack: null,
        severity: 'info',
        source: 'open_short stale-price guard; crank_price observation buckets',
    }),
    'lender.solRepayment': d({
        id: 'lender.solRepayment',
        title: 'You may be repaid in SOL',
        body: [LENDER_SOL_REPAYMENT],
        ack: LENDER_SOL_REPAYMENT,
        severity: 'danger',
        source: 'SPEC §5.3 (verbatim, required checkbox)',
    }),
    'lender.risks': d({
        id: 'lender.risks',
        title: 'How lending works here',
        body: [
            'Lending is not staking and not risk-free. You earn rent from borrowers who short this token.',
            'Losses stay in this market: if a position cannot be bought back, every lender in this pool shares the loss pro-rata. There is no insurance fund and no treasury backstop.',
            'You can withdraw only from tokens that are not lent out, as a pro-rata mix of tokens and SOL. While tokens are borrowed, part of your shares can stay locked until the positions they stand for have expired, plus up to {lockGrace}.',
            'Rent is credited continuously. One-off income (your share of liquidation penalties, expiry fees, settlement proceeds above market value and swept balances) vests over {vestPeriod}.',
            'SOL that arrives while this market has no lenders goes to the protocol.',
        ],
        ack: null,
        severity: 'warn',
        source: 'SPEC §2.4, §5.4, §7; ARCH §3.3, §7.3; ECON §9.3–§9.5, §15 (lock_grace_s, lump_vest_s)',
    }),
    'lender.lossHistory': d({
        id: 'lender.lossHistory',
        title: 'What gaps cost lenders',
        body: [
            'If the price jumps past a position faster than it can be liquidated, the lenders are repaid in SOL worth less than the tokens. In the protocol’s model, lending in Tier {tier} breaks even if such a jump happens on this token less than about once every {breakEvenDays} days. These figures are illustrative and not yet calibrated on real data.',
            'This market so far: {settlements} SOL settlements, realised lender loss {realisedLoss} SOL.',
        ],
        ack: null,
        severity: 'info',
        source: 'ECON §11.6 (alpha = 2 column); INDEXER §8.4',
    }),
    'lender.exitForfeitsVest': d({
        id: 'lender.exitForfeitsVest',
        title: 'Unvested income stays in the pool',
        body: [
            'You have {vesting} SOL of income still vesting. If you withdraw now, the unvested part stays with the lenders who remain.',
        ],
        ack: 'I understand I give up my unvested income.',
        severity: 'warn',
        source: 'ARCH §3.3 (last lender, R4B-4); ECON §9.5 (no rebase orphans, KI-65)',
    }),
    'lender.residuals': d({
        id: 'lender.residuals',
        title: 'Known limits (accepted by the protocol)',
        body: [
            'If lenders withdraw ahead of a loss, those who leave first can take more than their fair share and those who stay less: about 4% of value at 90% utilisation, up to about 11% at 50–60%.',
            'A borrower can pay to delay its own liquidation by disturbing the price check every second. It only delays, at the borrower’s cost.',
            'Settlement amounts can be pushed by up to ±1%, and a settlement can be delayed (about 13 seconds per second an attacker controls the price observations, or indefinitely at a cost paid every second).',
            'If the keeper loses coverage, 5 consecutive seconds of attacker-controlled observations can turn a capped settlement into a full one.',
            'Small positions depend on short.win’s own keeper. A liquidating keeper is paid at least the SOL-settlement keeper reward.',
            'A liquidation sets nothing aside for the lenders: when buying the tokens back uses up the position’s vault, the lenders and the protocol lose the penalty and, on a position open for a long time, the rent the vault no longer covers. In the protocol’s Tier A model this is about 3.34 SOL on a fresh position and about 4.54 SOL on one open for 25 days at 90% utilisation.',
            'A minimum-size position held open can stop a Raydium market from moving to a deeper pool, so the market can stay on a thinner pool than it could use. The emergency multisig can pause new shorts to clear it.',
            'A sale bundled into an open can make that open record up to 10% less notional than an honest quote, so the market’s notional cap — the limit on how much can be shorted while a market is new — can be exceeded by up to about 11%.',
            'Tiny dust amounts can be left behind when a pool rebases.',
            'After a pool rebase, only you can convert your position, but a position worth less than one new share can convert to none when you next claim, deposit or withdraw. A lender converting after you, at any later time, can receive that share, and if that leaves it holding every share, its exit takes the income still vesting.',
        ],
        ack: null,
        severity: 'info',
        source: 'ARCH §8.5, §10 O19 (OWNER-M7-11, R4B-10); ECON §8.2, §9.3, §9.5 (KI-65, KI65-REV-SEC1), §12.9–§12.11',
    }),
    'lender.unpaidRent': d({
        id: 'lender.unpaidRent',
        title: 'Rent this pool has not been paid yet',
        body: [
            'This pool has credited {receivable} SOL of rent that borrowers have not paid yet; it is paid when their positions close. If a position closes without enough SOL left to pay its rent, the unpaid part is taken from the lenders pro-rata to the shares they hold at that moment — including lenders who joined after the rent was credited.',
            'At the pool’s current size, this deposit would carry {exposure} SOL of it ({share} of the pool from this deposit). If other lenders withdraw first, that share — and this deposit’s part of such a loss — grows.',
            'Such a loss is taken from your SOL balance in this pool. If it leaves that balance below zero, you must pay the difference in SOL (Cover deficit, on the Claim tab) before you can withdraw your tokens.',
        ],
        ack: null,
        severity: 'warn',
        source: 'ECON §9.4 (a shortfall is debited pro-rata to current shares); the owner’s decision of 2026-10-03',
    }),
    'lender.subUnitConversion': d({
        id: 'lender.subUnitConversion',
        title: 'Your position may convert to no shares',
        body: [
            'This pool’s shares were rebased since your position last changed, and your position is worth less than one new share: this transaction can convert it to 0 shares. Whether it does depends on other lenders’ conversions that land before it, which this review cannot see. A lender converting after you can receive that share.',
            'Income is vesting in this pool until {vestEnd}. A lender that ends up holding every share is paid all of the income still vesting when it leaves. Unless you need to act now, we advise waiting until {vestEnd} before you claim, deposit or withdraw here.',
        ],
        ack: null,
        severity: 'warn',
        source: 'ECON §9.5 (KI-65, KI65-REV-SEC1); the owner’s decision of 2026-10-07 (seventh round)',
    }),
    'lender.potSyncs': d({
        id: 'lender.potSyncs',
        title: 'Other lenders are synced first',
        body: [
            'This market’s pot cannot pay all of the SOL credited to you yet. This transaction first syncs {count} other {lenders} of this market whose realised SOL is more than their own credit; the excess, {gain} SOL by this review, moves to the pot and your claim draws on it.',
            'A sync you send for another lender never converts its shares through a share rebase: behind one it only books what the position has earned and moves the excess, and the conversion stays with its owner. A sync that finds nothing more to move moves nothing.',
        ],
        ack: null,
        severity: 'info',
        source: 'ECON §9.4 R3A-2, §9.5; ARCH I-P6, §3.3 sync_lender (KI-66 full, KI66-R4); the owner’s decision of 2026-10-08 (eighth round)',
    }),
    'lender.potSyncsDeferred': d({
        id: 'lender.potSyncsDeferred',
        title: 'Not every sync fits this transaction',
        body: [
            'Syncing {count} more {lenders} would move about {gain} SOL more to the pot, but one transaction carries at most four syncs within its size limit. This claim pays what it can now; claim again once it has landed, and the next claim syncs them first.',
        ],
        ack: null,
        severity: 'info',
        source: 'FRONTEND.md §3.6 (KI66-R4: at most 4 syncs per claim, 1,232 bytes per transaction)',
    }),
    'lender.claimCloses': d({
        id: 'lender.claimCloses',
        title: 'This claim closes your position',
        body: [
            'After this claim your position in this market holds no shares and no SOL, so the claim closes it and returns its account rent ({rent} SOL) to your wallet. Depositing here again later opens a new position.',
        ],
        ack: null,
        severity: 'info',
        source: 'ARCH §3.3 claim_sol (KI-67); the owner’s decision of 2026-10-08 (eighth round)',
    }),
    'borrower.liquidation': d({
        id: 'borrower.liquidation',
        title: 'How this short can end',
        body: [
            'Your collateral is your margin plus the sale proceeds, locked in the position’s vault. Collateral ratio (CR) = vault SOL ÷ the SOL needed to buy the tokens back now, including price impact.',
            'Liquidation: if CR falls below {liqCr} on both the live price and the smoothed price, anyone can liquidate. The tokens are bought back with your vault SOL, a {penalty} penalty is taken, and the rest comes back to you.',
            // ¶3 amended by the owner on 2026-10-11 (thirteenth round): the expiry fee only when the
            // position cannot be liquidated (SPEC §5.3; expire.rs's `UseLiquidate` precondition).
            'Expiry: after {maxDuration} the position can be closed by anyone. Unless it can be liquidated at that point, you pay a {expiryFee} expiry fee instead of the penalty.',
            'Settlement in SOL, when the tokens cannot be bought back:',
            'S1 — the price jumped past your vault: the lenders take all vault SOL minus the keeper reward. You receive nothing.',
            'S2 — buying back would exceed the maximum liquidation slippage ({maxSlippage}): the lenders take the vault SOL minus the keeper reward, but no more than fair value (the buyback at the smoothed price plus the penalty and rent; the {expiryFee} expiry fee instead of the penalty for an expired position). Anything above that comes back to you.',
            'S3 — no route to buy back for {s3Wait} while the position is liquidatable (venue halted, pool drained, migration stuck): as S2.',
            'Rent accrues every second on the borrowed tokens’ value; at least {minRent} of rent is charged.',
        ],
        ack: 'I understand my position can be liquidated or settled in SOL.',
        severity: 'danger',
        source: 'SPEC §4, §5, §5.3, §6.1; ARCH §3.4–§3.8; ¶3: expire.rs (precondition: not CR-liquidatable, else `UseLiquidate`: the penalty through liquidate, ECON §8.4), amended by the owner on 2026-10-11 (thirteenth round)',
    }),
    'borrower.residuals': d({
        id: 'borrower.residuals',
        title: 'Known limits (accepted by the protocol)',
        body: [
            'A keeper that liquidates you may pay up to 2% more than the protocol’s reference price for the buyback; that comes out of what is returned to you.',
            'If someone holds the price up while your position is being liquidated, the keeper can take most of what would otherwise come back to you.',
            'A liquidating keeper is paid at least {keeperReward}, topped up from what would come back to you.',
            'If the venue halts or breaks, a healthy position can be neither closed nor settled until the route returns, an adapter upgrade lands (48-hour timelock), or the position reaches its deadline (up to {maxDuration}).',
            'A sale bundled into an open can make that open record up to 10% less notional than an honest quote, so a market’s notional cap can be exceeded by up to about 11%. Your margin is not affected.',
            'On low-fee Raydium pools, a basket of positions at the market’s open-interest cap can still be profitable to manipulate.',
        ],
        ack: null,
        severity: 'info',
        source: 'ARCH §8.5, §10 O19 (OWNER-M7-11); ECON §8.2, §12.2, §12.5, §12.6',
    }),
    'market.creatorSelfDealing': d({
        id: 'market.creatorSelfDealing',
        title: 'The token’s creator can be on both sides',
        body: [
            'The creator of this token can lend its own supply here and short it from another wallet — in effect selling into the market. If the token then rises, the creator may be repaid in SOL on its own lending.',
            'Deposits from the creator’s wallet are shown on this page. Other wallets the creator controls cannot be identified, and the per-wallet cap does not stop one person from using several wallets.',
            'The creator also earns the venue’s creator fee on every short.win trade of this token: {creatorFee} so far.',
        ],
        ack: null,
        severity: 'warn',
        source: 'SPEC §7, §12 R1; ARCH §7.3 (C0-18), §8.5; ECON §12.8',
    }),
    'market.tierAge': d({
        id: 'market.tierAge',
        title: 'Why this tier',
        body: [
            'Tiers are set automatically from on-chain liquidity and pool age, and checked again on every trade.',
            'short.win can only prove a pool’s age from the moment it first saw it: the real graduation time for tokens listed before they graduated, otherwise the listing time. A Raydium CPMM pool created after mid-2024 records its own creation time, which is used instead.',
            'An older pool listed today therefore starts in Tier C for 24 hours and is at most Tier B until day 7. This is conservative: higher margins, smaller caps and shorter durations.',
        ],
        ack: null,
        severity: 'info',
        source: 'SPEC §4 (O13, O13b)',
    }),
    'market.coveragePause': d({
        id: 'market.coveragePause',
        title: 'New shorts paused: keeper coverage',
        body: [
            'short.win’s keeper cannot currently cover this market (degraded mode), so new shorts are paused.',
            'Closing, adding margin, liquidations, settlements, withdrawals and claims continue. New shorts resume automatically once coverage is back.',
            'While coverage is degraded, positions here may be liquidated later than usual.',
        ],
        ack: null,
        severity: 'warn',
        source: 'SPEC §11 (O17, M7-3, M7-13/14)',
    }),
    'market.poolSwitch': d({
        id: 'market.poolSwitch',
        title: 'This market’s pool can change',
        body: [
            'If the current pool fails (halted, drained or too expensive), short.win can move this market to a deeper pool.',
            'Open positions move only if each one fits the new pool’s normal opening limits. Otherwise they stay and settle in SOL at fair value at their deadline (S3).',
            'Right after a change, the new pool’s prices are confirmed for 8 seconds before liquidations and new shorts resume.',
        ],
        ack: null,
        severity: 'info',
        source: 'SPEC §14 (#99, M4R-1, M4F-1); ARCH §2.6',
    }),
    'market.routeBroken': d({
        id: 'market.routeBroken',
        title: 'Route marked broken',
        body: [
            'The emergency multisig marked this market’s trading route as broken. New shorts are refused.',
            'Closes and liquidations are never paused by this flag, but they can fail while the venue cannot trade. Positions that are liquidatable settle in SOL after {s3Wait} without a route (S3).',
        ],
        ack: null,
        severity: 'danger',
        source: 'SPEC §11 (O6)',
    }),
    'market.venueUpgraded': d({
        id: 'market.venueUpgraded',
        title: 'Venue upgraded, under review',
        body: [
            'A venue program this market trades on was upgraded and has not been re-validated yet. New shorts wait for the review; exits are never blocked by it.',
        ],
        ack: null,
        severity: 'warn',
        source: 'ARCH §0 (8), §3.1 (ADOPTED Q13)',
    }),
    'market.migrating': d({
        id: 'market.migrating',
        title: 'Graduating to its pool',
        body: [
            'This token is migrating from the Pump.fun curve to its pool. Closes and liquidations pause during the migration and resume once the pool’s prices are confirmed.',
            'If the migration gets stuck, positions settle in SOL (S3) after the migration pause.',
        ],
        ack: null,
        severity: 'warn',
        source: 'SPEC §5.3; ARCH §5.4',
    }),
    'market.graduationHalted': d({
        id: 'market.graduationHalted',
        title: 'Graduation halted',
        body: [
            'PumpSwap pool creation is disabled, so this curve cannot graduate. New curve positions are refused until it is re-enabled; open positions are unaffected.',
        ],
        ack: null,
        severity: 'warn',
        source: 'SPEC §14 M6-SEC-3; ARCH §3.11 (6104)',
    }),
    'market.listOnly': d({
        id: 'market.listOnly',
        title: 'Lend only',
        body: [
            'This token has no pool short.win can price on-chain yet (for example it trades only on Orca, Meteora DLMM or Raydium CLMM). It can be listed and lent, but not shorted.',
        ],
        ack: null,
        severity: 'info',
        source: 'SPEC §3',
    }),
    'market.venueTrust': d({
        id: 'market.venueTrust',
        title: 'Venue trust assumptions',
        body: [
            'Pump.fun, PumpSwap and their fee program can be upgraded at any time by one upgrade authority, and are upgraded several times a month.',
            'Raydium AMM v4 and CPMM are upgraded by a 3-of-4 multisig with no timelock and have no verified builds. A 2-of-3 pool admin can halt a pool or raise its fees.',
            'Token-2022 is upgradeable. short.win itself is upgradeable by its governance multisig after a 48-hour timelock.',
            'A malicious venue upgrade can harm exits; no design can prevent that.',
        ],
        ack: null,
        severity: 'info',
        source: 'ARCH §8.5; SPEC §12 R5',
    }),
    'market.mintUnsafe': d({
        id: 'market.mintUnsafe',
        title: 'Token flagged unsafe',
        body: [
            'This token no longer passes the safety filter (freeze authority, permanent delegate, transfer hook or transfer fee). Deposits and new shorts are blocked; withdrawals, claims and exits continue.',
        ],
        ack: null,
        severity: 'danger',
        source: 'SPEC §3; ARCH §3.9 flag_unsafe_mint',
    }),
    'market.create': d({
        id: 'market.create',
        title: 'Creating a market',
        body: [
            'Anyone can create a market for any token that passes the safety filter. There is no approval and no special status for the creator.',
            'You pay the creation fee ({fee}) and the rent for the market’s accounts ({rent}). The rent stays in the accounts.',
        ],
        ack: null,
        severity: 'info',
        source: 'SPEC §3; ARCH §3.2',
    }),
    'token.cpiGuard': d({
        id: 'token.cpiGuard',
        title: 'CPI Guard is on for this token account',
        body: [
            'CPI Guard blocks transfers that programs make on your behalf, so short.win cannot deposit from this token account.',
            'Deposit from another account of this token without CPI Guard, or turn CPI Guard off for this one deposit: the transaction turns it off, deposits and turns it back on, and your wallet shows all three steps.',
        ],
        ack: null,
        severity: 'warn',
        source: 'ARCH §3.3; R-MINT §8.4',
    }),
    'token.memoRequired': d({
        id: 'token.memoRequired',
        title: 'Memo required',
        body: [
            'This token account only accepts transfers with a memo. short.win adds the memo when it pays tokens to it.',
        ],
        ack: null,
        severity: 'info',
        source: 'ARCH §3.3 (M1-SEC-7)',
    }),
    'tx.simulation': d({
        id: 'tx.simulation',
        title: 'Simulated result',
        body: [
            'These changes come from simulating the transaction against the current chain state. The result can differ if the market moves before it lands; the limits you set protect you.',
        ],
        ack: null,
        severity: 'info',
        source: 'SPEC §9; ARCH §7.3',
    }),
};

/** The placeholders a disclosure's body uses, in order of first appearance. */
export function placeholders(id: DisclosureId): string[] {
    const seen = new Set<string>();
    for (const p of DISCLOSURES[id].body) for (const m of p.matchAll(/\{(\w+)\}/g)) if (m[1]) seen.add(m[1]);
    return [...seen];
}

/** The body with its placeholders filled. Throws on a missing value, never renders `{name}`. */
export function renderDisclosure(id: DisclosureId, vars: Readonly<Record<string, string>> = {}): string[] {
    return DISCLOSURES[id].body.map((p) =>
        p.replace(/\{(\w+)\}/g, (_, k: string) => {
            const v = vars[k];
            if (v === undefined) throw new Error(`disclosure ${id}: missing {${k}}`);
            return v;
        }),
    );
}
