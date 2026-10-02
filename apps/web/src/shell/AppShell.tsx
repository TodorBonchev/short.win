/**
 * The app shell (FRONTEND.md §3.1): skip link, header (brand, nav, search, wallet), the routed
 * page, the status bar; plus the indexer-down banner when pages fall back to RPC reads.
 */
import { useQuery } from '@tanstack/react-query';
import { Link, Outlet } from '@tanstack/react-router';
import { indexerQueries } from '../api/queries.ts';
import { config } from '../config/env.ts';
import { T } from '../config/testids.ts';
import { useAppClient } from '../rpc/context.tsx';
import { useRpcSlotTracker } from '../rpc/slot.ts';
import { WalletButton } from '../wallet/WalletButton.tsx';
import { SearchBox } from './SearchBox.tsx';
import { StatusBar } from './StatusBar.tsx';

const NAV = [
    { to: '/markets', key: 'markets', label: 'Markets' },
    { to: '/positions', key: 'positions', label: 'Positions' },
    { to: '/lend', key: 'lend', label: 'Lend' },
    { to: '/status', key: 'status', label: 'Status' },
] as const;

export function AppShell() {
    const client = useAppClient();
    useRpcSlotTracker(client);
    const status = useQuery(indexerQueries.status());
    return (
        <div className="app">
            <a className="skip-link" href="#main">
                Skip to content
            </a>
            <header className="app-header" data-testid={T.header}>
                <Link to="/markets" className="brand" aria-label="ShortWin home">
                    <span className="brand-mark" aria-hidden="true">
                        ▼
                    </span>
                    ShortWin
                </Link>
                {config.cluster !== 'mainnet' && (
                    <span className="cluster-badge" data-testid={T.clusterBadge}>
                        {config.cluster}
                    </span>
                )}
                <nav className="nav" aria-label="Main">
                    {NAV.map((n) => (
                        <Link key={n.key} to={n.to} data-testid={T.nav(n.key)} activeProps={{ 'aria-current': 'page' }}>
                            {n.label}
                        </Link>
                    ))}
                </nav>
                <SearchBox />
                <span className="header-spacer" />
                <WalletButton />
            </header>
            <main id="main" className="app-main" tabIndex={-1}>
                {status.isError && (
                    <div className="banner" data-tone="warn" role="status" data-testid={T.fallbackBanner}>
                        The indexer is unavailable. Showing on-chain data only; lists, history and analytics are paused.
                    </div>
                )}
                <Outlet />
            </main>
            <StatusBar />
        </div>
    );
}
