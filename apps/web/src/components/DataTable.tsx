/**
 * The dense data table (FRONTEND.md §7.3): sticky header, right-aligned numeric columns, sortable
 * headers as buttons with `aria-sort`, rows that open on click or Enter (keyboard reachable with
 * Tab, moved with ArrowUp / ArrowDown), loading skeleton and empty state. A click or a key on a
 * control inside a cell (a link, a button, an input) belongs to that control, not to the row.
 */
import { type KeyboardEvent, type MouseEvent, type ReactNode, useRef } from 'react';

/** Whether the event started on a control inside the row (which then handles it itself). */
function fromControl(e: KeyboardEvent<HTMLTableRowElement> | MouseEvent<HTMLTableRowElement>): boolean {
    const t = e.target;
    if (t === e.currentTarget || !(t instanceof Element)) return false;
    const control = t.closest('a, button, input, select, textarea, label, summary, [role="button"]');
    return control !== null && e.currentTarget.contains(control);
}

export interface Column<T> {
    readonly key: string;
    readonly header: ReactNode;
    readonly render: (row: T) => ReactNode;
    readonly align?: 'left' | 'right';
    readonly sortable?: boolean;
    readonly width?: number | string;
    readonly testId?: string;
}

export interface SortState {
    readonly key: string;
    readonly order: 'asc' | 'desc';
}

export interface DataTableProps<T> {
    readonly columns: readonly Column<T>[];
    readonly rows: readonly T[] | undefined;
    readonly rowKey: (row: T) => string;
    readonly rowTestId?: (row: T) => string;
    /** Extra `data-*` state on a row (e.g. `data-status`), per the test-id contract (§11). */
    readonly rowAttrs?: (row: T) => Readonly<Record<`data-${string}`, string | undefined>>;
    readonly onRowActivate?: (row: T) => void;
    readonly sort?: SortState;
    readonly onSort?: (key: string) => void;
    readonly loading?: boolean;
    readonly empty?: ReactNode;
    readonly testId?: string;
    readonly caption: string;
    readonly skeletonRows?: number;
}

export function DataTable<T>({
    columns,
    rows,
    rowKey,
    rowTestId,
    rowAttrs,
    onRowActivate,
    sort,
    onSort,
    loading,
    empty,
    testId,
    caption,
    skeletonRows = 8,
}: DataTableProps<T>) {
    const body = useRef<HTMLTableSectionElement>(null);
    const onKey = (e: KeyboardEvent<HTMLTableRowElement>, row: T) => {
        if (fromControl(e)) return;
        if (e.key === 'Enter' && onRowActivate) {
            e.preventDefault();
            onRowActivate(row);
        } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
            e.preventDefault();
            const rowsEl = Array.from(body.current?.querySelectorAll<HTMLTableRowElement>('tr[tabindex]') ?? []);
            const i = rowsEl.indexOf(e.currentTarget);
            rowsEl[e.key === 'ArrowDown' ? i + 1 : i - 1]?.focus();
        }
    };
    return (
        <div className="tbl-wrap">
            <table className="tbl" data-testid={testId} aria-busy={loading || undefined}>
                <caption className="visually-hidden">{caption}</caption>
                <thead>
                    <tr>
                        {columns.map((c) => {
                            const active = sort?.key === c.key;
                            return (
                                <th
                                    key={c.key}
                                    scope="col"
                                    data-align={c.align}
                                    style={c.width !== undefined ? { width: c.width } : undefined}
                                    aria-sort={
                                        active ? (sort?.order === 'asc' ? 'ascending' : 'descending') : undefined
                                    }
                                >
                                    {c.sortable && onSort ? (
                                        <button type="button" onClick={() => onSort(c.key)} data-testid={c.testId}>
                                            {c.header}
                                            {active && (
                                                <span aria-hidden="true">{sort?.order === 'asc' ? ' ▲' : ' ▼'}</span>
                                            )}
                                        </button>
                                    ) : (
                                        c.header
                                    )}
                                </th>
                            );
                        })}
                    </tr>
                </thead>
                <tbody ref={body}>
                    {loading && !rows?.length
                        ? Array.from({ length: skeletonRows }, (_, i) => (
                              // biome-ignore lint/suspicious/noArrayIndexKey: placeholder rows have no identity
                              <tr key={i}>
                                  {columns.map((c) => (
                                      <td key={c.key} data-align={c.align}>
                                          <span className="skeleton" />
                                      </td>
                                  ))}
                              </tr>
                          ))
                        : rows?.map((row) => (
                              <tr
                                  key={rowKey(row)}
                                  {...rowAttrs?.(row)}
                                  data-testid={rowTestId?.(row)}
                                  data-interactive={onRowActivate ? 'true' : undefined}
                                  tabIndex={onRowActivate ? 0 : undefined}
                                  onClick={
                                      onRowActivate
                                          ? (e) => {
                                                if (!fromControl(e)) onRowActivate(row);
                                            }
                                          : undefined
                                  }
                                  onKeyDown={onRowActivate ? (e) => onKey(e, row) : undefined}
                              >
                                  {columns.map((c) => (
                                      <td key={c.key} data-align={c.align}>
                                          {c.render(row)}
                                      </td>
                                  ))}
                              </tr>
                          ))}
                </tbody>
            </table>
            {!loading && rows && rows.length === 0 && <div className="empty">{empty ?? 'Nothing here yet.'}</div>}
        </div>
    );
}
