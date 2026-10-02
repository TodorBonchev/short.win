/**
 * The bottom status bar (FRONTEND.md §3.1, §6.6): cluster, RPC slot, indexer lag and freshness,
 * live-feed state, SOL/USD. Always visible, so stale data is never silent.
 */
import { useQuery } from '@tanstack/react-query';
import { wsState } from '../api/live.tsx';
import { indexerQueries } from '../api/queries.ts';
import { config } from '../config/env.ts';
import { T } from '../config/testids.ts';
import { useStore } from '../lib/store.ts';
import { useMath } from '../math/useMath.ts';
import { rpcSlot } from '../rpc/slot.ts';
import { useIndexerFreshness } from './freshness.tsx';

export function StatusBar() {
    const slot = useStore(rpcSlot);
    const ws = useStore(wsState);
    const stats = useQuery(indexerQueries.stats());
    const { state: fresh, lagS: lag } = useIndexerFreshness();
    const solUsd = stats.data?.data.sol_usd ?? null;
    const { math, error: mathError } = useMath();
    return (
        <footer className="statusbar" data-testid={T.statusBar}>
            <span className="statusbar-item" data-testid={T.statusRpc} data-state={slot.ok ? 'ok' : 'bad'}>
                <span className="dot" data-state={slot.ok ? 'ok' : 'bad'} aria-hidden="true" />
                RPC <span className="num">{slot.slot?.toString() ?? '—'}</span>
            </span>
            <span className="statusbar-item stale-badge" data-testid={T.statusIndexer} data-state={fresh}>
                <span
                    className="dot"
                    data-state={fresh === 'live' ? 'ok' : fresh === 'lagging' ? 'warn' : 'bad'}
                    aria-hidden="true"
                />
                Indexer {fresh === 'down' ? 'unavailable' : fresh}
                {lag !== null && fresh !== 'down' && <span className="num"> +{lag}s</span>}
            </span>
            <span className="statusbar-item" data-testid={T.statusWs} data-state={ws}>
                <span
                    className="dot"
                    data-state={ws === 'open' ? 'ok' : ws === 'idle' ? undefined : 'warn'}
                    aria-hidden="true"
                />
                Live {ws}
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
            </span>
            <span className="statusbar-item muted">{config.cluster}</span>
        </footer>
    );
}
