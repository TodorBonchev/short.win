/**
 * How it works (FRONTEND.md §3.11; the owner's thirteenth round, O-3 (c)): the page in its order,
 * its numbers from the live config (a skeleton until read, never a word), the approved disclosures
 * verbatim without a checkbox (Tier C's terms in `borrower.liquidation`), an anchor focusing its
 * section, a link opening a collapsed disclosure, and the page reached from the status bar and the
 * phone's More sheet.
 */
import { act, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { COPY, copy } from '../../config/copy.ts';
import { renderDisclosure } from '../../config/disclosures.ts';
import { borrowerDisclosureVars, lenderRiskVars } from '../../config/disclosureVars.ts';
import { T } from '../../config/testids.ts';
import * as accounts from '../../rpc/accounts.ts';
import { setMedia } from '../../test/media.ts';
import { exists, fullGlobalConfig, MINT_A, missing } from '../markets/test/fixtures.ts';
import { mockIndexer, renderAt } from '../markets/test/harness.tsx';
import { guideVars } from './HowItWorksPage.tsx';

vi.mock('../../rpc/accounts.ts', async (orig) => ({
    ...(await orig<typeof import('../../rpc/accounts.ts')>()),
    readGlobalConfig: vi.fn(),
}));

const CONFIG = fullGlobalConfig();

beforeEach(() => {
    vi.mocked(accounts.readGlobalConfig).mockResolvedValue(exists(MINT_A, CONFIG));
    mockIndexer({ '/v1/markets': { data: [], next_cursor: null } });
});

afterEach(() => {
    vi.unstubAllGlobals();
});

const page = () => screen.getByTestId(T.page('how-it-works'));

describe('HowItWorksPage', () => {
    it('explains shorting, lending, the risks and the fees with the live numbers', async () => {
        const { queryClient } = await renderAt('/how-it-works');
        await waitFor(() => expect(page().querySelector('.skeleton')).toBeNull());
        expect(within(page()).getByRole('heading', { level: 1 }).textContent).toBe('How it works');
        const vars = guideVars(CONFIG);
        expect(page().textContent).toContain(copy('howItWorks.short.p3', vars));
        expect(page().textContent).toContain(copy('howItWorks.fees.open', vars));
        expect(page().textContent).toContain(copy('howItWorks.fees.penalty', vars));
        expect(page().textContent).toContain('Open fee: 0.1% of the position’s notional');
        // Sections in order, each a focusable heading.
        const sections = [...page().querySelectorAll<HTMLElement>('[data-testid^="how-it-works-section-"]')].map(
            (h) => h.id,
        );
        expect(sections).toEqual([
            'shorting',
            'lending',
            'what-can-go-wrong',
            'short-risks',
            'lend-risks',
            'every-market',
            'fees',
            'before-you-sign',
            'read-more',
        ]);
        // The approved texts verbatim, with Tier C's live terms; nothing to tick on this page.
        const liq = screen.getByTestId(T.disclosure('borrower.liquidation'));
        const tierC = CONFIG.tiers[2];
        if (!tierC) throw new Error('fixture');
        for (const p of renderDisclosure('borrower.liquidation', borrowerDisclosureVars(tierC, CONFIG.params)))
            expect(liq.textContent).toContain(p);
        expect(screen.queryByTestId(T.disclosureAck('borrower.liquidation'))).toBeNull();
        expect(screen.queryByTestId(T.disclosureAck('lender.solRepayment'))).toBeNull();
        const risks = screen.getByTestId(T.disclosure('lender.risks'));
        for (const p of renderDisclosure('lender.risks', lenderRiskVars(CONFIG.params)))
            expect(risks.textContent).toContain(p);
        expect(page().querySelectorAll('input[type="checkbox"]')).toHaveLength(0);
        queryClient.clear();
    });

    it('shows a skeleton for each number until the config is read, never a word', async () => {
        vi.mocked(accounts.readGlobalConfig).mockReturnValue(new Promise(() => {}));
        const { queryClient } = await renderAt('/how-it-works');
        expect(page().querySelectorAll('.skeleton').length).toBeGreaterThan(5);
        expect(page().textContent).not.toMatch(/\{\w+\}/);
        // The text around them is there.
        expect(page().textContent).toContain('Open fee: ');
        queryClient.clear();
    });

    it('says when the config cannot be read (or has no account), with Retry: no skeleton that never resolves', async () => {
        vi.mocked(accounts.readGlobalConfig).mockRejectedValue(new Error('rpc down'));
        const { queryClient } = await renderAt('/how-it-works');
        const alert = await screen.findByTestId(T.howItWorksConfigError, {}, { timeout: 5_000 });
        expect(within(alert).getByRole('alert').textContent).toContain(copy('howItWorks.configError'));
        expect(page().querySelector('.skeleton')).toBeNull();
        expect(page().querySelector('[aria-busy="true"]')).toBeNull();
        // The borrower disclosures that need the numbers say so in their place.
        expect(page().querySelectorAll('.guide-missing').length).toBeGreaterThan(0);
        // Retry reads again; the numbers then fill in.
        vi.mocked(accounts.readGlobalConfig).mockResolvedValue(exists(MINT_A, CONFIG));
        fireEvent.click(within(alert).getByRole('button', { name: 'Retry' }));
        await waitFor(() => expect(screen.queryByTestId(T.howItWorksConfigError)).toBeNull());
        expect(page().textContent).toContain(copy('howItWorks.fees.open', guideVars(CONFIG)));
        queryClient.clear();
    });

    it('treats a missing config account like a failed read', async () => {
        vi.mocked(accounts.readGlobalConfig).mockResolvedValue(missing(MINT_A));
        const { queryClient } = await renderAt('/how-it-works');
        await screen.findByTestId(T.howItWorksConfigError, {}, { timeout: 5_000 });
        expect(page().querySelector('.skeleton')).toBeNull();
        queryClient.clear();
    });

    it('focuses the section an anchor names, and opens a collapsed disclosure a link targets', async () => {
        const { queryClient } = await renderAt('/how-it-works#lend-risks');
        await waitFor(() => expect(document.activeElement?.id).toBe('lend-risks'));
        const limits = document.querySelector<HTMLDetailsElement>('#lender-limits details');
        expect(limits?.open).toBe(false);
        await act(async () => {
            fireEvent.click(screen.getByRole('link', { name: 'for lenders' }));
        });
        expect(limits?.open).toBe(true);
        expect(document.activeElement).toBe(limits?.querySelector('summary'));
        queryClient.clear();
    });

    it('is reached from the status bar and from the phone’s More sheet', async () => {
        const one = await renderAt('/markets');
        const link = screen.getByTestId(T.nav('how-it-works'));
        expect(link.textContent).toBe(COPY['nav.howItWorks'].text);
        await act(async () => {
            fireEvent.click(link);
        });
        await waitFor(() => expect(page()).toBeTruthy());
        one.queryClient.clear();
        one.view?.unmount();

        setMedia({ phone: true, coarse: true });
        const two = await renderAt('/markets');
        await act(async () => {
            fireEvent.click(screen.getByTestId(T.navMore));
        });
        const row = await screen.findByTestId(T.moreHowItWorks);
        expect(row.textContent).toContain(copy('more.howItWorks.sub'));
        await act(async () => {
            fireEvent.click(row);
        });
        await waitFor(() => expect(page()).toBeTruthy());
        // On a phone More stands for the page (its tab is current).
        expect(screen.getByTestId(T.navMore).getAttribute('aria-current')).toBe('true');
        two.queryClient.clear();
    });
});
