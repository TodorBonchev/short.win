/**
 * The market's stat strip (SPEC §8; FRONTEND.md §3.4): inventory and free tokens, utilisation, the
 * rate a new open pays now (`rateBps` with the live tier terms and the market's kink), open
 * interest, the binding size cap (`CapsSummary`, SHORT), TVL and lenders. Every value carries its
 * raw integer in `data-value`. The order is the decision's: what a new short pays and how full the
 * market is first (M10-RESP). On a phone the strip is 2-up and the priority-2 stats (inventory, open
 * interest, lenders) wait behind "More stats".
 */
import type { GlobalConfig } from '@shortwin/client';
import type { Address } from '@solana/kit';
import { type ReactNode, useId, useState } from 'react';
import { Term } from '../../../components/Glossary.tsx';
import { Sol, Tokens } from '../../../components/Num.tsx';
import { Skeleton } from '../../../components/States.tsx';
import { T } from '../../../config/testids.ts';
import { formatBps, formatSol } from '../../../lib/format.ts';
import type { MathModule } from '../../../math/load.ts';
import { TIER_CODE } from '../../../math/load.ts';
import { useMath } from '../../../math/useMath.ts';
import { CapsSummary } from '../../short/CapsSummary.tsx';
import { CompactTokens, formatUsd, UtilBar } from '../shared/cells.tsx';
import type { MarketView } from './marketView.ts';

function StatBox({
    label,
    testId,
    value,
    children,
    sub,
    priority,
}: {
    label: string;
    testId?: string;
    value?: string;
    children: ReactNode;
    sub?: ReactNode;
    priority?: 2;
}) {
    return (
        <div className="stat" data-testid={testId} data-value={value} data-priority={priority}>
            <div className="stat-label">
                <Term>{label}</Term>
            </div>
            <div className="stat-value">{children}</div>
            {sub && <span className="mk-stat-sub">{sub}</span>}
        </div>
    );
}

/** The utilisation in bps: the indexer's, else `utilizationBps(T, ΣB_Q)` from the chain. */
export function utilOf(view: MarketView, math: MathModule | undefined): number | null {
    if (view.utilBps !== null) return view.utilBps;
    if (!math) return null;
    try {
        return math.utilizationBps(view.tokensFree, view.borrowedQ);
    } catch {
        return null;
    }
}

/**
 * The APR (bps) a new open pays now: `rateBps(base, slope1, slope2, util, kink)` with the tier's
 * live terms from `GlobalConfig` and the market's own kink (else the config's).
 */
export function currentRateBps(
    view: MarketView,
    config: GlobalConfig | null,
    util: number | null,
    math: MathModule | undefined,
): number | null {
    if (!math || !config || util === null) return null;
    const terms = config.tiers[TIER_CODE[view.tier]];
    if (!terms) return null;
    const kink = view.utilKinkBps ?? config.kink.utilKinkBps;
    try {
        return math.rateBps(terms.rateBaseBps, terms.rateSlope1Bps, terms.rateSlope2Bps, util, kink);
    } catch {
        return null;
    }
}

export function StatStrip({ mint, view, config }: { mint: Address; view: MarketView; config: GlobalConfig | null }) {
    const { math } = useMath();
    const [expanded, setExpanded] = useState(false);
    const stripId = useId();
    const util = utilOf(view, math);
    const rate = currentRateBps(view, config, util, math);
    const decimals = view.decimals;
    return (
        <div className="mk-stats-wrap">
            <div id={stripId} className="stat-strip mk-stats" data-expanded={expanded ? '' : undefined}>
                <StatBox
                    label="Rate (APR)"
                    testId={T.marketStat('rate')}
                    value={rate === null ? undefined : String(rate)}
                    sub={util === null ? undefined : `for a new short at ${formatBps(util, 1)} utilisation`}
                >
                    {rate === null ? <span className="muted">—</span> : <span className="num">{formatBps(rate)}</span>}
                </StatBox>
                <StatBox
                    label="Utilisation"
                    testId={T.marketStat('utilisation')}
                    value={util === null ? undefined : String(util)}
                >
                    {util === null ? <Skeleton /> : <UtilBar bps={util} />}
                </StatBox>
                <StatBox
                    label="TVL"
                    testId={T.marketStat('tvl')}
                    value={view.tvlLamports?.toString()}
                    sub={view.tvlUsd === null ? undefined : formatUsd(view.tvlUsd)}
                >
                    {view.tvlLamports === null ? (
                        <span className="muted">—</span>
                    ) : (
                        <Sol lamports={view.tvlLamports} frac={2} />
                    )}
                </StatBox>
                <StatBox label="Size cap">
                    <CapsSummary mint={mint} />
                </StatBox>
                <StatBox
                    label="Inventory"
                    testId={T.marketStat('inventory')}
                    value={view.inventory.toString()}
                    priority={2}
                    sub={
                        <>
                            free <CompactTokens raw={view.tokensFree} decimals={decimals} /> · borrowed{' '}
                            <CompactTokens raw={view.borrowedQ} decimals={decimals} />
                        </>
                    }
                >
                    {decimals !== null ? (
                        <Tokens raw={view.inventory} decimals={decimals} frac={0} />
                    ) : (
                        <CompactTokens raw={view.inventory} decimals={null} />
                    )}
                </StatBox>
                <StatBox
                    label="Open interest"
                    testId={T.marketStat('oi')}
                    value={view.oiLamports?.toString()}
                    priority={2}
                    sub={
                        <>
                            {view.openPositions} {view.openPositions === 1 ? 'position' : 'positions'} · notional{' '}
                            <span className="num">{formatSol(view.totalNotionalOpen, 2)}</span> SOL
                        </>
                    }
                >
                    {view.oiLamports === null ? (
                        <span className="muted">—</span>
                    ) : (
                        <Sol lamports={view.oiLamports} frac={2} />
                    )}
                </StatBox>
                <StatBox
                    label="Lenders"
                    testId={T.marketStat('lenders')}
                    value={view.lenderCount === null ? undefined : String(view.lenderCount)}
                    priority={2}
                >
                    <span className="num" data-zero={view.lenderCount === 0 ? '' : undefined}>
                        {view.lenderCount ?? '—'}
                    </span>
                </StatBox>
            </div>
            <button
                type="button"
                className="btn mk-stats-more"
                data-size="sm"
                aria-expanded={expanded}
                aria-controls={stripId}
                data-testid={T.marketStatsMore}
                onClick={() => setExpanded((e) => !e)}
            >
                {expanded ? 'Fewer stats' : 'More stats (3)'}
                <span aria-hidden="true">{expanded ? '▴' : '▾'}</span>
            </button>
        </div>
    );
}
