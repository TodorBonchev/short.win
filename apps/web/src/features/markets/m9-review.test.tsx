/**
 * Regression of M9-SEC-3: the market page's token identity (the mint, its copy button) is the
 * route's, and the decimals that scale every header price and amount are the chain's; a detail the
 * indexer serves for another mint is not used (the page falls back to the chain's view and says so).
 */
import { screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { T } from '../../config/testids.ts';
import * as accounts from '../../rpc/accounts.ts';
import * as chainReads from './shared/chainReads.ts';
import { chainMarket, detail, exists, globalConfig, MINT_A, MINT_B, missing, oracleCache } from './test/fixtures.ts';
import { mockIndexer, renderAt } from './test/harness.tsx';

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
    vi.mocked(accounts.readMarket).mockResolvedValue(exists(MINT_A, chainMarket({ decimals: 6 })));
    vi.mocked(accounts.readGlobalConfig).mockResolvedValue(exists(MINT_A, globalConfig()));
    vi.mocked(accounts.readVenueGuard).mockResolvedValue(missing(MINT_A));
    vi.mocked(accounts.readOracle).mockResolvedValue(exists(MINT_A, oracleCache()));
    vi.mocked(chainReads.readRawAccount).mockResolvedValue(null);
});

afterEach(() => {
    vi.unstubAllGlobals();
});

describe('M9-SEC-3: the market header’s identity and decimals come from the route and the chain', () => {
    it('ignores a detail for another mint: shows and copies the route’s mint, prices at the chain’s decimals', async () => {
        mockIndexer({
            [`/v1/markets/${MINT_A}`]: detail({ mint: MINT_B, symbol: 'BONK', name: 'Bonk', decimals: 9 }),
            [`/v1/markets/${MINT_A}/pools`]: { mint: MINT_A, current: null, window: null, pools: [], events: [] },
        });
        const { queryClient } = await renderAt(`/m/${MINT_A}`);
        await screen.findByTestId(T.marketIndexerMismatch);
        expect(screen.getByTestId(T.marketMint).dataset.value).toBe(MINT_A);
        expect(screen.queryByRole('button', { name: `Copy ${MINT_B}` })).toBeNull();
        expect(screen.getByRole('button', { name: `Copy ${MINT_A}` })).toBeTruthy();
        // The indexer's symbol for another token is not shown as this one's.
        expect(screen.getByTestId(T.marketSymbol).textContent).not.toBe('BONK');
        expect(screen.getByTestId(T.marketSource).dataset.value).toBe('chain');
        const price = screen.getByTestId(T.marketStat('price'));
        // 28e12 WAD at the chain's 6 decimals: 0.000000028 SOL / token.
        expect(price.textContent).toContain('0.0₇28');
        expect(screen.getByTestId(T.shortPanel).dataset.mint).toBe(MINT_A);
        queryClient.clear();
    });

    it('a detail for this mint with wrong decimals still prices at the chain’s decimals', async () => {
        mockIndexer({
            [`/v1/markets/${MINT_A}`]: detail({ mint: MINT_A, symbol: 'AAA', name: 'Token A', decimals: 9 }),
            [`/v1/markets/${MINT_A}/pools`]: { mint: MINT_A, current: null, window: null, pools: [], events: [] },
        });
        const { queryClient } = await renderAt(`/m/${MINT_A}`);
        await screen.findByTestId(T.marketSymbol);
        expect(screen.queryByTestId(T.marketIndexerMismatch)).toBeNull();
        expect(screen.getByTestId(T.marketSource).dataset.value).toBe('indexer');
        expect(screen.getByTestId(T.marketMint).dataset.value).toBe(MINT_A);
        const price = screen.getByTestId(T.marketStat('price'));
        expect(price.dataset.value).toBe('28000000000000');
        expect(price.textContent).toContain('0.0₇28');
        expect(price.textContent).not.toContain('0.0₄28');
        queryClient.clear();
    });
});
