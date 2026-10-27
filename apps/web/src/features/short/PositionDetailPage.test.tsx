import type { ShortPosition } from '@shortwin/client';
import { focusManager } from '@tanstack/react-query';
import { act, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Position, PositionDetail } from '../../api/types.ts';
import { MINT_A, SIG_A } from '../markets/test/fixtures.ts';
import { mockIndexer, renderAt } from '../markets/test/harness.tsx';
import { decodePosition } from './snapshot.ts';
import { vectorCase, vectorSnapshot } from './testing.ts';

const chain = vi.hoisted(() => ({
    data: null as ShortPosition | null,
    isLoading: false,
    error: null,
    refetch: vi.fn(),
}));

vi.mock('./hooks.ts', async (orig) => ({
    ...(await orig<typeof import('./hooks.ts')>()),
    usePositionAccount: () => chain,
    useShortSnapshot: () => ({ data: null, error: null, math: undefined }),
    useChainNow: () => 0n,
    useTokenLabel: () => ({ symbol: 'TEST' }),
}));

function position(status: Position['status'] = 'open'): PositionDetail {
    return {
        position: {
            position: MINT_A,
            market: MINT_A,
            mint: MINT_A,
            borrower: MINT_A,
            nonce: '0',
            status,
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
            exit:
                status === 'open'
                    ? null
                    : {
                          kind:
                              status === 'closed'
                                  ? 'Close'
                                  : status === 'liquidated'
                                    ? 'Liquidate'
                                    : status === 'expired'
                                      ? 'Expire'
                                      : 'SolSettle',
                          ts: 1_799_999_100,
                          signature: SIG_A,
                          keeper: null,
                          buyback_cost: '100',
                          rent_paid: '1',
                          penalty: '0',
                          expiry_fee: '0',
                          keeper_paid: '0',
                          borrower_payout: '99',
                          settle_condition: null,
                          borrower_pnl: '-2',
                          duration_s: 100,
                      },
        },
        events: [],
        settlement: null,
    };
}

beforeEach(() => {
    chain.data = null;
});

afterEach(() => {
    vi.unstubAllGlobals();
    vi.useRealTimers();
});

describe('position exit history', () => {
    it('shows borrower token repayment with unavailable economic P&L rather than zero profit', async () => {
        const record = position('closed');
        const original = record.position.exit;
        if (!original) throw new Error('closed fixture');
        const repaid: PositionDetail = {
            ...record,
            position: {
                ...record.position,
                exit: { ...original, kind: 'Repay', buyback_cost: null, borrower_pnl: null },
            },
        };
        mockIndexer({ [`/v1/positions/${MINT_A}`]: repaid });
        const { queryClient, view } = await renderAt(`/positions/${MINT_A}`);
        try {
            expect(await screen.findByText('Tokens repaid by borrower')).toBeTruthy();
            expect(screen.getByText('Unavailable (external token cost)')).toBeTruthy();
        } finally {
            view?.unmount();
            queryClient.clear();
        }
    });

    it('shows history updating after the account closes, then picks up the exit without a WebSocket', async () => {
        vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });
        let exited = false;
        const path = `/v1/positions/${MINT_A}`;
        const api = mockIndexer({ [path]: () => position(exited ? 'closed' : 'open') });
        const { queryClient, view } = await renderAt(`/positions/${MINT_A}`);
        try {
            expect(await screen.findByText('History updating')).toBeTruthy();
            expect(screen.queryByRole('heading', { name: 'Closed' })).toBeNull();
            expect(screen.getByText(/Waiting for the indexer to record its exit/)).toBeTruthy();
            exited = true;
            await act(async () => {
                await vi.advanceTimersByTimeAsync(10_000);
            });
            expect(await screen.findByRole('heading', { name: 'Closed' })).toBeTruthy();
            expect(screen.queryByText('History updating')).toBeNull();
            expect(api.callsTo(path)).toHaveLength(2);
            await act(async () => {
                await vi.advanceTimersByTimeAsync(30_000);
            });
            // Terminal history uses the slower fallback rather than the open-position cadence.
            expect(api.callsTo(path)).toHaveLength(2);
            await act(async () => {
                await vi.advanceTimersByTimeAsync(30_000);
            });
            expect(api.callsTo(path)).toHaveLength(3);
        } finally {
            view?.unmount();
            queryClient.clear();
        }
    });

    it('recovers a reopened position through REST when the WebSocket rollback was missed', async () => {
        vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });
        let rolledBack = false;
        const path = `/v1/positions/${MINT_A}`;
        const api = mockIndexer({ [path]: () => position(rolledBack ? 'open' : 'closed') });
        const { queryClient, view } = await renderAt(`/positions/${MINT_A}`);
        try {
            expect(await screen.findByRole('heading', { name: 'Closed' })).toBeTruthy();
            const c = vectorCase('spl-close-after-fall');
            const { get } = await vectorSnapshot(c);
            chain.data = decodePosition(get(c.position as Parameters<typeof get>[0]));
            expect(chain.data).not.toBeNull();
            rolledBack = true;
            // No socket message or manual invalidation: terminal history must still refresh.
            await act(async () => {
                await vi.advanceTimersByTimeAsync(60_000);
            });
            await screen.findByText('open', { selector: '.badge-status' });
            expect(screen.queryByRole('heading', { name: 'Closed' })).toBeNull();
            expect(api.callsTo(path)).toHaveLength(2);
            // A reopened record returns to the faster refresh cadence.
            await act(async () => {
                await vi.advanceTimersByTimeAsync(10_000);
            });
            expect(api.callsTo(path)).toHaveLength(3);
            view?.unmount();
            await act(async () => {
                await vi.advanceTimersByTimeAsync(120_000);
            });
            expect(api.callsTo(path)).toHaveLength(3);
        } finally {
            view?.unmount();
            queryClient.clear();
        }
    });

    it('does not poll terminal history while hidden or after the page unmounts', async () => {
        vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });
        const path = `/v1/positions/${MINT_A}`;
        const api = mockIndexer({ [path]: position('closed') });
        const { queryClient, view } = await renderAt(`/positions/${MINT_A}`);
        try {
            expect(await screen.findByRole('heading', { name: 'Closed' })).toBeTruthy();
            focusManager.setFocused(false);
            await act(async () => {
                await vi.advanceTimersByTimeAsync(120_000);
            });
            expect(api.callsTo(path)).toHaveLength(1);
            view?.unmount();
            focusManager.setFocused(true);
            await act(async () => {
                await vi.advanceTimersByTimeAsync(120_000);
            });
            expect(api.callsTo(path)).toHaveLength(1);
        } finally {
            view?.unmount();
            queryClient.clear();
            focusManager.setFocused(undefined);
        }
    });

    it('uses recorded exit status instead of a cached pre-close chain account', async () => {
        const c = vectorCase('spl-close-after-fall');
        const { get } = await vectorSnapshot(c);
        chain.data = decodePosition(get(c.position as Parameters<typeof get>[0]));
        expect(chain.data).not.toBeNull();
        mockIndexer({ [`/v1/positions/${MINT_A}`]: position('liquidated') });
        const { queryClient, view } = await renderAt(`/positions/${MINT_A}`);
        try {
            expect(await screen.findByRole('heading', { name: 'Closed' })).toBeTruthy();
            const badges = [...document.querySelectorAll('.badge-status')].map((b) => b.textContent);
            expect(badges).toEqual(['liquidated', 'liquidated']);
            expect(screen.queryByText('History updating')).toBeNull();
        } finally {
            view?.unmount();
            queryClient.clear();
        }
    });
});
