/** The phone status strip's summary (FRONTEND.md §7.6): the worst state first, in words. */
import { describe, expect, it } from 'vitest';
import { type StatusInputs, statusSummary } from './StatusBar.tsx';

const OK: StatusInputs = { rpcOk: true, slot: 3012n, fresh: 'live', lagS: 1, ws: 'open', mathError: false };

describe('statusSummary', () => {
    it.each([
        [{}, 'ok', '●', 'All live · RPC 3,012'],
        [{ fresh: 'lagging', lagS: 7 }, 'warn', '▲', 'Indexer lagging +7s'],
        [{ fresh: 'stale', lagS: 42 }, 'bad', '✕', 'Indexer stale +42s'],
        [{ fresh: 'down', lagS: null }, 'bad', '✕', 'Indexer unavailable · chain only'],
        [{ rpcOk: false }, 'bad', '✕', 'RPC unreachable'],
        [{ ws: 'reconnecting' }, 'warn', '▲', 'Live feed reconnecting'],
        [{ ws: 'closed' }, 'warn', '▲', 'Live feed reconnecting'],
        [{ mathError: true }, 'bad', '✕', 'Math unavailable'],
        [{ fresh: 'unknown', lagS: null }, 'warn', '▲', 'Indexer freshness unknown'],
        [{ fresh: 'idle', lagS: null }, 'idle', '○', 'Indexer awaiting activity'],
        [{ ws: 'idle' }, 'ok', '●', 'All live · RPC 3,012'],
        [{ slot: null }, 'ok', '●', 'All live · RPC —'],
    ] as const)('%o → %s %s', (over, state, glyph, text) => {
        expect(statusSummary({ ...OK, ...over } as StatusInputs)).toEqual({ state, glyph, text });
    });

    it('puts the worst first: an unreachable RPC over a lagging indexer and a reconnecting feed', () => {
        expect(statusSummary({ ...OK, rpcOk: false, fresh: 'lagging', ws: 'reconnecting' }).text).toBe(
            'RPC unreachable',
        );
        expect(statusSummary({ ...OK, fresh: 'lagging', lagS: 9, ws: 'reconnecting' }).text).toBe(
            'Indexer lagging +9s',
        );
    });
});
