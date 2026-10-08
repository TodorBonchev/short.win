import * as client from '@shortwin/client';
import { describe, expect, it } from 'vitest';
import { PROGRAM_ERRORS, programError } from './errors.ts';

const pascal = (snake: string) =>
    snake
        .toLowerCase()
        .split('_')
        .map((w) => (w ? (w[0]?.toUpperCase() ?? '') + w.slice(1) : ''))
        .join('');

describe('program error messages', () => {
    const constants = Object.entries(client).filter(([k]) => k.startsWith('SHORTWIN_ERROR__')) as [string, number][];

    it('covers every error code of the generated client, by name', () => {
        expect(constants.length).toBeGreaterThanOrEqual(106);
        for (const [k, code] of constants) {
            const info = programError(code);
            expect(info, k).toBeDefined();
            const name = pascal(k.slice('SHORTWIN_ERROR__'.length));
            // Codama renders S3TimerRunning as S3_TIMER_RUNNING and EmaWarmingUp as EMA_WARMING_UP.
            expect(info?.name.toLowerCase(), k).toBe(name.toLowerCase());
        }
        expect(PROGRAM_ERRORS.size).toBe(constants.length);
    });

    it('has a non-empty message and a known action for each code, numbered from 6000 without gaps', () => {
        const codes = [...PROGRAM_ERRORS.keys()].sort((a, b) => a - b);
        codes.forEach((c, i) => {
            expect(c).toBe(6000 + i);
        });
        for (const e of PROGRAM_ERRORS.values()) {
            expect(e.message.length).toBeGreaterThan(10);
            expect(['retry', 'adjust', 'wait', 'blocked', 'report', 'admin']).toContain(e.action);
        }
    });

    it('maps the open-path slippage and pause codes to user actions', () => {
        expect(programError(6063)?.name).toBe('ProceedsBelowMin');
        expect(programError(6063)?.action).toBe('retry');
        expect(programError(6051)?.action).toBe('wait');
        expect(programError(6104)?.name).toBe('GraduationHalted');
        expect(programError(6105)?.name).toBe('AuthoritiesNotDistinct');
        expect(programError(6105)?.action).toBe('admin');
        expect(programError(6106)?.name).toBe('EpochRecordReadOnly');
        expect(programError(6106)?.action).toBe('retry');
        expect(programError(6107)?.name).toBe('RebaseNeedsOwner');
        expect(programError(6107)?.action).toBe('blocked');
        expect(programError(5999)).toBeUndefined();
    });
});
