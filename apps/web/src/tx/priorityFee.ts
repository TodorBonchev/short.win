/**
 * Priority fee (FRONTEND.md §5.6): the 75th percentile of `getRecentPrioritizationFees` over the
 * transaction's writable accounts, within [1,000, VITE_PRIORITY_FEE_MAX_MICROLAMPORTS] µlamports
 * per CU (localnet: 0). Shown in the review; the user can pick Normal (p75) or Fast (p95).
 */
import type { Address } from '@solana/kit';
import type { AppRpc } from '../rpc/client.ts';

export type FeeSpeed = 'normal' | 'fast';

export function percentile(values: readonly bigint[], p: number): bigint {
    if (values.length === 0) return 0n;
    const sorted = [...values].sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));
    const idx = Math.min(sorted.length - 1, Math.max(0, Math.ceil((p / 100) * sorted.length) - 1));
    return sorted[idx] ?? 0n;
}

export function clampFee(v: bigint, min: bigint, max: bigint): bigint {
    return v < min ? min : v > max ? max : v;
}

export async function estimatePriorityFee(
    rpc: AppRpc,
    writable: readonly Address[],
    speed: FeeSpeed,
    max: bigint,
    localnet: boolean,
): Promise<bigint> {
    if (localnet) return 0n;
    try {
        const recent = await rpc.getRecentPrioritizationFees(writable.slice(0, 128)).send();
        const fees = recent.map((r) => BigInt(r.prioritizationFee)).filter((f) => f > 0n);
        return clampFee(percentile(fees, speed === 'fast' ? 95 : 75), 1_000n, max);
    } catch {
        return clampFee(10_000n, 1_000n, max);
    }
}
