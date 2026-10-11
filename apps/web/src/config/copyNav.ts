/**
 * The links to How it works in the app shell (the status bar, the phone's More sheet), and the
 * shape every string of `copy.ts` has. A module of its own so the shell's first load carries these
 * three strings, not the whole of `copy.ts` (whose `COPY` spreads them: one home, one id each).
 */

export interface CopyEntry {
    readonly text: string;
    /** Where it shows (for the owner and reviewers; not rendered). */
    readonly where: string;
    /** Where each factual statement comes from (not rendered). */
    readonly sources: string;
}

export const c = (text: string, where: string, sources = 'none (no factual claim)'): CopyEntry => ({
    text,
    where,
    sources,
});

export const NAV_COPY = {
    'nav.howItWorks': c(
        'How it works',
        'Status bar (the app’s footer; desktop and tablet, and the phone’s expanded status list), before “Security”',
    ),
    'more.howItWorks.title': c('How it works', 'Phone: the More sheet’s first row'),
    'more.howItWorks.sub': c('Shorting, lending and the risks', 'Phone: the More sheet row’s muted subtitle'),
} as const satisfies Record<string, CopyEntry>;
