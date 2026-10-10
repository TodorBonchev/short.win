/**
 * How this position can end, now (FRONTEND.md §3.8; SPEC §5–§6): the state of the exits that do
 * not need the borrower — liquidation (both CR legs below the liquidation CR), expiry (past the
 * deadline), a settlement in SOL (armed S1 / S2 evidence; S3 after 10 minutes without a route while
 * liquidatable) — in plain words, then the `borrower.liquidation` disclosure with the terms locked
 * into this position (A26) and the known limits.
 */
import type { Market, ShortPosition, TierParams } from '@shortwin/client';
import { riskLevel } from '../../../components/Badge.tsx';
import { Disclosure } from '../../../components/Disclosure.tsx';
import { percent, proseDuration } from '../../../config/disclosureVars.ts';
import { formatCr, formatDuration } from '../../../lib/format.ts';
import type { VenueState } from '../../../venues/state.ts';
import { positionDisclosureVars } from '../plans.ts';
import type { PositionMetrics } from '../previews.ts';

export type ExitState = 'healthy' | 'watch' | 'spot-below' | 'liquidatable' | 'expired' | 'no-route' | 'armed';

/**
 * The most pressing exit state of an open position now. Its CR part is `riskLevel` (both legs,
 * strictly below, as the program), so the dashboard's badge and this line never disagree (M9-UX-4).
 */
export function exitState(pos: ShortPosition, mx: PositionMetrics, venue: VenueState, now: bigint): ExitState {
    if (now >= pos.deadline) return 'expired';
    if (pos.sArmed !== 0) return 'armed';
    if (!venue.executable) return 'no-route';
    const risk = mx.crBps === null ? null : riskLevel(mx.crBps, mx.crEmaBps, pos.crLiqBps);
    if (risk === 'critical') return 'liquidatable';
    if (risk === 'below') return 'spot-below';
    if (risk === 'danger' || risk === 'watch') return 'watch';
    return 'healthy';
}

/** One line per exit state for the dashboard (the position page explains it in full). */
export function exitShort(state: ExitState, pos: Pick<ShortPosition, 's3WaitS'>): string {
    const text: Record<ExitState, string> = {
        healthy: 'Healthy',
        watch: 'Within 30% of the liquidation CR',
        'spot-below': 'Below the liquidation CR at the live price (the smoothed price is not)',
        liquidatable: 'Liquidatable now',
        expired: 'Past its deadline: closable by anyone',
        'no-route': `No route: S3 settlement in SOL if liquidatable for ${proseDuration(pos.s3WaitS)}`,
        armed: 'Settlement in SOL being considered (S1 / S2)',
    };
    return text[state];
}

/** The banner's tone follows the risk (D16): healthy reads as good news, not as a notice. */
const TONE: Record<ExitState, 'ok' | 'warn' | 'danger'> = {
    healthy: 'ok',
    watch: 'warn',
    'spot-below': 'warn',
    liquidatable: 'danger',
    expired: 'warn',
    'no-route': 'danger',
    armed: 'danger',
};

/** The position's health in one banner: its most pressing exit state, in plain words. */
export function HealthBanner({
    pos,
    metrics,
    venue,
    market,
    now,
}: {
    pos: ShortPosition;
    metrics: PositionMetrics;
    venue: VenueState;
    market: Market;
    now: bigint;
}) {
    const state = exitState(pos, metrics, venue, now);
    const liq = formatCr(pos.crLiqBps);
    const text: Record<ExitState, string> = {
        healthy: `Healthy: your CR is above the liquidation CR of ${liq}.`,
        watch: `Close to the line: your CR is within 30% of the liquidation CR of ${liq}. Adding margin raises it.`,
        'spot-below': `At the live price your CR is below ${liq}. A liquidation also needs the smoothed price below it; add margin or close now to avoid the penalty.`,
        liquidatable: `Liquidatable: your CR is below ${liq} on both the live and the smoothed price. Anyone can liquidate this position now.`,
        // The terms this position locked at open (A26), not today's configuration (M9-UX-3).
        expired: `Past its deadline: anyone can close this position now, with a ${percent(pos.expiryFeeBps)} expiry fee instead of the penalty.`,
        'no-route': `The venue cannot trade now${
            market.noRouteSince > 0n ? ` (no route for ${formatDuration(now - market.noRouteSince)})` : ''
        }. If the position is liquidatable for ${proseDuration(pos.s3WaitS)} without a route, it settles in SOL (S3).`,
        armed: `A settlement in SOL is being considered: the price has been past your vault (S1 / S2 evidence${
            pos.sSince > 0n ? ` since ${formatDuration(now - pos.sSince)} ago` : ''
        }). It settles only if the evidence persists.`,
    };
    return (
        <div className="banner position-health" data-tone={TONE[state]} role="status" data-state={state}>
            {text[state]}
        </div>
    );
}

/**
 * How the position can end: its health banner (unless `banner={false}`: the position page shows
 * it above everything), then the borrower disclosures with the terms locked into it (A26).
 */
export function SettlementRisk({
    pos,
    metrics,
    venue,
    market,
    now,
    tier,
    banner = true,
}: {
    pos: ShortPosition;
    metrics: PositionMetrics;
    venue: VenueState;
    market: Market;
    now: bigint;
    tier: TierParams | undefined;
    banner?: boolean;
}) {
    return (
        <section className="stack" aria-label="How this short can end">
            {banner && <HealthBanner pos={pos} metrics={metrics} venue={venue} market={market} now={now} />}
            <Disclosure id="borrower.liquidation" vars={positionDisclosureVars(pos, tier)} collapsible />
            <Disclosure id="borrower.residuals" vars={positionDisclosureVars(pos, tier)} collapsible />
        </section>
    );
}
