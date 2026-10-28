export const U64_MAX = 0xffffffffffffffffn;
/** Parse decimal token/SOL input without passing through binary floating point. */
export function rawAmount(text: string, decimals: number, max = U64_MAX): bigint {
    if (!Number.isInteger(decimals) || decimals < 0 || decimals > 255)
        throw new Error('Invalid token decimals');
    const value = text.trim();
    if (!/^(0|[1-9][0-9]*)(\.[0-9]+)?$/.test(value))
        throw new Error('Enter a non-negative decimal amount');
    const [whole, fraction = ''] = value.split('.');
    if (fraction.length > decimals)
        throw new Error(`At most ${decimals} decimal places are allowed`);
    const result = BigInt(whole!) * 10n ** BigInt(decimals) + BigInt((fraction + '0'.repeat(decimals)).slice(0, decimals) || '0');
    if (result > max)
        throw new Error('Amount is too large');
    return result;
}
export function formatAmount(value: bigint, decimals: number): string {
    const scale = 10n ** BigInt(decimals);
    const integer = value / scale;
    const fraction = (value % scale).toString().padStart(decimals, '0').replace(/0+$/, '');
    return `${integer}${fraction ? '.' + fraction : ''}`;
}
export function index(limbs: readonly bigint[]): bigint { if (limbs.length !== 4)
    throw new Error('Bad income index'); return limbs.reduce((n, l, i) => n + (l << BigInt(64 * i)), 0n); }
export function claimable(pool: {
    solIndex: readonly bigint[];
    solReserve: bigint;
}, receipt: {
    shares: bigint;
    checkpoint: readonly bigint[];
    fraction: bigint;
    pendingSol: bigint;
}): bigint {
    const delta = index(pool.solIndex) - index(receipt.checkpoint);
    if (delta < 0n)
        throw new Error('Inconsistent receipt checkpoint');
    const amount = receipt.pendingSol + (receipt.shares * delta + receipt.fraction) / 1000000000000000000n;
    if (amount > pool.solReserve)
        throw new Error('Inconsistent income reserve');
    return amount;
}
export function depositShares(tokens: bigint, pool: {
    cash: bigint;
    principalOut: bigint;
    shares: bigint;
}): bigint {
    if (pool.shares === 0n)
        return tokens;
    const assets = pool.cash + pool.principalOut;
    if (assets === 0n)
        throw new Error('Wiped pool needs a new generation');
    return tokens * pool.shares / assets;
}
export function redemption(shares: bigint, pool: {
    cash: bigint;
    principalOut: bigint;
    shares: bigint;
}): bigint {
    if (pool.shares === 0n || shares > pool.shares)
        throw new Error('Insufficient pool shares');
    return shares * (pool.cash + pool.principalOut) / pool.shares;
}
