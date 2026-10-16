/**
 * The market page's indexer-backed tabs (FRONTEND.md §3.4): Positions (open), Lenders (largest
 * first, creator badges, the coin creator's share), Liquidations (kind, condition S1–S3, Q, buyback,
 * lender amount, keeper paid, borrower payout), Pools (current pool, candidates, the window,
 * adoption history), Coverage (gaps, pauses, observations) and Events.
 *
 * Every table is a DataTable (M10-RESP): it hides its secondary columns or becomes cards by the
 * width it has (the 680 px market column at 1,024 px, a tablet, a phone), keeping its test ids and
 * row state attributes.
 */
import type { Address } from '@solana/kit';
import { useQuery } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { useMemo } from 'react';
import { indexerQueries } from '../../../api/queries.ts';
import {
    big,
    type ExitEvent,
    type IndexerEvent,
    type LenderPosition,
    type MarketCoverage,
    type MarketDetail,
    type MarketPools,
    type Position,
} from '../../../api/types.ts';
import { AddressText } from '../../../components/Address.tsx';
import { Badge, venueLabel } from '../../../components/Badge.tsx';
import { Countdown } from '../../../components/Countdown.tsx';
import { type Column, DataTable } from '../../../components/DataTable.tsx';
import { Disclosure } from '../../../components/Disclosure.tsx';
import { Term } from '../../../components/Glossary.tsx';
import { Bps, Price, Sol, Tokens } from '../../../components/Num.tsx';
import { ErrorState, Skeleton } from '../../../components/States.tsx';
import { BREAK_EVEN_DAYS } from '../../../config/disclosures.ts';
import { T } from '../../../config/testids.ts';
import { formatDuration, formatFixed, shortAddress } from '../../../lib/format.ts';
import { formatUtc } from '../../../lib/time.ts';
import { useNow } from '../../../shell/freshness.tsx';
import { useMarketPositionsLive } from '../../short/positions/data.ts';
import { CrValue, RiskCell } from '../../short/ui.tsx';
import { Age, SignatureLink } from '../shared/cells.tsx';
import { KvLines } from '../shared/KvLines.tsx';
import { CreatorBadges } from './CreatorDeposits.tsx';
import type { MarketView } from './marketView.ts';

function useNowS(): number {
    return Math.floor(useNow(15_000) / 1000);
}

// ---- Positions ---------------------------------------------------------------------------------

export function PositionsTab({ mint, decimals }: { mint: Address; decimals: number }) {
    const q = useQuery(indexerQueries.marketPositions(mint, 'open'));
    const navigate = useNavigate();
    const now = useNowS();
    const keys = useMemo(() => (q.data?.data.data ?? []).map((p) => p.position as Address), [q.data]);
    const live = useMarketPositionsLive(mint, keys);
    const cols: Column<Position>[] = [
        {
            key: 'position',
            header: 'Position',
            card: 'title',
            render: (p) => <span className="num">{shortAddress(p.position)}</span>,
        },
        {
            key: 'borrower',
            header: 'Borrower',
            priority: 2,
            render: (p) => <span className="num">{shortAddress(p.borrower)}</span>,
        },
        {
            key: 'q',
            header: 'Q owed',
            align: 'right',
            render: (p) => <Tokens raw={big(p.q)} decimals={decimals} />,
        },
        { key: 'v', header: 'Vault', align: 'right', priority: 2, render: (p) => <Sol lamports={big(p.v)} pad /> },
        {
            key: 'm',
            header: 'Margin',
            align: 'right',
            priority: 2,
            render: (p) => <Sol lamports={big(p.m) + big(p.margin_added)} pad />,
        },
        {
            key: 'entry',
            header: 'Entry (SOL)',
            align: 'right',
            priority: 2,
            render: (p) => <Price wad={big(p.entry_price)} decimals={decimals} unit={false} />,
        },
        {
            key: 'cr',
            header: 'CR now',
            align: 'right',
            render: (p) => {
                const l = live.get(p.position as Address);
                return l ? <CrValue bps={l.crBps} liqBps={l.crLiqBps} /> : <span className="muted">—</span>;
            },
        },
        {
            key: 'risk',
            header: 'Risk',
            card: 'aside',
            render: (p) => {
                const l = live.get(p.position as Address);
                return l ? (
                    <RiskCell bps={l.crBps} emaBps={l.crEmaBps} liqBps={l.crLiqBps} />
                ) : (
                    <span className="muted">—</span>
                );
            },
        },
        {
            key: 'opened',
            header: 'Open for',
            align: 'right',
            priority: 3,
            render: (p) => <Age ts={p.opened_ts} now={now} />,
        },
        {
            key: 'left',
            header: 'Time left',
            align: 'right',
            priority: 2,
            render: (p) => <Countdown deadline={p.deadline} />,
        },
        {
            key: 'state',
            header: 'State',
            priority: 2,
            render: (p) =>
                p.armed ? (
                    <Term id="armed">
                        <Badge tone="danger" title="A SOL settlement is armed for this position">
                            Settlement armed
                        </Badge>
                    </Term>
                ) : (
                    <Badge>Open</Badge>
                ),
        },
    ];
    if (q.isError) return <ErrorState error={q.error} retry={() => void q.refetch()} />;
    return (
        <>
            <DataTable
                className="mk-tbl mk-positions-tbl"
                caption="Open positions"
                testId={T.marketPositions}
                columns={cols}
                rows={q.data?.data.data}
                loading={q.isPending}
                rowKey={(p) => p.position}
                rowTestId={(p) => T.marketPositionRow(p.position)}
                onRowActivate={(p) =>
                    void navigate({ to: '/positions/$position', params: { position: p.position as Address } })
                }
                skeletonRows={3}
                empty="No open positions. Opened shorts appear here."
            />
            <p className="mk-note">
                CR now and risk are computed in your browser from the chain (the vault against the SOL to buy the tokens
                back now). Open a position for its liquidation price and P&amp;L.
            </p>
        </>
    );
}

// ---- Lenders -----------------------------------------------------------------------------------

type Lender = LenderPosition;

function lenderColumns(decimals: number, now: number): Column<Lender>[] {
    return [
        {
            key: 'lender',
            header: 'Lender',
            card: 'title',
            render: (l) => (
                <>
                    <AddressText address={l.owner} />
                    <CreatorBadges l={l} />
                </>
            ),
        },
        {
            key: 'share',
            header: 'Share',
            align: 'right',
            card: 'aside',
            render: (l) => (l.share_bps === null ? '—' : <Bps value={l.share_bps} />),
        },
        {
            key: 'deposited',
            header: 'Deposited',
            align: 'right',
            render: (l) => <Tokens raw={big(l.deposited_tokens)} decimals={decimals} />,
        },
        {
            key: 'withdrawn',
            header: 'Withdrawn',
            align: 'right',
            priority: 2,
            render: (l) => <Tokens raw={big(l.withdrawn_tokens)} decimals={decimals} />,
        },
        {
            key: 'paid',
            header: 'SOL paid out',
            align: 'right',
            render: (l) => <Sol lamports={big(l.sol_paid)} pad />,
        },
        {
            key: 'lock',
            header: 'Locked until',
            align: 'right',
            priority: 2,
            render: (l) => <span className="num">{l.lock_until > now ? formatUtc(l.lock_until) : '—'}</span>,
        },
        {
            key: 'last',
            header: 'Last activity',
            align: 'right',
            priority: 2,
            render: (l) => <Age ts={l.last_ts} now={now} />,
        },
    ];
}

export function LendersTab({ mint, view, detail }: { mint: Address; view: MarketView; detail: MarketDetail }) {
    const q = useQuery(indexerQueries.marketLenders(mint));
    const now = useNowS();
    const decimals = view.decimals ?? 0;
    const exp = detail.creator_exposure;
    const loss = detail.lender_loss;
    if (q.isError) return <ErrorState error={q.error} retry={() => void q.refetch()} />;
    return (
        <>
            <div className="row mk-creator-share">
                <span className="secondary">Coin creator’s share of the pool:</span>
                <span data-testid={T.marketCreatorShare} data-value={exp.coin_creator_share_bps ?? ''}>
                    {exp.coin_creator_share_bps === null ? (
                        <span className="muted">—</span>
                    ) : (
                        <Bps value={exp.coin_creator_share_bps} />
                    )}
                </span>
                {exp.coin_creator && <AddressText address={exp.coin_creator} />}
            </div>
            <DataTable
                className="mk-tbl"
                caption="Lenders, largest share first"
                testId={T.marketLenders}
                columns={lenderColumns(decimals, now)}
                rows={q.data?.data.data}
                loading={q.isPending}
                skeletonRows={3}
                rowKey={(l) => l.owner}
                rowTestId={(l) => T.marketLenderRow(l.owner)}
                rowAttrs={(l) => ({
                    'data-coin-creator': String(l.is_coin_creator),
                    'data-market-creator': String(l.is_market_creator),
                })}
                empty="No lenders yet. Deposits appear here."
            />
            <Disclosure
                id="lender.lossHistory"
                vars={{
                    tier: view.tier,
                    breakEvenDays: String(BREAK_EVEN_DAYS[view.tier]),
                    settlements: String(loss.settlements),
                    realisedLoss: formatFixed(big(loss.realised_lender_loss), 9, 4),
                }}
            />
        </>
    );
}

// ---- Liquidations ------------------------------------------------------------------------------

const EXIT_TEXT: Readonly<Record<ExitEvent['kind'], string>> = {
    liquidation: 'Liquidation',
    expiry: 'Expiry',
    settlement: 'SOL settlement',
};

function solOrDash(v: string | null) {
    return v === null ? <span className="muted">—</span> : <Sol lamports={big(v)} pad />;
}

function exitColumns(decimals: number, now: number): Column<ExitEvent>[] {
    return [
        { key: 'when', header: 'When', card: 'aside', render: (x) => <Age ts={x.ts} now={now} /> },
        {
            key: 'kind',
            header: 'Kind',
            card: 'title',
            render: (x) => (
                <>
                    {EXIT_TEXT[x.kind]}
                    {x.expired && x.kind !== 'expiry' && <span className="muted"> (expired)</span>}
                </>
            ),
        },
        {
            key: 'condition',
            header: 'Condition',
            render: (x) =>
                x.settle_condition ? (
                    <Badge tone={x.settle_condition === 'S1' ? 'danger' : 'warn'}>{x.settle_condition}</Badge>
                ) : (
                    <span className="muted">—</span>
                ),
        },
        { key: 'q', header: 'Q', align: 'right', render: (x) => <Tokens raw={big(x.q)} decimals={decimals} /> },
        { key: 'buyback', header: 'Buyback', align: 'right', priority: 2, render: (x) => solOrDash(x.buyback_cost) },
        { key: 'lenders', header: 'To lenders', align: 'right', render: (x) => solOrDash(x.lender_amount) },
        {
            key: 'keeper',
            header: 'Keeper paid',
            align: 'right',
            priority: 2,
            render: (x) => <Sol lamports={big(x.keeper_paid)} pad />,
        },
        {
            key: 'borrower',
            header: 'To borrower',
            align: 'right',
            priority: 2,
            render: (x) => <Sol lamports={big(x.borrower_payout)} pad />,
        },
        {
            key: 'position',
            header: 'Position',
            priority: 2,
            render: (x) => <span className="num">{shortAddress(x.position)}</span>,
        },
        { key: 'tx', header: 'Transaction', render: (x) => <SignatureLink signature={x.signature} /> },
    ];
}

export function LiquidationsTab({ mint, decimals }: { mint: Address; decimals: number }) {
    const q = useQuery(indexerQueries.marketLiquidations(mint));
    const now = useNowS();
    if (q.isError) return <ErrorState error={q.error} retry={() => void q.refetch()} />;
    return (
        <DataTable
            className="mk-tbl"
            caption="Recent liquidations, expiries and SOL settlements"
            testId={T.marketLiquidations}
            columns={exitColumns(decimals, now)}
            rows={q.data?.data.data}
            loading={q.isPending}
            skeletonRows={3}
            rowKey={(x) => `${x.signature}-${x.ix_index}`}
            rowTestId={(x) => T.marketLiquidationRow(x.signature)}
            rowAttrs={(x) => ({ 'data-kind': x.kind, 'data-condition': x.settle_condition ?? undefined })}
            empty="No liquidations, expiries or SOL settlements on this market yet."
        />
    );
}

// ---- Pools -------------------------------------------------------------------------------------

type PoolRow = MarketPools['pools'][number];

function poolColumns(now: number): Column<PoolRow>[] {
    return [
        {
            key: 'pool',
            header: 'Pool',
            card: 'title',
            render: (x) => (
                <>
                    <AddressText address={x.pool} />
                    {x.is_current && (
                        <span className="mk-badges">
                            <Badge tone="ok">Current</Badge>
                        </span>
                    )}
                </>
            ),
        },
        { key: 'venue', header: 'Venue', card: 'aside', render: (x) => (x.venue ? venueLabel(x.venue) : '—') },
        {
            key: 'depth',
            header: 'Locked depth D',
            align: 'right',
            render: (x) => (
                <span className="num" data-value={x.last_d}>
                    {x.last_d}
                </span>
            ),
        },
        {
            key: 'proposals',
            header: 'Proposals',
            align: 'right',
            priority: 2,
            render: (x) => (
                <span className="num" data-zero={x.proposals === 0 ? '' : undefined}>
                    {x.proposals}
                </span>
            ),
        },
        {
            key: 'seen',
            header: 'First seen',
            align: 'right',
            priority: 2,
            render: (x) => <Age ts={x.first_seen_ts} now={now} />,
        },
        {
            key: 'adopted',
            header: 'Adopted / retired',
            render: (x) => (
                <span className="num">
                    {x.adopted_ts ? formatUtc(x.adopted_ts) : '—'}
                    {x.retired_ts ? ` → ${formatUtc(x.retired_ts)}` : ''}
                </span>
            ),
        },
    ];
}

export function PoolsTab({ mint, showPoolSwitch }: { mint: Address; showPoolSwitch: boolean }) {
    const q = useQuery(indexerQueries.marketPools(mint));
    const now = useNowS();
    if (q.isError) return <ErrorState error={q.error} retry={() => void q.refetch()} />;
    const p = q.data?.data;
    return (
        <>
            {p ? (
                <KvLines
                    rows={[
                        [
                            'Current pool',
                            p.current ? (
                                <AddressText key="r328" address={p.current} />
                            ) : (
                                <span key="r330" className="muted">
                                    None (lend only)
                                </span>
                            ),
                        ],
                        [
                            'Switch window',
                            p.window ? (
                                <span key="r336">
                                    candidate <AddressText address={p.window.candidate} /> · D{' '}
                                    <span className="num">{p.window.d_cand}</span> · ends in{' '}
                                    <Countdown deadline={p.window.window_ends} />
                                </span>
                            ) : (
                                <span key="r342" className="muted">
                                    No window open
                                </span>
                            ),
                        ],
                    ]}
                />
            ) : (
                <Skeleton width={240} />
            )}
            {p && (
                <DataTable
                    className="mk-tbl"
                    caption="Pools seen for this market"
                    testId={T.marketPools}
                    columns={poolColumns(now)}
                    rows={p.pools}
                    rowKey={(x) => x.pool}
                    empty="No pool proposals or adoptions yet."
                />
            )}
            {showPoolSwitch && <Disclosure id="market.poolSwitch" />}
        </>
    );
}

// ---- Coverage ----------------------------------------------------------------------------------

type CoverageData = MarketCoverage;

function gapColumns(now: number): Column<CoverageData['recent_gaps'][number]>[] {
    return [
        { key: 'when', header: 'When', card: 'title', render: (g) => <Age ts={g.ts} now={now} /> },
        {
            key: 'gap',
            header: 'Gap',
            align: 'right',
            card: 'aside',
            render: (g) => <span className="num">{formatDuration(g.gap_s)}</span>,
        },
        {
            key: 'open',
            header: 'Open positions',
            align: 'right',
            render: (g) => <span className="num">{g.open_positions}</span>,
        },
        { key: 'tx', header: 'Transaction', render: (g) => <SignatureLink signature={g.signature} /> },
    ];
}

const PAUSE_COLUMNS: readonly Column<CoverageData['pauses'][number]>[] = [
    {
        key: 'paused',
        header: 'Paused',
        card: 'title',
        render: (x) => <span className="num">{formatUtc(x.paused_at)}</span>,
    },
    {
        key: 'duration',
        header: 'Duration',
        align: 'right',
        card: 'aside',
        render: (x) => <span className="num">{formatDuration(x.paused_s)}</span>,
    },
    {
        key: 'resumed',
        header: 'Resumed',
        render: (x) =>
            x.ongoing ? <Badge tone="warn">Ongoing</Badge> : <span className="num">{formatUtc(x.resumed_at)}</span>,
    },
];

export function CoverageTab({ mint }: { mint: Address }) {
    const q = useQuery(indexerQueries.marketCoverage(mint));
    const now = useNowS();
    if (q.isError) return <ErrorState error={q.error} retry={() => void q.refetch()} />;
    const c = q.data?.data;
    if (!c) return <Skeleton width={240} />;
    const s = c.stats;
    return (
        <div className="stack" data-testid={T.marketCoverage} data-paused={String(s.paused_coverage)}>
            <div className="mk-overview">
                <KvLines
                    rows={[
                        [
                            'Coverage now',
                            s.paused_coverage ? (
                                <Badge key="r424" tone="warn">
                                    Paused (degraded)
                                </Badge>
                            ) : (
                                <Badge key="r426" tone="ok">
                                    Covered
                                </Badge>
                            ),
                        ],
                        ['Gaps (all time)', `${s.gaps} · ${formatDuration(s.gap_s_total)}`, { zero: s.gaps === 0 }],
                        ['Longest gap', s.gap_s_max === null ? '—' : formatDuration(s.gap_s_max)],
                        ['Gaps (24 h)', `${s.gaps_24h} · ${formatDuration(s.gap_s_24h)}`, { zero: s.gaps_24h === 0 }],
                    ]}
                />
                <KvLines
                    rows={[
                        [
                            'Coverage pauses',
                            `${s.coverage_pauses} · ${formatDuration(s.coverage_paused_s)}`,
                            { zero: s.coverage_pauses === 0 },
                        ],
                        ['Observations (24 h)', String(s.observations_24h)],
                        ['Failed observations (24 h)', String(s.failed_observations_24h)],
                    ]}
                />
            </div>
            <section aria-label="Recent coverage gaps">
                <h3 className="mk-section-title">Recent gaps</h3>
                {c.recent_gaps.length === 0 ? (
                    <p className="muted">No coverage gaps recorded.</p>
                ) : (
                    <DataTable
                        className="mk-tbl"
                        caption="Recent coverage gaps"
                        columns={gapColumns(now)}
                        rows={c.recent_gaps}
                        rowKey={(g) => `${g.signature}-${g.ix_index}`}
                    />
                )}
            </section>
            <section aria-label="Coverage pauses">
                <h3 className="mk-section-title">Pauses</h3>
                {c.pauses.length === 0 ? (
                    <p className="muted">New shorts were never paused for coverage here.</p>
                ) : (
                    <DataTable
                        className="mk-tbl"
                        caption="Coverage pauses"
                        columns={PAUSE_COLUMNS}
                        rows={c.pauses}
                        rowKey={(x) => String(x.paused_at)}
                    />
                )}
            </section>
        </div>
    );
}

// ---- Events ------------------------------------------------------------------------------------

type EventRow = IndexerEvent;

function eventColumns(now: number): Column<EventRow>[] {
    return [
        { key: 'when', header: 'When', card: 'aside', render: (e) => <Age ts={e.ts} now={now} /> },
        {
            key: 'event',
            header: 'Event',
            card: 'title',
            render: (e) => (
                <>
                    {e.kind}
                    {!e.finalized && <span className="muted"> · confirmed</span>}
                </>
            ),
        },
        {
            key: 'slot',
            header: 'Slot',
            align: 'right',
            render: (e) => <span className="num">{e.slot.toLocaleString('en-US')}</span>,
        },
        { key: 'tx', header: 'Transaction', render: (e) => <SignatureLink signature={e.signature} /> },
    ];
}

export function EventsTab({ mint }: { mint: Address }) {
    const q = useQuery(indexerQueries.marketEvents(mint));
    const now = useNowS();
    if (q.isError) return <ErrorState error={q.error} retry={() => void q.refetch()} />;
    return (
        <DataTable
            className="mk-tbl"
            caption="Events, newest first"
            testId={T.marketEvents}
            columns={eventColumns(now)}
            rows={q.data?.data.data}
            loading={q.isPending}
            skeletonRows={3}
            rowKey={(e) => String(e.id)}
            rowAttrs={(e) => ({ 'data-kind': e.kind })}
            empty="No events yet."
        />
    );
}
