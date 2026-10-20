import { getVenueGuardDecoder, VENUE_GUARD_DISCRIMINATOR } from '@shortwin/client';
import { address } from '@solana/kit';
import { describe, expect, it } from 'vitest';
import { SHORTWIN_PROGRAM, SYSTEM_PROGRAM } from '../../config/programs.ts';
import { venueProgramRowsFromChain } from '../markets/status/statusModel.ts';
import { decodeOwned } from './snapshot.ts';

const GUARD = address('So11111111111111111111111111111111111111112');

function guardBytes(): Uint8Array {
    const bytes = new Uint8Array(304);
    bytes.set(VENUE_GUARD_DISCRIMINATOR);
    const view = new DataView(bytes.buffer);
    view.setUint16(8, 1, true);
    bytes[10] = 254;
    view.setUint32(12, 1 << 4, true);
    view.setBigUint64(16 + 4 * 8, 50n, true);
    view.setBigUint64(64 + 4 * 8, 51n, true);
    view.setBigInt64(112 + 4 * 8, 1_800_000_000n, true);
    for (let i = 0; i < 4; i++) {
        view.setUint32(160 + i * 4, 1_000 + i, true);
        bytes.set(
            Uint8Array.from({ length: 32 }, (_, j) => i * 32 + j + 1),
            176 + i * 32,
        );
    }
    return bytes;
}

describe('full-digest VenueGuard in browser snapshots', () => {
    it('reads all 32 bytes of each digest and preserves upgrade-status fields', () => {
        const data = guardBytes();
        const decoded = decodeOwned(
            { address: GUARD, owner: SHORTWIN_PROGRAM, lamports: 1n, executable: false, data },
            getVenueGuardDecoder(),
        );
        expect(decoded).not.toBeNull();
        expect(decoded?.guardElf.map((hash) => hash.length)).toEqual([32, 32, 32, 32]);
        expect(decoded?.guardElf.map((hash) => hash[31])).toEqual([32, 64, 96, 128]);
        expect(venueProgramRowsFromChain(decoded, null).find((p) => p.name === 'raydium_cpmm')).toMatchObject({
            upgraded: true,
            guardSlot: '50',
            deploySlot: '51',
            flaggedAt: 1_800_000_000,
        });
    });

    it('does not accept truncated or foreign guard data as a full-digest snapshot', () => {
        const raw = { address: GUARD, owner: SHORTWIN_PROGRAM, lamports: 1n, executable: false };
        const decoder = getVenueGuardDecoder();
        expect(decodeOwned({ ...raw, data: guardBytes().slice(0, 224) }, decoder)).toBeNull();
        expect(decodeOwned({ ...raw, owner: SYSTEM_PROGRAM, data: guardBytes() }, decoder)).toBeNull();
    });
});
