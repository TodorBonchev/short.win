/**
 * Reveal (FRONTEND.md §7.6): a truncated value (a mint, an address) as a button that opens a
 * popover with the full value in mono, Copy and — when given — an Explorer link. Declarative
 * (`popover` / `popovertarget`; no `showPopover()` call), so the browser handles light dismiss and
 * Escape. A small card near the top on a desktop, a bottom sheet on a phone. `title` stays for
 * mouse users.
 */
import { type ReactNode, useEffect, useId, useRef, useState } from 'react';
import { T } from '../config/testids.ts';

export function Reveal({
    value,
    children,
    kind,
    label,
    explorer,
    className,
}: {
    /** The full value. */
    value: string;
    /** What the button shows (the truncated value). */
    children: ReactNode;
    /** The `reveal-<kind>` test id (e.g. `mint`, `address`, `wallet`). */
    kind: string;
    /** What the value is ("Mint", "Address"): the popover's heading and the button's name. */
    label: string;
    explorer?: string | null;
    className?: string;
}) {
    const id = useId();
    const pop = useRef<HTMLSpanElement>(null);
    const [open, setOpen] = useState(false);
    const [copied, setCopied] = useState(false);
    useEffect(() => {
        const el = pop.current;
        if (!el) return;
        const onToggle = (e: Event) => setOpen((e as Event & { newState?: string }).newState === 'open');
        el.addEventListener('toggle', onToggle);
        return () => el.removeEventListener('toggle', onToggle);
    }, []);
    return (
        <>
            <button
                type="button"
                className={`reveal ${className ?? ''}`.trim()}
                popoverTarget={id}
                aria-expanded={open}
                aria-controls={id}
                title={value}
                data-value={value}
            >
                {children}
                <span className="visually-hidden">, show the full {label.toLowerCase()}</span>
            </button>
            <span ref={pop} id={id} popover="auto" className="reveal-pop" data-testid={T.reveal(kind)}>
                <span className="menu-label">{label}</span>
                <code className="reveal-value">{value}</code>
                <span className="reveal-actions">
                    <button
                        type="button"
                        className="btn"
                        onClick={() => {
                            void navigator.clipboard?.writeText(value).then(() => {
                                setCopied(true);
                                setTimeout(() => setCopied(false), 1_200);
                            });
                        }}
                    >
                        {copied ? 'Copied' : 'Copy'}
                    </button>
                    {explorer && (
                        <a className="btn" href={explorer} target="_blank" rel="noopener noreferrer">
                            Explorer ↗
                        </a>
                    )}
                    <button
                        type="button"
                        className="btn"
                        data-variant="ghost"
                        popoverTarget={id}
                        popoverTargetAction="hide"
                    >
                        Close
                    </button>
                </span>
            </span>
        </>
    );
}
