/**
 * Sheets (FRONTEND.md §7.6): an element that is part of the page at desktop widths (the trade
 * panel, a position's Manage panel, the header search) becomes a modal sheet under a breakpoint —
 * full screen on a phone, a right-side sheet on a tablet — without moving in the DOM (one DOM: its
 * test ids stay unique, its state survives a rotation).
 *
 * `useSheetMode` gives it the modal semantics a native `<dialog>` would: `role="dialog"`,
 * `aria-modal`, everything else on the page `inert` (except `[data-sheet-keep]` — the toast region,
 * a scrim — and an open `<dialog>`), the page held still (`html[data-sheet-open]`), focus moved in
 * and returned to the opener, Escape to close. It does nothing while the query does not match, and
 * undoes everything when the sheet closes or the query flips (a rotation across the breakpoint).
 * CSS does the rest (`.sheet-host[data-sheet="open"]`, components.css).
 */
import { type ReactNode, type RefObject, useEffect, useRef } from 'react';
import { NavIcon } from '../shell/icons.tsx';
import { COMPACT, useMedia } from '../styles/breakpoints.ts';

export interface SheetModeOptions {
    /** The sheet is wanted (e.g. `?sheet=trade`); it only becomes one while `query` matches. */
    readonly open: boolean;
    readonly onClose: () => void;
    /** The id of the sheet's title (its accessible name; focused on open unless `initialFocus`). */
    readonly labelledBy?: string;
    /** Default: compact (under 1024 px). */
    readonly query?: string;
    /** The element to focus on open (default: the title, else the sheet). */
    readonly initialFocus?: () => HTMLElement | null;
}

/** Sheets open now (nested or overlapping ones share `html[data-sheet-open]`). */
let openSheets = 0;

/** Whether the element is an open `<dialog>` or keeps a sheet's background usable. */
function kept(el: Element): boolean {
    return el.matches('[data-sheet-keep], dialog[open], script') || el.querySelector('dialog[open]') !== null;
}

/** Turns `ref` into a modal sheet while `open` and `query` match; returns whether it is one. */
export function useSheetMode(
    ref: RefObject<HTMLElement | null>,
    { open, onClose, labelledBy, query = COMPACT, initialFocus }: SheetModeOptions,
): boolean {
    const narrow = useMedia(query);
    const active = open && narrow;
    const close = useRef(onClose);
    close.current = onClose;
    const focusFirst = useRef(initialFocus);
    focusFirst.current = initialFocus;

    useEffect(() => {
        const el = ref.current;
        if (!active || !el) return;
        const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
        const before = {
            role: el.getAttribute('role'),
            modal: el.getAttribute('aria-modal'),
            labelledBy: el.getAttribute('aria-labelledby'),
        };
        el.setAttribute('data-sheet', 'open');
        el.setAttribute('role', 'dialog');
        el.setAttribute('aria-modal', 'true');
        if (labelledBy) el.setAttribute('aria-labelledby', labelledBy);

        // Everything beside the path from the sheet up to <body> goes inert.
        const madeInert: HTMLElement[] = [];
        for (let node: HTMLElement | null = el; node && node !== document.body; node = node.parentElement) {
            const parent: HTMLElement | null = node.parentElement;
            if (!parent) break;
            for (const sib of Array.from(parent.children)) {
                if (sib === node || !(sib instanceof HTMLElement) || sib.hasAttribute('inert') || kept(sib)) continue;
                sib.setAttribute('inert', '');
                madeInert.push(sib);
            }
        }
        openSheets += 1;
        document.documentElement.setAttribute('data-sheet-open', '');

        const target =
            focusFirst.current?.() ?? (labelledBy ? document.getElementById(labelledBy) : null) ?? (el as HTMLElement);
        if (target === el && !el.hasAttribute('tabindex')) el.setAttribute('tabindex', '-1');
        target.focus({ preventScroll: true });

        const onKey = (e: KeyboardEvent) => {
            if (e.key !== 'Escape' || e.defaultPrevented) return;
            // A review dialog above the sheet handles its own Escape.
            if (document.querySelector('dialog[open]')) return;
            e.preventDefault();
            close.current();
        };
        document.addEventListener('keydown', onKey);

        return () => {
            document.removeEventListener('keydown', onKey);
            for (const s of madeInert) s.removeAttribute('inert');
            openSheets = Math.max(0, openSheets - 1);
            if (openSheets === 0) document.documentElement.removeAttribute('data-sheet-open');
            el.removeAttribute('data-sheet');
            for (const [attr, v] of [
                ['role', before.role],
                ['aria-modal', before.modal],
                ['aria-labelledby', before.labelledBy],
            ] as const) {
                if (v === null) el.removeAttribute(attr);
                else el.setAttribute(attr, v);
            }
            // Back to the control that opened it when focus was left inside the sheet — or was lost
            // with it: an unmounted sheet (a popup closed by Escape or by a pick) has already taken
            // focus to <body> when this runs. Without a usable opener (a deep-linked sheet), the
            // page's main region takes it, never <body>. `inert` is gone above, so it can.
            const a = document.activeElement;
            const focusWasInside = !a || a === document.body || !el.isConnected || el.contains(a);
            if (focusWasInside) {
                const back = opener?.isConnected && opener !== document.body ? opener : document.getElementById('main');
                if (back && document.activeElement !== back) back.focus({ preventScroll: true });
            }
        };
    }, [active, ref, labelledBy]);

    return active;
}

/** A sheet's title row with its 44 px close button; hidden from 1024 px up (the panel's own head shows). */
export function SheetHead({
    title,
    titleId,
    onClose,
    testId,
}: {
    title: ReactNode;
    titleId: string;
    onClose: () => void;
    testId?: string;
}) {
    return (
        <header className="sheet-head">
            <h2 id={titleId} className="sheet-title" tabIndex={-1}>
                {title}
            </h2>
            <button
                type="button"
                className="btn sheet-close"
                data-variant="ghost"
                aria-label="Close"
                data-testid={testId}
                onClick={onClose}
            >
                <NavIcon name="close" className="icon" />
            </button>
        </header>
    );
}

/** The dimmed page behind a tablet's side sheet; a tap closes the sheet. Hidden from 1024 px up. */
export function Scrim({ onClick }: { onClick: () => void }) {
    // The keyboard closes the sheet with Escape or its close button; the scrim is for pointers only.
    return <div className="scrim" aria-hidden="true" data-sheet-keep="" onClick={onClick} />;
}
