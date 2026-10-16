/**
 * The glossary control (FRONTEND.md §4.3, §8; the owner's thirteenth round, O-3 (b)): an ⓘ button
 * beside a jargon label that opens the term's definition from `config/glossary.ts`. It opens on a
 * tap, a click, or Enter / Space with focus on it — never on hover alone. Escape, its ✕ or a tap /
 * click outside closes it; Escape and ✕ return focus to the ⓘ, and Tab from inside moves on from
 * the ⓘ (the definition is not a trap). `aria-expanded` / `aria-controls` tie the two together.
 *
 * The definition is rendered in a portal (the open sheet or dialog it sits in, else `<body>`), at a
 * fixed position computed from the ⓘ (CSSOM: allowed under the CSP), so a table's or a scrolling
 * panel's overflow never clips it — it works the same in a table cell, a card and a sheet. On a
 * phone it is a bottom sheet over a scrim, like "Why this tier".
 *
 * `Term` is a label with its ⓘ when the glossary has that label; the shared label renderers use it.
 */
import {
    lazy,
    type ReactNode,
    Suspense,
    type SyntheticEvent,
    useCallback,
    useEffect,
    useId,
    useLayoutEffect,
    useRef,
    useState,
} from 'react';
import { createPortal } from 'react-dom';
import { GLOSSARY, GLOSSARY_UI, type GlossaryId, glossaryFor } from '../config/glossary.ts';
import { T } from '../config/testids.ts';
import { NavIcon } from '../shell/icons.tsx';
import { PHONE, useMedia } from '../styles/breakpoints.ts';

/** Space kept between the definition and the screen's edges (px). */
const EDGE = 8;

interface Place {
    readonly top: number;
    readonly left: number;
}

/** Below the ⓘ, left-aligned with it and kept on screen; above it when there is no room below. */
function placeFor(button: DOMRect, box: { width: number; height: number }): Place {
    const vw = document.documentElement.clientWidth;
    const vh = window.innerHeight;
    const left = Math.max(EDGE, Math.min(button.left - EDGE, vw - box.width - EDGE));
    const below = button.bottom + 6;
    const top = below + box.height <= vh - EDGE ? below : Math.max(EDGE, button.top - 6 - box.height);
    return { top: Math.round(top), left: Math.round(left) };
}

/** The layer the definition renders into: the open sheet or dialog holding the ⓘ, else `<body>`. */
function layerOf(el: HTMLElement | null): HTMLElement {
    return el?.closest<HTMLElement>('dialog[open], [data-sheet="open"]') ?? document.body;
}

/** Keeps an event inside the definition from the ⓘ's React ancestors. */
function stop(e: SyntheticEvent) {
    e.stopPropagation();
}

/** The definition's text: its own chunk, loaded with the first definition opened. */
const GlossaryDefinition = lazy(() => import('./GlossaryDefinition.tsx'));

function Definition({ id }: { id: GlossaryId }) {
    return (
        <p>
            <Suspense fallback={<span className="skeleton" aria-hidden="true" />}>
                <GlossaryDefinition id={id} />
            </Suspense>
        </p>
    );
}

export function GlossaryTip({ id, className }: { id: GlossaryId; className?: string }) {
    const [open, setOpen] = useState(false);
    const [place, setPlace] = useState<Place | null>(null);
    const panelId = useId();
    const titleId = useId();
    const button = useRef<HTMLButtonElement>(null);
    const panel = useRef<HTMLDivElement>(null);
    const phone = useMedia(PHONE);
    const entry = GLOSSARY[id];

    const close = useCallback((focusBack: boolean) => {
        setOpen(false);
        setPlace(null);
        if (focusBack) button.current?.focus({ preventScroll: true });
    }, []);

    // Position (desktop and tablet): after the definition has rendered, and again on scroll / resize.
    useLayoutEffect(() => {
        if (!open || phone) return;
        const update = () => {
            const b = button.current?.getBoundingClientRect();
            const p = panel.current;
            if (!b || !p) return;
            setPlace(placeFor(b, { width: p.offsetWidth, height: p.offsetHeight }));
        };
        update();
        window.addEventListener('scroll', update, true);
        window.addEventListener('resize', update);
        return () => {
            window.removeEventListener('scroll', update, true);
            window.removeEventListener('resize', update);
        };
    }, [open, phone]);

    useEffect(() => {
        if (!open) return;
        panel.current?.focus({ preventScroll: true });
        const onDown = (e: PointerEvent) => {
            const t = e.target as Node;
            if (panel.current?.contains(t) || button.current?.contains(t)) return;
            // The phone's scrim closes on its own click: closing on pointerdown would unmount it
            // before the tap's click, which would then land on (and open) whatever lies under it.
            if (t instanceof Element && t.closest('.glossary-scrim')) return;
            close(false);
        };
        document.addEventListener('pointerdown', onDown);
        return () => document.removeEventListener('pointerdown', onDown);
    }, [open, close]);

    const name = GLOSSARY_UI['glossary.trigger.aria'].replace('{term}', entry.term);
    return (
        <>
            <button
                ref={button}
                type="button"
                className={`glossary-tip hit${className ? ` ${className}` : ''}`}
                aria-label={name}
                aria-expanded={open}
                aria-controls={open ? panelId : undefined}
                data-testid={T.glossaryTip(id)}
                onClick={(e) => {
                    // The ⓘ sits in clickable rows and labels: it is its own control.
                    e.stopPropagation();
                    if (open) close(false);
                    else setOpen(true);
                }}
                onKeyDown={(e) => {
                    // Enter / Space are the button's own; keep them from a row's handler.
                    if (e.key === 'Enter' || e.key === ' ') e.stopPropagation();
                }}
            >
                <NavIcon name="info" className="glossary-tip-icon" />
            </button>
            {open &&
                createPortal(
                    <>
                        {phone && (
                            <span
                                className="glossary-scrim"
                                aria-hidden="true"
                                data-sheet-keep=""
                                onPointerDown={stop}
                                onClick={(e) => {
                                    e.stopPropagation();
                                    close(true);
                                }}
                            />
                        )}
                        <div
                            ref={panel}
                            id={panelId}
                            role="dialog"
                            aria-labelledby={titleId}
                            tabIndex={-1}
                            className="glossary-popover"
                            data-testid={T.glossaryPopover(id)}
                            data-placed={phone || place ? '' : undefined}
                            data-sheet-keep=""
                            style={!phone && place ? { top: place.top, left: place.left } : undefined}
                            // A portal's events still bubble through the React tree: keep them from
                            // the ⓘ's ancestors (a table row that opens on a click, Enter or an
                            // arrow key; a clickable card or label).
                            onClick={stop}
                            onPointerDown={stop}
                            onKeyDown={(e) => {
                                if (e.key !== 'Tab') e.stopPropagation();
                                if (e.key === 'Escape') {
                                    // Ours alone: not the sheet's or the dialog's (both read it too).
                                    e.preventDefault();
                                    e.stopPropagation();
                                    close(true);
                                } else if (e.key === 'Tab' && (e.shiftKey || e.target !== panel.current)) {
                                    // Leaving it (Shift+Tab, or Tab past its ✕): focus goes back to
                                    // the ⓘ, and a forward Tab then moves on from there.
                                    if (e.shiftKey) e.preventDefault();
                                    close(true);
                                }
                            }}
                        >
                            <div className="glossary-head">
                                <h2 id={titleId} className="glossary-title">
                                    {entry.term.charAt(0).toUpperCase() + entry.term.slice(1)}
                                </h2>
                                <button
                                    type="button"
                                    className="btn glossary-close"
                                    data-variant="ghost"
                                    aria-label={GLOSSARY_UI['glossary.popover.close']}
                                    data-testid={T.glossaryClose}
                                    onClick={(e) => {
                                        e.stopPropagation();
                                        close(true);
                                    }}
                                >
                                    <NavIcon name="close" className="icon" />
                                </button>
                            </div>
                            <Definition id={id} />
                        </div>
                    </>,
                    layerOf(button.current),
                )}
        </>
    );
}

/** A label's last word and its ⓘ never part: a label that wraps keeps the ⓘ on its last line. */
function LastWordWith({ text, tip }: { text: string; tip: ReactNode }) {
    const at = text.trimEnd().lastIndexOf(' ') + 1;
    return (
        <>
            {text.slice(0, at)}
            <span className="nowrap">
                {text.slice(at)}
                {tip}
            </span>
        </>
    );
}

/**
 * A label with its glossary ⓘ: `id`, else the entry the label text attaches to; plain text when
 * the glossary has none. The ⓘ is outside the label's own text (a screen reader reads the label,
 * then the button "What is …?").
 */
export function Term({ children, id }: { children: ReactNode; id?: GlossaryId }) {
    const term = id ?? glossaryFor(children);
    if (!term) return <>{children}</>;
    const tip = <GlossaryTip id={term} />;
    return (
        <span className="term">
            {typeof children === 'string' ? (
                <LastWordWith text={children} tip={tip} />
            ) : (
                <>
                    {children}
                    {tip}
                </>
            )}
        </span>
    );
}
