/**
 * Modal dialog on the native `<dialog>` element (FRONTEND.md §8): `showModal()` gives the focus
 * trap, the inert background and Escape; focus returns to the opener on close — or, when the opener
 * is gone or disabled (a form reset by a confirmed send), to the region the dialog sits in, never
 * <body> (see `focusBack`).
 *
 * On a phone (FRONTEND.md §7.6) a `dialog` fills the screen (sticky head and foot, the primary
 * action on its own full-width bottom row) and a `sheet` rises from the bottom edge.
 */
import { type ReactNode, useEffect, useRef } from 'react';
import { NavIcon } from '../shell/icons.tsx';

export interface ModalProps {
    readonly open: boolean;
    readonly onClose: () => void;
    readonly title: ReactNode;
    readonly children: ReactNode;
    readonly footer?: ReactNode;
    readonly testId?: string;
    /** When false, Escape and the backdrop do not close (e.g. while the wallet is signing). */
    readonly dismissible?: boolean;
    /** `dialog` (default): centred; full screen on a phone. `sheet`: a bottom sheet on a phone. */
    readonly variant?: 'dialog' | 'sheet';
}

/**
 * Where focus goes when a dialog closes: its opener while that can take focus. A disconnected or
 * disabled opener (the lend and short submits are disabled once the form resets after a confirmed
 * review) would leave focus on <body>, so instead: inside an open sheet (`useSheetMode`), the
 * sheet's title — still inside the sheet, so the sheet hands focus to its own opener when it closes;
 * elsewhere, the page's main region.
 */
function focusBack(dialog: HTMLDialogElement, opener: Element | null): void {
    if (
        opener instanceof HTMLElement &&
        opener.isConnected &&
        opener !== document.body &&
        !opener.matches(':disabled')
    ) {
        opener.focus();
        return;
    }
    const sheet = dialog.parentElement?.closest<HTMLElement>('[data-sheet]');
    const titleId = sheet?.getAttribute('aria-labelledby');
    const title = titleId ? document.getElementById(titleId) : null;
    const back = title && sheet?.contains(title) ? title : document.getElementById('main');
    if (back && document.activeElement !== back) back.focus({ preventScroll: true });
}

export function Modal({
    open,
    onClose,
    title,
    children,
    footer,
    testId,
    dismissible = true,
    variant = 'dialog',
}: ModalProps) {
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
            focusBack(d, opener.current);
        }
    }, [open]);

    return (
        <dialog
            ref={ref}
            className="modal"
            data-variant={variant}
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
                                className="btn modal-close"
                                data-variant="ghost"
                                data-size="sm"
                                onClick={onClose}
                                aria-label="Close"
                            >
                                <NavIcon name="close" className="icon" />
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
