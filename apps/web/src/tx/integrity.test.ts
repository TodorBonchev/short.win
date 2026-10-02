import { getClaimSolInstructionDataEncoder } from '@shortwin/client';
import { AccountRole, address, compileTransaction, createNoopSigner, type Instruction } from '@solana/kit';
import { describe, expect, it } from 'vitest';
import { RAYDIUM_V4_PROGRAM, SHORTWIN_PROGRAM } from '../config/programs.ts';
import { assertAllowedInstructions, assertSameIntent, IntegrityError } from './integrity.ts';

const CLAIM = Array.from(getClaimSolInstructionDataEncoder().encode({}));

import { buildMessage } from './message.ts';

const payer = createNoopSigner(address('9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin'));
const other = address('So11111111111111111111111111111111111111112');
const lifetime = { blockhash: 'EkSnNWid2cvwEVnVx9aBqawnmiCNiDgp3gUdkDPTKN1N' as never, lastValidBlockHeight: 100n };

function ix(data: number[]): Instruction {
    return {
        programAddress: SHORTWIN_PROGRAM,
        accounts: [
            { address: payer.address, role: AccountRole.WRITABLE_SIGNER },
            { address: other, role: AccountRole.WRITABLE },
        ],
        data: new Uint8Array(data),
    };
}

function bytes(ixs: Instruction[], price = 0n, limit = 200_000) {
    return compileTransaction(
        buildMessage({
            instructions: ixs,
            feePayer: payer,
            lifetime,
            computeUnitLimit: limit,
            computeUnitPrice: price,
            lookupTables: {},
            version: 0,
        }),
    ).messageBytes;
}

describe('no blind signing', () => {
    it('refuses instructions for programs outside the allowlist', () => {
        expect(() => assertAllowedInstructions([ix(CLAIM)])).not.toThrow();
        expect(() => assertAllowedInstructions([{ ...ix(CLAIM), programAddress: RAYDIUM_V4_PROGRAM }])).toThrow(
            IntegrityError,
        );
        // Data that is no short.win instruction is refused too (the gate is per instruction).
        expect(() => assertAllowedInstructions([ix([1])])).toThrow(IntegrityError);
    });

    it('accepts identical bytes and compute-budget-only changes by the wallet', () => {
        const want = bytes([ix([1, 2, 3])]);
        expect(() => assertSameIntent(want, want, {})).not.toThrow();
        expect(() => assertSameIntent(want, bytes([ix([1, 2, 3])], 50_000n, 300_000), {})).not.toThrow();
    });

    it('refuses a wallet-set priority fee above the ceiling', () => {
        const want = bytes([ix([1, 2, 3])]);
        const raised = bytes([ix([1, 2, 3])], 2_000_000n);
        expect(() => assertSameIntent(want, raised, {}, { maxComputeUnitPrice: 1_000_000n })).toThrow(
            /above the ceiling/,
        );
        expect(() => assertSameIntent(want, raised, {}, { maxComputeUnitPrice: 2_000_000n })).not.toThrow();
    });

    it('refuses a changed, added or removed instruction', () => {
        const want = bytes([ix([1, 2, 3])]);
        expect(() => assertSameIntent(want, bytes([ix([1, 2, 4])]), {})).toThrow(/changed instruction 0/);
        expect(() => assertSameIntent(want, bytes([ix([1, 2, 3]), ix([9])]), {})).toThrow(/added or removed/);
        expect(() => assertSameIntent(want, bytes([]), {})).toThrow(IntegrityError);
    });
});
