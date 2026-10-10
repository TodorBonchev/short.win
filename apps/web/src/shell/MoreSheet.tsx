/**
 * The phone tab bar's "More" (FRONTEND.md §7.6): the destinations that do not fit four tabs —
 * protocol status, the security policy, create a market — and the network facts the desktop status
 * bar shows on its right (SOL/USD, math version, cluster). A bottom sheet on the native `<dialog>`.
 */
import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { indexerQueries } from '../api/queries.ts';
import { Modal } from '../components/Modal.tsx';
import { config } from '../config/env.ts';
import { T } from '../config/testids.ts';
import { useMath } from '../math/useMath.ts';

export function MoreSheet({ open, onClose }: { open: boolean; onClose: () => void }) {
    const stats = useQuery({ ...indexerQueries.stats(), enabled: open });
    const solUsd = stats.data?.data.sol_usd ?? null;
    const { math, error: mathError } = useMath();
    return (
        <Modal open={open} onClose={onClose} title="More" testId={T.moreSheet} variant="sheet">
            <nav aria-label="More" className="more-links">
                <Link to="/status" className="more-row" data-testid={T.moreStatus} onClick={onClose}>
                    Status
                    <span className="muted">Indexer, venues, paused markets</span>
                </Link>
                <Link to="/security" className="more-row" data-testid={T.moreSecurity} onClick={onClose}>
                    Security policy
                    <span className="muted">Report a vulnerability</span>
                </Link>
                <Link to="/markets/new" className="more-row" data-testid={T.moreCreateMarket} onClick={onClose}>
                    Create market
                    <span className="muted">List a token for lending and shorting</span>
                </Link>
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
                        <dt>Math</dt>
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
