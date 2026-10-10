/**
 * The closed breakpoint set (FRONTEND.md §7.6; the comment at the top of `tokens.css`), for the few
 * places JavaScript needs a width or an input type: a sheet's inert / focus handling, the tablist's
 * `scrollIntoView`, the mobile wallet links. Layout itself is CSS only (one DOM: nothing renders
 * twice per width). `breakpoints.test.ts` checks every `@media` / `@container` width in `src` against
 * {@link MEDIA_WIDTHS} and {@link CONTAINER_WIDTHS}.
 */
import { useSyncExternalStore } from 'react';

/** Phone: under 640 px (app shell, cards, full-screen sheets, action bar). */
export const PHONE = '(max-width: 639.98px)';
/** Tablet and phone: under 1024 px (one column, sheets, action bar). */
export const COMPACT = '(max-width: 1023.98px)';
/** A finger: 44 px targets, 16 px inputs. */
export const COARSE = '(pointer: coarse)';
/** A pointer that hovers (hover styles only then). */
export const HOVER = '(hover: hover)';

/** Every width an `@media` condition may name: `max-width` (desktop-first) and `min-width` forms. */
export const MEDIA_WIDTHS = {
    max: ['399.98px', '639.98px', '1023.98px', '1179.98px', '1279.98px'],
    min: ['640px', '1024px', '1280px', '1680px'],
} as const;

/** Container queries: the name, and the `max-width`s it may use (no other element is a container). */
export const CONTAINER_WIDTHS = {
    tbl: ['959.98px', '559.98px'],
    dialog: ['479.98px'],
} as const;

/** The only selectors that may set `container-type` / `container-name` / `container`. */
export const CONTAINER_SELECTORS = ['.tbl-wrap', '.modal-body'] as const;

/** Whether `query` matches now; false without `matchMedia` (SSR, old jsdom): the desktop layout. */
export function matches(query: string): boolean {
    return typeof window !== 'undefined' && typeof window.matchMedia === 'function'
        ? window.matchMedia(query).matches
        : false;
}

const subscribers = new Map<string, (onChange: () => void) => () => void>();

/** One stable subscribe function per query (`useSyncExternalStore` resubscribes on a new one). */
function subscribe(query: string) {
    let s = subscribers.get(query);
    if (s) return s;
    s = (onChange: () => void) => {
        if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return () => {};
        const mql = window.matchMedia(query);
        mql.addEventListener?.('change', onChange);
        return () => mql.removeEventListener?.('change', onChange);
    };
    subscribers.set(query, s);
    return s;
}

/** `matches(query)`, re-rendering when it flips (a rotation, a resized window). */
export function useMedia(query: string): boolean {
    return useSyncExternalStore(
        subscribe(query),
        () => matches(query),
        () => false,
    );
}

interface NavigatorUAData {
    readonly mobile?: boolean;
}

/**
 * A phone or tablet (FRONTEND.md §5.4 "mobile wallets"): where no extension wallet can exist, so a
 * page without a wallet offers to open itself in a wallet app's browser. Client hints first; then
 * the user agent (iPadOS reports a Macintosh with touch points); then a coarse, non-hovering
 * pointer.
 */
export function isMobileDevice(): boolean {
    if (typeof navigator === 'undefined') return false;
    const uaData = (navigator as Navigator & { userAgentData?: NavigatorUAData }).userAgentData;
    if (uaData?.mobile) return true;
    const ua = navigator.userAgent ?? '';
    if (/Android|iPhone|iPod/.test(ua)) return true;
    if (/Macintosh/.test(ua) && (navigator.maxTouchPoints ?? 0) > 1) return true;
    return matches('(pointer: coarse) and (hover: none)');
}
