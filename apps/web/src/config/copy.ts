/**
 * The newcomer copy of the owner's thirteenth round (FRONTEND.md §4.3; audit findings D-03, D-21,
 * D-23): the markets list's intro strip, the How it works page, the links to it, the Short panel's
 * link to the coverage banner (O-2) and the Claim button (O-5). This file is the single home of
 * every string, each with its id: components render them through {@link copy} / {@link CopyText},
 * `copy.test.ts` pins them, and `scripts/owner-copy.ts` prints them with their sources (the owner's
 * file) and FRONTEND.md §4.3's catalogue. Approved as written by the owner on 2026-10-11 (thirteenth
 * round); a change needs the owner's approval, this code and FRONTEND §4.3 together.
 *
 * `{name}` is a placeholder filled from the live `GlobalConfig` (governable numbers are never
 * written into the copy, §4.1 rule 5); until the config is read the page shows a placeholder
 * skeleton, never a word in its place. `[label](target)` is a link: `/path`, `/path#anchor` or
 * `#anchor` on the same page.
 */

import { type CopyEntry, c, NAV_COPY } from './copyNav.ts';

export type { CopyEntry };

export const COPY = {
    // ---- O-3 (a): the intro strip above the markets list (`markets-intro`) ----------------------
    'intro.heading': c('New to short.win?', 'Intro strip: heading (an h2)'),
    'intro.short.title': c('Short a token', 'Intro strip: part 1 title'),
    'intro.short.body': c(
        'Borrow a token and sell it. You gain if the price falls by more than the rent, fees and price impact; if it rises, you can lose all your margin and the fees you paid.',
        'Intro strip: part 1',
        'Borrow and sell in one transaction: SPEC §1, §2.1. Gain only past rent, fees and price impact: SPEC §2.2 (“Profit if buyback cost < S”), §6.1, §6.2; venue fees and price impact: SPEC §4 (the CR and the margin use the *executable* buyback cost “including price impact”; the size caps let one buyback move the price by up to 5%, Tier D 3%, on top of the sale’s own impact). Lose the margin: S1 “You receive nothing” (D:borrower.liquidation; SPEC §5.3); the keeper top-ups (ECON G15, SPEC §6.3 N22) come from the remainder, i.e. the margin; the open and network fees are paid; the position account’s rent is returned on every exit (liquidate.rs, expire.rs, sol_settle.rs).',
    ),
    'intro.lend.title': c('Lend a token', 'Intro strip: part 2 title'),
    'intro.lend.body': c(
        'Lend your tokens for rent paid by the people who short them. It is not staking, and not risk-free.',
        'Intro strip: part 2',
        'SPEC §1, §2.4; “not staking and not risk-free”: D:lender.risks ¶1, SPEC §7.',
    ),
    'intro.risks.title': c('What can go wrong', 'Intro strip: part 3 title'),
    'intro.risks.body': c(
        'A short can be liquidated or settled in SOL. Lenders can be repaid in SOL worth less than the tokens they lent, and can owe SOL for rent borrowers never pay.',
        'Intro strip: part 3',
        'D:borrower.liquidation (acknowledgement “liquidated or settled in SOL”); D:lender.solRepayment (SPEC §5.3); owing SOL: SPEC §2.4 step 5, ECON §9.4 “The late depositor’s exposure”, KI-53, D:lender.unpaidRent ¶3.',
    ),
    'intro.risks.linkShort': c(
        'How a short can end',
        'Intro strip: part 3, link 1 → How it works #short-risks (D:borrower.liquidation verbatim)',
        'Link label; the disclosure’s own title, “How this short can end”, refers to one position.',
    ),
    'intro.risks.linkLend': c(
        'How lending works here',
        'Intro strip: part 3, link 2 → How it works #lend-risks (D:lender.risks verbatim)',
        'The title of D:lender.risks, verbatim.',
    ),
    'intro.more': c('How it works', 'Intro strip: link in the heading row → /how-it-works'),
    'intro.dismiss': c('Hide', 'Intro strip: the dismiss button (visible text)'),
    'intro.dismiss.aria': c(
        'Hide the introduction',
        'Intro strip: the dismiss button’s accessible name (contains the visible “Hide”, WCAG 2.5.3)',
    ),
    'intro.dismissed.note': c(
        'Introduction hidden. You can still open How it works from the status bar or the More menu.',
        'Markets page: a polite status message after hiding (screen readers; focus moves to the “Markets” heading)',
        'The page is linked from the status bar (desktop, tablet; on a phone its expanded list) and the phone’s More sheet (§3.3 below).',
    ),

    // ---- O-3 (c): the How it works page (`/how-it-works`) -----------------------------------
    'howItWorks.title': c('How it works', 'How it works: h1'),
    'howItWorks.lead': c(
        'short.win lets you short a Solana token by borrowing it from the people who lend it, and lets holders lend their tokens for rent. Both sides carry real risk. This page covers the basics; each market shows its own terms before you sign.',
        'How it works: lead paragraph',
        'SPEC §1; “real risk”: SPEC §5.3, §5.4, §7; own terms: FE §3.7 tier-terms.',
    ),
    'howItWorks.toc.aria': c('On this page', 'How it works: the contents nav (accessible name and visible heading)'),
    'howItWorks.short.h': c('Shorting a token', 'How it works: h2 #shorting'),
    'howItWorks.short.p1': c(
        'You choose a size in tokens and a slippage limit. In one transaction, short.win borrows the tokens from the market’s lending pool and sells them on the token’s venue: the Pump.fun bonding curve, PumpSwap or Raydium.',
        'How it works: Shorting ¶1',
        'SPEC §2.1 steps 1–3, §3 “Supported venues”; FE §3.7 (size in tokens).',
    ),
    'howItWorks.short.p2': c(
        'The sale proceeds stay locked in your position’s vault together with the margin you post. You never hold the borrowed tokens or the proceeds.',
        'How it works: Shorting ¶2',
        'SPEC §2.1 step 4 and the paragraph after it (“The borrower never holds the tokens or the proceeds”).',
    ),
    'howItWorks.short.p3': c(
        'Your margin is the SOL it takes to bring your collateral ratio (CR) up to the market’s opening CR, for example {exCrOpen} in Tier {exTier}. The preview shows the exact margin, fee and costs before you sign.',
        'How it works: Shorting ¶3',
        'SPEC §4 “Margin rule”; ECON §15 cr_open_bps (T); FE §3.7 preview fields. Fill: {exTier} = C, {exCrOpen} = formatBps(tiers[2].crOpenBps, 0).',
    ),
    'howItWorks.short.p4': c(
        'To close, short.win buys the tokens back with your vault’s SOL, returns them to the pool, pays the rent, and sends you the rest. You gain only if the price has fallen by more than the venue fees, price impact, rent and the open fee together; otherwise you get back less than you put in.',
        'How it works: Shorting ¶4',
        'SPEC §2.2 (“Profit if buyback cost < S”); the buyback includes impact and fees (SPEC §4 CR definition, margin rule); rent paid at close: SPEC §2.2 step 3, §6.1; open fee: SPEC §6.2 (paid at open, on top of the margin: the preview’s P&L is payout − margin − open fee, previews.ts).',
    ),
    'howItWorks.short.p5': c(
        'A position pays rent every second and can stay open up to the market’s maximum duration ({durMin} to {durMax} depending on the tier). After that, anyone can close it: if it is still above its liquidation CR, you pay an expiry fee and a small keeper reward instead of the liquidation penalty.',
        'How it works: Shorting ¶5',
        'SPEC §4 “Duration”, §14 row 15, §5.3 (the expiry fee only for a position “not below its liquidation CR”); D:borrower.liquidation ¶3; expire.rs (precondition: **not** CR-liquidatable, else `UseLiquidate` and the penalty; K_e from the borrower’s remainder); ECON G15. Fill: proseDuration of the smallest and largest tiers[].maxDurationS.',
    ),
    'howItWorks.lend.h': c('Lending a token', 'How it works: h2 #lending'),
    'howItWorks.lend.p1': c(
        'You deposit tokens into the market’s pool and receive pool shares. Every lender in a market shares the same pool, so income and losses are shared pro-rata.',
        'How it works: Lending ¶1',
        'SPEC §2.4 step 1, §5.3 (“pools are shared, so losses are shared”), §5.4.',
    ),
    'howItWorks.lend.p2': c(
        'Borrowers pay rent on the tokens they borrow. Your share is credited to you as it accrues and is paid in when their positions end (closed, liquidated, expired or settled in SOL); rent a position never pays is taken back (see What can go wrong). You claim it on the market’s Lend → Claim tab or on the Lending page.',
        'How it works: Lending ¶2',
        'ECON §9.4 (“Entitlement accrues continuously, but rent is paid only at exit”; repay / settle on every exit path; a shortfall is debited pro-rata); SPEC §2.4 steps 2 and 5, §5.2; FE §3.6, §3.8 (the Lending page’s Claim). Paid-in rent can still wait on other lenders’ syncs (claim.note.shortfall), so the text does not say “claimable once paid”.',
    ),
    'howItWorks.lend.p3': c(
        'Before you deposit into a pool with unpaid rent, the Lend tab shows that rent. It also shows how much of it your deposit would carry if it is never paid.',
        'How it works: Lending ¶3',
        'SPEC §2.4 step 5; FE §3.6 (lend-preview-unpaid-rent, -share; shown when the pool has unpaid rent, DepositForm.tsx poolUnpaid > 0); D:lender.unpaidRent.',
    ),
    'howItWorks.lend.more': c(
        'The rules for withdrawals, locks and income are under What can go wrong, below.',
        'How it works: Lending, last line',
    ),
    'howItWorks.risk.h': c('What can go wrong', 'How it works: h2 #what-can-go-wrong'),
    'howItWorks.risk.lead': c(
        'Below are the main risk texts the app shows on its market pages and before you sign, word for word, with short notes on others. Each market page and each review shows every text that applies there; read them before you trade or lend.',
        'How it works: What can go wrong, lead',
        'FE §4.1 rule 1; FE-D4; the market page’s banners (MarketPage.tsx) and Overview (OverviewTab.tsx, market.venueTrust). Embedded verbatim: borrower.liquidation, borrower.residuals, lender.risks, lender.solRepayment, lender.residuals, market.coveragePause, market.venueTrust; the notes (risk.lend.p2, risk.lend.p1, risk.all.p1) paraphrase others (disclosures.ts holds them all).',
    ),
    'howItWorks.risk.short.h': c('If you short', 'How it works: h3 #short-risks'),
    'howItWorks.risk.shortCaption': c(
        'Shown with Tier {exTier}’s current terms as an example. Each market’s Short panel shows the same text with that market’s terms.',
        'How it works: caption above the embedded borrower.liquidation',
        'ECON §15 (tier terms differ, T→P); FE §3.7 (the Short panel renders D:borrower.liquidation with the live tier terms).',
    ),
    'howItWorks.risk.lend.h': c('If you lend', 'How it works: h3 #lend-risks'),
    'howItWorks.risk.lend.p2': c(
        'Rent that borrowers never pay is taken from the lenders who hold shares when the position ends, including lenders who deposited after that rent was credited. Such a loss is taken from your SOL balance in this pool. If it leaves that balance below zero, you must pay the difference in SOL (Cover deficit, on the Claim tab) before you can withdraw your tokens. The amount is not limited to what you have received from the pool; a market’s Lend tab shows any unpaid rent before you deposit.',
        'How it works: If you lend, after the embedded lender.solRepayment',
        'SPEC §2.4 step 5; ECON §9.4 “The late depositor’s exposure” (the debit is not bounded by what the lender received or deposited); KI-53; sentences 2–3 verbatim from D:lender.unpaidRent ¶3; DepositForm.tsx (the unpaid-rent rows when the pool has any).',
    ),
    'howItWorks.risk.lend.p1': c(
        'Each market’s Lend tab also shows what price gaps cost lenders in its tier, and the losses that market has had so far.',
        'How it works: If you lend, before the collapsed lender.residuals',
        'D:lender.lossHistory (rendered per market, FE §3.6).',
    ),
    'howItWorks.risk.all.h': c('In every market', 'How it works: h3'),
    'howItWorks.risk.all.p1': c(
        'A token’s creator can lend and short its own token from different wallets — in effect selling into the market — and the per-wallet cap does not stop one person from using several wallets. Each market’s Overview shows the deposits made from the creator’s wallet.',
        'How it works: In every market ¶1',
        'D:market.creatorSelfDealing ¶1–2 (rendered per market, with {creatorFee}); SPEC §12 R1 (“one actor with four wallets fills a market’s OI cap”); KI-15; FE §3.4 (Overview).',
    ),
    'howItWorks.fees.h': c('Fees and rent', 'How it works: h2 #fees'),
    'howItWorks.fees.open': c(
        'Open fee: {openFee} of the position’s notional, to the protocol. There is no close fee.',
        'How it works: Fees list',
        'SPEC §6.2; ECON §15 open_fee_bps (G). Fill: percent(params.openFeeBps).',
    ),
    'howItWorks.fees.venue': c(
        'Venue fees: the venue charges its own trading fee on the sale when you open and on the buyback when the position ends; on some venues part of it goes to the token’s creator. The preview’s figures include it.',
        'How it works: Fees list',
        'SPEC §4 margin rule (“fees (~1.25% each way)”); D:market.creatorSelfDealing ¶3 (the venue’s creator fee); FE §3.7 (proceeds and buyback quotes are the venue’s, fees included).',
    ),
    'howItWorks.fees.rent': c(
        'Rent: a yearly rate on the value of the borrowed tokens, charged every second. It rises with the market’s utilisation along the tier’s curve: gently at first, steeply once most of the pool is lent out. Every position pays at least its minimum rent. Lenders receive the rent less the protocol’s share ({rentProtocol}).',
        'How it works: Fees list',
        'SPEC §6.1, §6.3; ECON §7; ECON §15 util_kink_bps, rent_protocol_bps, min_rent_s. Fill: percent(params.rentProtocolBps).',
    ),
    'howItWorks.fees.rate': c(
        'The Rate (APR) on each market page is what a new short pays at the market’s utilisation now.',
        'How it works: Fees list',
        'features/markets/market/StatStrip.tsx (“for a new short at … utilisation”); FE §3.4.',
    ),
    'howItWorks.fees.penalty': c(
        'Liquidation penalty: set by the tier and split between the lenders ({penLender}), the protocol ({penProtocol}) and the keeper that liquidates ({penKeeper}). If the keeper’s share is less than the settlement keeper reward, the difference comes out of what would come back to you.',
        'How it works: Fees list',
        'SPEC §4 table, §6.3 (N22: “Any top-up above its 30% penalty share comes from the borrower’s remainder”); ECON §15 penalty_bps (T), penalty split (G); D:borrower.residuals ¶3. Fill: percent(params.penalty{Lender,Protocol,Keeper}Bps).',
    ),
    'howItWorks.fees.expiry': c(
        'Expiry fee: {expiryFee} of the buyback cost, paid instead of the penalty by a position closed after its deadline while still above its liquidation CR, all to the lenders. The keeper that closes it is also paid a small, capped reward from the position.',
        'How it works: Fees list',
        'SPEC §4 “Duration”, §5.3, §6.3; ECON §15 expiry_fee_bps, expiry_keeper_fixed / _bps / _cap; ECON G15 (fee base min(B̂_live, B̂_ref), the buyback cost); expire.rs (not CR-liquidatable, else `UseLiquidate`). Fill: percent(params.expiryFeeBps).',
    ),
    'howItWorks.fees.settle': c(
        'Settlement in SOL: the keeper is paid {keeperReward}; the lenders take the rest of the vault, capped at fair value in S2 and S3.',
        'How it works: Fees list',
        'SPEC §5.3, §6.3; D:borrower.liquidation (S1–S3 lines). Fill: keeperRewardText(params.settleKeeperFixed, settleKeeperBps, settleKeeperCap).',
    ),
    'howItWorks.fees.network': c(
        'Network fees and account deposits: every transaction pays Solana’s network fee. A new position, or your first deposit in a market, also pays a refundable deposit for a new account (Solana’s account rent, not the rent lenders earn). It comes back when that account closes.',
        'How it works: Fees list',
        'FE §3.7 total-cost (“the position’s rent returns on close”); DepositForm.tsx row “New account rent (refunded on exit)”; D:lender.claimCloses.',
    ),
    'howItWorks.fees.create': c(
        'Creating a market: a creation fee of {creationFee} plus Solana’s account rent for the market’s accounts, which stays in the accounts.',
        'How it works: Fees list',
        'D:market.create ¶2; SPEC §3; ECON §15 market_creation_fee. Fill: formatSol(params.marketCreationFee) + “ SOL”.',
    ),
    'howItWorks.sign.h': c('Before you sign', 'How it works: h2 #before-you-sign'),
    'howItWorks.sign.p1': c(
        'Every transaction is first simulated against the chain as it is now. You review what it will change before your wallet asks you to sign. If the market moves before the transaction is confirmed, it can still go through at a worse price, up to the limits you set (slippage and tolerances); beyond them it fails instead.',
        'How it works: Before you sign ¶1',
        'FE-D2, FE-D6, §5.3; SPEC §9 (“transaction simulation before signing”); D:tx.simulation (same meaning); the bound is the limit, not the quote: close_short(max_buyback_cost = ⌈B̂·(1 + slippage)⌉), FE §3.8.',
    ),
    'howItWorks.more.h': c('Read more', 'How it works: h2 #read-more'),
    'howItWorks.more.security': c(
        '[Security policy](/security): how to report a vulnerability, and the audit reports once they are published.',
        'How it works: Read more list',
        'features/security/SecurityPolicyPage.tsx (Reporting; Audits: “Reports will be listed on this page once published”).',
    ),
    'howItWorks.more.limits': c(
        'Known limits: the residual risks the protocol accepts, [for lenders](#lender-limits) and [for borrowers](#borrower-limits).',
        'How it works: Read more list (the links open the two collapsed disclosures above)',
        'D:lender.residuals, D:borrower.residuals (title “Known limits (accepted by the protocol)”).',
    ),
    'howItWorks.more.status': c(
        '[Status](/status): the indexer, the venues and any markets with paused shorts.',
        'How it works: Read more list',
        'FE §3.9; the More sheet’s subtitle “Indexer, venues, paused markets” (shell/MoreSheet.tsx).',
    ),
    'howItWorks.more.glossary': c(
        'Terms: tap or click ⓘ next to a label for a short definition.',
        'How it works: Read more list',
        'The glossary control (components/Glossary.tsx; FE §4.3).',
    ),
    'howItWorks.back': c('Back to markets', 'How it works: the foot link → /markets', 'The Security page’s foot link.'),
    'howItWorks.configError': c(
        'The protocol’s current numbers could not be read, so they are missing on this page.',
        'How it works: an alert above the sections when the GlobalConfig read fails or finds no account (with Retry); also in place of the risk texts that need those numbers',
        'The page’s fees, tier terms and the borrower disclosures’ figures come from GlobalConfig (§4.1 rule 5); ErrorState’s Retry (components/States.tsx).',
    ),

    // ---- links to the page (the shell's; `copyNav.ts`) ----------------------------------------
    ...NAV_COPY,

    // ---- O-2: the Short panel's link to the page's coverage banner ------------------------------
    'coverage.link.inline': c(
        'Read the full coverage notice',
        'Short panel beside the page (≥ 1,024 px), under the “New shorts unavailable” note: scrolls to the banner and focuses it',
        'The page banner market.coveragePause (FE §3.4 “Banners”).',
    ),
    'coverage.link.sheet': c(
        'Close and read the full coverage notice',
        'Trade sheet (< 1,024 px), under the same note: closes the sheet, then scrolls to the banner and focuses it',
        'FE §3.4 (the sheet ?sheet=trade; ✕ / Escape / Back close it).',
    ),

    // ---- O-5: the Claim button ---------------------------------------------------------------
    'claim.label.full': c(
        'Review claim · {c} SOL',
        'Lend → Claim submit and each Lending page row’s Claim when the syncs cannot add 10,000 lamports or more (no shortfall, dust under 10,000 lamports, or a shortfall no sync can fund; the owner’s, 2026-10-11), or while the search for syncs runs with something claimable now (unchanged; also “Review claim · 0 SOL”, disabled, with “Nothing to claim yet”)',
        'ClaimForm.tsx (unchanged string). {c} = formatSol(claimable, solFrac(claimable)).',
    ),
    'claim.label.syncs': c(
        'Review claim · {c} SOL now · syncs may add up to {g} SOL',
        'Lend → Claim submit and each Lending page row’s Claim when the syncs the search picks can add 10,000 lamports or more of what is credited but not claimable now (CLAIM_DUST_LAMPORTS; the owner’s, 2026-10-11: only when syncs can pay); two lines, the second from “syncs”',
        'previews.ts previewClaimView: credited = claimable + shortfall when own + pot < pending, so the claim never pays more than {c} + {g} (claim_sol.rs pay ≤ pending); build.ts searchPotSyncs (gatherPotSyncs, run first by claim/syncSearch.ts and again by claimPlanFrom unless its pick is fresh) looks for other lenders only while shortfall > 0 (from 10,000 lamports: CLAIM_DUST_LAMPORTS, ECON §9.4 rounding dust), whose sync can move their excess to the pot (ECON §9.4 “Why the pot”); plans.ts review title. {c} = formatSol(claimable, solFrac(claimable)), {g} = formatSol(min(shortfall, the picked syncs’ gain), solFrac(…)).',
    ),
    'claim.note.shortfall': c(
        '{g} SOL of what is credited to you cannot be claimed now. Some of it may be rent that open positions still owe: it is paid when they end, and taken back from the lenders if it never is. Some may be allocated to other lenders’ shares beyond their own credit. A claim first syncs those lenders where it finds any, which moves that excess to the pot; the review shows what the claim will pay.',
        'Lend → Claim: the note above the button, appended when there is a shortfall (10,000 lamports or more: CLAIM_DUST_LAMPORTS, owner, 2026-10-11) (replaces “{g} SOL credited to you is not paid in yet. A claim first syncs other lenders whose surplus can fund it, where there are any.”)',
        'ECON §9.4 (“rent is paid only at exit”; a shortfall is debited pro-rata; touch: surplus = (real − claimed) − max(pending, 0) moves to MarketVault.unmatched; “Why the pot”); build.ts claimPlanFrom, gatherPotSyncs; plans.ts (the review title).',
    ),
} as const satisfies Record<string, CopyEntry>;

export type CopyId = keyof typeof COPY;

/** A string with its placeholders filled; a missing value throws (never renders `{name}`). */
export function copy(id: CopyId, vars: Readonly<Record<string, string>> = {}): string {
    return COPY[id].text.replace(/\{(\w+)\}/g, (_, k: string) => {
        const v = vars[k];
        if (v === undefined) throw new Error(`copy ${id}: missing {${k}}`);
        return v;
    });
}

/** The placeholders a string uses, in order of first appearance. */
export function copyPlaceholders(id: CopyId): string[] {
    return [...new Set([...COPY[id].text.matchAll(/\{(\w+)\}/g)].map((m) => m[1] as string))];
}

/** A run of copy text: plain text, a placeholder, or a link `[label](target)`. */
export type CopyPart =
    | { readonly kind: 'text'; readonly text: string }
    | { readonly kind: 'var'; readonly name: string }
    | { readonly kind: 'link'; readonly label: string; readonly target: string };

/** Splits a string into text, placeholders and links (the page renders each part). */
export function copyParts(id: CopyId): CopyPart[] {
    const out: CopyPart[] = [];
    const re = /\[([^\]]+)\]\(([^)]+)\)|\{(\w+)\}/g;
    const text = COPY[id].text;
    let at = 0;
    for (const m of text.matchAll(re)) {
        if (m.index > at) out.push({ kind: 'text', text: text.slice(at, m.index) });
        if (m[3]) out.push({ kind: 'var', name: m[3] });
        else out.push({ kind: 'link', label: m[1] as string, target: m[2] as string });
        at = m.index + m[0].length;
    }
    if (at < text.length) out.push({ kind: 'text', text: text.slice(at) });
    return out;
}

/**
 * The How it works page, top to bottom (`features/guide/HowItWorksPage.tsx` renders exactly this;
 * `scripts/owner-copy.ts` prints it): headings, paragraphs, list items and the approved disclosures
 * it embeds verbatim (`open`: expanded; else collapsed; `anchor`: its id on the page).
 */
export type GuideBlock =
    | { readonly kind: 'h2' | 'h3'; readonly id: CopyId; readonly anchor: string }
    | { readonly kind: 'p'; readonly id: CopyId; readonly className?: string }
    | { readonly kind: 'list'; readonly ids: readonly CopyId[] }
    | {
          readonly kind: 'disclosure';
          readonly disclosure:
              | 'borrower.liquidation'
              | 'borrower.residuals'
              | 'lender.risks'
              | 'lender.solRepayment'
              | 'lender.residuals'
              | 'market.coveragePause'
              | 'market.venueTrust';
          readonly open: boolean;
          readonly anchor?: string;
      };

export const HOW_IT_WORKS: readonly GuideBlock[] = [
    { kind: 'h2', id: 'howItWorks.short.h', anchor: 'shorting' },
    { kind: 'p', id: 'howItWorks.short.p1' },
    { kind: 'p', id: 'howItWorks.short.p2' },
    { kind: 'p', id: 'howItWorks.short.p3' },
    { kind: 'p', id: 'howItWorks.short.p4' },
    { kind: 'p', id: 'howItWorks.short.p5' },
    { kind: 'h2', id: 'howItWorks.lend.h', anchor: 'lending' },
    { kind: 'p', id: 'howItWorks.lend.p1' },
    { kind: 'p', id: 'howItWorks.lend.p2' },
    { kind: 'p', id: 'howItWorks.lend.p3' },
    { kind: 'p', id: 'howItWorks.lend.more' },
    { kind: 'h2', id: 'howItWorks.risk.h', anchor: 'what-can-go-wrong' },
    { kind: 'p', id: 'howItWorks.risk.lead' },
    { kind: 'h3', id: 'howItWorks.risk.short.h', anchor: 'short-risks' },
    { kind: 'p', id: 'howItWorks.risk.shortCaption', className: 'guide-caption' },
    { kind: 'disclosure', disclosure: 'borrower.liquidation', open: true },
    { kind: 'disclosure', disclosure: 'borrower.residuals', open: false, anchor: 'borrower-limits' },
    { kind: 'h3', id: 'howItWorks.risk.lend.h', anchor: 'lend-risks' },
    { kind: 'disclosure', disclosure: 'lender.risks', open: true },
    { kind: 'disclosure', disclosure: 'lender.solRepayment', open: true },
    { kind: 'p', id: 'howItWorks.risk.lend.p2' },
    { kind: 'p', id: 'howItWorks.risk.lend.p1' },
    { kind: 'disclosure', disclosure: 'lender.residuals', open: false, anchor: 'lender-limits' },
    { kind: 'h3', id: 'howItWorks.risk.all.h', anchor: 'every-market' },
    { kind: 'p', id: 'howItWorks.risk.all.p1' },
    { kind: 'disclosure', disclosure: 'market.coveragePause', open: false },
    { kind: 'disclosure', disclosure: 'market.venueTrust', open: false },
    { kind: 'h2', id: 'howItWorks.fees.h', anchor: 'fees' },
    {
        kind: 'list',
        ids: [
            'howItWorks.fees.open',
            'howItWorks.fees.venue',
            'howItWorks.fees.rent',
            'howItWorks.fees.rate',
            'howItWorks.fees.penalty',
            'howItWorks.fees.expiry',
            'howItWorks.fees.settle',
            'howItWorks.fees.network',
            'howItWorks.fees.create',
        ],
    },
    { kind: 'h2', id: 'howItWorks.sign.h', anchor: 'before-you-sign' },
    { kind: 'p', id: 'howItWorks.sign.p1' },
    { kind: 'h2', id: 'howItWorks.more.h', anchor: 'read-more' },
    {
        kind: 'list',
        ids: [
            'howItWorks.more.security',
            'howItWorks.more.limits',
            'howItWorks.more.status',
            'howItWorks.more.glossary',
        ],
    },
];
