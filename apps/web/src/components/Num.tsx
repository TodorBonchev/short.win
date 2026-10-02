/**
 * Number cells (FRONTEND.md §7.2): every amount renders through one of these, so formatting,
 * tabular figures, `data-value` (the raw integer, for tests) and the screen-reader text (the plain
 * value with its unit; the visual subscript notation and arrows are hidden from it) are uniform.
 */
import { formatBps, formatPnlSol, formatPriceSol, formatSol, formatTokens, MINUS } from '../lib/format.ts';

interface Common {
    readonly testId?: string;
    readonly className?: string;
}

export function Sol({
    lamports,
    frac = 4,
    unit = true,
    testId,
    className,
}: Common & { lamports: bigint; frac?: number; unit?: boolean }) {
    const text = formatSol(lamports, frac);
    return (
        <span className={`num ${className ?? ''}`.trim()} data-testid={testId} data-value={lamports.toString()}>
            {text}
            {unit ? <span className="unit">SOL</span> : <span className="visually-hidden"> SOL</span>}
        </span>
    );
}

export function Tokens({
    raw,
    decimals,
    symbol,
    frac,
    testId,
    className,
}: Common & { raw: bigint; decimals: number; symbol?: string | null; frac?: number }) {
    const text = formatTokens(raw, decimals, frac);
    return (
        <span className={`num ${className ?? ''}`.trim()} data-testid={testId} data-value={raw.toString()}>
            {text}
            {symbol && (
                <bdi className="unit" title={symbol}>
                    {symbol}
                </bdi>
            )}
        </span>
    );
}

export function Bps({ value, frac = 2, testId, className }: Common & { value: number | bigint; frac?: number }) {
    return (
        <span className={`num ${className ?? ''}`.trim()} data-testid={testId} data-value={String(value)}>
            {formatBps(value, frac)}
        </span>
    );
}

/**
 * A WAD price shown as SOL per whole token, with subscript-zero notation for tiny prices and a
 * visible unit (M9-UX-8: traders used to USD prices and market caps must not read a bare number).
 */
export function Price({
    wad,
    decimals,
    testId,
    className,
    unit = true,
}: Common & { wad: bigint; decimals: number; unit?: boolean }) {
    const { display, plain } = formatPriceSol(wad, decimals);
    return (
        <span
            className={`num ${className ?? ''}`.trim()}
            data-testid={testId}
            data-value={wad.toString()}
            title={`${plain} SOL per token`}
        >
            <span aria-hidden="true">{display}</span>
            {unit && (
                <span className="unit" aria-hidden="true">
                    SOL
                </span>
            )}
            <span className="visually-hidden">{plain} SOL per token</span>
        </span>
    );
}

/** Signed P&L in SOL: sign, arrow and colour (gain / loss), never colour alone. */
export function Pnl({ lamports, frac = 4, testId, className }: Common & { lamports: bigint; frac?: number }) {
    const sign = lamports > 0n ? 'up' : lamports < 0n ? 'down' : 'flat';
    const text = formatPnlSol(lamports, frac);
    const arrow = sign === 'up' ? '▲ ' : sign === 'down' ? '▼ ' : '';
    const word = sign === 'down' ? 'loss' : sign === 'up' ? 'gain' : 'no change';
    return (
        <span
            className={`num pnl ${className ?? ''}`.trim()}
            data-sign={sign}
            data-testid={testId}
            data-value={lamports.toString()}
        >
            <span aria-hidden="true">
                {arrow}
                {text}
            </span>
            <span className="visually-hidden">
                {word} {text.replace(MINUS, '').replace('+', '')}
            </span>
            <span className="unit">SOL</span>
        </span>
    );
}
