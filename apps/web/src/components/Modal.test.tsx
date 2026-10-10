/**
 * Modal's focus return (FRONTEND.md §8, §10.1): to the opener while it can take focus; a disabled
 * or removed opener (a form reset by a confirmed send) gives it to the dialog's region — an open
 * sheet's title, which the sheet then hands to its own opener, or the main region — never <body>.
 */
import { fireEvent, render, screen } from '@testing-library/react';
import { useRef, useState } from 'react';
import { describe, expect, it } from 'vitest';
import { setMedia } from '../test/media.ts';
import { Modal } from './Modal.tsx';
import { SheetHead, useSheetMode } from './Sheet.tsx';

/** A form whose submit opens a review; `after` is what confirming does to the submit. */
function Form({ after }: { after: 'keep' | 'disable' | 'remove' }) {
    const [open, setOpen] = useState(false);
    const [done, setDone] = useState(false);
    const close = () => {
        setDone(true);
        setOpen(false);
    };
    return (
        <>
            {!(done && after === 'remove') && (
                <button type="button" disabled={done && after === 'disable'} onClick={() => setOpen(true)}>
                    Deposit
                </button>
            )}
            <Modal open={open} onClose={close} title="Review" testId="review">
                <button type="button" onClick={close}>
                    Done
                </button>
            </Modal>
        </>
    );
}

function Page({ after }: { after: 'keep' | 'disable' | 'remove' }) {
    return (
        <main id="main" tabIndex={-1}>
            <Form after={after} />
        </main>
    );
}

/** The trade sheet: Lend opens it, the form (and its review) inside; Done also closes the sheet. */
function SheetPage() {
    const ref = useRef<HTMLElement>(null);
    const [sheet, setSheet] = useState(false);
    const [open, setOpen] = useState(false);
    const [done, setDone] = useState(false);
    useSheetMode(ref, { open: sheet, onClose: () => setSheet(false), labelledBy: 'sheet-title' });
    return (
        <main id="main" tabIndex={-1}>
            <button type="button" onClick={() => setSheet(true)}>
                Lend
            </button>
            <section ref={ref}>
                <SheetHead title="Trade" titleId="sheet-title" onClose={() => setSheet(false)} testId="close" />
                <button type="button" disabled={done} onClick={() => setOpen(true)}>
                    Deposit
                </button>
                <Modal
                    open={open}
                    onClose={() => {
                        setDone(true);
                        setOpen(false);
                    }}
                    title="Review"
                    testId="review"
                >
                    <button
                        type="button"
                        onClick={() => {
                            setDone(true);
                            setOpen(false);
                        }}
                    >
                        Done
                    </button>
                </Modal>
            </section>
        </main>
    );
}

function openReview(): HTMLElement {
    const deposit = screen.getByRole('button', { name: 'Deposit' });
    deposit.focus();
    fireEvent.click(deposit);
    const done = screen.getByRole('button', { name: 'Done' });
    done.focus();
    return deposit;
}

describe('Modal focus return', () => {
    it('returns focus to the opener while it can take it', () => {
        render(<Page after="keep" />);
        const deposit = openReview();
        fireEvent.click(screen.getByRole('button', { name: 'Done' }));
        expect(document.activeElement).toBe(deposit);
    });

    it('gives focus to the main region when the opener is disabled on close (never <body>)', () => {
        render(<Page after="disable" />);
        openReview();
        fireEvent.click(screen.getByRole('button', { name: 'Done' }));
        expect(document.activeElement?.id).toBe('main');
    });

    it('gives focus to the main region when the opener is gone', () => {
        render(<Page after="remove" />);
        openReview();
        fireEvent(screen.getByTestId('review'), new Event('cancel', { cancelable: true }));
        expect(screen.queryByRole('button', { name: 'Deposit' })).toBeNull();
        expect(document.activeElement?.id).toBe('main');
    });

    it("inside an open sheet gives it to the sheet's title, and the sheet then to its own opener", () => {
        setMedia({ compact: true });
        render(<SheetPage />);
        const lend = screen.getByRole('button', { name: 'Lend' });
        lend.focus();
        fireEvent.click(lend);
        openReview();
        fireEvent.click(screen.getByRole('button', { name: 'Done' }));
        expect(document.activeElement?.id).toBe('sheet-title');
        fireEvent.click(screen.getByTestId('close'));
        expect(document.activeElement).toBe(lend);
    });
});
