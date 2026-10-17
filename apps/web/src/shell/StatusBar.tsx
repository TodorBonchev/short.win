/**
 * The bottom status bar (FRONTEND.md §3.1, §6.6): cluster, RPC slot, indexer lag and freshness,
 * live-feed state, SOL/USD. Always visible, so stale data is never silent.
 *
 * On a phone (FRONTEND.md §7.6) the bar is a one-line strip: a button with the worst state in
 * words and a shape (● ok, ▲ warn, ✕ bad — never colour alone) that expands to every item over a
 * scrim (a tap on it folds the list — on its click, so the tap cannot fall through to the page once
 * the scrim is gone; the strip itself stays clear of it). Tablet and desktop show the items, as
 * before, and end with the GitHub, Discord and X icons (the phone has them in More). The summary is not
 * a live region: the pages' banners announce a degradation.
 */
import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { useEffect, useRef, useState } from 'react';
import { wsState } from '../api/live.tsx';
import { indexerQueries } from '../api/queries.ts';
import type { WsState } from '../api/ws.ts';
import { GlossaryTip } from '../components/Glossary.tsx';
import { NAV_COPY } from '../config/copyNav.ts';
import { config } from '../config/env.ts';
import { DISCORD_URL, SOURCE_URL, X_HANDLE, X_URL } from '../config/links.ts';
import { T } from '../config/testids.ts';
import type { Freshness } from '../lib/staleness.ts';
import { useStore } from '../lib/store.ts';
import { useMath } from '../math/useMath.ts';
import { rpcSlot } from '../rpc/slot.ts';
import { useIndexerFreshness } from './freshness.tsx';
import { BrandIcon, NavIcon } from './icons.tsx';

export type SummaryState = 'ok' | 'warn' | 'bad';

export interface StatusInputs {
    readonly rpcOk: boolean;
    readonly slot: bigint | number | null;
    readonly fresh: Freshness;
    readonly lagS: number | null;
    readonly ws: WsState;
    readonly mathError: boolean;
}

const GLYPH: Record<SummaryState, string> = { ok: '●', warn: '▲', bad: '✕' };

/** The phone strip's text: the worst state first (bad, then warn), else "All live · RPC <slot>". */
export function statusSummary(s: StatusInputs): { state: SummaryState; glyph: string; text: string } {
    const out = (state: SummaryState, text: string) => ({ state, glyph: GLYPH[state], text });
    const lag = s.lagS === null ? '' : ` +${s.lagS}s`;
    if (!s.rpcOk) return out('bad', 'RPC unreachable');
    if (s.fresh === 'down') return out('bad', 'Indexer unavailable · chain only');
    if (s.fresh === 'stale') return out('bad', `Indexer stale${lag}`);
    if (s.mathError) return out('bad', 'Math unavailable');
    if (s.fresh === 'lagging') return out('warn', `Indexer lagging${lag}`);
    if (s.ws === 'reconnecting' || s.ws === 'closed') return out('warn', 'Live feed reconnecting');
    if (s.fresh === 'unknown') return out('warn', 'Indexer freshness unknown');
    const slot = s.slot === null ? '—' : Number(s.slot).toLocaleString('en-US');
    return out('ok', `All live · RPC ${slot}`);
}

export function StatusBar() {
    const slot = useStore(rpcSlot);
    const ws = useStore(wsState);
    const stats = useQuery(indexerQueries.stats());
    const { state: fresh, lagS: lag } = useIndexerFreshness();
    const solUsd = stats.data?.data.sol_usd ?? null;
    const { math, error: mathError } = useMath();
    const [expanded, setExpanded] = useState(false);
    const bar = useRef<HTMLElement>(null);
    const summaryButton = useRef<HTMLButtonElement>(null);
    const summary = statusSummary({ rpcOk: slot.ok, slot: slot.slot, fresh, lagS: lag, ws, mathError: !!mathError });

    // The expanded list closes on Escape and on a tap outside the strip.
    useEffect(() => {
        if (!expanded) return;
        const onDown = (e: PointerEvent) => {
            // A glossary definition opened from an item renders outside the bar: not "outside".
            if ((e.target as Element).closest?.('.glossary-popover, .glossary-scrim')) return;
            if (bar.current && !bar.current.contains(e.target as Node)) setExpanded(false);
        };
        // Escape from inside the list: focus back to the summary first (the list is about to hide).
        const onKey = (e: KeyboardEvent) => {
            if (e.key !== 'Escape') return;
            if (bar.current?.contains(document.activeElement)) summaryButton.current?.focus();
            setExpanded(false);
        };
        document.addEventListener('pointerdown', onDown);
        document.addEventListener('keydown', onKey);
        return () => {
            document.removeEventListener('pointerdown', onDown);
            document.removeEventListener('keydown', onKey);
        };
    }, [expanded]);

    return (
        <footer ref={bar} className="statusbar" data-testid={T.statusBar} data-expanded={expanded ? '' : undefined}>
            {/* Inside the footer, so the document's outside-press handler ignores it: this click is the only
                closer. It closes on click, not on pointerdown: the scrim is still mounted when the tap's click is
                dispatched, so nothing underneath receives it (unmounted on pointerdown, it let that click land on
                the page — a tap over a link navigated). */}
            {expanded && <div className="status-scrim" aria-hidden="true" onClick={() => setExpanded(false)} />}
            <button
                ref={summaryButton}
                type="button"
                className="status-summary"
                data-testid={T.statusSummary}
                data-state={summary.state}
                aria-expanded={expanded}
                aria-controls="statusbar-items"
                onClick={() => setExpanded((x) => !x)}
            >
                <span className="status-glyph" aria-hidden="true">
                    {summary.glyph}
                </span>
                <span className="grow">{summary.text}</span>
                <span className="visually-hidden">, network status details</span>
                <NavIcon name="chevron" className="status-chevron" />
            </button>
            <div id="statusbar-items" className="statusbar-items">
                <span className="statusbar-item" data-testid={T.statusRpc} data-state={slot.ok ? 'ok' : 'bad'}>
                    <span className="dot" data-state={slot.ok ? 'ok' : 'bad'} aria-hidden="true" />
                    RPC{' '}
                    <span className="num">{slot.slot === null ? '—' : Number(slot.slot).toLocaleString('en-US')}</span>
                    <GlossaryTip id="rpc" />
                </span>
                <span className="statusbar-item stale-badge" data-testid={T.statusIndexer} data-state={fresh}>
                    <span
                        className="dot"
                        data-state={fresh === 'live' ? 'ok' : fresh === 'lagging' ? 'warn' : 'bad'}
                        aria-hidden="true"
                    />
                    Indexer {fresh === 'down' ? 'unavailable' : fresh}
                    {lag !== null && fresh !== 'down' && <span className="num"> +{lag}s</span>}
                    <GlossaryTip id="indexer" />
                </span>
                <span className="statusbar-item" data-testid={T.statusWs} data-state={ws}>
                    <span
                        className="dot"
                        data-state={ws === 'open' ? 'ok' : ws === 'idle' ? undefined : 'warn'}
                        aria-hidden="true"
                    />
                    Live {ws}
                    <GlossaryTip id="liveFeed" />
                </span>
                <span className="header-spacer" />
                <span className="statusbar-item" data-testid={T.statusSolUsd}>
                    SOL <span className="num">{solUsd === null ? '—' : `$${solUsd.toFixed(2)}`}</span>
                </span>
                <span
                    className="statusbar-item muted"
                    data-testid={T.statusMath}
                    data-state={math ? 'ready' : mathError ? 'error' : 'loading'}
                >
                    math {math ? math.version() : mathError ? 'unavailable' : '…'}
                    <GlossaryTip id="math" />
                </span>
                <span className="statusbar-item statusbar-cluster muted">{config.cluster}</span>
                <Link
                    to="/how-it-works"
                    className="statusbar-item"
                    data-testid={T.nav('how-it-works')}
                    onClick={() => setExpanded(false)}
                >
                    {NAV_COPY['nav.howItWorks'].text}
                </Link>
                <Link
                    to="/security"
                    className="statusbar-item"
                    data-testid={T.nav('security')}
                    onClick={() => setExpanded(false)}
                >
                    Security
                </Link>
                {/* The source, the community and X, as icons (a div: the phone's open list places its spans and links
                    by type, and hides this; the phone reaches them through More). */}
                <div className="statusbar-social">
                    <a
                        href={SOURCE_URL}
                        className="statusbar-item statusbar-icon"
                        target="_blank"
                        rel="noopener noreferrer"
                        aria-label="GitHub"
                        title="GitHub"
                        data-testid={T.statusGithub}
                    >
                        <BrandIcon name="github" />
                    </a>
                    <a
                        href={DISCORD_URL}
                        className="statusbar-item statusbar-icon"
                        target="_blank"
                        rel="noopener noreferrer"
                        aria-label="Discord"
                        title="Discord"
                        data-testid={T.statusDiscord}
                    >
                        <BrandIcon name="discord" />
                    </a>
                    <a
                        href={X_URL}
                        className="statusbar-item statusbar-icon"
                        target="_blank"
                        rel="noopener noreferrer"
                        aria-label={`X (@${X_HANDLE})`}
                        title={`X · @${X_HANDLE}`}
                        data-testid={T.statusX}
                    >
                        <BrandIcon name="x" />
                    </a>
                </div>
            </div>
        </footer>
    );
}
