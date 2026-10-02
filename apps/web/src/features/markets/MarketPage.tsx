/**
 * MARKETS track — the market page (FRONTEND.md §3.4): header (symbol, mint, tier and why, venue,
 * flags, price), banners in severity order, stat strip (inventory, utilisation, rate, OI, caps,
 * TVL, lenders), tabs (overview, positions, lenders incl. creator-wallet deposits, liquidations,
 * pools, coverage, events) and the trade panel (Short | Lend tabs) on the right. The layout and the
 * panel composition are the contract: `ShortPanel` (SHORT) and `LendPanel` (LEND) take only the mint
 * and read their own data.
 *
 * Sources (FE-D1): the indexer for analytics and history, the chain for the state that gates opens;
 * without the indexer (down, or a market it does not know yet) the page renders from the chain.
 */
import type { Address } from '@solana/kit';
import { Link, useNavigate, useSearch } from '@tanstack/react-router';
import { useLiveChannels } from '../../api/live.tsx';
import { Panel } from '../../components/Panel.tsx';
import { ErrorState, Skeleton } from '../../components/States.tsx';
import { type TabDef, TabPanel, Tabs } from '../../components/Tabs.tsx';
import { T } from '../../config/testids.ts';
import type { MarketTab } from '../../router.tsx';
import { LendPanel } from '../lend/LendPanel.tsx';
import { ShortPanel } from '../short/ShortPanel.tsx';
import './markets.css';
import { CoverageTab, EventsTab, LendersTab, LiquidationsTab, PoolsTab, PositionsTab } from './market/HistoryTabs.tsx';
import { MarketBanners } from './market/MarketBanners.tsx';
import { MarketHeader } from './market/MarketHeader.tsx';
import type { MarketView } from './market/marketView.ts';
import { OverviewTab } from './market/OverviewTab.tsx';
import { StatStrip } from './market/StatStrip.tsx';
import { useMarketData } from './market/useMarketData.ts';
import type { MarketFlagKey } from './shared/marketFlags.ts';

const TABS: readonly TabDef<MarketTab>[] = [
    { key: 'overview', label: 'Overview', testId: T.marketTab('overview') },
    { key: 'positions', label: 'Positions', testId: T.marketTab('positions') },
    { key: 'lenders', label: 'Lenders', testId: T.marketTab('lenders') },
    { key: 'liquidations', label: 'Liquidations', testId: T.marketTab('liquidations') },
    { key: 'pools', label: 'Pools', testId: T.marketTab('pools') },
    { key: 'coverage', label: 'Coverage', testId: T.marketTab('coverage') },
    { key: 'events', label: 'Events', testId: T.marketTab('events') },
];

function MarketTabs({
    mint,
    view,
    flags,
    tab,
    onTab,
}: {
    mint: Address;
    view: MarketView;
    flags: readonly MarketFlagKey[];
    tab: MarketTab;
    onTab: (t: MarketTab) => void;
}) {
    const detail = view.detail;
    // History and analytics need the indexer (FRONTEND.md §6.5): the chain view has Overview only.
    const tabs = detail ? TABS : TABS.filter((t) => t.key === 'overview');
    const active = tabs.some((t) => t.key === tab) ? tab : 'overview';
    const decimals = view.decimals ?? 0;
    const poolSwitchBanner = flags.includes('warming-up') || flags.includes('pool-window');
    return (
        <Panel flush label="Market data">
            <Tabs label="Market data" idPrefix="market" value={active} onChange={onTab} tabs={tabs} />
            <TabPanel idPrefix="market" tabKey={active}>
                <div className={`mk-tabs-body${active === 'positions' ? ' flush' : ''}`}>
                    {active === 'overview' && <OverviewTab mint={mint} view={view} />}
                    {detail && active === 'positions' && <PositionsTab mint={mint} decimals={decimals} />}
                    {detail && active === 'lenders' && <LendersTab mint={mint} view={view} detail={detail} />}
                    {detail && active === 'liquidations' && <LiquidationsTab mint={mint} decimals={decimals} />}
                    {detail && active === 'pools' && <PoolsTab mint={mint} showPoolSwitch={!poolSwitchBanner} />}
                    {detail && active === 'coverage' && <CoverageTab mint={mint} />}
                    {detail && active === 'events' && <EventsTab mint={mint} />}
                </div>
            </TabPanel>
        </Panel>
    );
}

export function MarketPage({ mint }: { mint: Address }) {
    const search = useSearch({ from: '/m/$mint' });
    const navigate = useNavigate({ from: '/m/$mint' });
    const panel = search.panel ?? 'short';
    const data = useMarketData(mint);
    useLiveChannels([`market:${mint}`]);

    if (data.status === 'not-found') {
        return (
            <div className="page" data-testid={T.page('market')}>
                <div className="page-head" data-testid={T.marketHeader}>
                    <h1 className="num" data-testid={T.marketMint} data-value={mint}>
                        {mint}
                    </h1>
                </div>
                <Panel title="No market" testId={T.marketNotFound}>
                    <p>This token has no short.win market yet. Anyone can create one: listing is permissionless.</p>
                    <Link to="/markets/new" search={{ mint }} className="btn" data-variant="primary">
                        Create a market for this mint
                    </Link>
                </Panel>
            </div>
        );
    }

    const view = data.view;
    return (
        <div className="page" data-testid={T.page('market')}>
            {view ? (
                <MarketHeader view={view} flags={data.flags} config={data.config} oracle={data.oracle} />
            ) : (
                <div className="page-head" data-testid={T.marketHeader}>
                    <h1 className="num" data-testid={T.marketMint} data-value={mint}>
                        {mint}
                    </h1>
                    {data.status === 'loading' && <Skeleton width={160} />}
                </div>
            )}
            {data.indexerMismatch && (
                <div className="banner" data-tone="danger" role="alert" data-testid={T.marketIndexerMismatch}>
                    The indexer answered for this mint with another token’s data. This page shows the chain’s view of{' '}
                    <span className="num">{mint}</span> only; history and analytics are hidden until the indexer agrees.
                </div>
            )}
            {view && <MarketBanners flags={data.flags} config={data.config} />}
            <div className="layout-trade">
                <div className="stack">
                    {view ? (
                        <>
                            <StatStrip mint={mint} view={view} config={data.config} />
                            <MarketTabs
                                mint={mint}
                                view={view}
                                flags={data.flags}
                                tab={search.tab ?? 'overview'}
                                onTab={(t) => void navigate({ search: (s) => ({ ...s, tab: t }), replace: true })}
                            />
                        </>
                    ) : data.status === 'error' ? (
                        <Panel title="Market">
                            <ErrorState error={data.error} retry={data.retry} />
                        </Panel>
                    ) : (
                        <div className="stat-strip mk-stats" aria-busy="true">
                            {['Inventory', 'Utilisation', 'Rate', 'Open interest', 'TVL'].map((l) => (
                                <div key={l} className="stat">
                                    <div className="stat-label">{l}</div>
                                    <div className="stat-value">
                                        <Skeleton />
                                    </div>
                                </div>
                            ))}
                        </div>
                    )}
                </div>
                <Panel label="Trade" flush className="trade-panel">
                    <Tabs
                        label="Trade"
                        idPrefix="trade"
                        value={panel}
                        onChange={(p) => void navigate({ search: (s) => ({ ...s, panel: p }) })}
                        tabs={[
                            { key: 'short', label: 'Short', testId: T.marketPanelTab('short'), tone: 'short' },
                            { key: 'lend', label: 'Lend', testId: T.marketPanelTab('lend') },
                        ]}
                    />
                    <div className="panel-body">
                        <TabPanel idPrefix="trade" tabKey={panel}>
                            {panel === 'short' ? (
                                <ShortPanel mint={mint} />
                            ) : (
                                <LendPanel mint={mint} action={search.action} />
                            )}
                        </TabPanel>
                    </div>
                </Panel>
            </div>
        </div>
    );
}
