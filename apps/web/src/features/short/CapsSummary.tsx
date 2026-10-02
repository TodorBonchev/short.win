/**
 * SHORT track — the size cap of the market's next open (FRONTEND.md §3.4 stat strip, §6.7): the
 * largest `Q` the program would accept now — the per-position impact cap (and its fee-scaled
 * cap), the per-market OI cap, the utilisation cap, the Tier D curve guard and the free inventory,
 * at the tier a new open gets — with the binding cap named. The per-wallet cap applies to the
 * connected wallet's own exposure (none when disconnected). Rendered by the market page (MARKETS)
 * inside its `market-stat-caps` stat; `data-value` is the raw maximum.
 */
import type { Address } from '@solana/kit';
import { useMemo } from 'react';
import { Skeleton } from '../../components/States.tsx';
import { T } from '../../config/testids.ts';
import { formatCompact, formatTokens } from '../../lib/format.ts';
import { useWallet } from '../../wallet/useWallet.ts';
import { useExposureQ, useShortSnapshot, useTokenLabel } from './hooks.ts';
import { assignTier, capsFor, openBlocked } from './previews.ts';
import { CAP_TEXT } from './ui.tsx';

export function CapsSummary({ mint }: { mint: Address }) {
    const { connected } = useWallet();
    const snap = useShortSnapshot(mint);
    const exposure = useExposureQ(mint, connected?.address ?? null);
    const { symbol } = useTokenLabel(mint);
    const s = snap.data;
    const math = snap.math;
    const walletQ = exposure.data ?? 0n;
    const view = useMemo(() => {
        if (!s || !math) return null;
        const tier = assignTier(math, s);
        const tp = tier === null ? undefined : s.config.tiers[tier];
        const caps = tp ? capsFor(math, s, tp, walletQ) : null;
        return { caps, blocked: openBlocked(math, s) };
    }, [s, math, walletQ]);

    if (!view) {
        return (
            <span data-testid={T.marketStat('caps')} data-mint={mint}>
                {snap.data === null ? '—' : <Skeleton width={80} />}
            </span>
        );
    }
    const { caps, blocked } = view;
    if (!caps) {
        return (
            <span className="muted" data-testid={T.marketStat('caps')} data-mint={mint} title="No opens on this venue">
                —
            </span>
        );
    }
    return (
        <span
            data-testid={T.marketStat('caps')}
            data-mint={mint}
            data-value={caps.maxQ.toString()}
            data-cap={caps.binding}
            title={`${formatTokens(caps.maxQ, s?.decimals ?? 0)} ${symbol} · ${
                blocked ? `Opens refused now: ${blocked.code}` : `Binding: ${CAP_TEXT[caps.binding]}`
            }`}
        >
            {/* Compact in the stat strip (the exact maximum is in the title and the panel's preview). */}
            <span className="num">
                {formatCompact(caps.maxQ, s?.decimals ?? 0)}
                <span className="unit">{symbol}</span>
            </span>
            <span className="short-preview-sub">{CAP_TEXT[caps.binding]}</span>
        </span>
    );
}
