/**
 * The page's banners, as a panel inside the page sees them (the owner's thirteenth round, O-2;
 * audit finding D-23): which banners the page shows now, whether the panel is in a sheet, and a way
 * to take the user to one. The market page provides it; the Short panel reads it to replace a
 * disclosure the page's banner already shows in full with a link to that banner — and keeps the
 * disclosure when there is no banner to link to (FRONTEND.md §3.4).
 */
import { createContext, useContext } from 'react';

export interface PageBanners {
    /** The banner keys the page renders now (`MarketFlagKey`s, `T.marketBanner(key)`). */
    readonly shown: ReadonlySet<string>;
    /** The panel is a sheet over the page now (under 1,024 px): the banners are behind it. */
    readonly inSheet: boolean;
    /** Scrolls to the banner and focuses it; from a sheet, closes the sheet first. */
    readonly focus: (key: string) => void;
}

export const PageBannersContext = createContext<PageBanners | null>(null);

/** The page's banners, or `null` outside a page that provides them. */
export function usePageBanners(): PageBanners | null {
    return useContext(PageBannersContext);
}

/** Brings the page's banner into view (smoothly unless reduced motion) and focuses it. */
export function focusBanner(testId: string): boolean {
    const el = document.querySelector<HTMLElement>(`[data-testid="${testId}"]`);
    if (!el) return false;
    const reduced = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? true;
    // `nearest`: no scroll when it is already in view, and never more than it takes (a `start`
    // alignment tucked the market header's controls under the sticky app header).
    el.scrollIntoView?.({ block: 'nearest', behavior: reduced ? 'auto' : 'smooth' });
    el.focus({ preventScroll: true });
    return true;
}
