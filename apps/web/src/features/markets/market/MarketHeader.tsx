/**
 * The market page's header (FRONTEND.md §3.4): symbol and name (untrusted text), the mint with a
 * copy button (the token's identity, FE-D8), the tier badge with "Why this tier", the venue badge,
 * the state flags, and the price (EMA and spot) straight under the title at the large size
 * (M10-RESP D9). One DOM at every width: the title line wraps on a phone. A flag that says what the
 * venue badge already says ("Lend only", "Migrating") is not repeated: the venue badge carries
 * that flag's test id.
 */
import type { GlobalConfig } from '@shortwin/client';
import { useState } from 'react';
import { Badge, VenueBadge, venueLabel } from '../../../components/Badge.tsx';
import { Price } from '../../../components/Num.tsx';
import { Reveal } from '../../../components/Reveal.tsx';
import { explorerAddress } from '../../../config/env.ts';
import { T } from '../../../config/testids.ts';
import { shortAddress } from '../../../lib/format.ts';
import { nowSeconds } from '../../../lib/time.ts';
import { FreshnessBadge } from '../../../shell/freshness.tsx';
import { FlagBadge } from '../shared/cells.tsx';
import { useDuplicateSymbol } from '../shared/duplicates.ts';
import { FLAG_META, type MarketFlagKey } from '../shared/marketFlags.ts';
import type { MarketView } from './marketView.ts';
import { TierWhyPopover } from './TierWhyPopover.tsx';
import type { TierWhyInput } from './tierWhy.ts';

export interface MarketHeaderProps {
    readonly view: MarketView;
    readonly flags: readonly MarketFlagKey[];
    readonly config: GlobalConfig | null;
    readonly oracle: TierWhyInput['oracle'];
}

/** The mint: the short form opens the full mint with Copy and Explorer; ⧉ copies it at once. */
function MintText({ mint }: { mint: string }) {
    const [copied, setCopied] = useState(false);
    return (
        <span className="address-text mk-head-mint">
            <Reveal value={mint} kind="mint" label="Mint" explorer={explorerAddress(mint)}>
                <span className="num" data-testid={T.marketMint} data-value={mint}>
                    {shortAddress(mint)}
                </span>
            </Reveal>
            <button
                type="button"
                className="btn hit"
                data-variant="ghost"
                data-size="sm"
                aria-label={copied ? 'Copied' : `Copy ${mint}`}
                onClick={() => {
                    void navigator.clipboard?.writeText(mint).then(() => {
                        setCopied(true);
                        setTimeout(() => setCopied(false), 1_200);
                    });
                }}
            >
                {copied ? '✓' : '⧉'}
            </button>
        </span>
    );
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
    // A flag whose text is the venue badge's ("Lend only", "Migrating"): one badge, both ids.
    const venueFlag = flags.find((f) => FLAG_META[f].label === venueLabel(view.venue));
    const venueBadge = <VenueBadge venue={view.venue} testId={T.marketVenue} />;
    return (
        <div className="page-head mk-head" data-testid={T.marketHeader}>
            <div className="mk-head-main">
                <div className="mk-head-title">
                    <h1 data-testid={T.marketSymbol}>
                        <bdi>{view.symbol ?? shortAddress(view.mint)}</bdi>
                    </h1>
                    <span className="mk-head-badges">
                        <TierWhyPopover input={tierInput} />
                        {venueFlag ? (
                            <span className="mk-flag-venue" data-testid={T.marketFlag(venueFlag)}>
                                {venueBadge}
                            </span>
                        ) : (
                            venueBadge
                        )}
                        {flags
                            .filter((f) => f !== venueFlag)
                            .map((f) => (
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
                    </span>
                </div>
                <div className="mk-head-sub">
                    {view.name && (
                        <bdi className="mk-head-name" title={view.name}>
                            {view.name}
                        </bdi>
                    )}
                    <MintText mint={view.mint} />
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
            </div>
            <div className="mk-head-prices">
                <span className="mk-head-price">
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
                <span className="mk-head-price" data-kind="spot">
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
