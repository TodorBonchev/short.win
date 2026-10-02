/**
 * Protocol status (FRONTEND.md §3.9, §10.1) against a mocked indexer: the global pause, the venue
 * program rows with their latch and bits, the degraded and route-broken markets aggregated from
 * the scanned list (FE-5), indexer health and keeper coverage; and the chain fallback for the
 * venue rows while `/v1/venues` fails.
 */
import { screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { T } from '../../config/testids.ts';
import * as accounts from '../../rpc/accounts.ts';
import { GUARD, VENUE_BIT } from './shared/marketFlags.ts';
import {
    exists,
    globalConfig,
    INGEST_OK,
    MINT_A,
    MINT_B,
    MINT_C,
    missing,
    NO_FLAGS,
    stats,
    summary,
    venueGuard,
    venues,
} from './test/fixtures.ts';
import { mockIndexer, problem, renderAt } from './test/harness.tsx';

vi.mock('../../rpc/accounts.ts', async (orig) => ({
    ...(await orig<typeof import('../../rpc/accounts.ts')>()),
    readGlobalConfig: vi.fn(),
    readVenueGuard: vi.fn(),
}));

const MARKETS = {
    data: [
        summary({ mint: MINT_A, symbol: 'AAA', flags: { ...NO_FLAGS, paused_coverage: true } }),
        summary({ mint: MINT_B, symbol: 'BBB', venue: 'raydium_cpmm' }),
        summary({ mint: MINT_C, symbol: 'CCC', venue: 'raydium_v4', flags: { ...NO_FLAGS, route_broken: true } }),
    ],
    next_cursor: null,
};

beforeEach(() => {
    vi.mocked(accounts.readGlobalConfig).mockResolvedValue(exists(MINT_A, globalConfig()));
    vi.mocked(accounts.readVenueGuard).mockResolvedValue(missing(MINT_A));
});

afterEach(() => {
    vi.unstubAllGlobals();
});

describe('ProtocolStatusPage', () => {
    it('aggregates the pause, the venue programs, degraded and route-broken markets and health', async () => {
        const guards = venues().guards.map((g) =>
            g.venue === GUARD.raydium_v4
                ? { ...g, upgraded: true, programdata_slot: '222', flagged_at: 1_800_000_000 }
                : g,
        );
        const api = mockIndexer({
            '/v1/admin/config': { paused_opens: true },
            '/v1/venues': venues({ guards, route_broken_bits: VENUE_BIT.raydium_cpmm, venue_enabled: 0b0111 }),
            '/v1/markets': MARKETS,
            '/v1/status': { ...INGEST_OK, quarantined: 3 },
            '/v1/stats': stats({ coverage_gap_s: 95 }),
        });
        const { queryClient } = await renderAt('/status');
        expect(await screen.findByTestId(T.statusPage)).toBeTruthy();
        await waitFor(() => expect(screen.getByTestId(T.statusPausedOpens).dataset.value).toBe('true'));
        const v4 = await screen.findByTestId(T.statusVenueRow('raydium_v4'));
        expect(v4.dataset).toMatchObject({ upgraded: 'true', enabled: 'true', routeBroken: 'false' });
        expect(v4.textContent).toContain('222');
        expect(screen.getByTestId(T.statusVenueRow('raydium_cpmm')).dataset).toMatchObject({
            upgraded: 'false',
            enabled: 'false',
            routeBroken: 'true',
        });
        expect(screen.getByTestId(T.statusVenueRow('pump_fees')).dataset.enabled).toBe('true');
        // FE-5: the list scanned client-side, largest first.
        const req = api.callsTo('/v1/markets')[0];
        expect(req?.searchParams.get('limit')).toBe('500');
        expect(req?.searchParams.get('sort')).toBe('tvl');
        await waitFor(() => expect(screen.getByTestId(T.statusScanned).dataset.value).toBe('3'));
        const degraded = screen.getByTestId(T.statusDegraded);
        expect(within(degraded).getByTestId(T.statusDegradedRow(MINT_A))).toBeTruthy();
        expect(within(degraded).queryByTestId(T.statusDegradedRow(MINT_B))).toBeNull();
        const broken = screen.getByTestId(T.statusRouteBroken);
        expect(within(broken).getByTestId(T.statusRouteBrokenRow('raydium_cpmm')).dataset.kind).toBe('venue');
        expect(within(broken).getByTestId(T.statusRouteBrokenRow(MINT_B)).dataset.kind).toBe('market');
        expect(within(broken).getByTestId(T.statusRouteBrokenRow(MINT_C))).toBeTruthy();
        expect(within(broken).queryByTestId(T.statusRouteBrokenRow(MINT_A))).toBeNull();
        expect(screen.getByTestId(T.statusIndexerHealth).textContent).toContain('3');
        expect(screen.getByTestId(T.statusCoverageGap).dataset.value).toBe('95');
        expect(screen.getByTestId(T.statusIndexerLag).dataset.state).toBeTruthy();
        queryClient.clear();
    });

    it('reads the venue rows and the pause from the chain while the indexer fails', async () => {
        vi.mocked(accounts.readVenueGuard).mockResolvedValue(exists(MINT_A, venueGuard(1 << GUARD.pump_amm)));
        vi.mocked(accounts.readGlobalConfig).mockResolvedValue(
            exists(MINT_A, globalConfig({ flags: 1, venueRouteBroken: VENUE_BIT.pump_curve })),
        );
        mockIndexer({
            '/v1/admin/config': problem(500, 'internal'),
            '/v1/venues': problem(500, 'internal'),
            '/v1/markets': problem(500, 'internal'),
            '/v1/status': problem(500, 'internal'),
        });
        const { queryClient } = await renderAt('/status');
        await screen.findByTestId(T.statusVenueRow('pump_amm'));
        await waitFor(() => expect(screen.getByTestId(T.statusVenueRow('pump_amm')).dataset.upgraded).toBe('true'));
        expect(screen.getByTestId(T.statusVenueRow('pump')).dataset.routeBroken).toBe('true');
        await waitFor(() => expect(screen.getByTestId(T.statusPausedOpens).dataset.value).toBe('true'));
        expect(screen.getByText(/Read from the chain/)).toBeTruthy();
        expect(screen.getByTestId(T.statusIndexerLag).dataset.state).toBe('down');
        queryClient.clear();
    });
});
