import { address } from '@solana/kit';
import { describe, expect, it } from 'vitest';
import { change, computeDeltas } from './deltas.ts';
import type { WatchedAccount } from './types.ts';

const A = address('So11111111111111111111111111111111111111112');

function tokenData(amount: bigint): Uint8Array {
    const b = new Uint8Array(165);
    for (let i = 0; i < 8; i++) b[64 + i] = Number((amount >> BigInt(8 * i)) & 0xffn);
    return b;
}

describe('computeDeltas', () => {
    it('reports lamport, token and decoded program-field changes', () => {
        const watch: WatchedAccount[] = [
            { kind: 'lamports', address: A, label: 'Your SOL' },
            { kind: 'token', address: A, label: 'Your BONK', decimals: 5, symbol: 'BONK' },
            {
                kind: 'program',
                address: A,
                label: 'Position',
                decode: (d) => ({ q: BigInt(d[0] ?? 0), v: BigInt(d[1] ?? 0) }),
                fields: {
                    q: { label: 'Q owed', unit: 'BONK', scale: 5 },
                    v: { label: 'Vault', unit: 'SOL', scale: 9 },
                },
            },
        ];
        const pre = [{ lamports: 10n, data: new Uint8Array() }, { lamports: 0n, data: tokenData(500n) }, null];
        const post = [
            { lamports: 4n, data: new Uint8Array() },
            { lamports: 0n, data: tokenData(200n) },
            { lamports: 1n, data: new Uint8Array([7, 9]) },
        ];
        const d = computeDeltas(watch, pre, post);
        expect(d.map((x) => [x.label, x.before, x.after])).toEqual([
            ['Your SOL', 10n, 4n],
            ['Your BONK', 500n, 200n],
            ['Position · Q owed', null, 7n],
            ['Position · Vault', null, 9n],
        ]);
        expect(d.map(change)).toEqual([-6n, -300n, 7n, 9n]);
    });

    it('survives a decoder that throws', () => {
        const d = computeDeltas(
            [
                {
                    kind: 'program',
                    address: A,
                    label: 'Bad',
                    decode: () => {
                        throw new Error('layout');
                    },
                    fields: { x: { label: 'x', unit: '', scale: 0 } },
                },
            ],
            [{ lamports: 0n, data: new Uint8Array([1]) }],
            [{ lamports: 0n, data: new Uint8Array([1]) }],
        );
        expect(d).toEqual([]);
    });
});
