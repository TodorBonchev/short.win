/**
 * O-2 on the market page (FRONTEND.md §3.4; the owner's thirteenth round, audit finding D-23): the
 * Short panel's link to the coverage banner. Beside the page it focuses the banner; from the trade
 * sheet (under 1,024 px) it closes the sheet first and then focuses the banner. The Short panel is
 * stubbed to the link itself (its own test covers when it shows); the page, its banners, its sheet
 * and the link's behaviour are real.
 */
import { act, fireEvent, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { T } from '../../config/testids.ts';
import * as accounts from '../../rpc/accounts.ts';
import { setMedia } from '../../test/media.ts';
import { CoverageLink } from '../short/CoverageLink.tsx';
import * as chainReads from './shared/chainReads.ts';
import { MARKET_FLAG } from './shared/marketFlags.ts';
import { chainMarket, detail, exists, globalConfig, MINT_A, missing, NO_FLAGS, oracleCache } from './test/fixtures.ts';
import { mockIndexer, renderAt } from './test/harness.tsx';

vi.mock('../short/ShortPanel.tsx', () => ({ ShortPanel: () => <CoverageLink /> }));
vi.mock('../../rpc/accounts.ts', async (orig) => ({
    ...(await orig<typeof import('../../rpc/accounts.ts')>()),
    readMarket: vi.fn(),
    readGlobalConfig: vi.fn(),
    readVenueGuard: vi.fn(),
    readOracle: vi.fn(),
}));
vi.mock('./shared/chainReads.ts', async (orig) => ({
    ...(await orig<typeof import('./shared/chainReads.ts')>()),
    readRawAccount: vi.fn(),
}));

beforeEach(() => {
    vi.mocked(accounts.readMarket).mockResolvedValue(
        exists(MINT_A, chainMarket({ flags: MARKET_FLAG.pausedCoverage })),
    );
    vi.mocked(accounts.readGlobalConfig).mockResolvedValue(exists(MINT_A, globalConfig()));
    vi.mocked(accounts.readVenueGuard).mockResolvedValue(missing(MINT_A));
    vi.mocked(accounts.readOracle).mockResolvedValue(exists(MINT_A, oracleCache()));
    vi.mocked(chainReads.readRawAccount).mockResolvedValue(null);
    mockIndexer({
        [`/v1/markets/${MINT_A}`]: detail({ flags: { ...NO_FLAGS, paused_coverage: true } }),
        [`/v1/markets/${MINT_A}/pools`]: { mint: MINT_A, current: null, window: null, pools: [], events: [] },
    });
});

afterEach(() => {
    vi.unstubAllGlobals();
});

const banner = () => screen.getByTestId(T.marketBanner('coverage-paused'));

describe('the coverage link (O-2)', () => {
    it('beside the page: scrolls to the banner and focuses it', async () => {
        const { queryClient } = await renderAt(`/m/${MINT_A}?panel=short`);
        const link = await screen.findByTestId(T.shortCoverageLink);
        expect(link.textContent).toBe('Read the full coverage notice');
        expect(banner().tabIndex).toBe(-1);
        await act(async () => {
            fireEvent.click(link);
        });
        expect(document.activeElement).toBe(banner());
        queryClient.clear();
    });

    it('in the phone’s trade sheet: closes the sheet, then focuses the banner', async () => {
        setMedia({ phone: true, coarse: true });
        const { queryClient, router } = await renderAt(`/m/${MINT_A}`);
        // Opened as a user does: the action bar's Short.
        await act(async () => {
            fireEvent.click(await screen.findByTestId(T.marketActionShort));
        });
        const sheet = document.querySelector('.trade-panel');
        await waitFor(() => expect(sheet?.getAttribute('data-sheet')).toBe('open'));
        const link = await screen.findByTestId(T.shortCoverageLink);
        expect(link.textContent).toBe('Close and read the full coverage notice');
        await act(async () => {
            fireEvent.click(link);
        });
        await waitFor(() => expect(sheet?.getAttribute('data-sheet')).toBeNull());
        expect(router.state.location.search).not.toHaveProperty('sheet');
        await waitFor(() => expect(document.activeElement).toBe(banner()));
        queryClient.clear();
    });
});
