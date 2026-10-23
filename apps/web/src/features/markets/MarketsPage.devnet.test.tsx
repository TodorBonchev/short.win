import { act, fireEvent, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { indexerMeta } from '../../api/queries.ts';
import { T } from '../../config/testids.ts';
import * as accounts from '../../rpc/accounts.ts';
import { rpcSlot } from '../../rpc/slot.ts';
import { exists, globalConfig, INGEST_OK, MINT_A, MINT_D, missing } from './test/fixtures.ts';
import { mockIndexer, problem, renderAt } from './test/harness.tsx';

vi.mock('../../config/env.ts', async (orig) => {
    const actual = await orig<typeof import('../../config/env.ts')>();
    return { ...actual, config: { ...actual.config, cluster: 'devnet', chain: 'solana:devnet' } };
});

vi.mock('../../rpc/accounts.ts', async (orig) => ({
    ...(await orig<typeof import('../../rpc/accounts.ts')>()),
    readGlobalConfig: vi.fn(),
    readMarket: vi.fn(),
}));

beforeEach(() => {
    indexerMeta.set(null);
    rpcSlot.set({ slot: null, at: 0, ok: false });
    mockIndexer({ '/v1/markets': { data: [], next_cursor: null } });
    vi.mocked(accounts.readGlobalConfig).mockResolvedValue(missing(MINT_A));
    vi.mocked(accounts.readMarket).mockResolvedValue(missing(MINT_A));
});

afterEach(() => {
    vi.unstubAllGlobals();
    vi.clearAllMocks();
});

describe('devnet before protocol initialization', () => {
    it('shows neutral indexer activity while the empty devnet API responds despite an old indexed slot', async () => {
        mockIndexer({ '/v1/status': { ...INGEST_OK, lag_s: 30_000 } });
        const { queryClient } = await renderAt('/status');
        await act(async () => rpcSlot.set({ slot: 2000n, at: Date.now(), ok: true }));
        await waitFor(() => expect(screen.getByTestId(T.statusIndexer).dataset.state).toBe('idle'));
        expect(screen.getByTestId(T.statusIndexer).textContent).toContain('awaiting activity');
        expect(screen.getByTestId(T.statusIndexer).textContent).not.toContain('+400s');
        expect(screen.getByTestId(T.statusIndexerLag).dataset.state).toBe('idle');
        expect(screen.getByTestId(T.statusIndexerLag).textContent).not.toContain('behind the RPC node');
        expect(screen.getByTestId(T.statusIndexerHealth).textContent).toContain('Last indexed activity');
        expect(screen.getByTestId(T.staleBadge).textContent).toBe('Awaiting activity');
        queryClient.clear();
    });

    it('keeps the same slot gap stale after initialization', async () => {
        vi.mocked(accounts.readGlobalConfig).mockResolvedValue(exists(MINT_A, globalConfig()));
        const { queryClient } = await renderAt('/status');
        await act(async () => rpcSlot.set({ slot: 2000n, at: Date.now(), ok: true }));
        await waitFor(() => expect(screen.getByTestId(T.statusIndexer).dataset.state).toBe('stale'));
        expect(screen.getByTestId(T.statusIndexerLag).dataset.state).toBe('stale');
        queryClient.clear();
    });

    it('uses the reconciled chain watermark when initialized but no program transaction is recent', async () => {
        mockIndexer({ '/v1/status': { ...INGEST_OK, lag_s: 300 } });
        vi.mocked(accounts.readGlobalConfig).mockResolvedValue(exists(MINT_A, globalConfig()));
        const { queryClient } = await renderAt('/status');
        await screen.findByTestId(T.statusIndexerHealth);
        await act(async () => {
            indexerMeta.set({ indexerSlot: 1000, finalizedSlot: 1995, receivedAt: Date.now() });
            rpcSlot.set({ slot: 2000n, at: Date.now(), ok: true });
        });
        await waitFor(() => expect(screen.getByTestId(T.statusIndexer).dataset.state).toBe('live'));
        expect(screen.getByTestId(T.statusIndexerLag).textContent).toContain('2 s behind the RPC node');
        expect(screen.getByTestId(T.statusIndexerHealth).textContent).toContain('Last indexed activity');
        queryClient.clear();
    });

    it('marks the indexer down when status fails even before initialization', async () => {
        mockIndexer({ '/v1/status': problem(500, 'internal') });
        const { queryClient } = await renderAt('/status');
        await waitFor(() => expect(screen.getByTestId(T.statusIndexer).dataset.state).toBe('down'));
        expect(screen.getByTestId(T.statusIndexerLag).dataset.state).toBe('down');
        queryClient.clear();
    });

    it('announces the preview and does not offer market creation, including for an unlisted mint', async () => {
        const { queryClient } = await renderAt('/markets');
        expect((await screen.findByTestId(T.devnetPreviewBanner)).textContent).toContain('protocol not initialized');
        expect(screen.queryByTestId(T.marketsCreateLink)).toBeNull();
        expect((await screen.findByTestId(T.marketsEmpty)).textContent).toContain('not initialized');

        await act(async () => {
            fireEvent.change(screen.getByTestId(T.marketsSearch), { target: { value: MINT_D } });
        });
        await waitFor(() => expect((screen.getByTestId(T.marketsSearch) as HTMLInputElement).value).toBe(MINT_D));
        expect(screen.queryByTestId(T.marketsEmptyCreate)).toBeNull();
        expect(screen.queryByTestId(T.marketsCreateLink)).toBeNull();
        await act(async () => {
            fireEvent.click(screen.getByTestId(T.navMore));
        });
        expect(screen.getByTestId(T.moreSheet)).toBeTruthy();
        expect(screen.queryByTestId(T.moreCreateMarket)).toBeNull();
        queryClient.clear();
    });

    it('keeps the create action when devnet GlobalConfig exists', async () => {
        vi.mocked(accounts.readGlobalConfig).mockResolvedValue(exists(MINT_A, globalConfig()));
        const { queryClient } = await renderAt('/markets');
        expect(await screen.findByTestId(T.marketsCreateLink)).toBeTruthy();
        expect(screen.queryByTestId(T.devnetPreviewBanner)).toBeNull();
        queryClient.clear();
    });

    it('explains why a direct create-market visit cannot submit', async () => {
        const { queryClient } = await renderAt('/markets/new');
        await screen.findByTestId(T.devnetPreviewBanner);
        await waitFor(() =>
            expect(screen.getByTestId(T.createSubmitReason).textContent).toContain('configuration is not initialized'),
        );
        expect((screen.getByTestId(T.createSubmit) as HTMLButtonElement).disabled).toBe(true);
        queryClient.clear();
    });

    it('does not offer market creation from a missing mint page', async () => {
        const { queryClient } = await renderAt(`/m/${MINT_A}`);
        const missingMarket = await screen.findByTestId(T.marketNotFound);
        expect(missingMarket.textContent).toContain('devnet protocol readiness');
        expect(missingMarket.querySelector('a[href^="/markets/new"]')).toBeNull();
        queryClient.clear();
    });
});
