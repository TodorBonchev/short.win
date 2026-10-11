/**
 * A key / value list with hairline rows (`.kv[data-lines]`, M10-RESP D10): the market page's long
 * blocks (market state, exits, settlements, creator exposure). A zero or an absent value ("0", "—",
 * a zero amount) stays in place but reads quieter (`dd[data-zero]`), so the figures that matter
 * stand out. Plain-text values are judged here; a rendered amount says so with `{ zero }`.
 */
import type { ReactNode } from 'react';
import { Term } from '../../../components/Glossary.tsx';

export interface KvOpts {
    /** The value is zero or absent (muted). */
    readonly zero?: boolean;
    readonly testId?: string;
}

export type KvLine = readonly [ReactNode, ReactNode, KvOpts?];

/** "0", "—", "0 / 0 / 0", "0 (0)": a value with nothing in it. */
export function zeroText(v: ReactNode): boolean {
    return typeof v === 'number' ? v === 0 : typeof v === 'string' && /^[\s0—/()·]*$/.test(v) && v.trim() !== '';
}

export function KvLines({ rows, testId }: { rows: readonly KvLine[]; testId?: string }) {
    return (
        <dl className="kv mk-kv" data-lines="" data-testid={testId}>
            {rows.map(([k, v, o], i) => (
                // biome-ignore lint/suspicious/noArrayIndexKey: static row order
                <div key={i} className="kv-row">
                    <dt>
                        <Term>{k}</Term>
                    </dt>
                    <dd data-testid={o?.testId} data-zero={(o?.zero ?? zeroText(v)) ? '' : undefined}>
                        {v}
                    </dd>
                </div>
            ))}
        </dl>
    );
}
