/**
 * The dense data table (FRONTEND.md §7.3): sticky header, right-aligned numeric columns, sortable
 * headers as buttons with `aria-sort`, rows that open on click or Enter (keyboard reachable with
 * Tab, moved with ArrowUp / ArrowDown), loading skeleton and empty state. A click or a key on a
 * control inside a cell (a link, a button, an input) belongs to that control, not to the row.
 *
 * Responsive (FRONTEND.md §7.6), by the width the table has (a container query on `.tbl-wrap`):
 * under 960 px the `priority: 3` columns hide; under 560 px each row is a card — `card: 'title'`
 * top left, `'aside'` top right, `'actions'` a full-width button row, every other cell labelled by
 * its header (`label`). A sort bar (a select and a direction button) replaces the header buttons
 * there. Explicit table roles keep the semantics a CSS display change drops in some browsers.
 *
 * A header the glossary knows (`config/glossary.ts`, by its text or `label`) gets its ⓘ beside the
 * header text — outside the sort button, which it must not nest in (FRONTEND.md §4.3).
 */
/* biome-ignore-all lint/a11y/noRedundantRoles: CSS display change; roles restore semantics */
import { type KeyboardEvent, type MouseEvent, type ReactNode, useId, useRef } from 'react';
import { type GlossaryId, glossaryFor } from '../config/glossary.ts';
import { T } from '../config/testids.ts';
import { GlossaryTip } from './Glossary.tsx';

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
    /**
     * 1 (default): always shown; 2: shown, but a card may group it; 3: hidden under 960 px of
     * table width — only for data shown elsewhere (a detail page) or not essential.
     */
    readonly priority?: 1 | 2 | 3;
    /** The cell's label in card mode and in the sort select (default: `header` when a string). */
    readonly label?: string;
    /** Its place on a card: `title` (top left), `aside` (top right), `actions` (a button row). */
    readonly card?: 'title' | 'aside' | 'actions';
    /** The header's glossary entry (default: the one its text or `label` attaches to; `false`: none). */
    readonly term?: GlossaryId | false;
}

function termOf<T>(c: Column<T>): GlossaryId | undefined {
    if (c.term === false) return undefined;
    return c.term ?? glossaryFor(c.header) ?? glossaryFor(c.label);
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
    /** Extra class names on `.tbl-wrap` (feature CSS may place card cells by `[data-col]`). */
    readonly className?: string;
    /** The sort select's label (default "Sort"). */
    readonly sortLabel?: string;
}

/** The text label of a column (its `label`, else its header when that is plain text). */
function labelOf<T>(c: Column<T>): string {
    return c.label ?? (typeof c.header === 'string' ? c.header : '');
}

function cellAttrs<T>(c: Column<T>) {
    return {
        'data-col': c.key,
        'data-label': labelOf(c),
        'data-priority': String(c.priority ?? 1),
        'data-card': c.card,
    };
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
    className,
    sortLabel = 'Sort',
}: DataTableProps<T>) {
    const body = useRef<HTMLTableSectionElement>(null);
    const sortId = useId();
    const sortable = onSort ? columns.filter((c) => c.sortable) : [];
    const activeSort = sortable.find((c) => c.key === sort?.key);
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
        <div className={`tbl-wrap ${className ?? ''}`.trim()} data-cards="">
            {sortable.length > 0 && onSort && (
                <div className="tbl-sortbar" data-hidden-sort={sortable.some((c) => c.priority === 3) ? '' : undefined}>
                    <label htmlFor={sortId}>{sortLabel}</label>
                    <select
                        id={sortId}
                        className="select"
                        value={sort?.key ?? ''}
                        data-testid={testId ? T.tableSort(testId) : undefined}
                        onChange={(e) => {
                            if (e.target.value) onSort(e.target.value);
                        }}
                    >
                        {!activeSort && <option value="">—</option>}
                        {sortable.map((c) => (
                            <option key={c.key} value={c.key}>
                                {labelOf(c) || c.key}
                            </option>
                        ))}
                    </select>
                    {/* The label says the order now (a tap flips it); no pressed state, no accent. */}
                    <button
                        type="button"
                        className="btn"
                        data-size="sm"
                        data-order={sort?.order === 'asc' ? 'asc' : 'desc'}
                        disabled={!activeSort}
                        data-testid={testId ? T.tableSortDir(testId) : undefined}
                        onClick={() => {
                            if (activeSort) onSort(activeSort.key);
                        }}
                    >
                        <span className="visually-hidden">Order: </span>
                        <span aria-hidden="true">{sort?.order === 'asc' ? '↑' : '↓'}</span>
                        {sort?.order === 'asc' ? 'Ascending' : 'Descending'}
                    </button>
                </div>
            )}
            <table className="tbl" role="table" data-testid={testId} aria-busy={loading || undefined}>
                <caption className="visually-hidden">{caption}</caption>
                <thead role="rowgroup">
                    <tr role="row">
                        {columns.map((c) => {
                            const active = sort?.key === c.key;
                            const term = termOf(c);
                            return (
                                <th
                                    key={c.key}
                                    role="columnheader"
                                    scope="col"
                                    {...cellAttrs(c)}
                                    data-align={c.align}
                                    style={c.width !== undefined ? { width: c.width } : undefined}
                                    aria-sort={
                                        active ? (sort?.order === 'asc' ? 'ascending' : 'descending') : undefined
                                    }
                                >
                                    {c.sortable && onSort ? (
                                        <button type="button" onClick={() => onSort(c.key)} data-testid={c.testId}>
                                            {c.header}
                                            {active ? (
                                                <span aria-hidden="true">{sort?.order === 'asc' ? ' ▲' : ' ▼'}</span>
                                            ) : (
                                                <span className="sort-hint" aria-hidden="true">
                                                    ↕
                                                </span>
                                            )}
                                        </button>
                                    ) : (
                                        c.header
                                    )}
                                    {term && <GlossaryTip id={term} className="th-tip" />}
                                </th>
                            );
                        })}
                    </tr>
                </thead>
                <tbody ref={body} role="rowgroup">
                    {loading && !rows?.length
                        ? Array.from({ length: skeletonRows }, (_, i) => (
                              // biome-ignore lint/suspicious/noArrayIndexKey: placeholder rows have no identity
                              <tr key={i} role="row">
                                  {columns.map((c) => (
                                      <td key={c.key} role="cell" {...cellAttrs(c)} data-align={c.align}>
                                          <span className="skeleton" />
                                      </td>
                                  ))}
                              </tr>
                          ))
                        : rows?.map((row) => (
                              <tr
                                  key={rowKey(row)}
                                  role="row"
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
                                      <td key={c.key} role="cell" {...cellAttrs(c)} data-align={c.align}>
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
