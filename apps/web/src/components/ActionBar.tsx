/**
 * The page's primary actions within thumb reach (FRONTEND.md §7.6): a bar fixed above the phone's
 * tab bar and status strip (above the status bar on a tablet), hidden from 1024 px up where the page
 * shows the same actions in place. At most one per page; placed in the DOM right after the page
 * header, so its reading and focus order is the page's. While it is on the page the shell reserves
 * its height (`--actionbar-h`).
 */
import type { ReactNode } from 'react';

export function ActionBar({ label, children, testId }: { label: string; children: ReactNode; testId?: string }) {
    return (
        <section className="action-bar" aria-label={label} data-testid={testId}>
            {children}
        </section>
    );
}
