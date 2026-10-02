/**
 * The market page's header (FRONTEND.md §3.4): symbol and name (untrusted text), the mint with a
 * copy button (the token's identity, FE-D8), the tier badge with "Why this tier", the venue badge,
 * the state flags, and the price (EMA and spot).
 */
import type { GlobalConfig } from '@shortwin/client';
import { AddressText } from '../../../components/Address.tsx';
import { Badge, VenueBadge } from '../../../components/Badge.tsx';
import { Price } from '../../../components/Num.tsx';
import { T } from '../../../config/testids.ts';
import { shortAddress } from '../../../lib/format.ts';
import { nowSeconds } from '../../../lib/time.ts';
import { FreshnessBadge } from '../../../shell/freshness.tsx';
import { FlagBadge } from '../shared/cells.tsx';
import { useDuplicateSymbol } from '../shared/duplicates.ts';
import type { MarketFlagKey } from '../shared/marketFlags.ts';
import type { MarketView } from './marketView.ts';
import { TierWhyPopover } from './TierWhyPopover.tsx';
import type { TierWhyInput } from './tierWhy.ts';

export interface MarketHeaderProps {
    readonly view: MarketView;
    readonly flags: readonly MarketFlagKey[];
    readonly config: GlobalConfig | null;
    readonly oracle: TierWhyInput['oracle'];
}

export function MarketHeader({ view, flags, config, oracle }: MarketHeaderProps) {
    const tierInput: TierWhyInput = {
        venue: view.venue,
        tier: view.tier,
        now: nowSeconds(),
        graduatedAt: view.graduatedAt,
        createdTs: view.createdTs,
        thetaAUsdE6: config?.params.thetaAUsdE6 ?? null,
        thetaBUsdE6: config?.params.thetaBUsdE6 ?? null,
        oracle,
        oracleStaleS: config?.params.oracleStaleS ?? null,
    };
    const decimals = view.decimals;
    const duplicate = useDuplicateSymbol(view.symbol, view.mint);
    return (
        <div className="page-head mk-head" data-testid={T.marketHeader}>
            <div className="mk-head-title">
                <h1 data-testid={T.marketSymbol}>
                    <bdi>{view.symbol ?? shortAddress(view.mint)}</bdi>
                </h1>
                {view.name && (
                    <bdi className="mk-head-name" title={view.name}>
                        {view.name}
                    </bdi>
                )}
                {duplicate && (
                    <span
                        className="badge"
                        data-tone="warn"
                        data-testid={T.marketDuplicateSymbol}
                        title="Another listed market uses this symbol: the token is the mint, not the symbol."
                    >
                        Same symbol as another market · check the mint
                    </span>
                )}
            </div>
            <span className="mk-head-mint">
                <AddressText address={view.mint} testId={T.marketMint} />
            </span>
            <TierWhyPopover input={tierInput} />
            <VenueBadge venue={view.venue} testId={T.marketVenue} />
            {flags.map((f) => (
                <FlagBadge key={f} flag={f} testId={T.marketFlag(f)} />
            ))}
            <span data-testid={T.marketSource} data-value={view.source}>
                {view.source === 'chain' ? (
                    <Badge tone="warn" title="The indexer is unavailable or does not know this market yet">
                        On-chain view
                    </Badge>
                ) : (
                    <FreshnessBadge />
                )}
            </span>
            <div className="mk-head-prices">
                <span>
                    <span className="mk-head-price-label">Price (EMA)</span>
                    {view.emaPriceWad !== null && decimals !== null ? (
                        <span data-testid={T.marketStat('price')} data-value={view.emaPriceWad.toString()}>
                            <Price wad={view.emaPriceWad} decimals={decimals} />
                        </span>
                    ) : (
                        <span className="muted" data-testid={T.marketStat('price')}>
                            —
                        </span>
                    )}
                </span>
                <span>
                    <span className="mk-head-price-label">Spot</span>
                    {view.spotWad !== null && decimals !== null ? (
                        <span data-testid={T.marketSpotPrice} data-value={view.spotWad.toString()}>
                            <Price wad={view.spotWad} decimals={decimals} />
                        </span>
                    ) : (
                        <span className="muted" data-testid={T.marketSpotPrice}>
                            —
                        </span>
                    )}
                </span>
            </div>
        </div>
    );
}
