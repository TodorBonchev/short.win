/**
 * MARKETS — the newcomer's intro strip above the markets list (the owner's thirteenth round, O-3 (a);
 * audit finding D-03): "Short a token", "Lend a token", "What can go wrong" in a sentence each, the
 * last linking to the approved risk texts on How it works (`#short-risks`: `borrower.liquidation`;
 * `#lend-risks`: `lender.risks`, verbatim there), and How it works itself. Text: `config/copy.ts`.
 *
 * Dismissible: Hide is remembered in this browser (`localStorage`, inside try / catch). Where
 * storage throws (a private window, blocked site data) the strip still renders, and Hide still
 * hides it for the rest of the session (a module flag: navigating back to the list keeps it hidden).
 */
import { Link } from '@tanstack/react-router';
import { useId, useState } from 'react';
import { copy } from '../../config/copy.ts';
import { T } from '../../config/testids.ts';

export const INTRO_HIDDEN_KEY = 'shortwin:markets-intro:hidden';

/** Hidden in this page session (also when storage could not remember it). */
let hiddenThisSession = false;

/** Test hook: forget the session's Hide (the module outlives a test's render). */
export function resetIntroSession(): void {
    hiddenThisSession = false;
}

function storedHidden(): boolean {
    try {
        return globalThis.localStorage?.getItem(INTRO_HIDDEN_KEY) === '1';
    } catch {
        return false;
    }
}

/** Whether the strip shows (not hidden by the user, now or before). */
export function introShown(): boolean {
    return !hiddenThisSession && !storedHidden();
}

export function MarketsIntro({ onHide }: { onHide: () => void }) {
    const [shown, setShown] = useState(introShown);
    const headingId = useId();
    if (!shown) return null;
    const hide = () => {
        hiddenThisSession = true;
        try {
            globalThis.localStorage?.setItem(INTRO_HIDDEN_KEY, '1');
        } catch {
            // storage blocked: hidden for this session only
        }
        setShown(false);
        onHide();
    };
    return (
        <section className="mk-intro" data-testid={T.marketsIntro} aria-labelledby={headingId}>
            <div className="mk-intro-head">
                <h2 id={headingId} className="mk-intro-title">
                    {copy('intro.heading')}
                </h2>
                <span className="grow" />
                <Link to="/how-it-works" className="mk-intro-link" data-testid={T.marketsIntroLink('how-it-works')}>
                    {copy('intro.more')}
                </Link>
                <button
                    type="button"
                    className="btn mk-intro-hide"
                    data-variant="ghost"
                    data-size="sm"
                    aria-label={copy('intro.dismiss.aria')}
                    data-testid={T.marketsIntroDismiss}
                    onClick={hide}
                >
                    {copy('intro.dismiss')}
                </button>
            </div>
            <div className="mk-intro-parts">
                <div className="mk-intro-part">
                    <h3>{copy('intro.short.title')}</h3>
                    <p>{copy('intro.short.body')}</p>
                </div>
                <div className="mk-intro-part">
                    <h3>{copy('intro.lend.title')}</h3>
                    <p>{copy('intro.lend.body')}</p>
                </div>
                <div className="mk-intro-part" data-tone="risk">
                    <h3 className="mk-intro-risk-title">{copy('intro.risks.title')}</h3>
                    <p>{copy('intro.risks.body')}</p>
                    <p className="mk-intro-links">
                        <Link
                            to="/how-it-works"
                            hash="short-risks"
                            className="mk-intro-link"
                            data-testid={T.marketsIntroLink('short-risks')}
                        >
                            {copy('intro.risks.linkShort')}
                        </Link>
                        <Link
                            to="/how-it-works"
                            hash="lend-risks"
                            className="mk-intro-link"
                            data-testid={T.marketsIntroLink('lend-risks')}
                        >
                            {copy('intro.risks.linkLend')}
                        </Link>
                    </p>
                </div>
            </div>
        </section>
    );
}
