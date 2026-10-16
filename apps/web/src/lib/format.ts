/**
 * Number formatting for the terminal UI (FRONTEND.md §7.2). Every on-chain amount is a `bigint`
 * in its raw unit (lamports, raw token units, WAD prices) and is formatted here only — no `Number`
 * conversion of an amount, so nothing is rounded twice. Numbers render in tabular figures
 * (`.num` in styles/base.css); the minus sign is U+2212.
 */

export const LAMPORTS_PER_SOL = 1_000_000_000n;
export const WAD = 10n ** 18n;
export const MINUS = '−';

const SUBSCRIPT = ['₀', '₁', '₂', '₃', '₄', '₅', '₆', '₇', '₈', '₉'];

function groupInt(s: string): string {
    return s.replace(/\B(?=(\d{3})+(?!\d))/g, ',');
}

function pow10(n: number): bigint {
    return 10n ** BigInt(n);
}

/**
 * `value / 10^scale` as a decimal string with at most `maxFrac` fraction digits (truncated toward
 * zero, never rounded up: a balance is never shown larger than it is), trailing zeros trimmed down
 * to `minFrac`, integer part grouped with commas.
 */
export function formatFixed(value: bigint, scale: number, maxFrac: number, minFrac = 0): string {
    const neg = value < 0n;
    const abs = neg ? -value : value;
    const unit = pow10(scale);
    const int = abs / unit;
    let frac = (abs % unit).toString().padStart(scale, '0').slice(0, Math.max(0, maxFrac));
    while (frac.length > minFrac && frac.endsWith('0')) frac = frac.slice(0, -1);
    const body = frac.length ? `${groupInt(int.toString())}.${frac}` : groupInt(int.toString());
    return neg && (int > 0n || /[1-9]/.test(frac)) ? `${MINUS}${body}` : body;
}

/** Lamports as SOL ("1.2345"); 4 fraction digits by default. */
export function formatSol(lamports: bigint, maxFrac = 4, minFrac = 0): string {
    return formatFixed(lamports, 9, maxFrac, minFrac);
}

/** Raw token units as UI tokens; `pad` keeps `maxFrac` fraction digits (a column's decimal points align). */
export function formatTokens(raw: bigint, decimals: number, maxFrac = Math.min(decimals, 4), pad = false): string {
    return formatFixed(raw, decimals, maxFrac, pad ? maxFrac : 0);
}

/** Basis points as a percentage ("12.34%"). */
export function formatBps(bps: number | bigint, frac = 2): string {
    const v = typeof bps === 'bigint' ? bps : BigInt(Math.trunc(bps));
    return `${formatFixed(v, 2, frac)}%`;
}

/** A collateral ratio in bps as a percentage without fraction ("187%"). */
export function formatCr(bps: bigint | number): string {
    return formatBps(bps, 0);
}

/** A signed P&L in lamports: always signed ("+1.20", "−0.45", "0"). */
export function formatPnlSol(lamports: bigint, maxFrac = 4): string {
    if (lamports === 0n) return '0';
    const body = formatSol(lamports < 0n ? -lamports : lamports, maxFrac);
    return lamports < 0n ? `${MINUS}${body}` : `+${body}`;
}

/**
 * A small positive decimal `num / den` with `sig` significant digits; runs of ≥ 4 leading fraction
 * zeros collapse to subscript notation ("0.0₅1234" = 0.000001234), the convention of memecoin
 * terminals. Returns the display string and the plain string (for `aria-label` / `title`).
 */
export function formatSignificant(num: bigint, den: bigint, sig = 4): { display: string; plain: string } {
    if (den <= 0n) throw new RangeError('den must be positive');
    if (num < 0n) {
        const r = formatSignificant(-num, den, sig);
        return { display: `${MINUS}${r.display}`, plain: `-${r.plain}` };
    }
    if (num === 0n) return { display: '0', plain: '0' };
    const int = num / den;
    if (int >= 1n) {
        const intStr = int.toString();
        const fracDigits = Math.max(0, sig - intStr.length);
        const plain = formatFixedRatio(num, den, fracDigits);
        return { display: plain, plain };
    }
    // Count leading zeros of the fraction.
    let zeros = 0;
    let r = num;
    while (r * 10n < den) {
        r *= 10n;
        zeros++;
        if (zeros > 60) break;
    }
    const digits = ((r * pow10(sig)) / den).toString().slice(0, sig).replace(/0+$/, '') || '0';
    const plain = `0.${'0'.repeat(zeros)}${digits}`;
    const display =
        zeros >= 4
            ? `0.0${zeros
                  .toString()
                  .split('')
                  .map((c) => SUBSCRIPT[Number(c)])
                  .join('')}${digits}`
            : plain;
    return { display, plain };
}

function formatFixedRatio(num: bigint, den: bigint, frac: number): string {
    const scaled = (num * pow10(frac)) / den;
    return formatFixed(scaled, frac, frac);
}

/**
 * A WAD price (lamports per raw token unit × 10¹⁸, ARCH §2.2) as SOL per whole token.
 * `decimals` is the mint's.
 */
export function formatPriceSol(priceWad: bigint, decimals: number, sig = 4): { display: string; plain: string } {
    // SOL/token = priceWad / WAD × 10^decimals / 10^9
    const num = priceWad * pow10(decimals);
    const den = WAD * LAMPORTS_PER_SOL;
    return formatSignificant(num, den, sig);
}

/** "5yvy…pKK" */
export function shortAddress(a: string, head = 4, tail = 4): string {
    return a.length <= head + tail + 1 ? a : `${a.slice(0, head)}…${a.slice(-tail)}`;
}

/** A duration in seconds as "2d 4h", "3h 12m", "4m 05s", "45s"; negative → "0s". */
export function formatDuration(seconds: number | bigint): string {
    let s = typeof seconds === 'bigint' ? Number(seconds) : Math.floor(seconds);
    if (!Number.isFinite(s) || s <= 0) return '0s';
    const d = Math.floor(s / 86_400);
    s -= d * 86_400;
    const h = Math.floor(s / 3_600);
    s -= h * 3_600;
    const m = Math.floor(s / 60);
    s -= m * 60;
    if (d > 0) return `${d}d ${h}h`;
    if (h > 0) return `${h}h ${String(m).padStart(2, '0')}m`;
    if (m > 0) return `${m}m ${String(s).padStart(2, '0')}s`;
    return `${s}s`;
}

/** A slot number grouped with commas ("53,422"); "—" for none. */
export function formatSlot(n: bigint | number | null): string {
    return n === null ? '—' : n.toLocaleString('en-US');
}

/**
 * An age (how long ago) for a dense table: "45s" under a minute, then minutes only ("11m"), then
 * "3h 12m", then "2d 4h" — a list that refreshes every 15–30 s never churns seconds past the first
 * minute. Negative or not finite → "0s". `formatDuration` keeps the seconds for countdowns.
 */
export function formatAge(seconds: number | bigint): string {
    let s = typeof seconds === 'bigint' ? Number(seconds) : Math.floor(seconds);
    if (!Number.isFinite(s) || s <= 0) return '0s';
    if (s < 60) return `${s}s`;
    const d = Math.floor(s / 86_400);
    s -= d * 86_400;
    const h = Math.floor(s / 3_600);
    s -= h * 3_600;
    const m = Math.floor(s / 60);
    if (d > 0) return `${d}d ${h}h`;
    if (h > 0) return `${h}h ${String(m).padStart(2, '0')}m`;
    return `${m}m`;
}

/** Compact magnitude for dense tables ("1.23K", "4.5M", "12B"); `value` is in units of 10^-scale. */
export function formatCompact(value: bigint, scale: number): string {
    const neg = value < 0n;
    const abs = neg ? -value : value;
    let s = formatFixed(abs, scale, 2);
    for (const [e, suffix] of [
        [12, 'T'],
        [9, 'B'],
        [6, 'M'],
        [3, 'K'],
    ] as const) {
        if (abs >= pow10(e + scale)) {
            s = `${formatFixed(abs, e + scale, 2)}${suffix}`;
            break;
        }
    }
    return neg ? `${MINUS}${s}` : s;
}
