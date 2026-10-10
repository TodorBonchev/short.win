/**
 * A form's submit kept within reach (FRONTEND.md §7.6): sticky at the bottom of its panel or sheet,
 * with the reason it is disabled in one line above it. The reason region always exists (a live
 * region announces a change only when it was there before) and updates only when the reason does.
 * With `reasonTarget` (an element id) the reason is a button that scrolls to and focuses that
 * control (e.g. the acknowledgement to tick). DOM order: reason, then submit.
 */
import type { ReactNode } from 'react';

export function FormFoot({
    reason,
    reasonTestId,
    reasonTarget,
    children,
}: {
    /** Why the submit is disabled; `null` when it is enabled. */
    reason: string | null;
    reasonTestId?: string;
    reasonTarget?: string;
    children: ReactNode;
}) {
    return (
        <div className="form-foot">
            <div role="status" data-testid={reasonTestId} data-reason={reason ? '' : undefined}>
                {reason &&
                    (reasonTarget ? (
                        <button
                            type="button"
                            className="form-reason"
                            onClick={() => {
                                const el = document.getElementById(reasonTarget);
                                el?.scrollIntoView?.({ block: 'center' });
                                el?.focus({ preventScroll: true });
                            }}
                        >
                            {reason}
                        </button>
                    ) : (
                        <p className="form-reason">{reason}</p>
                    ))}
            </div>
            {children}
        </div>
    );
}
