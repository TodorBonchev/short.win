/** Placeholder body for a route whose track has not built it yet (scaffold only). */
import type { ReactNode } from 'react';
import { T } from '../config/testids.ts';

export function RouteStub({
    name,
    title,
    children,
}: {
    name: 'markets' | 'create-market' | 'positions' | 'position' | 'lend' | 'status';
    title: string;
    children?: ReactNode;
}) {
    return (
        <div className="page" data-testid={T.page(name)}>
            <div className="page-head">
                <h1>{title}</h1>
            </div>
            <div className="panel route-stub" data-testid={T.routeStub(name)}>
                {children ?? 'Coming in M9.'}
            </div>
        </div>
    );
}
