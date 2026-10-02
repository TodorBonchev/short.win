/**
 * User amount input → raw `bigint` (FRONTEND.md §9.5, input validation). Strict: digits with one
 * optional dot, at most `decimals` fraction digits, no sign, no exponent, no grouping, within u64.
 */
export const U64_MAX = (1n << 64n) - 1n;

export type ParsedAmount =
    | { readonly ok: true; readonly raw: bigint }
    | { readonly ok: false; readonly error: 'empty' | 'format' | 'decimals' | 'zero' | 'range' };

export function parseAmount(input: string, decimals: number, opts: { allowZero?: boolean } = {}): ParsedAmount {
    const s = input.trim();
    if (s === '') return { ok: false, error: 'empty' };
    if (!/^(\d+\.?\d*|\.\d+)$/.test(s)) return { ok: false, error: 'format' };
    const [intPart = '', fracPart = ''] = s.split('.');
    if (fracPart.length > decimals) return { ok: false, error: 'decimals' };
    const raw = BigInt((intPart || '0') + fracPart.padEnd(decimals, '0'));
    if (raw > U64_MAX) return { ok: false, error: 'range' };
    if (raw === 0n && !opts.allowZero) return { ok: false, error: 'zero' };
    return { ok: true, raw };
}

export const AMOUNT_ERROR_TEXT: Readonly<Record<Exclude<ParsedAmount, { ok: true }>['error'], string>> = {
    empty: 'Enter an amount.',
    format: 'Use digits and at most one decimal point.',
    decimals: 'Too many decimal places for this token.',
    zero: 'Enter an amount above zero.',
    range: 'Amount is too large.',
};

/** Basis points from a percent string ("0.5" → 50); at most 2 fraction digits, 0 < bps ≤ max. */
export function parseBps(input: string, maxBps: number): number | null {
    const r = parseAmount(input, 2);
    if (!r.ok) return null;
    const bps = Number(r.raw);
    return bps > 0 && bps <= maxBps ? bps : null;
}
