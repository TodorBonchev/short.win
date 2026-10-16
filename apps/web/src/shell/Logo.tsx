/**
 * The brand mark (FRONTEND.md §7): the W of "Win" drawn as a price chart — a full-height fall and a
 * small bounce in the text colour (the short), joined through a low, sharp apex to a check in the
 * accent gold (the win). Inline SVG, presentation attributes only (CSP `style-src 'self'`); the fall
 * half follows `currentColor` (`.brand`'s text colour), the win half takes `fill: var(--accent)` from
 * `.brand-mark-win` (components.css). `public/favicon.svg` and the app icons are the same W on its tile.
 * Hidden from assistive technology: the link's `aria-label` names the destination.
 */
export function BrandMark({ size = 24 }: { size?: number }) {
    return (
        <svg className="brand-mark" width={size} height={size} viewBox="0 0 24 24" aria-hidden="true" focusable="false">
            <path fill="currentColor" d="M1 4L4.75 20L8.44 20L12 13.53L12.4 14.25L12.4 7.2L12 6.47L7.14 15.3L4.49 4Z" />
            <path className="brand-mark-win" d="M12 13.53L15.56 20L19.25 20L23 4L19.51 4L16.86 15.3L12 6.47Z" />
        </svg>
    );
}
