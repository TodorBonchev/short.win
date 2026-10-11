import type { ReactNode } from 'react';
import { ApiError } from '../api/client.ts';
import { Term } from './Glossary.tsx';

/**
 * An empty (or not-found) state: an optional title, one muted line, an optional action (a link or
 * a button that gets the user out of it; FRONTEND.md §3.10).
 */
export function Empty({
    title,
    children,
    action,
    testId,
    role,
}: {
    title?: ReactNode;
    children?: ReactNode;
    action?: ReactNode;
    testId?: string;
    role?: 'alert' | 'status';
}) {
    if (title === undefined && action === undefined) {
        return (
            <div className="empty" data-testid={testId} role={role}>
                {children}
            </div>
        );
    }
    return (
        <div className="empty" data-testid={testId} role={role} data-layout="block">
            {title !== undefined && <div className="empty-title">{title}</div>}
            {children !== undefined && <div className="empty-text">{children}</div>}
            {action !== undefined && <div className="empty-action">{action}</div>}
        </div>
    );
}

export function ErrorState({ error, retry }: { error: unknown; retry?: () => void }) {
    const msg =
        error instanceof ApiError
            ? error.status === 404
                ? 'Not found.'
                : error.message
            : error instanceof Error
              ? error.message
              : 'Something went wrong.';
    return (
        <div className="error-state" role="alert">
            {msg}{' '}
            {retry && (
                <button type="button" className="btn" data-size="sm" onClick={retry}>
                    Retry
                </button>
            )}
        </div>
    );
}

/** A loading placeholder; `width` is a computed CSSOM value (allowed under the CSP). */
export function Skeleton({ width }: { width?: number }) {
    return <span className="skeleton" style={width ? { width } : undefined} aria-hidden="true" />;
}

/**
 * A labelled statistic for stat strips. `priority` 2: on a phone it waits behind the strip's "More
 * stats" toggle (`.stat-strip[data-expanded]`).
 */
export function Stat({
    label,
    children,
    testId,
    priority,
}: {
    label: string;
    children: ReactNode;
    testId?: string;
    priority?: 1 | 2;
}) {
    return (
        <div className="stat" data-testid={testId} data-priority={priority}>
            <div className="stat-label">
                <Term>{label}</Term>
            </div>
            <div className="stat-value">{children}</div>
        </div>
    );
}

/** A definition list row set: `[["Margin", <Sol …/>], …]`; a glossary label gets its ⓘ. */
export function Kv({ rows, testId }: { rows: readonly (readonly [ReactNode, ReactNode, string?])[]; testId?: string }) {
    return (
        <dl className="kv" data-testid={testId}>
            {rows.map(([k, v, id], i) => (
                // biome-ignore lint/suspicious/noArrayIndexKey: static row order
                <div key={i} className="kv-row">
                    <dt>
                        <Term>{k}</Term>
                    </dt>
                    <dd data-testid={id}>{v}</dd>
                </div>
            ))}
        </dl>
    );
}
