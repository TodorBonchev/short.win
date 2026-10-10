/**
 * The phone tab bar's icons, and the small info mark (FRONTEND.md §7.6): 20 px inline SVG, stroked in `currentColor`,
 * presentation attributes only (CSP `style-src 'self'`: no style attributes), hidden from assistive
 * technology (the link text names the destination). `.nav-icon` is hidden from 640 px up.
 */
import type { ReactNode } from 'react';

export type NavIconName = 'markets' | 'positions' | 'lend' | 'status' | 'more' | 'search' | 'info';

const PATHS: Record<NavIconName, ReactNode> = {
    // A list of rows (the markets table).
    markets: (
        <>
            <rect x="3" y="4" width="14" height="12" rx="1.5" />
            <path d="M3 8h14M3 12h14M8 4v12" />
        </>
    ),
    // A falling line (a short).
    positions: (
        <>
            <path d="M3 5l5 5 3-3 6 6" />
            <path d="M13 13h4v-4" />
        </>
    ),
    // Two arrows (deposit, withdraw).
    lend: (
        <>
            <path d="M7 3v13M4 6l3-3 3 3" />
            <path d="M13 17V4M10 14l3 3 3-3" />
        </>
    ),
    // A pulse.
    status: <path d="M2 10h4l2-5 4 10 2-5h4" />,
    more: (
        <>
            <circle cx="4.5" cy="10" r="1.25" />
            <circle cx="10" cy="10" r="1.25" />
            <circle cx="15.5" cy="10" r="1.25" />
        </>
    ),
    search: (
        <>
            <circle cx="8.5" cy="8.5" r="5" />
            <path d="M12.5 12.5L17 17" />
        </>
    ),
    // An "i" in a circle (more about this; the tier badge).
    info: (
        <>
            <circle cx="10" cy="10" r="8" />
            <path d="M10 9v5.5" strokeWidth="2" />
            <path d="M10 5.9v.2" strokeWidth="2.4" />
        </>
    ),
};

export function NavIcon({ name, className = 'nav-icon' }: { name: NavIconName; className?: string }) {
    return (
        <svg
            className={className}
            width="20"
            height="20"
            viewBox="0 0 20 20"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.6"
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden="true"
            focusable="false"
        >
            {PATHS[name]}
        </svg>
    );
}
