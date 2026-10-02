/** Accessible tabs (WAI-ARIA tabs pattern: arrow keys move, Home / End jump). */
import { type KeyboardEvent, useRef } from 'react';

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
}: {
    tabs: readonly TabDef<K>[];
    value: K;
    onChange: (k: K) => void;
    label: string;
    idPrefix: string;
}) {
    const list = useRef<HTMLDivElement>(null);
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
        <div className="tabs" role="tablist" aria-label={label} ref={list} onKeyDown={onKey}>
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
