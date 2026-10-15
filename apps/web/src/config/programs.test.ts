import { SHORTWIN_PROGRAM_ADDRESS } from '@shortwin/client';
import { describe, expect, it } from 'vitest';
import { readRepo } from '../test/repo.ts';
import * as P from './programs.ts';

describe('pinned program ids', () => {
    it('match the generated client', () => {
        expect(P.SHORTWIN_PROGRAM).toBe(SHORTWIN_PROGRAM_ADDRESS);
    });

    it('match programs/shortwin/src/constants.rs', () => {
        const rs = readRepo('programs/shortwin/src/constants.rs');
        const pinned: [string, string][] = [
            ['PUMP_PROGRAM_ID', P.PUMP_PROGRAM],
            ['PUMP_AMM_PROGRAM_ID', P.PUMP_AMM_PROGRAM],
            ['PUMP_FEES_PROGRAM_ID', P.PUMP_FEES_PROGRAM],
            ['RAYDIUM_V4_PROGRAM_ID', P.RAYDIUM_V4_PROGRAM],
            ['RAYDIUM_CPMM_PROGRAM_ID', P.RAYDIUM_CPMM_PROGRAM],
            ['TOKEN_PROGRAM_ID', P.TOKEN_PROGRAM],
            ['TOKEN_2022_PROGRAM_ID', P.TOKEN_2022_PROGRAM],
            ['WSOL_MINT', P.WSOL_MINT],
            ['WSOL_2022_MINT', P.WSOL_2022_MINT],
            ['MEMO_PROGRAM_ID', P.MEMO_PROGRAM],
            ['MOCK_CPAMM_PROGRAM_ID', P.MOCK_CPAMM_PROGRAM],
        ];
        for (const [name, value] of pinned) {
            const m = new RegExp(`pub const ${name}: Pubkey =\\s*pubkey!\\("(\\w+)"\\)`).exec(rs);
            expect(m?.[1], name).toBe(value);
        }
    });

    it('pin no complete_curve helper (retired with pump v3, D2)', () => {
        expect(Object.values(P)).not.toContain('4B5mGykX9EWwaYdbTvsp8qXCUTbc7caQourz16Gat6LJ');
    });

    it('allow only pinned programs at the top level', () => {
        expect(P.TOP_LEVEL_ALLOWLIST).toContain(P.SHORTWIN_PROGRAM);
        expect(P.TOP_LEVEL_ALLOWLIST).not.toContain(P.RAYDIUM_V4_PROGRAM);
        expect(P.TOP_LEVEL_ALLOWLIST).not.toContain(P.MEMO_PROGRAM);
        expect(new Set(P.TOP_LEVEL_ALLOWLIST).size).toBe(P.TOP_LEVEL_ALLOWLIST.length);
    });
});
