/**
 * How it works (`/how-it-works`; the owner's thirteenth round, O-3 (c), audit finding D-03): the
 * newcomer's page on shorting, lending, what can go wrong, fees, and the review before signing. Its
 * own chunk (`lazyRouteComponent`); no wallet needed. The text is `config/copy.ts`'s
 * {@link HOW_IT_WORKS}, rendered top to bottom; the risk sections embed the approved disclosures
 * verbatim (`<Disclosure>`, no acknowledgement checkbox: nothing is signed here, FRONTEND.md §4.1
 * rule 3), `borrower.liquidation` with Tier C's live terms as a captioned example.
 *
 * Governable numbers come from the live `GlobalConfig` (§4.1 rule 5); until it is read each shows a
 * placeholder skeleton, never a word in its place. Linked from the markets list's intro strip, the
 * status bar and the phone's More sheet; `#short-risks` / `#lend-risks` (the strip's risk links)
 * scroll to and focus their heading, and a link to a collapsed disclosure opens it.
 */
import type { GlobalConfig } from '@shortwin/client';
import { Link, useRouterState } from '@tanstack/react-router';
import { type MouseEvent, type ReactNode, useEffect } from 'react';
import { Disclosure } from '../../components/Disclosure.tsx';
import { ErrorState, Skeleton } from '../../components/States.tsx';
import { COPY, type CopyId, copy, copyParts, type GuideBlock, HOW_IT_WORKS } from '../../config/copy.ts';
import {
    borrowerDisclosureVars,
    keeperRewardText,
    lenderRiskVars,
    percent,
    proseDuration,
} from '../../config/disclosureVars.ts';
import { T } from '../../config/testids.ts';
import { formatBps, formatSol } from '../../lib/format.ts';
import { useChainConfig } from '../../rpc/queries.ts';
import './guide.css';

/** The example tier for the borrower's disclosure (index 2 = C; open question 1 to the owner). */
const EXAMPLE_TIER = 2;
const TIER_LETTERS = ['A', 'B', 'C', 'D'] as const;

/** The page's placeholders from the live config. */
export function guideVars(cfg: GlobalConfig): Record<string, string> {
    const p = cfg.params;
    const example = cfg.tiers[EXAMPLE_TIER];
    const durations = cfg.tiers.map((t) => t.maxDurationS);
    return {
        exTier: TIER_LETTERS[EXAMPLE_TIER],
        exCrOpen: example ? formatBps(example.crOpenBps, 0) : '—',
        durMin: proseDuration(Math.min(...durations)),
        durMax: proseDuration(Math.max(...durations)),
        openFee: percent(p.openFeeBps),
        rentProtocol: percent(p.rentProtocolBps),
        penLender: percent(p.penaltyLenderBps),
        penProtocol: percent(p.penaltyProtocolBps),
        penKeeper: percent(p.penaltyKeeperBps),
        expiryFee: percent(p.expiryFeeBps),
        keeperReward: keeperRewardText(p.settleKeeperFixed, p.settleKeeperBps, p.settleKeeperCap),
        creationFee: `${formatSol(p.marketCreationFee)} SOL`,
    };
}

/** Opens a collapsed disclosure the anchor holds (or is), then scrolls to and focuses the anchor. */
function reveal(anchor: string, smooth: boolean): boolean {
    const el = document.getElementById(anchor);
    if (!el) return false;
    const details = el instanceof HTMLDetailsElement ? el : el.querySelector('details');
    if (details) details.open = true;
    const reduced = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? true;
    el.scrollIntoView?.({ block: 'start', behavior: smooth && !reduced ? 'smooth' : 'auto' });
    const target = details?.querySelector('summary') ?? el;
    target.focus({ preventScroll: true });
    return true;
}

/** One copy string: its text, its placeholders (a skeleton until read) and its links. */
function CopyText({
    id,
    vars,
    failed = false,
}: {
    id: CopyId;
    vars: Readonly<Record<string, string>> | null;
    /** The config could not be read: a quiet dash, not a skeleton that never resolves. */
    failed?: boolean;
}) {
    return (
        <>
            {copyParts(id).map((part, i) => {
                const key = `${id}-${i}`;
                if (part.kind === 'text') return <span key={key}>{part.text}</span>;
                if (part.kind === 'var') {
                    const v = vars?.[part.name];
                    if (v !== undefined) return <span key={key}>{v}</span>;
                    return failed ? (
                        <span key={key} className="muted">
                            —
                        </span>
                    ) : (
                        <Skeleton key={key} width={48} />
                    );
                }
                if (part.target.startsWith('#')) {
                    const anchor = part.target.slice(1);
                    return (
                        <a
                            key={key}
                            href={part.target}
                            onClick={(e: MouseEvent) => {
                                if (reveal(anchor, true)) e.preventDefault();
                            }}
                        >
                            {part.label}
                        </a>
                    );
                }
                return (
                    // The copy links only to the app's own static pages (copy.test.ts checks them).
                    <Link key={key} to={part.target as '/security' | '/status'}>
                        {part.label}
                    </Link>
                );
            })}
        </>
    );
}

function Block({
    b,
    vars,
    failed,
    disclosureVars,
}: {
    b: GuideBlock;
    vars: Readonly<Record<string, string>> | null;
    failed: boolean;
    disclosureVars: (d: string) => Readonly<Record<string, string>> | null;
}): ReactNode {
    switch (b.kind) {
        case 'h2':
            return (
                <h2 id={b.anchor} tabIndex={-1} data-testid={T.howItWorksSection(b.anchor)}>
                    {COPY[b.id].text}
                </h2>
            );
        case 'h3':
            return (
                <h3 id={b.anchor} tabIndex={-1} data-testid={T.howItWorksSection(b.anchor)}>
                    {COPY[b.id].text}
                </h3>
            );
        case 'p':
            return (
                <p className={b.className}>
                    <CopyText id={b.id} vars={vars} failed={failed} />
                </p>
            );
        case 'list':
            return (
                <ul className="guide-list">
                    {b.ids.map((id) => (
                        <li key={id}>
                            <CopyText id={id} vars={vars} failed={failed} />
                        </li>
                    ))}
                </ul>
            );
        case 'disclosure': {
            const dv = disclosureVars(b.disclosure);
            const body =
                dv === null ? (
                    failed ? (
                        <p className="guide-missing">{copy('howItWorks.configError')}</p>
                    ) : (
                        <div className="guide-loading" aria-busy="true">
                            <Skeleton width={240} />
                        </div>
                    )
                ) : (
                    <Disclosure id={b.disclosure} vars={dv} collapsible={!b.open} />
                );
            return b.anchor ? (
                <div id={b.anchor} className="guide-anchor">
                    {body}
                </div>
            ) : (
                body
            );
        }
    }
}

export function HowItWorksPage() {
    const config = useChainConfig();
    const cfg = config.data?.exists ? config.data.data : null;
    const vars = cfg ? guideVars(cfg) : null;
    const example = cfg?.tiers[EXAMPLE_TIER];
    const disclosureVars = (d: string): Readonly<Record<string, string>> | null => {
        if (d === 'borrower.liquidation' || d === 'borrower.residuals')
            return cfg && example ? borrowerDisclosureVars(example, cfg.params) : null;
        if (d === 'lender.risks') return lenderRiskVars(cfg?.params);
        return {};
    };

    // `#short-risks` / `#lend-risks` from the intro strip (or any anchor): once the page has its
    // numbers (the layout above the anchor settles), scroll to it and focus it.
    const hash = useRouterState({ select: (s) => s.location.hash });
    // The read failed, or found no config account: say so (with Retry) rather than shimmer forever.
    const failed = config.isError || (config.data !== undefined && !config.data.exists);
    const settled = cfg !== null || failed;
    useEffect(() => {
        if (!hash || !settled) return;
        const raf = requestAnimationFrame(() => reveal(hash, false));
        return () => cancelAnimationFrame(raf);
    }, [hash, settled]);

    const sections = HOW_IT_WORKS.flatMap((b) => (b.kind === 'h2' ? [b] : []));
    return (
        <div className="page guide-page" data-testid={T.page('how-it-works')}>
            <div className="page-head">
                <h1>{copy('howItWorks.title')}</h1>
            </div>
            <p className="guide-lede">{copy('howItWorks.lead')}</p>
            <nav className="guide-toc" aria-labelledby="guide-toc-title">
                <h2 id="guide-toc-title" className="guide-toc-title">
                    {copy('howItWorks.toc.aria')}
                </h2>
                <ul>
                    {sections.map((s) => (
                        <li key={s.anchor}>
                            <a
                                href={`#${s.anchor}`}
                                onClick={(e) => {
                                    if (reveal(s.anchor, true)) e.preventDefault();
                                }}
                            >
                                {COPY[s.id].text}
                            </a>
                        </li>
                    ))}
                </ul>
            </nav>
            {failed && (
                <div className="guide-error" data-testid={T.howItWorksConfigError}>
                    <ErrorState error={new Error(copy('howItWorks.configError'))} retry={() => void config.refetch()} />
                </div>
            )}
            <article className="guide-body">
                {HOW_IT_WORKS.map((b, i) => (
                    <Block key={i} b={b} vars={vars} failed={failed} disclosureVars={disclosureVars} />
                ))}
            </article>
            <p className="guide-foot">
                <Link to="/markets">{copy('howItWorks.back')}</Link>
            </p>
        </div>
    );
}
