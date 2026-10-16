import type { ReactNode } from 'react';
import type { TierName, VenueName } from '../api/types.ts';

export type Tone = 'neutral' | 'ok' | 'info' | 'warn' | 'danger';
export type RiskLevel = 'safe' | 'watch' | 'danger' | 'below' | 'critical';

export function Badge({
    tone = 'neutral',
    children,
    testId,
    title,
    className,
}: {
    tone?: Tone;
    children: ReactNode;
    testId?: string;
    title?: string;
    /** Merged after `badge` (e.g. `badge-status`: a single-word enum shown capitalised). */
    className?: string;
}) {
    return (
        <span className={`badge ${className ?? ''}`.trim()} data-tone={tone} data-testid={testId} title={title}>
            {children}
        </span>
    );
}

export function TierBadge({ tier, testId }: { tier: TierName; testId?: string }) {
    return (
        <span className="badge tier" data-tier={tier} data-testid={testId} title={`Risk tier ${tier}`}>
            <span className="visually-hidden">Tier </span>
            {tier}
        </span>
    );
}

const RISK_TEXT: Record<RiskLevel, string> = {
    safe: 'Safe',
    watch: 'Watch',
    danger: 'At risk',
    below: 'At risk (live price below)',
    critical: 'Liquidatable',
};

/**
 * Position health from its CR on both legs against the liquidation CR locked into it (FRONTEND.md
 * §7.1), as the program judges it (`shortwin-math risk::cr_below`: strictly below, on the live AND
 * the smoothed leg; M9-UX-4): `critical` (Liquidatable) only when both legs are below; `below` when
 * only the live leg is; then, on the live leg, `danger` within 10 % above the line, `watch` within
 * 30 %, `safe` beyond. A missing smoothed leg (no EMA quote) is not below.
 */
export function riskLevel(crBps: bigint, crEmaBps: bigint | null, liqCrBps: number): RiskLevel {
    const liq = BigInt(liqCrBps);
    const spotBelow = crBps < liq;
    const emaBelow = crEmaBps !== null && crEmaBps < liq;
    if (spotBelow && emaBelow) return 'critical';
    if (spotBelow) return 'below';
    if (crBps * 100n < liq * 110n) return 'danger';
    if (crBps * 100n < liq * 130n) return 'watch';
    return 'safe';
}

/** The band of one CR figure (a single leg, e.g. a preview's CR at open): never "Liquidatable". */
export function crBand(crBps: bigint, liqCrBps: number): Exclude<RiskLevel, 'critical'> {
    const level = riskLevel(crBps, null, liqCrBps);
    return level === 'critical' ? 'below' : level;
}

export function RiskBadge({ level, testId }: { level: RiskLevel; testId?: string }) {
    return (
        <span className="badge" data-risk={level} data-testid={testId}>
            {RISK_TEXT[level]}
        </span>
    );
}

const VENUE_TEXT: Record<VenueName, string> = {
    none: 'Lend only',
    pump_curve: 'Pump curve',
    pump_migrating: 'Migrating',
    pump_swap: 'PumpSwap',
    raydium_v4: 'Raydium v4',
    raydium_cpmm: 'Raydium CPMM',
    mock: 'Mock (test)',
    unknown: 'Unknown',
};

export function venueLabel(v: VenueName): string {
    return VENUE_TEXT[v];
}

export function VenueBadge({ venue, testId }: { venue: VenueName; testId?: string }) {
    const tone: Tone =
        venue === 'pump_migrating' ? 'warn' : venue === 'none' || venue === 'unknown' ? 'neutral' : 'info';
    return (
        <Badge tone={tone} testId={testId}>
            {VENUE_TEXT[venue]}
        </Badge>
    );
}
