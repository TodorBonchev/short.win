/**
 * Before / after deltas of the watched accounts (FRONTEND.md §5.3): pre-state from
 * `getMultipleAccounts`, post-state from the simulation's `accounts`. Pure.
 */
import { tokenAmount } from './tokenAccount.ts';
import type { Delta, WatchedAccount } from './types.ts';

export interface AccountSnapshot {
    readonly lamports: bigint;
    readonly data: Uint8Array;
}

export function computeDeltas(
    watch: readonly WatchedAccount[],
    pre: readonly (AccountSnapshot | null)[],
    post: readonly (AccountSnapshot | null)[],
): Delta[] {
    const out: Delta[] = [];
    watch.forEach((w, i) => {
        const a = pre[i] ?? null;
        const b = post[i] ?? null;
        switch (w.kind) {
            case 'lamports':
                out.push({
                    key: `${w.address}:lamports`,
                    label: w.label,
                    before: a ? a.lamports : 0n,
                    after: b ? b.lamports : 0n,
                    unit: 'SOL',
                    scale: 9,
                });
                return;
            case 'token':
                out.push({
                    key: `${w.address}:amount`,
                    label: w.label,
                    before: a ? tokenAmount(a.data) : null,
                    after: b ? tokenAmount(b.data) : null,
                    unit: w.symbol,
                    scale: w.decimals,
                });
                return;
            case 'program': {
                const before = a && a.data.length > 0 ? safeDecode(w, a.data) : null;
                const after = b && b.data.length > 0 ? safeDecode(w, b.data) : null;
                for (const [field, meta] of Object.entries(w.fields)) {
                    const x = before?.[field] ?? null;
                    const y = after?.[field] ?? null;
                    if (x === null && y === null) continue;
                    out.push({
                        key: `${w.address}:${field}`,
                        label: `${w.label} · ${meta.label}`,
                        before: x,
                        after: y,
                        unit: meta.unit,
                        scale: meta.scale,
                        ...(meta.kind ? { kind: meta.kind } : {}),
                    });
                }
                return;
            }
        }
    });
    return out;
}

function safeDecode(
    w: Extract<WatchedAccount, { kind: 'program' }>,
    data: Uint8Array,
): Readonly<Record<string, bigint>> | null {
    try {
        return w.decode(data);
    } catch {
        return null;
    }
}

/** `after − before` (an absent side counts as 0). */
export function change(d: Delta): bigint {
    return (d.after ?? 0n) - (d.before ?? 0n);
}
