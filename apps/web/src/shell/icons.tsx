/**
 * The icon registry (FRONTEND.md §7.6; design pass P2): the phone tab bar's icons, the small info mark and the
 * glyphs every control shares (close, chevron, arrow, sort, copy, check, external, shield, plus). 20 px inline SVG,
 * stroked 1.6 in `currentColor`, presentation attributes only (CSP `style-src 'self'`: no style attributes),
 * hidden from assistive technology (the control's text or aria-label names it). The default class `.nav-icon`
 * is hidden from 640 px up: every use outside the tab bar passes `.icon` (16 px), `.icon-sm` (14 px) or its
 * own class. The brand mark is `Logo.tsx`, not an icon.
 */
import type { ReactNode } from 'react';

export type NavIconName =
    | 'markets'
    | 'positions'
    | 'lend'
    | 'status'
    | 'more'
    | 'search'
    | 'info'
    | 'close'
    | 'chevron'
    | 'arrow'
    | 'sort'
    | 'copy'
    | 'check'
    | 'external'
    | 'shield'
    | 'plus';

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
    // A cross (close, dismiss).
    close: <path d="M5 5l10 10M15 5L5 15" />,
    // Points down; rotated for up (a folded list) and right (a row that opens a page).
    chevron: <path d="M5 7.5l5 5 5-5" />,
    // Points down (the card sort bar's order: descending; turned for ascending) — an arrow, not a second chevron
    // beside the sort select's own (design pass P11).
    arrow: <path d="M10 4v12M5 11l5 5 5-5" />,
    // Both ways (a sortable column that is not the sort).
    sort: <path d="M6 8l4-4 4 4M6 12l4 4 4-4" />,
    // Two sheets (copy to the clipboard).
    copy: (
        <>
            <rect x="7" y="7" width="9" height="9" rx="1.5" />
            <path d="M4 13V5.5A1.5 1.5 0 0 1 5.5 4H13" />
        </>
    ),
    // A tick (done, copied).
    check: <path d="M4 10.5l4 4 8-8" />,
    // An arrow out of a box (opens elsewhere: the explorer, a wallet app).
    external: <path d="M8 4h8v8M16 4l-9 9M6 7H4v9h9v-2" />,
    // The security policy.
    shield: <path d="M10 2.5l6 2.5v5c0 3.5-2.6 6.2-6 7.5-3.4-1.3-6-4-6-7.5V5z" />,
    // Create.
    plus: <path d="M10 4v12M4 10h12" />,
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
