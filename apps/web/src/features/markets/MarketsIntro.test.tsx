/**
 * The markets list's intro strip (FRONTEND.md §3.3, §4.3; the owner's thirteenth round, O-3 (a)):
 * the three parts and their links, Hide remembered in `localStorage`, a storage that throws (the
 * strip still renders, and Hide holds for the session), the status message and the focus after Hide,
 * and How it works reached from the strip.
 */
import { act, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { copy } from '../../config/copy.ts';
import { T } from '../../config/testids.ts';
import { INTRO_HIDDEN_KEY, resetIntroSession } from './MarketsIntro.tsx';
import { MINT_A, summary } from './test/fixtures.ts';
import { mockIndexer, renderAt } from './test/harness.tsx';

beforeEach(() => {
    resetIntroSession();
    globalThis.localStorage.clear();
    mockIndexer({ '/v1/markets': { data: [summary({ mint: MINT_A, symbol: 'BONK' })], next_cursor: null } });
});

afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
});

const intro = () => screen.queryByTestId(T.marketsIntro);

describe('MarketsIntro', () => {
    it('says what shorting, lending and the risks are, linking to the approved texts', async () => {
        const { queryClient } = await renderAt('/markets');
        const strip = await screen.findByTestId(T.marketsIntro);
        expect(within(strip).getByRole('heading', { level: 2 }).textContent).toBe(copy('intro.heading'));
        const parts = within(strip)
            .getAllByRole('heading', { level: 3 })
            .map((h) => h.textContent);
        expect(parts).toEqual([copy('intro.short.title'), copy('intro.lend.title'), copy('intro.risks.title')]);
        for (const id of ['intro.short.body', 'intro.lend.body', 'intro.risks.body'] as const)
            expect(strip.textContent).toContain(copy(id));
        const href = (k: Parameters<typeof T.marketsIntroLink>[0]) =>
            screen.getByTestId(T.marketsIntroLink(k)).getAttribute('href');
        expect(href('how-it-works')).toBe('/how-it-works');
        expect(href('short-risks')).toBe('/how-it-works#short-risks');
        expect(href('lend-risks')).toBe('/how-it-works#lend-risks');
        // Hide's accessible name contains its visible word (WCAG 2.5.3).
        const hide = screen.getByTestId(T.marketsIntroDismiss);
        expect(hide.textContent).toBe('Hide');
        expect(hide.getAttribute('aria-label')).toBe('Hide the introduction');
        // It sits above the list.
        const table = await screen.findByTestId(T.marketsTable);
        expect(strip.compareDocumentPosition(table) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
        queryClient.clear();
    });

    it('remembers Hide in this browser, says where How it works is, and moves focus to the heading', async () => {
        const first = await renderAt('/markets');
        await act(async () => {
            fireEvent.click(await screen.findByTestId(T.marketsIntroDismiss));
        });
        expect(intro()).toBeNull();
        expect(globalThis.localStorage.getItem(INTRO_HIDDEN_KEY)).toBe('1');
        const note = [...document.querySelectorAll('[role="status"]')].find(
            (e) => e.textContent === copy('intro.dismissed.note'),
        );
        expect(note).toBeTruthy();
        expect(document.activeElement).toBe(screen.getByRole('heading', { level: 1, name: 'Markets' }));
        first.queryClient.clear();
        first.view?.unmount();
        // A new visit (a fresh session): still hidden, from storage.
        resetIntroSession();
        const again = await renderAt('/markets');
        await screen.findByTestId(T.marketsTable);
        expect(intro()).toBeNull();
        again.queryClient.clear();
    });

    it('renders and hides for the session when storage throws', async () => {
        vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
            throw new Error('SecurityError: storage blocked');
        });
        vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
            throw new Error('SecurityError: storage blocked');
        });
        const first = await renderAt('/markets');
        expect(await screen.findByTestId(T.marketsIntro)).toBeTruthy();
        await act(async () => {
            fireEvent.click(screen.getByTestId(T.marketsIntroDismiss));
        });
        expect(intro()).toBeNull();
        first.queryClient.clear();
        first.view?.unmount();
        // Back on the list in the same session: still hidden.
        const again = await renderAt('/markets');
        await screen.findByTestId(T.marketsTable);
        expect(intro()).toBeNull();
        again.queryClient.clear();
        again.view?.unmount();
        // A new session with the same blocked storage: shown again.
        resetIntroSession();
        const fresh = await renderAt('/markets');
        expect(await screen.findByTestId(T.marketsIntro)).toBeTruthy();
        fresh.queryClient.clear();
    });

    it('leads to How it works', async () => {
        const { queryClient } = await renderAt('/markets');
        await act(async () => {
            fireEvent.click(await screen.findByTestId(T.marketsIntroLink('how-it-works')));
        });
        await waitFor(() => expect(screen.getByTestId(T.page('how-it-works'))).toBeTruthy());
        queryClient.clear();
    });
});
