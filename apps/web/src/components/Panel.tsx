import type { ReactNode, Ref } from 'react';

export interface PanelProps {
    readonly title?: ReactNode;
    readonly actions?: ReactNode;
    readonly children: ReactNode;
    readonly flush?: boolean;
    readonly testId?: string;
    readonly className?: string;
    /** Accessible name when the title is not plain text. */
    readonly label?: string;
    /** The section element (a sheet host: `useSheetMode`, FRONTEND.md §7.6). */
    readonly ref?: Ref<HTMLElement>;
    /** Rendered first, before the panel's own head (a sheet's `SheetHead`). */
    readonly head?: ReactNode;
}

/** A titled surface. Rendered as a `<section>` labelled by its title. */
export function Panel({ title, actions, children, flush, testId, className, label, ref, head }: PanelProps) {
    return (
        <section ref={ref} className={`panel ${className ?? ''}`.trim()} data-testid={testId} aria-label={label}>
            {head}
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
