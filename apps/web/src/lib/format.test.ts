import { describe, expect, it } from 'vitest';
import {
    formatBps,
    formatCompact,
    formatDuration,
    formatFixed,
    formatPnlSol,
    formatPriceSol,
    formatSignificant,
    formatSol,
    formatTokens,
    MINUS,
    shortAddress,
    WAD,
} from './format.ts';

describe('format', () => {
    it('formats lamports as SOL, truncating (never rounding a balance up)', () => {
        expect(formatSol(1_234_567_890n)).toBe('1.2345');
        expect(formatSol(1_999_999_999n, 2)).toBe('1.99');
        expect(formatSol(0n)).toBe('0');
        expect(formatSol(25_000_000_000_000n)).toBe('25,000');
        expect(formatSol(-1_500_000_000n)).toBe(`${MINUS}1.5`);
        expect(formatFixed(-1n, 9, 4)).toBe('0');
    });

    it('formats tokens by their decimals', () => {
        expect(formatTokens(1_000_000_000_000n, 6)).toBe('1,000,000');
        expect(formatTokens(123_456n, 6, 6)).toBe('0.123456');
    });

    it('formats bps and signed P&L', () => {
        expect(formatBps(12_500)).toBe('125%');
        expect(formatBps(1234)).toBe('12.34%');
        expect(formatBps(5)).toBe('0.05%');
        expect(formatPnlSol(1_200_000_000n)).toBe('+1.2');
        expect(formatPnlSol(-450_000_000n)).toBe(`${MINUS}0.45`);
        expect(formatPnlSol(0n)).toBe('0');
    });

    it('formats tiny prices with subscript zeros and keeps the plain form', () => {
        expect(formatSignificant(1234n, 1_000_000_000n)).toEqual({ display: '0.0₅1234', plain: '0.000001234' });
        expect(formatSignificant(12n, 1000n)).toEqual({ display: '0.012', plain: '0.012' });
        expect(formatSignificant(31_415n, 10_000n).plain).toBe('3.141');
        // A memecoin at 2.8e-8 SOL per token, 6 decimals: price_wad = lamports per raw unit × 1e18.
        const wad = (28n * WAD) / 1_000_000n; // 2.8e-5 lamports per raw unit
        expect(formatPriceSol(wad, 6).plain).toBe('0.000000028');
        expect(formatPriceSol(wad, 6).display).toBe('0.0₇28');
    });

    it('formats durations, compact magnitudes and addresses', () => {
        expect(formatDuration(45)).toBe('45s');
        expect(formatDuration(245)).toBe('4m 05s');
        expect(formatDuration(3 * 3600 + 12 * 60)).toBe('3h 12m');
        expect(formatDuration(2 * 86400 + 4 * 3600 + 5)).toBe('2d 4h');
        expect(formatDuration(-3)).toBe('0s');
        expect(formatCompact(1_234_000n, 0)).toBe('1.23M');
        expect(formatCompact(999n, 0)).toBe('999');
        expect(formatCompact(12_500_000_000n, 9)).toBe('12.5');
        expect(shortAddress('5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK')).toBe('5yvy…VpKK');
    });
});
