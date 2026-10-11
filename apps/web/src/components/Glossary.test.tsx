/**
 * The glossary control (FRONTEND.md §4.3, §8; the owner's thirteenth round, O-3 (b)): the ⓘ opens
 * its definition on a click / tap and from the keyboard (Enter, Space) — never on hover —, Escape,
 * ✕ and a tap outside close it, Escape and ✕ return focus to the ⓘ, Tab moves on, `aria-expanded`
 * and `aria-controls` tie the two; it works in a table cell, a table header and a card, and a field's
 * ⓘ stays outside its `<label>`.
 */
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { glossaryText, glossaryVars } from '../config/glossaryText.ts';
import { T } from '../config/testids.ts';
import { setMedia } from '../test/media.ts';
import { DataTable } from './DataTable.tsx';
import { Field } from './Field.tsx';
import { GlossaryTip, Term } from './Glossary.tsx';
import { Kv } from './States.tsx';

// The definition reads the live config for its placeholders; none here (the fallback words).
vi.mock('../rpc/queries.ts', () => ({ useChainConfig: () => ({ data: undefined }) }));

const tip = (id: Parameters<typeof T.glossaryTip>[0]) => screen.getByTestId(T.glossaryTip(id));
const popover = (id: Parameters<typeof T.glossaryPopover>[0]) => screen.queryByTestId(T.glossaryPopover(id));

describe('GlossaryTip', () => {
    it('opens on a click with the definition, tied to the ⓘ by aria-expanded / aria-controls', async () => {
        render(<Term>CR</Term>);
        const b = tip('cr');
        expect(b.getAttribute('aria-label')).toBe('What is CR (the collateral ratio)?');
        expect(b.getAttribute('aria-expanded')).toBe('false');
        expect(b.getAttribute('aria-controls')).toBeNull();
        await act(async () => {
            fireEvent.click(b);
        });
        const p = popover('cr');
        expect(p).toBeTruthy();
        expect(b.getAttribute('aria-expanded')).toBe('true');
        expect(b.getAttribute('aria-controls')).toBe(p?.id);
        expect(p?.getAttribute('role')).toBe('dialog');
        // The text arrives with the definitions' chunk (loaded on the first open).
        await waitFor(() => expect(p?.textContent).toContain(glossaryText('cr')));
        // Focus moves into the definition, which is named by its title.
        expect(document.activeElement).toBe(p);
        const title = document.getElementById(p?.getAttribute('aria-labelledby') ?? '');
        expect(title?.textContent).toBe('CR (the collateral ratio)');
        // A second click closes it.
        await act(async () => {
            fireEvent.click(b);
        });
        expect(popover('cr')).toBeNull();
        expect(b.getAttribute('aria-expanded')).toBe('false');
    });

    it('opens from the keyboard (Enter and Space) and never on hover', async () => {
        const user = userEvent.setup();
        render(<GlossaryTip id="liveFeed" />);
        const b = tip('liveFeed');
        await user.hover(b);
        expect(popover('liveFeed')).toBeNull();
        b.focus();
        await user.keyboard('{Enter}');
        expect(popover('liveFeed')).toBeTruthy();
        await user.keyboard('{Escape}');
        expect(popover('liveFeed')).toBeNull();
        expect(document.activeElement).toBe(b);
        await user.keyboard(' ');
        expect(popover('liveFeed')).toBeTruthy();
    });

    it('closes on Escape, ✕ and a tap outside; Escape and ✕ return focus to the ⓘ', async () => {
        render(
            <div>
                <button type="button">elsewhere</button>
                <Term>Utilisation</Term>
            </div>,
        );
        const b = tip('utilisation');
        await act(async () => {
            fireEvent.click(b);
        });
        await act(async () => {
            fireEvent.keyDown(popover('utilisation') as HTMLElement, { key: 'Escape' });
        });
        expect(popover('utilisation')).toBeNull();
        expect(document.activeElement).toBe(b);

        await act(async () => {
            fireEvent.click(b);
        });
        await act(async () => {
            fireEvent.click(screen.getByTestId(T.glossaryClose));
        });
        expect(popover('utilisation')).toBeNull();
        expect(document.activeElement).toBe(b);

        await act(async () => {
            fireEvent.click(b);
        });
        await act(async () => {
            fireEvent.pointerDown(screen.getByRole('button', { name: 'elsewhere' }));
        });
        expect(popover('utilisation')).toBeNull();
    });

    it('lets Tab move on: past its ✕, focus goes back to the ⓘ (no trap)', async () => {
        const user = userEvent.setup();
        render(<Term>TVL</Term>);
        const b = tip('tvl');
        await act(async () => {
            fireEvent.click(b);
        });
        await user.tab();
        expect(document.activeElement).toBe(screen.getByTestId(T.glossaryClose));
        await act(async () => {
            fireEvent.keyDown(screen.getByTestId(T.glossaryClose), { key: 'Tab' });
        });
        expect(popover('tvl')).toBeNull();
        expect(document.activeElement).toBe(b);
    });

    it('fills its placeholders (the fallback words before the config is read)', async () => {
        render(<Term>Vesting</Term>);
        await act(async () => {
            fireEvent.click(tip('vesting'));
        });
        await waitFor(() =>
            expect(popover('vesting')?.textContent).toContain(glossaryText('vesting', glossaryVars(null))),
        );
        expect(popover('vesting')?.textContent).toContain('over the vesting period');
        expect(popover('vesting')?.textContent).not.toMatch(/\{\w+\}/);
    });

    it('works in a table cell and a header: its own control, never the row’s or the sort’s', async () => {
        const activate = vi.fn();
        const sort = vi.fn();
        render(
            <DataTable
                caption="Positions"
                rows={[{ id: 'a' }]}
                rowKey={(r) => r.id}
                onRowActivate={activate}
                onSort={sort}
                sort={{ key: 'cr', order: 'desc' }}
                columns={[
                    { key: 'cr', header: 'CR', sortable: true, render: () => '150%' },
                    { key: 'q', header: 'Q owed', render: () => '10' },
                    {
                        key: 'flags',
                        header: 'Flags',
                        card: 'aside',
                        render: () => <Term id="flagCoveragePaused">Coverage paused</Term>,
                    },
                ]}
            />,
        );
        // The header's ⓘ sits beside the sort button, not in it.
        const head = tip('cr');
        expect(head.closest('th')).toBeTruthy();
        expect(head.parentElement?.tagName).toBe('TH');
        expect(within(head.closest('th') as HTMLElement).getAllByRole('button')).toHaveLength(2);
        await act(async () => {
            fireEvent.click(head);
        });
        expect(sort).not.toHaveBeenCalled();
        expect(popover('cr')).toBeTruthy();
        // The definition renders outside the table (no clipping by its scroll container).
        expect(popover('cr')?.closest('.tbl-wrap')).toBeNull();

        const cell = tip('flagCoveragePaused');
        expect(cell.closest('td')).toBeTruthy();
        await act(async () => {
            fireEvent.click(cell);
        });
        expect(activate).not.toHaveBeenCalled();
        await waitFor(() => expect(popover('flagCoveragePaused')?.textContent).toContain('new shorts are paused'));
        // Enter on the ⓘ is the ⓘ's, not the row's.
        await act(async () => {
            fireEvent.keyDown(cell, { key: 'Enter' });
        });
        expect(activate).not.toHaveBeenCalled();
    });

    it('keeps the definition’s own clicks and keys from a row that opens (the portal still bubbles)', async () => {
        const user = userEvent.setup();
        const activate = vi.fn();
        render(
            <DataTable
                caption="Markets"
                rows={[{ id: 'a' }, { id: 'b' }]}
                rowKey={(r) => r.id}
                onRowActivate={activate}
                columns={[
                    {
                        key: 'flags',
                        header: 'Flags',
                        render: (r) => (r.id === 'a' ? <Term id="flagCoveragePaused">Coverage paused</Term> : '—'),
                    },
                ]}
            />,
        );
        const b = tip('flagCoveragePaused');
        await user.click(b);
        const p = popover('flagCoveragePaused') as HTMLElement;
        await waitFor(() => expect(p.textContent).toContain('new shorts are paused'));
        // A click on the definition's text: nothing opens, the definition stays.
        await user.click(p.querySelector('p') as HTMLElement);
        expect(popover('flagCoveragePaused')).toBeTruthy();
        // An arrow key inside it: no move to the next row, focus stays in the definition.
        (p as HTMLElement).focus();
        await user.keyboard('{ArrowDown}');
        expect(p.contains(document.activeElement)).toBe(true);
        // Enter on its ✕: closes it, focus back to the ⓘ.
        const x = within(p).getByTestId(T.glossaryClose);
        x.focus();
        await user.keyboard('{Enter}');
        expect(popover('flagCoveragePaused')).toBeNull();
        expect(document.activeElement).toBe(b);
        // A click on its ✕: the same.
        await user.click(b);
        await user.click(within(popover('flagCoveragePaused') as HTMLElement).getByTestId(T.glossaryClose));
        expect(popover('flagCoveragePaused')).toBeNull();
        expect(document.activeElement).toBe(b);
        expect(activate).not.toHaveBeenCalled();
    });

    it('closes the phone sheet on the scrim’s own click: the tap opens nothing under it, focus returns', async () => {
        setMedia({ compact: true, phone: true, coarse: true });
        const activate = vi.fn();
        render(
            <DataTable
                caption="Markets"
                rows={[{ id: 'a' }]}
                rowKey={(r) => r.id}
                onRowActivate={activate}
                columns={[
                    {
                        key: 'flags',
                        header: 'Flags',
                        render: () => <Term id="flagCoveragePaused">Coverage paused</Term>,
                    },
                ]}
            />,
        );
        const b = tip('flagCoveragePaused');
        await act(async () => {
            fireEvent.click(b);
        });
        const scrim = document.querySelector('.glossary-scrim') as HTMLElement;
        expect(scrim).toBeTruthy();
        // The pointerdown alone keeps it open (it would unmount the scrim before the click).
        await act(async () => {
            fireEvent.pointerDown(scrim);
        });
        expect(popover('flagCoveragePaused')).toBeTruthy();
        await act(async () => {
            fireEvent.click(scrim);
        });
        expect(popover('flagCoveragePaused')).toBeNull();
        expect(document.activeElement).toBe(b);
        expect(activate).not.toHaveBeenCalled();
    });

    it('works in a card: a phone gets a bottom sheet over a scrim, not a placed popover', async () => {
        setMedia({ compact: true, phone: true, coarse: true });
        render(
            <div className="card">
                <Kv rows={[['Q owed', '10']]} />
            </div>,
        );
        await act(async () => {
            fireEvent.click(tip('q'));
        });
        const p = popover('q') as HTMLElement;
        expect(p).toBeTruthy();
        expect(p.dataset.placed).toBe('');
        expect(p.getAttribute('style')).toBeNull();
        expect(document.querySelector('.glossary-scrim')).toBeTruthy();
    });

    it('labels shared renderers by their text, and keeps a field’s ⓘ outside its <label>', () => {
        render(
            <>
                <Kv rows={[['Share epoch', '3']]} />
                <Field label="Slippage">{({ inputId }) => <input id={inputId} data-testid="slip" />}</Field>
                <Term>Not a glossary term</Term>
            </>,
        );
        expect(tip('shareEpoch')).toBeTruthy();
        const t = tip('slippage');
        expect(t.closest('label')).toBeNull();
        expect(screen.getByLabelText('Slippage')).toBe(screen.getByTestId('slip'));
        // A label the glossary does not know stays plain text: two ⓘ in all.
        expect(screen.getAllByRole('button')).toHaveLength(2);
    });
});
