/**
 * MARKETS track — the market page (FRONTEND.md §3.4): header (symbol, mint, tier and why, venue,
 * flags, price), banners in severity order, stat strip (inventory, utilisation, rate, OI, caps,
 * TVL, lenders), tabs (overview, positions, lenders incl. creator-wallet deposits, liquidations,
 * pools, coverage, events) and the trade panel (Short | Lend tabs) on the right. The layout and the
 * panel composition are the contract: `ShortPanel` (SHORT) and `LendPanel` (LEND) take only the mint
 * and read their own data.
 *
 * Under 1024 px (M10-RESP, FRONTEND.md §7.6) the page is one column and the trade panel is a sheet
 * (`?sheet=trade`; full screen on a phone, at the right on a tablet), opened from the action bar's
 * Short / Lend and closed by ✕, Escape, the scrim or Back. Same DOM at every width.
 *
 * The page tells its panels which banners it shows (`PageBannersContext`, the owner's thirteenth
 * round O-2): the Short panel links to the coverage banner instead of repeating it, and from the
 * trade sheet that link closes the sheet, then scrolls to the banner and focuses it.
 *
 * Sources (FE-D1): the indexer for analytics and history, the chain for the state that gates opens;
 * without the indexer (down, or a market it does not know yet) the page renders from the chain.
 */
import type { Address } from '@solana/kit';
import { Link, useNavigate, useSearch } from '@tanstack/react-router';
import { useCallback, useEffect, useId, useMemo, useRef } from 'react';
import { useLiveChannels } from '../../api/live.tsx';
import { ActionBar } from '../../components/ActionBar.tsx';
import { focusBanner, type PageBanners, PageBannersContext } from '../../components/PageBanners.tsx';
import { Panel } from '../../components/Panel.tsx';
import { Scrim, SheetHead, useSheetMode } from '../../components/Sheet.tsx';
import { ErrorState, Skeleton } from '../../components/States.tsx';
import { type TabDef, TabPanel, Tabs } from '../../components/Tabs.tsx';
import { isControlledDevnetSmokeMint } from '../../config/devnetSmokeToken.ts';
import { config as appConfig } from '../../config/env.ts';
import { T } from '../../config/testids.ts';
import { shortAddress } from '../../lib/format.ts';
import type { MarketTab } from '../../router.tsx';
import { useSheetParam } from '../../shell/sheetParam.ts';
import { TxDoneContext } from '../../tx/TxReviewModal.tsx';
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
import { type MarketFlagKey, marketBanners, SHORT_BLOCKED } from './shared/marketFlags.ts';

const TABS: readonly TabDef<MarketTab>[] = [
    { key: 'overview', label: 'Overview', testId: T.marketTab('overview') },
    { key: 'positions', label: 'Positions', testId: T.marketTab('positions') },
    { key: 'lenders', label: 'Lenders', testId: T.marketTab('lenders') },
    { key: 'liquidations', label: 'Liquidations', testId: T.marketTab('liquidations') },
    { key: 'pools', label: 'Pools', testId: T.marketTab('pools') },
    { key: 'coverage', label: 'Coverage', testId: T.marketTab('coverage') },
    { key: 'events', label: 'Events', testId: T.marketTab('events') },
];

const DEEPER_DEVNET_TEST_MINT = '2isJ4phZS9Nv1fVHRLnjGkgvrFW4cb13qa8Y6kknpump' as Address;

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
    const data = useMarketData(mint);
    const canCreateMarket = appConfig.cluster !== 'devnet' || data.config !== null;
    // A market that cannot be shorted opens on Lend (design pass P5-2); an explicit ?panel= still wins. The
    // default waits for the market's view (its flags): decided on the loading state's empty flags, the Short
    // panel mounted and then flipped to Lend (design pass P11). A failed read decides on what it has.
    const canShort = !data.flags.some((f) => SHORT_BLOCKED.has(f));
    const settled = data.view !== null || data.status !== 'loading';
    const panel = search.panel ?? (settled ? (canShort ? 'short' : 'lend') : null);
    useLiveChannels([`market:${mint}`]);
    const sheet = useSheetParam('/m/$mint');
    const tradeRef = useRef<HTMLElement>(null);
    const sheetTitleId = useId();
    const inSheet = useSheetMode(tradeRef, {
        open: sheet.open === 'trade' && data.status !== 'not-found',
        onClose: sheet.closeSheet,
        labelledBy: sheetTitleId,
    });

    // O-2: a panel's link to a banner. From the sheet, the sheet closes first (its own focus return
    // runs as it closes); the banner is focused once it has.
    const pendingBanner = useRef<string | null>(null);
    const closeSheet = sheet.closeSheet;
    const goToBanner = useCallback(
        (key: string) => {
            if (inSheet) {
                pendingBanner.current = key;
                closeSheet();
            } else focusBanner(T.marketBanner(key));
        },
        [inSheet, closeSheet],
    );
    useEffect(() => {
        const key = pendingBanner.current;
        if (inSheet || key === null) return;
        pendingBanner.current = null;
        const raf = requestAnimationFrame(() => focusBanner(T.marketBanner(key)));
        return () => cancelAnimationFrame(raf);
    }, [inSheet]);
    const bannerKeys = data.view ? marketBanners(data.flags).map((b) => b.key) : [];
    const bannerSig = bannerKeys.join(',');
    const banners = useMemo<PageBanners>(
        () => ({ shown: new Set(bannerSig ? bannerSig.split(',') : []), inSheet, focus: goToBanner }),
        [bannerSig, inSheet, goToBanner],
    );

    if (data.status === 'not-found') {
        return (
            <div className="page" data-testid={T.page('market')}>
                <div className="page-head mk-head" data-testid={T.marketHeader}>
                    <h1 className="num mk-mint-title" data-testid={T.marketMint} data-value={mint}>
                        {mint}
                    </h1>
                </div>
                <Panel title="No market" testId={T.marketNotFound}>
                    <div className="stack">
                        <p>
                            {canCreateMarket
                                ? 'This token has no short.win market yet. Anyone can create one: listing is permissionless.'
                                : 'This token has no short.win market yet. Market creation is unavailable while devnet protocol readiness cannot be confirmed.'}
                        </p>
                        {canCreateMarket && (
                            <Link
                                to="/markets/new"
                                search={{ mint }}
                                className="btn mk-not-listed-cta"
                                data-variant="primary"
                            >
                                Create a market for this mint
                            </Link>
                        )}
                    </div>
                </Panel>
            </div>
        );
    }

    const view = data.view;
    const symbol = view?.symbol ?? shortAddress(mint);
    return (
        <div className="page" data-testid={T.page('market')}>
            {/* The header, the action bar and the banners open the market column, so from 1024 px the
                trade panel starts at the top of the page: its sticky top is where it rests, and its
                submit stays above the status bar from the first screen (FRONTEND.md §3.4). */}
            <div className="layout-trade">
                <div className="stack">
                    {view ? (
                        <MarketHeader view={view} flags={data.flags} config={data.config} oracle={data.oracle} />
                    ) : (
                        <div className="page-head mk-head" data-testid={T.marketHeader}>
                            <h1 className="num mk-mint-title" data-testid={T.marketMint} data-value={mint}>
                                {mint}
                            </h1>
                            {data.status === 'loading' && <Skeleton width={160} />}
                        </div>
                    )}
                    {isControlledDevnetSmokeMint(appConfig.cluster, mint) && (
                        <div className="banner" data-tone="warn" role="status" data-testid="devnet-smoke-market-limit">
                            <span>
                                <strong>SWTEST cannot support a short on this devnet curve.</strong> Its 3% position
                                impact cap permits less than the protocol’s 0.1 SOL minimum notional. For shorting
                                tests, see the{' '}
                                <Link to="/m/$mint" params={{ mint: DEEPER_DEVNET_TEST_MINT }}>
                                    deeper Pump devnet market (2isJ…pump)
                                </Link>
                                . Opens there still depend on live price coverage and RPC availability.
                            </span>
                        </div>
                    )}
                    <ActionBar label={`Trade ${symbol}`}>
                        <button
                            type="button"
                            className="btn mk-action"
                            data-variant="short"
                            data-unavailable={canShort ? undefined : ''}
                            aria-label={`Short ${symbol}`}
                            data-testid={T.marketActionShort}
                            onClick={() => sheet.openSheet('trade', { panel: 'short' })}
                        >
                            <span aria-hidden="true">▼</span> Short
                        </button>
                        <button
                            type="button"
                            className="btn mk-action"
                            data-variant={canShort ? undefined : 'primary'}
                            aria-label={`Lend ${symbol}`}
                            data-testid={T.marketActionLend}
                            onClick={() => sheet.openSheet('trade', { panel: 'lend' })}
                        >
                            Lend
                        </button>
                    </ActionBar>
                    {data.indexerMismatch && (
                        <div className="banner" data-tone="danger" role="alert" data-testid={T.marketIndexerMismatch}>
                            The indexer answered for this mint with another token’s data. This page shows the chain’s
                            view of <span className="num">{mint}</span> only; history and analytics are hidden until the
                            indexer agrees.
                        </div>
                    )}
                    {view && <MarketBanners flags={data.flags} config={data.config} />}
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
                            {['Rate', 'Utilisation', 'TVL', 'Size cap'].map((l) => (
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
                <Panel
                    ref={tradeRef}
                    label="Trade"
                    flush
                    className="trade-panel sheet-host"
                    head={
                        <SheetHead
                            title={`Trade ${symbol}`}
                            titleId={sheetTitleId}
                            onClose={sheet.closeSheet}
                            testId={T.tradeSheetClose}
                        />
                    }
                >
                    <Tabs
                        label="Trade"
                        idPrefix="trade"
                        value={panel ?? 'short'}
                        onChange={(p) =>
                            void navigate({
                                search: (s) => ({ ...s, panel: p }),
                                // In the sheet: switch in place, so Back still closes the sheet.
                                replace: sheet.open === 'trade',
                                state: (prev) => prev,
                            })
                        }
                        tabs={[
                            { key: 'short', label: 'Short', testId: T.marketPanelTab('short'), tone: 'short' },
                            { key: 'lend', label: 'Lend', testId: T.marketPanelTab('lend') },
                        ]}
                    />
                    <div className="panel-body">
                        {/* In the sheet, closing a confirmed review (Done, ✕, Escape) also closes the
                            sheet: the user lands on the market page, the toast (View position,
                            Explorer) whole. */}
                        <TxDoneContext.Provider value={inSheet ? sheet.closeSheet : null}>
                            <PageBannersContext.Provider value={banners}>
                                <TabPanel idPrefix="trade" tabKey={panel ?? 'short'}>
                                    {panel === null ? (
                                        <div className="stack" aria-busy="true">
                                            <Skeleton width={200} />
                                            <Skeleton width={260} />
                                            <Skeleton width={180} />
                                        </div>
                                    ) : panel === 'short' ? (
                                        <ShortPanel mint={mint} />
                                    ) : (
                                        <LendPanel mint={mint} action={search.action} />
                                    )}
                                </TabPanel>
                            </PageBannersContext.Provider>
                        </TxDoneContext.Provider>
                    </div>
                </Panel>
                <Scrim onClick={sheet.closeSheet} />
            </div>
        </div>
    );
}
