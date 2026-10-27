import { describe, expect, it } from 'vitest';
import type { Position } from '../../api/types.ts';
import { MINT_A, SIG_A } from '../markets/test/fixtures.ts';
import { positionTotals } from './PositionsPage.tsx';
import type { PositionRowData } from './positions/data.ts';

function exited(pnl: string | null): PositionRowData {
    const api: Position = {
        position: MINT_A,
        market: MINT_A,
        mint: MINT_A,
        borrower: MINT_A,
        nonce: '0',
        status: 'closed',
        tier: 'C',
        venue: 'mock',
        q: '100',
        s: '100',
        m: '100',
        margin_added: '0',
        v: '200',
        n_open: '100',
        fee_open: '1',
        entry_price: '1000000000000000000',
        deadline: 1_800_000_000,
        opened_ts: 1_799_999_000,
        open_signature: SIG_A,
        armed: false,
        exit: {
            kind: pnl === null ? 'Repay' : 'Close',
            ts: 1_799_999_100,
            signature: SIG_A,
            keeper: null,
            buyback_cost: pnl === null ? null : '100',
            rent_paid: '1',
            penalty: null,
            expiry_fee: null,
            keeper_paid: null,
            borrower_payout: '199',
            settle_condition: null,
            borrower_pnl: pnl,
            duration_s: 100,
        },
    };
    return {
        position: MINT_A,
        market: MINT_A,
        mint: MINT_A,
        status: 'closed',
        api,
        chain: null,
        snapshot: null,
        metrics: null,
        mismatch: false,
    };
}

describe('exited position P&L totals', () => {
    it('does not turn unknown external token acquisition cost into zero P&L', () => {
        expect(positionTotals([exited(null)])).toMatchObject({ pnl: 0n, missingPnl: 1, count: 1 });
        expect(positionTotals([exited('10'), exited(null), exited('-4'), exited('0')])).toMatchObject({
            pnl: 6n,
            missingPnl: 1,
            count: 4,
        });
        expect(positionTotals([exited('0')])).toMatchObject({ pnl: 0n, missingPnl: 0 });
    });
});
