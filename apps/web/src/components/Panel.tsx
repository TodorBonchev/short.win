import type { ReactNode } from 'react';

export interface PanelProps {
    readonly title?: ReactNode;
    readonly actions?: ReactNode;
    readonly children: ReactNode;
    readonly flush?: boolean;
    readonly testId?: string;
    readonly className?: string;
    /** Accessible name when the title is not plain text. */
    readonly label?: string;
}

/** A titled surface. Rendered as a `<section>` labelled by its title. */
export function Panel({ title, actions, children, flush, testId, className, label }: PanelProps) {
    return (
        <section className={`panel ${className ?? ''}`.trim()} data-testid={testId} aria-label={label}>
            {(title || actions) && (
                <header className="panel-head">
                    {title && <h2 className="panel-title">{title}</h2>}
                    {actions && <div className="panel-actions">{actions}</div>}
                </header>
            )}
            <div className={`panel-body${flush ? ' flush' : ''}`}>{children}</div>
        </section>
    );
}
