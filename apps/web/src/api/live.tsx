/**
 * Binds the indexer WebSocket to the query cache (FRONTEND.md §6.4): events invalidate the queries
 * they change, `markets` summaries patch cached market lists in place, `rollback` and a lagged
 * reconnect refetch every indexer query. `useLiveChannels` subscribes for as long as a component
 * is mounted.
 */
import { type QueryClient, useQueryClient } from '@tanstack/react-query';
import { createContext, type ReactNode, useContext, useEffect, useMemo } from 'react';
import { config } from '../config/env.ts';
import { createStore } from '../lib/store.ts';
import type { ApiResult } from './client.ts';
import { qk } from './keys.ts';
import { indexerMeta } from './queries.ts';
import type { MarketSummary, Page, WsChannel, WsServerMessage } from './types.ts';
import { IndexerSocket, type WsState } from './ws.ts';

export const wsState = createStore<WsState>('idle');

const SocketContext = createContext<IndexerSocket | null>(null);

/** Applies one server message to the cache. Exported for tests. */
export function applyWsMessage(qc: QueryClient, msg: WsServerMessage): void {
    switch (msg.type) {
        case 'event': {
            const [kind, key] = msg.channel.split(':') as [string, string | undefined];
            if (kind === 'market' && key) void qc.invalidateQueries({ queryKey: ['indexer', 'market', key] });
            else if (kind === 'position' && key) {
                void qc.invalidateQueries({ queryKey: qk.position(key) });
                void qc.invalidateQueries({ queryKey: ['indexer', 'borrower'] });
            } else if (kind === 'lender' && key) void qc.invalidateQueries({ queryKey: ['indexer', 'lender', key] });
            else if (kind === 'liquidations') void qc.invalidateQueries({ queryKey: qk.liquidations() });
            else if (kind === 'admin' || kind === 'coverage') {
                void qc.invalidateQueries({ queryKey: qk.venues() });
                void qc.invalidateQueries({ queryKey: qk.adminConfig() });
            }
            return;
        }
        case 'market': {
            const row = msg.data;
            qc.setQueriesData<ApiResult<Page<MarketSummary>>>({ queryKey: ['indexer', 'markets'] }, (old) =>
                old
                    ? { ...old, data: { ...old.data, data: old.data.data.map((m) => (m.mint === row.mint ? row : m)) } }
                    : old,
            );
            return;
        }
        case 'stats': {
            const meta = indexerMeta.get() ?? { indexerSlot: null, finalizedSlot: null, receivedAt: Date.now() };
            qc.setQueryData(qk.stats(), { data: msg.data, meta });
            return;
        }
        case 'rollback':
            void qc.invalidateQueries({ queryKey: qk.indexer });
            return;
        default:
            return;
    }
}

export function IndexerLiveProvider({ children }: { children: ReactNode }) {
    const qc = useQueryClient();
    const socket = useMemo(
        () =>
            new IndexerSocket({
                url: config.indexerWsUrl,
                onMessage: (m) => applyWsMessage(qc, m),
                onState: (s) => wsState.set(s),
                onResync: () => void qc.invalidateQueries({ queryKey: qk.indexer }),
            }),
        [qc],
    );
    useEffect(() => () => socket.close(), [socket]);
    return <SocketContext.Provider value={socket}>{children}</SocketContext.Provider>;
}

/** Subscribes to `channels` while mounted. Pass a stable, sorted list (or a joined key). */
export function useLiveChannels(channels: readonly WsChannel[]): void {
    const socket = useContext(SocketContext);
    const key = channels.join('|');
    useEffect(() => {
        if (!socket || key === '') return;
        const list = key.split('|') as WsChannel[];
        socket.subscribe(list);
        return () => socket.unsubscribe(list);
    }, [socket, key]);
}
