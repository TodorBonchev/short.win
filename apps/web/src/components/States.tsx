import type { ReactNode } from 'react';
import { ApiError } from '../api/client.ts';

export function Empty({ children, testId }: { children: ReactNode; testId?: string }) {
    return (
        <div className="empty" data-testid={testId}>
            {children}
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

export function Skeleton({ width }: { width?: number }) {
    return <span className="skeleton" style={width ? { width } : undefined} aria-hidden="true" />;
}

/** A labelled statistic for stat strips. */
export function Stat({ label, children, testId }: { label: string; children: ReactNode; testId?: string }) {
    return (
        <div className="stat" data-testid={testId}>
            <div className="stat-label">{label}</div>
            <div className="stat-value">{children}</div>
        </div>
    );
}

/** A definition list row set: `[["Margin", <Sol …/>], …]`. */
export function Kv({ rows, testId }: { rows: readonly (readonly [ReactNode, ReactNode, string?])[]; testId?: string }) {
    return (
        <dl className="kv" data-testid={testId}>
            {rows.map(([k, v, id], i) => (
                // biome-ignore lint/suspicious/noArrayIndexKey: static row order
                <div key={i} style={{ display: 'contents' }}>
                    <dt>{k}</dt>
                    <dd data-testid={id}>{v}</dd>
                </div>
            ))}
        </dl>
    );
}
