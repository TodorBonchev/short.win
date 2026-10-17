/**
 * The icon registry (FRONTEND.md §7.6; design pass P2): the phone tab bar's icons, the small info mark and the
 * glyphs every control shares (close, chevron, arrow, sort, copy, check, external, shield, plus). 20 px inline SVG,
 * stroked 1.6 in `currentColor`, presentation attributes only (CSP `style-src 'self'`: no style attributes),
 * hidden from assistive technology (the control's text or aria-label names it). The default class `.nav-icon`
 * is hidden from 640 px up: every use outside the tab bar passes `.icon` (16 px), `.icon-sm` (14 px) or its
 * own class. The brand mark is `Logo.tsx`, not an icon; other projects' marks are `BrandIcon` below.
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

export type BrandIconName = 'github' | 'discord' | 'x';

/**
 * GitHub's, Discord's and X's marks, filled in `currentColor` on a 24-unit grid (the paths of Simple
 * Icons, CC0 1.0): the links to the source, the community and the X account (`config/links.ts`). Sized
 * by the same classes as `NavIcon`; hidden from assistive technology, so the link names itself.
 */
const BRAND_PATHS: Record<BrandIconName, string> = {
    github: 'M12 .297c-6.63 0-12 5.373-12 12 0 5.303 3.438 9.8 8.205 11.385.6.113.82-.258.82-.577 0-.285-.01-1.04-.015-2.04-3.338.724-4.042-1.61-4.042-1.61C4.422 18.07 3.633 17.7 3.633 17.7c-1.087-.744.084-.729.084-.729 1.205.084 1.838 1.236 1.838 1.236 1.07 1.835 2.809 1.305 3.495.998.108-.776.417-1.305.76-1.605-2.665-.3-5.466-1.332-5.466-5.93 0-1.31.465-2.38 1.235-3.22-.135-.303-.54-1.523.105-3.176 0 0 1.005-.322 3.3 1.23.96-.267 1.98-.399 3-.405 1.02.006 2.04.138 3 .405 2.28-1.552 3.285-1.23 3.285-1.23.645 1.653.24 2.873.12 3.176.765.84 1.23 1.91 1.23 3.22 0 4.61-2.805 5.625-5.475 5.92.42.36.81 1.096.81 2.22 0 1.606-.015 2.896-.015 3.286 0 .315.21.69.825.57C20.565 22.092 24 17.592 24 12.297c0-6.627-5.373-12-12-12',
    discord:
        'M20.317 4.37a19.791 19.791 0 0 0-4.885-1.515.074.074 0 0 0-.079.037c-.21.375-.444.865-.608 1.25a18.27 18.27 0 0 0-5.487 0 12.64 12.64 0 0 0-.617-1.25.077.077 0 0 0-.079-.037A19.736 19.736 0 0 0 3.677 4.37a.07.07 0 0 0-.032.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 0 0 .031.057 19.9 19.9 0 0 0 5.993 3.03.078.078 0 0 0 .084-.028 14.09 14.09 0 0 0 1.226-1.994.076.076 0 0 0-.041-.106 13.107 13.107 0 0 1-1.872-.892.077.077 0 0 1-.008-.128 10.2 10.2 0 0 0 .372-.292.074.074 0 0 1 .077-.01c3.928 1.793 8.18 1.793 12.062 0a.074.074 0 0 1 .078.01c.12.098.246.198.373.292a.077.077 0 0 1-.006.127 12.299 12.299 0 0 1-1.873.892.077.077 0 0 0-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .084.028 19.839 19.839 0 0 0 6.002-3.03.077.077 0 0 0 .032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 0 0-.031-.03zM8.02 15.33c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.956-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.956 2.418-2.157 2.418zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.955-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.946 2.418-2.157 2.418z',
    x: 'M18.901 1.153h3.68l-8.04 9.19L24 22.846h-7.406l-5.8-7.584-6.638 7.584H.474l8.6-9.83L0 1.154h7.594l5.243 6.932zM17.61 20.644h2.039L6.486 3.24H4.298z',
};

export function BrandIcon({ name, className = 'icon' }: { name: BrandIconName; className?: string }) {
    return (
        <svg
            className={className}
            width="20"
            height="20"
            viewBox="0 0 24 24"
            fill="currentColor"
            aria-hidden="true"
            focusable="false"
        >
            <path d={BRAND_PATHS[name]} />
        </svg>
    );
}
