import * as client from '@shortwin/client';
import { describe, expect, it } from 'vitest';
import { readRepo } from '../test/repo.ts';
import { PROGRAM_ERRORS, programError } from './errors.ts';

const pascal = (snake: string) =>
    snake
        .toLowerCase()
        .split('_')
        .map((w) => (w ? (w[0]?.toUpperCase() ?? '') + w.slice(1) : ''))
        .join('');

/** Rust source without its line comments and with every string literal emptied (`""`). */
function stripRust(src: string): string {
    let out = '';
    let i = 0;
    while (i < src.length) {
        if (src[i] === '"') {
            i++;
            while (i < src.length && src[i] !== '"') i += src[i] === '\\' ? 2 : 1;
            i++;
            out += '""';
        } else if (src[i] === '/' && src[i + 1] === '/') {
            while (i < src.length && src[i] !== '\n') i++;
        } else {
            out += src[i];
            i++;
        }
    }
    return out;
}

/**
 * The variants of the program's `ShortwinError` (`programs/shortwin/src/errors.rs`), in declaration
 * order: Anchor's `#[error_code]` numbers them from 6000. Comments, string literals and attributes
 * (`#[msg(…)]`) are dropped first, so only the variant names are left between the commas.
 */
function programVariants(): string[] {
    const src = stripRust(readRepo('programs/shortwin/src/errors.rs'));
    const head = 'pub enum ShortwinError {';
    const start = src.indexOf(head);
    expect(start).toBeGreaterThan(0);
    return src
        .slice(start + head.length, src.indexOf('\n}', start))
        .replace(/#\[[^\]]*\]/g, '')
        .split(',')
        .map((v) => v.trim())
        .filter((v) => v.length > 0);
}

describe('program error messages', () => {
    const constants = Object.entries(client).filter(([k]) => k.startsWith('SHORTWIN_ERROR__')) as [string, number][];

    it('has one row per variant of the program’s ShortwinError, by code and name', () => {
        const variants = programVariants();
        expect(variants.length).toBeGreaterThanOrEqual(109);
        for (const v of variants) expect(v, v).toMatch(/^[A-Z][A-Za-z0-9]*$/);
        variants.forEach((name, i) => {
            expect(programError(6000 + i)?.name, `${6000 + i}`).toBe(name);
        });
        expect(PROGRAM_ERRORS.size).toBe(variants.length);
    });

    it('covers every error code of the generated client, by name', () => {
        expect(constants.length).toBeGreaterThanOrEqual(109);
        for (const [k, code] of constants) {
            const info = programError(code);
            expect(info, k).toBeDefined();
            const name = pascal(k.slice('SHORTWIN_ERROR__'.length));
            // Codama renders S3TimerRunning as S3_TIMER_RUNNING and EmaWarmingUp as EMA_WARMING_UP.
            expect(info?.name.toLowerCase(), k).toBe(name.toLowerCase());
        }
        // The client is generated from the program's IDL (CI's `codama` job keeps it current), so
        // it has a constant for every row: no row is ahead of it.
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

    it('blocks deposits until unassigned income has an accounting review', () => {
        const info = programError(6110);
        expect(info).toMatchObject({ name: 'LegacyPoolCarry', action: 'blocked' });
        expect(info?.message).toContain('unassigned income from earlier shares');
        expect(info?.message).toContain('until the accounting is reviewed');
        expect(info?.message).toContain('Sweeping or retrying will not unblock deposits');
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
        // AIR-07: a drained pool takes deposits again once repayments or settlements free tokens.
        expect(programError(6108)?.name).toBe('PoolDrained');
        expect(programError(6108)?.action).toBe('wait');
        // pump-final §6.3 (owner D1 (a)): a live cashback curve is refused at listing.
        expect(programError(6109)?.name).toBe('CurveCashbackCoin');
        expect(programError(6109)?.action).toBe('blocked');
        expect(programError(5999)).toBeUndefined();
    });
});
