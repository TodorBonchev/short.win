import { act, fireEvent, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { T } from '../../config/testids.ts';
import * as accounts from '../../rpc/accounts.ts';
import { exists, globalConfig, MINT_A, MINT_D, missing } from './test/fixtures.ts';
import { mockIndexer, renderAt } from './test/harness.tsx';

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
    mockIndexer({ '/v1/markets': { data: [], next_cursor: null } });
    vi.mocked(accounts.readGlobalConfig).mockResolvedValue(missing(MINT_A));
    vi.mocked(accounts.readMarket).mockResolvedValue(missing(MINT_A));
});

afterEach(() => {
    vi.unstubAllGlobals();
    vi.clearAllMocks();
});

describe('devnet before protocol initialization', () => {
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
