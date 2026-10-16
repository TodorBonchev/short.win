/**
 * Accessible tabs (WAI-ARIA tabs pattern: arrow keys move, Home / End jump). A tablist wider than
 * its space scrolls sideways (FRONTEND.md §7.6): `data-overflow` (`start` | `end` | `both`) fades
 * the edge where more tabs are out of view, and the selected tab is scrolled into view. `variant`
 * `sub` is the sub-level look (a segmented row inside a tabpanel: Deposit | Withdraw | Claim).
 */
import { type KeyboardEvent, useEffect, useRef } from 'react';

export interface TabDef<K extends string> {
    readonly key: K;
    readonly label: string;
    readonly testId?: string;
    readonly tone?: 'short';
}

export function Tabs<K extends string>({
    tabs,
    value,
    onChange,
    label,
    idPrefix,
    variant,
}: {
    tabs: readonly TabDef<K>[];
    value: K;
    onChange: (k: K) => void;
    label: string;
    idPrefix: string;
    variant?: 'sub';
}) {
    const list = useRef<HTMLDivElement>(null);
    // The fade at the edge(s) where tabs are out of view (no ResizeObserver in jsdom: no fade).
    useEffect(() => {
        const el = list.current;
        if (!el || typeof ResizeObserver === 'undefined') return;
        const update = () => {
            const start = el.scrollLeft > 4;
            const end = el.scrollWidth - el.clientWidth - el.scrollLeft > 4;
            if (start || end) el.setAttribute('data-overflow', start && end ? 'both' : start ? 'start' : 'end');
            else el.removeAttribute('data-overflow');
        };
        const ro = new ResizeObserver(update);
        ro.observe(el);
        el.addEventListener('scroll', update, { passive: true });
        update();
        return () => {
            ro.disconnect();
            el.removeEventListener('scroll', update);
        };
    }, []);
    // The selected tab stays in view sideways (a deep link to the last tab, a key press). Only the
    // tablist scrolls — never the page (a scrollIntoView on load would move the document).
    // biome-ignore lint/correctness/useExhaustiveDependencies: re-runs when the selection changes (read from the DOM)
    useEffect(() => {
        const el = list.current;
        const sel = el?.querySelector<HTMLElement>('[role="tab"][aria-selected="true"]');
        if (!el || !sel || el.scrollWidth <= el.clientWidth) return;
        const left = sel.offsetLeft - el.offsetLeft;
        const right = left + sel.offsetWidth;
        if (left < el.scrollLeft) el.scrollLeft = left;
        else if (right > el.scrollLeft + el.clientWidth) el.scrollLeft = right - el.clientWidth;
    }, [value]);
    const onKey = (e: KeyboardEvent) => {
        const i = tabs.findIndex((t) => t.key === value);
        let j = i;
        if (e.key === 'ArrowRight') j = (i + 1) % tabs.length;
        else if (e.key === 'ArrowLeft') j = (i - 1 + tabs.length) % tabs.length;
        else if (e.key === 'Home') j = 0;
        else if (e.key === 'End') j = tabs.length - 1;
        else return;
        e.preventDefault();
        const next = tabs[j];
        if (next) {
            onChange(next.key);
            list.current?.querySelectorAll<HTMLButtonElement>('[role="tab"]')[j]?.focus();
        }
    };
    return (
        <div className="tabs" role="tablist" aria-label={label} ref={list} onKeyDown={onKey} data-variant={variant}>
            {tabs.map((t) => (
                <button
                    key={t.key}
                    type="button"
                    role="tab"
                    className="tab"
                    id={`${idPrefix}-tab-${t.key}`}
                    aria-controls={`${idPrefix}-panel-${t.key}`}
                    aria-selected={t.key === value}
                    tabIndex={t.key === value ? 0 : -1}
                    data-tone={t.tone}
                    data-testid={t.testId}
                    onClick={() => onChange(t.key)}
                >
                    {t.label}
                </button>
            ))}
        </div>
    );
}

export function TabPanel({
    idPrefix,
    tabKey,
    children,
}: {
    idPrefix: string;
    tabKey: string;
    children: React.ReactNode;
}) {
    return (
        <div role="tabpanel" id={`${idPrefix}-panel-${tabKey}`} aria-labelledby={`${idPrefix}-tab-${tabKey}`}>
            {children}
        </div>
    );
}
