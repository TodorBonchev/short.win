/**
 * Small display cells shared by the MARKETS pages: compact token amounts, ages, a utilisation bar,
 * flag badges, a token label that keeps duplicate tickers apart (R-MINT §10.11) and a signature
 * link. Symbols and names are untrusted indexer text: rendered as text only (§9.5).
 */
import type { ReactNode } from 'react';
import { Badge } from '../../../components/Badge.tsx';
import { Term } from '../../../components/Glossary.tsx';
import { explorerTx } from '../../../config/env.ts';
import { glossaryFor } from '../../../config/glossary.ts';
import { formatBps, formatCompact, formatDuration, formatFixed, shortAddress } from '../../../lib/format.ts';
import { formatUtc } from '../../../lib/time.ts';
import { FLAG_META, type MarketFlagKey } from './marketFlags.ts';

/** Raw token units in compact form ("1.23B"); the exact amount in `title`. */
export function CompactTokens({ raw, decimals, testId }: { raw: bigint; decimals: number | null; testId?: string }) {
    if (decimals === null) {
        return (
            <span className="num" data-testid={testId} data-value={raw.toString()} title="raw units (decimals unknown)">
                {formatCompact(raw, 0)}
            </span>
        );
    }
    return (
        <span
            className="num"
            data-testid={testId}
            data-value={raw.toString()}
            title={formatFixed(raw, decimals, decimals)}
        >
            {formatCompact(raw, decimals)}
        </span>
    );
}

/** Time since `ts` (unix seconds), "3d 4h"; the UTC time in `title`. */
export function Age({ ts, now, testId }: { ts: number | null; now: number; testId?: string }) {
    if (ts === null || ts <= 0) return <span className="muted">—</span>;
    return (
        <span className="num" title={formatUtc(ts)} data-testid={testId} data-value={String(ts)}>
            {formatDuration(Math.max(0, now - ts))}
        </span>
    );
}

/** Utilisation as a bar and a percentage (the bar is decorative; the text carries the value). */
export function UtilBar({ bps, testId }: { bps: number | null; testId?: string }) {
    if (bps === null) return <span className="muted">—</span>;
    const pct = Math.min(100, Math.max(0, bps / 100));
    const tone = bps >= 9_000 ? 'danger' : bps >= 7_500 ? 'warn' : 'ok';
    return (
        <span className="mk-util" data-testid={testId} data-value={String(bps)}>
            <span className="mk-util-track" aria-hidden="true">
                <span className="mk-util-fill" data-tone={tone} style={{ width: `${pct}%` }} />
            </span>
            <span className="num">{formatBps(bps, 1)}</span>
        </span>
    );
}

/** A state flag's badge, with its glossary ⓘ (the flag's approved banner text; FRONTEND.md §4.3). */
export function FlagBadge({ flag, testId }: { flag: MarketFlagKey; testId?: string }) {
    const meta = FLAG_META[flag];
    return (
        <Term id={glossaryFor(meta.label)}>
            <Badge tone={meta.tone} testId={testId}>
                {meta.label}
            </Badge>
        </Term>
    );
}

/**
 * A token's label: the symbol and the short mint (or the short mint alone when the symbol is
 * unknown) and, when another listed market shares the symbol, the mint's suffix next to it so two
 * "BONK" rows are visibly different.
 */
export function TokenLabel({
    mint,
    symbol,
    name,
    duplicate,
}: {
    mint: string;
    symbol: string | null;
    name?: string | null;
    duplicate?: boolean;
}) {
    return (
        <span className="mk-token" title={`${name ? `${name} · ` : ''}${mint}`}>
            {symbol ? (
                <bdi className="mk-token-symbol">{symbol}</bdi>
            ) : (
                <span className="mk-token-symbol num">{shortAddress(mint)}</span>
            )}
            {duplicate && symbol && (
                <span className="badge mk-dup" data-tone="warn" title="Another market uses this symbol">
                    {mint.slice(-4)}
                </span>
            )}
            {/* Without a symbol the short mint already is the label; it is not repeated. */}
            {symbol && <span className="muted num mk-token-mint">{shortAddress(mint)}</span>}
        </span>
    );
}

/** A transaction signature linking to the explorer. */
export function SignatureLink({ signature, children }: { signature: string; children?: ReactNode }) {
    return (
        <a className="num" href={explorerTx(signature)} target="_blank" rel="noopener noreferrer" title={signature}>
            {children ?? shortAddress(signature, 6, 4)}
        </a>
    );
}

/** A USD amount for display ("$12,345"); USD values are informational only. */
export function formatUsd(v: number | null): string {
    if (v === null || !Number.isFinite(v)) return '—';
    return `$${v.toLocaleString('en-US', { maximumFractionDigits: v >= 100 ? 0 : 2 })}`;
}
