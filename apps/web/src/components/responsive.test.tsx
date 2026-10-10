/**
 * The responsive building blocks (FRONTEND.md §7.6, §10.1): DataTable's card attributes, roles and
 * sort bar; useSheetMode's modal semantics under compact and nothing on a desktop; FormFoot's
 * reason; the Tabs' scrolling hooks under jsdom.
 */
import { act, fireEvent, render, screen } from '@testing-library/react';
import { useRef, useState } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { T } from '../config/testids.ts';
import { setMedia } from '../test/media.ts';
import { DataTable, type SortState } from './DataTable.tsx';
import { FormFoot } from './FormFoot.tsx';
import { SheetHead, useSheetMode } from './Sheet.tsx';
import { Tabs } from './Tabs.tsx';
import { ToastProvider } from './Toast.tsx';

interface Row {
    readonly id: string;
    readonly tvl: number;
}

function Table({ onSort, sort }: { onSort: (k: string) => void; sort?: SortState }) {
    return (
        <DataTable<Row>
            testId="t"
            caption="Rows"
            rows={[
                { id: 'a', tvl: 1 },
                { id: 'b', tvl: 2 },
            ]}
            rowKey={(r) => r.id}
            sort={sort}
            onSort={onSort}
            columns={[
                { key: 'id', header: 'Token', render: (r) => r.id, card: 'title' },
                { key: 'tvl', header: 'TVL', render: (r) => r.tvl, sortable: true, align: 'right', priority: 2 },
                { key: 'age', header: <span>Age</span>, label: 'Age', render: () => '', sortable: true, priority: 3 },
            ]}
        />
    );
}

describe('DataTable (responsive)', () => {
    it('labels every cell, keeps table roles and the row count', () => {
        render(<Table onSort={() => {}} sort={{ key: 'tvl', order: 'desc' }} />);
        const table = screen.getByTestId('t');
        expect(table.getAttribute('role')).toBe('table');
        expect(table.querySelectorAll('tbody > tr[role="row"]')).toHaveLength(2);
        for (const cell of table.querySelectorAll('th, td')) {
            expect(cell.getAttribute('data-col')).toBeTruthy();
            expect(cell.hasAttribute('data-label')).toBe(true);
            expect(cell.getAttribute('data-priority')).toMatch(/^[123]$/);
        }
        const tvl = table.querySelector('td[data-col="tvl"]');
        expect(tvl?.getAttribute('data-label')).toBe('TVL');
        expect(tvl?.getAttribute('role')).toBe('cell');
        expect(table.querySelector('td[data-col="id"]')?.getAttribute('data-card')).toBe('title');
        expect(table.querySelector('th[data-col="age"]')?.getAttribute('data-label')).toBe('Age');
        expect(table.querySelector('th[data-col="tvl"]')?.getAttribute('aria-sort')).toBe('descending');
        expect(table.closest('.tbl-wrap')?.hasAttribute('data-cards')).toBe(true);
    });

    it('sorts from the sort bar: the select picks a key, the button toggles the order', () => {
        const onSort = vi.fn();
        render(<Table onSort={onSort} sort={{ key: 'tvl', order: 'desc' }} />);
        const select = screen.getByTestId(T.tableSort('t')) as HTMLSelectElement;
        expect(select.value).toBe('tvl');
        expect([...select.options].map((o) => o.textContent)).toEqual(['TVL', 'Age']);
        fireEvent.change(select, { target: { value: 'age' } });
        expect(onSort).toHaveBeenLastCalledWith('age');
        const dir = screen.getByTestId(T.tableSortDir('t'));
        expect(dir.hasAttribute('aria-pressed')).toBe(false);
        expect(dir.textContent).toBe('Order: ↓Descending');
        expect(dir.getAttribute('data-order')).toBe('desc');
        fireEvent.click(dir);
        expect(onSort).toHaveBeenLastCalledWith('tvl');
    });

    it('names the order the direction button shows: ascending reads Ascending with an up arrow', () => {
        render(<Table onSort={vi.fn()} sort={{ key: 'tvl', order: 'asc' }} />);
        const dir = screen.getByTestId(T.tableSortDir('t'));
        expect(dir.textContent).toBe('Order: ↑Ascending');
        expect(dir.getAttribute('data-order')).toBe('asc');
    });
});

function SheetHarness({ onClose }: { onClose: () => void }) {
    const ref = useRef<HTMLElement>(null);
    const [open, setOpen] = useState(false);
    useSheetMode(ref, {
        open,
        onClose: () => {
            setOpen(false);
            onClose();
        },
        labelledBy: 'sheet-title',
    });
    return (
        <ToastProvider>
            <div id="page">
                <button type="button" onClick={() => setOpen(true)}>
                    Open
                </button>
                <section ref={ref} className="sheet-host" data-testid="sheet">
                    <SheetHead title="Trade" titleId="sheet-title" onClose={() => setOpen(false)} testId="close" />
                    <input aria-label="size" />
                </section>
                <p data-testid="sibling">beside</p>
            </div>
        </ToastProvider>
    );
}

/** A popup that unmounts on close (the header wallet menu), its opener a sibling made inert. */
function PopupHarness({ startOpen = false }: { startOpen?: boolean }) {
    const ref = useRef<HTMLDivElement>(null);
    const [open, setOpen] = useState(startOpen);
    useSheetMode(ref, { open, onClose: () => setOpen(false), labelledBy: 'popup-title' });
    return (
        <main id="main" tabIndex={-1}>
            <div>
                <button type="button" onClick={() => setOpen(true)}>
                    Wallet
                </button>
                {open && (
                    <div ref={ref} data-testid="popup">
                        <h2 id="popup-title" tabIndex={-1}>
                            Wallet
                        </h2>
                        <button type="button">Copy address</button>
                    </div>
                )}
            </div>
        </main>
    );
}

describe('useSheetMode', () => {
    it('returns focus to the opener when the sheet unmounts as it closes (focus went to <body>)', () => {
        setMedia({ compact: true });
        render(<PopupHarness />);
        const opener = screen.getByRole('button', { name: 'Wallet' });
        opener.focus();
        fireEvent.click(opener);
        expect(opener.hasAttribute('inert')).toBe(true);
        screen.getByRole('button', { name: 'Copy address' }).focus();
        fireEvent.keyDown(document, { key: 'Escape' });
        expect(screen.queryByTestId('popup')).toBeNull();
        expect(opener.hasAttribute('inert')).toBe(false);
        expect(document.activeElement).toBe(opener);
    });

    it('without an opener (a deep-linked sheet) gives focus to the main region, never <body>', () => {
        setMedia({ compact: true });
        render(<PopupHarness startOpen />);
        expect(document.activeElement?.id).toBe('popup-title');
        fireEvent.keyDown(document, { key: 'Escape' });
        expect(screen.queryByTestId('popup')).toBeNull();
        expect(document.activeElement?.id).toBe('main');
    });

    it('is a modal sheet under compact: role, inert siblings (not the toasts), focus, Escape, focus back', () => {
        setMedia({ compact: true });
        const onClose = vi.fn();
        render(<SheetHarness onClose={onClose} />);
        const opener = screen.getByRole('button', { name: 'Open' });
        opener.focus();
        fireEvent.click(opener);
        const sheet = screen.getByTestId('sheet');
        expect(sheet.getAttribute('role')).toBe('dialog');
        expect(sheet.getAttribute('aria-modal')).toBe('true');
        expect(sheet.dataset.sheet).toBe('open');
        expect(sheet.getAttribute('aria-labelledby')).toBe('sheet-title');
        expect(screen.getByTestId('sibling').hasAttribute('inert')).toBe(true);
        expect(screen.getByTestId(T.toastRegion).closest('[inert]')).toBeNull();
        expect(document.documentElement.hasAttribute('data-sheet-open')).toBe(true);
        expect(document.activeElement?.id).toBe('sheet-title');
        fireEvent.keyDown(document, { key: 'Escape' });
        expect(onClose).toHaveBeenCalledOnce();
        expect(sheet.hasAttribute('role')).toBe(false);
        expect(screen.getByTestId('sibling').hasAttribute('inert')).toBe(false);
        expect(document.documentElement.hasAttribute('data-sheet-open')).toBe(false);
        expect(document.activeElement).toBe(opener);
    });

    it('lets go when the query flips (a rotation past 1024 px)', () => {
        setMedia({ compact: true });
        render(<SheetHarness onClose={() => {}} />);
        fireEvent.click(screen.getByRole('button', { name: 'Open' }));
        expect(screen.getByTestId('sheet').dataset.sheet).toBe('open');
        act(() => setMedia({ compact: false }));
        expect(screen.getByTestId('sheet').dataset.sheet).toBeUndefined();
        expect(screen.getByTestId('sibling').hasAttribute('inert')).toBe(false);
    });

    it('does nothing on a desktop', () => {
        render(<SheetHarness onClose={() => {}} />);
        fireEvent.click(screen.getByRole('button', { name: 'Open' }));
        expect(screen.getByTestId('sheet').hasAttribute('role')).toBe(false);
        expect(screen.getByTestId('sibling').hasAttribute('inert')).toBe(false);
    });
});

describe('FormFoot', () => {
    it('keeps the reason region, empty while the submit is enabled; a reason button focuses its target', () => {
        const { rerender } = render(
            <>
                <input id="ack" aria-label="ack" type="checkbox" />
                <FormFoot reason="Tick the acknowledgement above" reasonTestId="why" reasonTarget="ack">
                    <button type="button">Short</button>
                </FormFoot>
            </>,
        );
        const why = screen.getByTestId('why');
        expect(why.getAttribute('role')).toBe('status');
        fireEvent.click(screen.getByRole('button', { name: 'Tick the acknowledgement above' }));
        expect(document.activeElement?.id).toBe('ack');
        rerender(
            <FormFoot reason={null} reasonTestId="why">
                <button type="button">Short</button>
            </FormFoot>,
        );
        expect(screen.getByTestId('why').textContent).toBe('');
        // The submit is last in the DOM.
        const foot = screen.getByTestId('why').parentElement;
        expect(foot?.lastElementChild?.textContent).toBe('Short');
    });
});

describe('Tabs', () => {
    it('render without ResizeObserver or scrolling (jsdom) and keep the ARIA pattern', () => {
        const onChange = vi.fn();
        render(
            <Tabs
                tabs={[
                    { key: 'a', label: 'A' },
                    { key: 'b', label: 'B' },
                ]}
                value="a"
                onChange={onChange}
                label="Sections"
                idPrefix="x"
            />,
        );
        const tabs = screen.getAllByRole('tab');
        expect(tabs[0]?.getAttribute('aria-selected')).toBe('true');
        fireEvent.keyDown(tabs[0] as HTMLElement, { key: 'End' });
        expect(onChange).toHaveBeenCalledWith('b');
    });
});
