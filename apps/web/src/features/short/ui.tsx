/** SHORT track — small presentational pieces shared by the Short panel and the position pages. */
import type { Params, TierParams } from '@shortwin/client';
import type { ReactNode } from 'react';
import { crBand, RiskBadge, riskLevel } from '../../components/Badge.tsx';
import { Disclosure } from '../../components/Disclosure.tsx';
import { Term } from '../../components/Glossary.tsx';
import { PROGRAM_ERRORS, type ProgramErrorInfo } from '../../config/errors.ts';
import { T } from '../../config/testids.ts';
import { formatBps, formatCr, formatDuration, formatFixed, formatSol } from '../../lib/format.ts';
import { type Blocked, type CapName, TIER_NAMES, type TierIndex } from './previews.ts';

export const CAP_TEXT: Readonly<Record<CapName, string>> = {
    position: 'Position impact cap',
    market: 'Market open-interest cap',
    utilisation: 'Utilisation cap',
    wallet: 'Per-wallet cap',
    gamma: 'Curve graduation guard',
    inventory: 'Free inventory',
    notional: 'Market notional cap',
};

/** A raw amount as an input accepts it back (`lib/amount.ts`): exact, no grouping. */
export function plainAmount(raw: bigint, decimals: number): string {
    return formatFixed(raw, decimals, decimals).replaceAll(',', '');
}

export function errorByName(name: string): ProgramErrorInfo | undefined {
    for (const e of PROGRAM_ERRORS.values()) if (e.name === name) return e;
    return undefined;
}

/** A collateral ratio as a percentage, coloured and labelled by its risk band (never colour alone). */
export function CrValue({ bps, liqBps, testId }: { bps: bigint | null; liqBps: number; testId?: string }) {
    if (bps === null) {
        return (
            <span className="num muted" data-testid={testId}>
                —
            </span>
        );
    }
    const risk = crBand(bps, liqBps);
    // A CR in bps can be astronomically large on a tiny buyback: cap the display.
    const shown = bps > 10_000_000n ? '>100,000%' : formatCr(bps);
    return (
        <span
            className="num cr"
            data-risk={risk}
            data-testid={testId}
            data-value={bps.toString()}
            title={`Risk: ${risk}`}
        >
            {shown}
        </span>
    );
}

/** The risk badge of an open position: both CR legs, as the program judges a liquidation (M9-UX-4). */
export function RiskCell({
    bps,
    emaBps,
    liqBps,
    testId,
}: {
    bps: bigint | null;
    emaBps: bigint | null;
    liqBps: number;
    testId?: string;
}) {
    if (bps === null) return <span className="muted">—</span>;
    const level = riskLevel(bps, emaBps, liqBps);
    return (
        <span data-testid={testId} data-risk={level}>
            <RiskBadge level={level} />
        </span>
    );
}

/**
 * Why opens are refused now (`shortBlocked`, `data-code` = the program error name) and its
 * disclosure — or, when the page's banner already shows that disclosure in full, `instead` (the
 * link to the banner, the owner's thirteenth round O-2).
 */
export function BlockedNotice({
    blocked,
    vars,
    instead,
}: {
    blocked: Blocked;
    vars?: Readonly<Record<string, string>>;
    instead?: ReactNode;
}) {
    const info = errorByName(blocked.code);
    return (
        <div className="short-blocked">
            <div
                className="banner"
                data-tone={info?.action === 'wait' ? 'warn' : 'danger'}
                role="status"
                data-testid={T.shortBlocked}
                data-code={blocked.code}
            >
                <div>
                    <strong>New shorts unavailable.</strong> {info?.message ?? blocked.code}
                    {blocked.waitS !== undefined && blocked.waitS > 0n && (
                        <span className="muted"> Ends in {formatDuration(blocked.waitS)}.</span>
                    )}
                </div>
            </div>
            {instead ?? (blocked.disclosure && <Disclosure id={blocked.disclosure} vars={vars} />)}
        </div>
    );
}

/** The terms an open locks into its position (§3.7 `tier-terms`), from the live `GlobalConfig`. */
export function TierTerms({ tier, tp, params }: { tier: TierIndex; tp: TierParams; params: Params }) {
    const rows: [string, string][] = [
        ['Opening CR (D4)', formatBps(tp.crOpenBps, 0)],
        ['Liquidation CR', formatBps(tp.crLiqBps, 0)],
        ['Liquidation penalty', formatBps(tp.penaltyBps)],
        ['Max liquidation slippage (S2)', formatBps(tp.maxLiqSlipBps)],
        [
            'Rent: base / slope 1 / slope 2',
            `${formatBps(tp.rateBaseBps)} / ${formatBps(tp.rateSlope1Bps)} / ${formatBps(tp.rateSlope2Bps)}`,
        ],
        ['Rent to the protocol', formatBps(params.rentProtocolBps)],
        ['Minimum rent', formatDuration(tp.minRentS)],
        ['Max duration', formatDuration(tp.maxDurationS)],
        ['Expiry fee', formatBps(params.expiryFeeBps)],
        ['Venue fee hard cap', formatBps(params.venueFeeHardCapBps)],
        [
            'Keeper reward (settlement)',
            `${formatSol(params.settleKeeperFixed)} SOL + ${formatBps(params.settleKeeperBps)}`,
        ],
    ];
    return (
        <details className="short-terms" data-testid={T.shortPreview('tier-terms')} data-value={TIER_NAMES[tier]}>
            <summary>Tier {TIER_NAMES[tier]} terms locked at open</summary>
            <dl className="kv">
                {rows.map(([k, v]) => (
                    <div key={k} style={{ display: 'contents' }}>
                        <dt>
                            <Term>{k}</Term>
                        </dt>
                        <dd className="num">{v}</dd>
                    </div>
                ))}
            </dl>
            <Disclosure id="market.tierAge" collapsible />
        </details>
    );
}
