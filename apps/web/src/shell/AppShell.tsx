/**
 * The app shell (FRONTEND.md §3.1, §7.6): skip link, header (brand, nav, search, wallet), the routed
 * page, the status bar; plus the indexer-down banner when pages fall back to RPC reads and a
 * devnet preview notice while GlobalConfig is absent.
 *
 * One DOM at every width; CSS places it. On a phone the `<nav>` is the bottom tab bar (icons, with
 * Status moved into the More sheet that its fourth cell opens), the search folds into a button and
 * the status bar into a one-line summary strip. The document scrolls (not `main`).
 */
import { useQuery } from '@tanstack/react-query';
import { Link, Outlet, useRouterState } from '@tanstack/react-router';
import { useState } from 'react';
import { indexerQueries } from '../api/queries.ts';
import { config } from '../config/env.ts';
import { T } from '../config/testids.ts';
import { useAppClient } from '../rpc/context.tsx';
import { useChainConfig } from '../rpc/queries.ts';
import { useRpcSlotTracker } from '../rpc/slot.ts';
import { WalletButton } from '../wallet/WalletButton.tsx';
import { NavIcon } from './icons.tsx';
import { BrandMark } from './Logo.tsx';
import { MoreSheet } from './MoreSheet.tsx';
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
    const devnetConfig = useChainConfig(config.cluster === 'devnet');
    const canCreateMarket = config.cluster !== 'devnet' || devnetConfig.data?.exists === true;
    const [more, setMore] = useState(false);
    // On a phone, Status, the security policy and How it works are reached through More: it marks
    // where you are.
    const pathname = useRouterState({ select: (s) => s.location.pathname });
    const inMore = ['/status', '/security', '/how-it-works'].some((p) => pathname.startsWith(p));
    return (
        <div className="app">
            <a className="skip-link" href="#main">
                Skip to content
            </a>
            <header className="app-header" data-testid={T.header}>
                <Link to="/markets" className="brand" aria-label="ShortWin home">
                    <BrandMark />
                    <span className="brand-word">ShortWin</span>
                </Link>
                {config.cluster !== 'mainnet' && (
                    <span className="cluster-badge" data-testid={T.clusterBadge}>
                        {config.cluster}
                    </span>
                )}
                <nav className="nav" aria-label="Main">
                    {NAV.map((n) => (
                        <Link
                            key={n.key}
                            to={n.to}
                            data-nav={n.key}
                            data-testid={T.nav(n.key)}
                            activeProps={{ 'aria-current': 'page' }}
                        >
                            <NavIcon name={n.key} />
                            <span className="nav-label">{n.label}</span>
                        </Link>
                    ))}
                    <button
                        type="button"
                        className="nav-more"
                        data-testid={T.navMore}
                        data-current={inMore ? '' : undefined}
                        // The More tab stands for /status, /security and /how-it-works on a phone (their own tab
                        // links are hidden there): the current state, for assistive technology too.
                        aria-current={inMore ? 'true' : undefined}
                        aria-haspopup="dialog"
                        aria-expanded={more}
                        onClick={() => setMore(true)}
                    >
                        <NavIcon name="more" />
                        <span className="nav-label">More</span>
                    </button>
                </nav>
                <SearchBox />
                <span className="header-spacer" />
                <WalletButton />
            </header>
            <main id="main" className="app-main" tabIndex={-1}>
                {config.cluster === 'devnet' && devnetConfig.data?.exists === false && (
                    <div className="banner" data-tone="warn" role="status" data-testid={T.devnetPreviewBanner}>
                        <strong>Devnet preview — protocol not initialized.</strong> Markets cannot be created or traded
                        until setup is complete.
                    </div>
                )}
                {status.isError && (
                    <div className="banner" data-tone="warn" role="status" data-testid={T.fallbackBanner}>
                        The indexer is unavailable. Showing on-chain data only; lists, history and analytics are paused.
                    </div>
                )}
                <Outlet />
            </main>
            <StatusBar />
            <MoreSheet open={more} onClose={() => setMore(false)} canCreateMarket={canCreateMarket} />
        </div>
    );
}
