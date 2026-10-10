/** A position page reads only short.win `ShortPosition` accounts (D8: never a market as a "position"). */
import { getShortPositionSize, MARKET_DISCRIMINATOR, SHORT_POSITION_DISCRIMINATOR } from '@shortwin/client';
import { address } from '@solana/kit';
import { describe, expect, it } from 'vitest';
import { SHORTWIN_PROGRAM } from '../../config/programs.ts';
import { isShortPositionAccount } from './positionAccount.ts';

function bytes(disc: ArrayLike<number>, size = getShortPositionSize()): Uint8Array {
    const b = new Uint8Array(size);
    b.set(Array.from(disc));
    return b;
}

describe('isShortPositionAccount', () => {
    it('accepts a short.win position', () => {
        expect(isShortPositionAccount(SHORTWIN_PROGRAM, bytes(SHORT_POSITION_DISCRIMINATOR))).toBe(true);
    });

    it('refuses another short.win account, another owner and a short buffer', () => {
        expect(isShortPositionAccount(SHORTWIN_PROGRAM, bytes(MARKET_DISCRIMINATOR))).toBe(false);
        const other = address('11111111111111111111111111111111');
        expect(isShortPositionAccount(other, bytes(SHORT_POSITION_DISCRIMINATOR))).toBe(false);
        expect(isShortPositionAccount(SHORTWIN_PROGRAM, bytes(SHORT_POSITION_DISCRIMINATOR, 40))).toBe(false);
    });
});
