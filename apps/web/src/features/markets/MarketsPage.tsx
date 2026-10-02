/**
 * MARKETS track — the markets table (FRONTEND.md §3.3): search by mint / ticker, venue and tier
 * filters, sortable columns (TVL, OI, utilisation, created, open positions; server-side sort),
 * live updates from the `markets` channel (rows patched in place), keyset paging ("Load more").
 * While the indexer is unreachable the list is replaced by "open a mint" (§6.5): the market page
 * then reads the chain.
 */
import type { Address } from '@solana/kit';
import { Link, useNavigate, useSearch } from '@tanstack/react-router';
import { useMemo, useState } from 'react';
import type { MarketsParams } from '../../api/keys.ts';
import { useLiveChannels } from '../../api/live.tsx';
import { big, type MarketSort, type MarketSummary, type TierName, type VenueName } from '../../api/types.ts';
import { TierBadge, VenueBadge, venueLabel } from '../../components/Badge.tsx';
import { Button } from '../../components/Button.tsx';
import { type Column, DataTable } from '../../components/DataTable.tsx';
import { Checkbox, Select } from '../../components/Field.tsx';
import { Price, Sol } from '../../components/Num.tsx';
import { Panel } from '../../components/Panel.tsx';
import { ErrorState } from '../../components/States.tsx';
import { T } from '../../config/testids.ts';
import { parseAddress } from '../../lib/address.ts';
import './markets.css';
import { FreshnessBadge, useNow } from '../../shell/freshness.tsx';
import { Age, CompactTokens, FlagBadge, formatUsd, TokenLabel, UtilBar } from './shared/cells.tsx';
import { type MarketFlagKey, marketFlags } from './shared/marketFlags.ts';
import { useMarketPages } from './shared/useMarketPages.ts';
import { useUrlText } from './shared/useUrlText.ts';

const VENUE_OPTIONS: readonly VenueName[] = [
    'pump_curve',
    'pump_migrating',
    'pump_swap',
    'raydium_v4',
    'raydium_cpmm',
    'none',
];
const TIER_OPTIONS: readonly TierName[] = ['A', 'B', 'C', 'D'];
const SORTABLE: readonly MarketSort[] = ['tvl', 'oi', 'util', 'created', 'open_positions'];
export const MARKETS_PAGE_SIZE = 100;

/** The flags a summary row carries (venue bits and the latch are on the market page). */
function rowFlags(m: MarketSummary): MarketFlagKey[] {
    return marketFlags({ venue: m.venue, flags: m.flags }).filter((f) => f !== 'list-only' && f !== 'migrating');
}

function columns(now: number, dupSymbols: ReadonlySet<string>): Column<MarketSummary>[] {
    return [
        {
            key: 'token',
            header: 'Token',
            render: (m) => (
                <TokenLabel
                    mint={m.mint}
                    symbol={m.symbol}
                    name={m.name}
                    duplicate={!!m.symbol && dupSymbols.has(m.symbol)}
                />
            ),
        },
        { key: 'venue', header: 'Venue', render: (m) => <VenueBadge venue={m.venue} /> },
        { key: 'tier', header: 'Tier', render: (m) => <TierBadge tier={m.tier} /> },
        {
            key: 'price',
            header: 'Price (SOL)',
            align: 'right',
            render: (m) =>
                m.ema_price !== null && m.decimals !== null ? (
                    <Price wad={big(m.ema_price)} decimals={m.decimals} />
                ) : (
                    <span className="muted">—</span>
                ),
        },
        {
            key: 'tvl',
            header: 'TVL',
            align: 'right',
            sortable: true,
            testId: T.marketsSort('tvl'),
            render: (m) => (
                <span title={formatUsd(m.tvl_usd)}>
                    <Sol lamports={big(m.tvl_lamports)} frac={2} />
                </span>
            ),
        },
        {
            key: 'inventory',
            header: 'Inventory · free',
            align: 'right',
            render: (m) => (
                <span className="mk-pair">
                    <CompactTokens raw={big(m.inventory_tokens)} decimals={m.decimals} />
                    <span className="muted">
                        {' · '}
                        <CompactTokens raw={big(m.tokens_free)} decimals={m.decimals} />
                    </span>
                </span>
            ),
        },
        {
            key: 'util',
            header: 'Util.',
            align: 'right',
            sortable: true,
            testId: T.marketsSort('util'),
            render: (m) => <UtilBar bps={m.util_bps} />,
        },
        {
            key: 'oi',
            header: 'Open interest',
            align: 'right',
            sortable: true,
            testId: T.marketsSort('oi'),
            render: (m) => <Sol lamports={big(m.oi_lamports)} frac={2} />,
        },
        {
            key: 'open_positions',
            header: 'Positions',
            align: 'right',
            sortable: true,
            testId: T.marketsSort('open_positions'),
            render: (m) => <span className="num">{m.open_positions}</span>,
        },
        {
            key: 'flags',
            header: 'Flags',
            render: (m) => (
                <span className="mk-flags">
                    {rowFlags(m).map((f) => (
                        <FlagBadge key={f} flag={f} />
                    ))}
                </span>
            ),
        },
        {
            key: 'created',
            header: 'Age',
            align: 'right',
            sortable: true,
            testId: T.marketsSort('created'),
            render: (m) => <Age ts={m.created_ts} now={now} />,
        },
    ];
}

/** "Open a mint": the list's replacement while the indexer is unreachable (FRONTEND.md §6.5). */
function OpenMint() {
    const [text, setText] = useState('');
    const navigate = useNavigate();
    const mint = parseAddress(text);
    return (
        <form
            className="mk-open-mint"
            onSubmit={(e) => {
                e.preventDefault();
                if (mint) void navigate({ to: '/m/$mint', params: { mint } });
            }}
        >
            <label className="field-label" htmlFor="mk-open-mint">
                Open a market by its mint (read from the chain)
            </label>
            <div className="row">
                <div className="input-wrap grow">
                    <input
                        id="mk-open-mint"
                        className="input num"
                        value={text}
                        spellCheck={false}
                        autoComplete="off"
                        placeholder="Mint address"
                        data-testid={T.marketsOpenMint}
                        aria-invalid={text !== '' && !mint ? true : undefined}
                        onChange={(e) => setText(e.target.value)}
                    />
                </div>
                <Button type="submit" variant="primary" disabled={!mint}>
                    Open
                </Button>
            </div>
        </form>
    );
}

export function MarketsPage() {
    const search = useSearch({ from: '/markets' });
    const navigate = useNavigate({ from: '/markets' });
    const [text, setText] = useUrlText(
        search.q ?? '',
        (q) => void navigate({ search: (s) => ({ ...s, q: q === '' ? undefined : q }), replace: true }),
        (t) => t.trim().slice(0, 64),
    );

    const sort = search.sort ?? 'tvl';
    const order = search.order ?? 'desc';
    const params: MarketsParams = {
        q: search.q,
        venue: search.venue,
        tier: search.tier,
        has_positions: search.active,
        sort,
        order,
        limit: MARKETS_PAGE_SIZE,
    };
    const pages = useMarketPages(params);
    useLiveChannels(['markets']);

    const now = Math.floor(useNow(30_000) / 1000);
    const dupSymbols = useMemo(() => {
        const seen = new Map<string, number>();
        for (const m of pages.rows ?? []) if (m.symbol) seen.set(m.symbol, (seen.get(m.symbol) ?? 0) + 1);
        return new Set([...seen].filter(([, n]) => n > 1).map(([s]) => s));
    }, [pages.rows]);
    const cols = useMemo(() => columns(now, dupSymbols), [now, dupSymbols]);

    const searchedMint = parseAddress(search.q ?? '');
    const mintWithoutMarket =
        searchedMint !== null && pages.rows !== undefined && !pages.rows.some((m) => m.mint === searchedMint)
            ? searchedMint
            : null;

    const onSort = (key: string) => {
        if (!(SORTABLE as readonly string[]).includes(key)) return;
        const nextOrder = key === sort ? (order === 'desc' ? 'asc' : 'desc') : 'desc';
        void navigate({ search: (s) => ({ ...s, sort: key as MarketSort, order: nextOrder }) });
    };

    return (
        <div className="page" data-testid={T.page('markets')}>
            <div className="page-head">
                <h1>Markets</h1>
                <FreshnessBadge />
                <span className="grow" />
                <Link
                    to="/markets/new"
                    search={mintWithoutMarket ? { mint: mintWithoutMarket } : {}}
                    className="btn"
                    data-variant={mintWithoutMarket ? 'primary' : 'default'}
                    data-testid={T.marketsCreateLink}
                >
                    {mintWithoutMarket ? 'Create a market for this mint' : 'Create market'}
                </Link>
            </div>
            <Panel flush label="Markets">
                <div className="mk-toolbar">
                    <div className="input-wrap mk-search">
                        <input
                            className="input"
                            type="search"
                            aria-label="Filter by mint, ticker or name"
                            placeholder="Filter by mint, ticker or name"
                            value={text}
                            maxLength={64}
                            spellCheck={false}
                            autoComplete="off"
                            data-testid={T.marketsSearch}
                            onChange={(e) => setText(e.target.value)}
                        />
                    </div>
                    <Select<VenueName | 'all'>
                        label="Venue"
                        value={search.venue ?? 'all'}
                        testId={T.marketsFilterVenue}
                        options={[
                            { value: 'all', label: 'All venues' },
                            ...VENUE_OPTIONS.map((v) => ({ value: v, label: venueLabel(v) })),
                        ]}
                        onChange={(v) =>
                            void navigate({ search: (s) => ({ ...s, venue: v === 'all' ? undefined : v }) })
                        }
                    />
                    <Select<TierName | 'all'>
                        label="Tier"
                        value={search.tier ?? 'all'}
                        testId={T.marketsFilterTier}
                        options={[
                            { value: 'all', label: 'All tiers' },
                            ...TIER_OPTIONS.map((t) => ({ value: t, label: `Tier ${t}` })),
                        ]}
                        onChange={(t) =>
                            void navigate({ search: (s) => ({ ...s, tier: t === 'all' ? undefined : t }) })
                        }
                    />
                    <Checkbox
                        checked={!!search.active}
                        testId={T.marketsFilterActive}
                        onChange={(v) => void navigate({ search: (s) => ({ ...s, active: v ? true : undefined }) })}
                    >
                        With open positions
                    </Checkbox>
                    <span className="grow" />
                    {pages.rows && (
                        <span className="muted num" aria-live="polite">
                            {pages.rows.length} {pages.rows.length === 1 ? 'market' : 'markets'}
                            {pages.hasMore ? '+' : ''}
                        </span>
                    )}
                </div>
                {pages.error && !pages.rows ? (
                    <div className="stack mk-pad">
                        <ErrorState error={pages.error} retry={pages.refetch} />
                        <OpenMint />
                    </div>
                ) : (
                    <DataTable
                        caption="Markets"
                        testId={T.marketsTable}
                        columns={cols}
                        rows={pages.rows}
                        loading={pages.loading}
                        rowKey={(m) => m.mint}
                        rowTestId={(m) => T.marketsRow(m.mint)}
                        onRowActivate={(m) => void navigate({ to: '/m/$mint', params: { mint: m.mint as Address } })}
                        sort={{ key: sort, order }}
                        onSort={onSort}
                        empty={
                            <span data-testid={T.marketsEmpty}>
                                {search.q
                                    ? mintWithoutMarket
                                        ? 'This mint has no market yet. Anyone can create one: use “Create a market for this mint”.'
                                        : 'No market matches this search.'
                                    : 'No markets match these filters.'}
                            </span>
                        }
                    />
                )}
                {pages.hasMore && (
                    <div className="mk-more">
                        <Button onClick={pages.loadMore} data-testid={T.marketsLoadMore}>
                            Load more
                        </Button>
                    </div>
                )}
                {pages.loadingMore && (
                    <div className="mk-more muted" role="status">
                        Loading more markets…
                    </div>
                )}
            </Panel>
        </div>
    );
}
