/**
 * Modal dialog on the native `<dialog>` element (FRONTEND.md §8): `showModal()` gives the focus
 * trap, the inert background and Escape; focus returns to the opener on close.
 */
import { type ReactNode, useEffect, useRef } from 'react';

export interface ModalProps {
    readonly open: boolean;
    readonly onClose: () => void;
    readonly title: ReactNode;
    readonly children: ReactNode;
    readonly footer?: ReactNode;
    readonly testId?: string;
    /** When false, Escape and the backdrop do not close (e.g. while the wallet is signing). */
    readonly dismissible?: boolean;
}

export function Modal({ open, onClose, title, children, footer, testId, dismissible = true }: ModalProps) {
    const ref = useRef<HTMLDialogElement>(null);
    const opener = useRef<Element | null>(null);

    useEffect(() => {
        const d = ref.current;
        if (!d) return;
        if (open && !d.open) {
            opener.current = document.activeElement;
            if (typeof d.showModal === 'function') d.showModal();
            else d.setAttribute('open', '');
        } else if (!open && d.open) {
            if (typeof d.close === 'function') d.close();
            else d.removeAttribute('open');
            if (opener.current instanceof HTMLElement) opener.current.focus();
        }
    }, [open]);

    return (
        <dialog
            ref={ref}
            className="modal"
            data-testid={testId}
            aria-labelledby={testId ? `${testId}-title` : undefined}
            onCancel={(e) => {
                e.preventDefault();
                if (dismissible) onClose();
            }}
        >
            {open && (
                <>
                    <header className="modal-head">
                        <h2 id={testId ? `${testId}-title` : undefined} className="grow">
                            {title}
                        </h2>
                        {dismissible && (
                            <button
                                type="button"
                                className="btn"
                                data-variant="ghost"
                                data-size="sm"
                                onClick={onClose}
                                aria-label="Close"
                            >
                                ✕
                            </button>
                        )}
                    </header>
                    <div className="modal-body">{children}</div>
                    {footer && <footer className="modal-foot">{footer}</footer>}
                </>
            )}
        </dialog>
    );
}
