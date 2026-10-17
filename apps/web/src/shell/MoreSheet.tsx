/**
 * The phone tab bar's "More" (FRONTEND.md §7.6): the destinations that do not fit four tabs —
 * How it works (the owner's thirteenth round, O-3), protocol status, the security policy, create a
 * market, then GitHub, Discord and X (new tabs) — and the network facts the desktop status
 * bar shows on its right (SOL/USD, math version, cluster). A bottom sheet on the native `<dialog>`.
 */
import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { indexerQueries } from '../api/queries.ts';
import { Term } from '../components/Glossary.tsx';
import { Modal } from '../components/Modal.tsx';
import { NAV_COPY } from '../config/copyNav.ts';
import { config } from '../config/env.ts';
import { DISCORD_URL, SOURCE_URL, X_HANDLE, X_URL } from '../config/links.ts';
import { T } from '../config/testids.ts';
import { useMath } from '../math/useMath.ts';
import { BrandIcon, NavIcon } from './icons.tsx';

export function MoreSheet({ open, onClose }: { open: boolean; onClose: () => void }) {
    const stats = useQuery({ ...indexerQueries.stats(), enabled: open });
    const solUsd = stats.data?.data.sol_usd ?? null;
    const { math, error: mathError } = useMath();
    return (
        <Modal open={open} onClose={onClose} title="More" testId={T.moreSheet} variant="sheet">
            <nav aria-label="More" className="more-links">
                {/* A row: a leading icon, the title, its muted line, a trailing chevron (CSS places them). */}
                <Link to="/how-it-works" className="more-row" data-testid={T.moreHowItWorks} onClick={onClose}>
                    <NavIcon name="info" className="more-icon" />
                    {NAV_COPY['more.howItWorks.title'].text}
                    <span className="muted">{NAV_COPY['more.howItWorks.sub'].text}</span>
                    <NavIcon name="chevron" className="more-chevron" />
                </Link>
                <Link to="/status" className="more-row" data-testid={T.moreStatus} onClick={onClose}>
                    <NavIcon name="status" className="more-icon" />
                    Status
                    <span className="muted">Indexer, venues, paused markets</span>
                    <NavIcon name="chevron" className="more-chevron" />
                </Link>
                <Link to="/security" className="more-row" data-testid={T.moreSecurity} onClick={onClose}>
                    <NavIcon name="shield" className="more-icon" />
                    Security policy
                    <span className="muted">Report a vulnerability</span>
                    <NavIcon name="chevron" className="more-chevron" />
                </Link>
                <Link to="/markets/new" className="more-row" data-testid={T.moreCreateMarket} onClick={onClose}>
                    <NavIcon name="plus" className="more-icon" />
                    Create market
                    <span className="muted">List a token for lending and shorting</span>
                    <NavIcon name="chevron" className="more-chevron" />
                </Link>
                {/* The source, the community and X: other sites, so the trailing glyph is the arrow out of a box. */}
                <a
                    href={SOURCE_URL}
                    className="more-row"
                    target="_blank"
                    rel="noopener noreferrer"
                    data-testid={T.moreGithub}
                    onClick={onClose}
                >
                    <BrandIcon name="github" className="more-icon" />
                    GitHub
                    <span className="muted">The source code</span>
                    <NavIcon name="external" className="more-external" />
                </a>
                <a
                    href={DISCORD_URL}
                    className="more-row"
                    target="_blank"
                    rel="noopener noreferrer"
                    data-testid={T.moreDiscord}
                    onClick={onClose}
                >
                    <BrandIcon name="discord" className="more-icon" />
                    Discord
                    <span className="muted">Talk with the community</span>
                    <NavIcon name="external" className="more-external" />
                </a>
                <a
                    href={X_URL}
                    className="more-row"
                    target="_blank"
                    rel="noopener noreferrer"
                    data-testid={T.moreX}
                    onClick={onClose}
                >
                    <BrandIcon name="x" className="more-icon" />X<span className="muted">@{X_HANDLE}</span>
                    <NavIcon name="external" className="more-external" />
                </a>
            </nav>
            <section aria-label="Network" className="more-network">
                <h3>Network</h3>
                <dl className="kv">
                    <div className="kv-row">
                        <dt>SOL/USD</dt>
                        <dd className="num" data-testid={T.moreSolUsd}>
                            {solUsd === null ? '—' : `$${solUsd.toFixed(2)}`}
                        </dd>
                    </div>
                    <div className="kv-row">
                        <dt>
                            <Term>Math</Term>
                        </dt>
                        <dd className="num">{math ? math.version() : mathError ? 'unavailable' : '…'}</dd>
                    </div>
                    <div className="kv-row">
                        <dt>Cluster</dt>
                        <dd>{config.cluster}</dd>
                    </div>
                </dl>
            </section>
        </Modal>
    );
}
