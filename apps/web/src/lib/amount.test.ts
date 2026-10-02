import { describe, expect, it } from 'vitest';
import { parseAmount, parseBps, U64_MAX } from './amount.ts';

describe('parseAmount', () => {
    it('parses decimal input into raw units', () => {
        expect(parseAmount('1.5', 9)).toEqual({ ok: true, raw: 1_500_000_000n });
        expect(parseAmount('.25', 6)).toEqual({ ok: true, raw: 250_000n });
        expect(parseAmount('10.', 2)).toEqual({ ok: true, raw: 1000n });
        expect(parseAmount(' 42 ', 0)).toEqual({ ok: true, raw: 42n });
    });

    it('rejects anything that is not a plain non-negative decimal within u64', () => {
        expect(parseAmount('', 9)).toEqual({ ok: false, error: 'empty' });
        for (const bad of ['-1', '1e9', '1,000', '0x10', '1.2.3', 'NaN', '∞', '١']) {
            expect(parseAmount(bad, 9), bad).toEqual({ ok: false, error: 'format' });
        }
        expect(parseAmount('1.1234567', 6)).toEqual({ ok: false, error: 'decimals' });
        expect(parseAmount('0', 6)).toEqual({ ok: false, error: 'zero' });
        expect(parseAmount('0', 6, { allowZero: true })).toEqual({ ok: true, raw: 0n });
        expect(parseAmount(U64_MAX.toString(), 0)).toEqual({ ok: true, raw: U64_MAX });
        expect(parseAmount((U64_MAX + 1n).toString(), 0)).toEqual({ ok: false, error: 'range' });
    });

    it('parses slippage percentages into bps within a ceiling', () => {
        expect(parseBps('0.5', 5000)).toBe(50);
        expect(parseBps('1', 5000)).toBe(100);
        expect(parseBps('0', 5000)).toBeNull();
        expect(parseBps('60', 5000)).toBeNull();
        expect(parseBps('0.001', 5000)).toBeNull();
    });
});
