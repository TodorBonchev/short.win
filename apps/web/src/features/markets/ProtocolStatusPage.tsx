/**
 * MARKETS track — protocol status (FRONTEND.md §3.9): the global pause, the venue programs (upgrade
 * latch with guard vs deploy slot, enabled and route-broken bits), the venue extras in force, the
 * markets in degraded mode (coverage-paused) and with a broken route (FE-5: filtered client-side
 * from the first pages of `/v1/markets`), the indexer's health and the keeper's coverage totals.
 * While the indexer is unavailable the pause, the bits and the latch come from the chain.
 */
import type { Address } from '@solana/kit';
import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { useLiveChannels } from '../../api/live.tsx';
import { indexerQueries } from '../../api/queries.ts';
import type { MarketSummary } from '../../api/types.ts';
import { Badge, TierBadge, VenueBadge, venueLabel } from '../../components/Badge.tsx';
import { Button } from '../../components/Button.tsx';
import { Panel } from '../../components/Panel.tsx';
import { Empty, ErrorState, Kv, Skeleton } from '../../components/States.tsx';
import { T } from '../../config/testids.ts';
import { formatDuration } from '../../lib/format.ts';
import { formatUtc } from '../../lib/time.ts';
import './markets.css';
import { useChainConfig, useChainVenueGuard } from '../../rpc/queries.ts';
import { FreshnessBadge, useIndexerFreshness } from '../../shell/freshness.tsx';
import { TokenLabel } from './shared/cells.tsx';
import { CONFIG_FLAG_PAUSED_OPENS, venueName } from './shared/marketFlags.ts';
import { useMarketPages } from './shared/useMarketPages.ts';
import {
    brokenVenues,
    degradedMarkets,
    routeBrokenMarkets,
    type VenueProgramRow,
    venueProgramRows,
    venueProgramRowsFromChain,
} from './status/statusModel.ts';

/** Markets scanned per page for the degraded lists (FE-5). */
export const STATUS_SCAN_PAGE = 500;

const VENUE_BIT_LABEL: Readonly<Record<string, string>> = {
    pump_curve: 'Pump curve',
    pump_swap: 'PumpSwap',
    raydium_v4: 'Raydium v4',
    raydium_cpmm: 'Raydium CPMM',
    jupiter: 'Jupiter',
    mock: 'Mock (test)',
};

function YesNo({ v, yes, no, tone }: { v: boolean | null; yes: string; no: string; tone: 'ok' | 'warn' | 'danger' }) {
    if (v === null) return <span className="muted">—</span>;
    return v ? <Badge tone={tone}>{yes}</Badge> : <span className="secondary">{no}</span>;
}

function VenuesTable({ rows }: { rows: readonly VenueProgramRow[] }) {
    return (
        <div className="tbl-wrap">
            <table className="tbl" data-testid={T.statusVenues}>
                <caption className="visually-hidden">Venue programs</caption>
                <thead>
                    <tr>
                        <th scope="col">Program</th>
                        <th scope="col">Serves</th>
                        <th scope="col">Opens</th>
                        <th scope="col">Route</th>
                        <th scope="col">Upgrade latch</th>
                        <th scope="col" data-align="right">
                            Guard slot
                        </th>
                        <th scope="col" data-align="right">
                            Deploy slot
                        </th>
                    </tr>
                </thead>
                <tbody>
                    {rows.map((r) => (
                        <tr
                            key={r.name}
                            data-testid={T.statusVenueRow(r.name)}
                            data-upgraded={String(r.upgraded)}
                            data-enabled={r.enabled === null ? 'unknown' : String(r.enabled)}
                            data-route-broken={String(r.routeBroken)}
                        >
                            <td>{r.label}</td>
                            <td className="secondary">{r.serves.join(', ')}</td>
                            <td>
                                {r.enabled === null ? (
                                    <span className="muted">—</span>
                                ) : r.enabled ? (
                                    <Badge tone="ok">Enabled</Badge>
                                ) : (
                                    <Badge>Disabled</Badge>
                                )}
                            </td>
                            <td>
                                <YesNo v={r.routeBroken} yes="Broken" no="OK" tone="danger" />
                            </td>
                            <td>
                                {r.upgraded ? (
                                    <Badge
                                        tone="warn"
                                        title={r.flaggedAt ? `Flagged ${formatUtc(r.flaggedAt)}` : undefined}
                                    >
                                        Upgraded, under review
                                    </Badge>
                                ) : (
                                    <span className="secondary">Validated</span>
                                )}
                            </td>
                            <td data-align="right" className="num">
                                {r.guardSlot}
                            </td>
                            <td data-align="right" className="num">
                                {r.deploySlot}
                            </td>
                        </tr>
                    ))}
                </tbody>
            </table>
        </div>
    );
}

function MarketList({
    rows,
    testId,
    rowTestId,
    empty,
    caption,
}: {
    rows: readonly MarketSummary[];
    testId?: string;
    rowTestId: (m: MarketSummary) => string;
    empty: string;
    caption: string;
}) {
    return (
        <div className="tbl-wrap">
            <table className="tbl" data-testid={testId}>
                <caption className="visually-hidden">{caption}</caption>
                <thead>
                    <tr>
                        <th scope="col">Token</th>
                        <th scope="col">Venue</th>
                        <th scope="col">Tier</th>
                        <th scope="col" data-align="right">
                            Open positions
                        </th>
                    </tr>
                </thead>
                <tbody>
                    {rows.map((m) => (
                        <tr key={m.mint} data-testid={rowTestId(m)} data-kind="market">
                            <td>
                                <Link to="/m/$mint" params={{ mint: m.mint as Address }}>
                                    <TokenLabel mint={m.mint} symbol={m.symbol} name={m.name} />
                                </Link>
                            </td>
                            <td>
                                <VenueBadge venue={m.venue} />
                            </td>
                            <td>
                                <TierBadge tier={m.tier} />
                            </td>
                            <td data-align="right" className="num">
                                {m.open_positions}
                            </td>
                        </tr>
                    ))}
                </tbody>
            </table>
            {rows.length === 0 && <Empty>{empty}</Empty>}
        </div>
    );
}

export function ProtocolStatusPage() {
    useLiveChannels(['admin', 'coverage', 'markets']);
    const venues = useQuery(indexerQueries.venues());
    const admin = useQuery(indexerQueries.adminConfig());
    const status = useQuery(indexerQueries.status());
    const stats = useQuery(indexerQueries.stats());
    const fresh = useIndexerFreshness();
    const configQ = useChainConfig();
    const guardQ = useChainVenueGuard(venues.isError);
    const scan = useMarketPages({ sort: 'tvl', order: 'desc', limit: STATUS_SCAN_PAGE });

    const config = configQ.data?.exists ? configQ.data.data : null;
    const guard = guardQ.data?.exists ? guardQ.data.data : null;
    const v = venues.data?.data ?? null;
    const programRows = v ? venueProgramRows(v) : venues.isError ? venueProgramRowsFromChain(guard, config) : null;
    const pausedOpens: boolean | null =
        admin.data?.data.paused_opens ??
        v?.paused_opens ??
        (config ? (config.flags & CONFIG_FLAG_PAUSED_OPENS) !== 0 : null);
    const routeBits = v?.route_broken_bits ?? config?.venueRouteBroken ?? 0;
    const scanned = scan.rows ?? [];
    const degraded = degradedMarkets(scanned);
    const broken = routeBrokenMarkets(scanned, routeBits);
    const brokenV = brokenVenues(routeBits);
    const s = status.data?.data;

    return (
        <div className="page" data-testid={T.page('status')}>
            <div className="page-head">
                <h1>Protocol status</h1>
                <FreshnessBadge />
            </div>
            <div className="stack" data-testid={T.statusPage}>
                <div className="mk-status-grid">
                    <Panel title="New shorts">
                        <div
                            className="mk-status-pause"
                            data-testid={T.statusPausedOpens}
                            data-value={pausedOpens === null ? 'unknown' : String(pausedOpens)}
                        >
                            {pausedOpens === null ? (
                                <Skeleton width={160} />
                            ) : pausedOpens ? (
                                <Badge tone="danger">Paused on every market</Badge>
                            ) : (
                                <Badge tone="ok">Open</Badge>
                            )}
                        </div>
                        <p className="mk-note">
                            The emergency pause stops new shorts only; closes, liquidations, settlements, withdrawals
                            and claims always continue.
                        </p>
                    </Panel>
                    <Panel title="Indexer">
                        <div className="stack">
                            <div className="row" data-testid={T.statusIndexerLag} data-state={fresh.state}>
                                <Badge
                                    tone={fresh.state === 'live' ? 'ok' : fresh.state === 'lagging' ? 'warn' : 'danger'}
                                >
                                    {fresh.state === 'down' ? 'Unavailable' : fresh.state}
                                </Badge>
                                {fresh.lagS !== null && fresh.state !== 'down' && (
                                    <span className="num">{fresh.lagS} s behind the RPC node</span>
                                )}
                            </div>
                            {status.isError ? (
                                <ErrorState error={status.error} retry={() => void status.refetch()} />
                            ) : s ? (
                                <div data-testid={T.statusIndexerHealth}>
                                    <Kv
                                        rows={[
                                            [
                                                'Confirmed slot',
                                                <span key="c" className="num">
                                                    {s.confirmed_slot.toLocaleString('en-US')}
                                                </span>,
                                            ],
                                            [
                                                'Finalized slot',
                                                <span key="f" className="num">
                                                    {s.finalized_slot.toLocaleString('en-US')}
                                                </span>,
                                            ],
                                            ['Ingest lag', s.lag_s === null ? '—' : formatDuration(s.lag_s)],
                                            ['Quarantined events (24 h)', String(s.quarantined)],
                                            ['Layout warnings (24 h)', String(s.layout_warnings)],
                                            ['Missing transactions (24 h)', String(s.missing)],
                                            ['Truncated logs (24 h)', String(s.logs_truncated)],
                                            [
                                                'Backfill',
                                                s.streams.every((x) => x.backfill_complete)
                                                    ? 'complete'
                                                    : `running (${s.streams
                                                          .filter((x) => !x.backfill_complete)
                                                          .map((x) => x.stream)
                                                          .join(', ')})`,
                                            ],
                                        ]}
                                    />
                                </div>
                            ) : (
                                <Skeleton width={200} />
                            )}
                        </div>
                    </Panel>
                    <Panel title="Keeper coverage">
                        {stats.data ? (
                            <div
                                className="stack"
                                data-testid={T.statusCoverageGap}
                                data-value={String(stats.data.data.coverage_gap_s)}
                            >
                                <Kv
                                    rows={[
                                        ['Coverage gaps (total)', formatDuration(stats.data.data.coverage_gap_s)],
                                        ['Markets in degraded mode', String(degraded.length)],
                                        ['Open positions', String(stats.data.data.open_positions)],
                                    ]}
                                />
                            </div>
                        ) : stats.isError ? (
                            <ErrorState error={stats.error} retry={() => void stats.refetch()} />
                        ) : (
                            <Skeleton width={200} />
                        )}
                    </Panel>
                </div>

                <Panel title="Venue programs">
                    {programRows ? (
                        <VenuesTable rows={programRows} />
                    ) : venues.isPending ? (
                        <Skeleton width={240} />
                    ) : (
                        <ErrorState error={venues.error} retry={() => void venues.refetch()} />
                    )}
                    {venues.isError && programRows && (
                        <p className="mk-note">Read from the chain: the indexer is unavailable.</p>
                    )}
                </Panel>

                {v && (
                    <Panel title="Venue extras in force">
                        {v.extras.length === 0 ? (
                            <Empty>No venue extras set.</Empty>
                        ) : (
                            <table className="tbl" data-testid={T.statusExtras}>
                                <caption className="visually-hidden">Venue extras sets</caption>
                                <thead>
                                    <tr>
                                        <th scope="col">Venue</th>
                                        <th scope="col" data-align="right">
                                            Instruction
                                        </th>
                                        <th scope="col" data-align="right">
                                            Version
                                        </th>
                                        <th scope="col" data-align="right">
                                            Stable version
                                        </th>
                                        <th scope="col" data-align="right">
                                            Accounts
                                        </th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {v.extras.map((x) => (
                                        <tr key={`${x.venue}-${x.ix}`}>
                                            <td>{venueLabel(venueName(x.venue))}</td>
                                            <td data-align="right" className="num">
                                                {x.ix}
                                            </td>
                                            <td data-align="right" className="num">
                                                {x.version}
                                            </td>
                                            <td data-align="right" className="num">
                                                {x.stable_version}
                                            </td>
                                            <td data-align="right" className="num">
                                                {x.keys.length}
                                            </td>
                                        </tr>
                                    ))}
                                </tbody>
                            </table>
                        )}
                    </Panel>
                )}

                <div className="mk-status-grid">
                    <Panel
                        title="Degraded markets (coverage paused)"
                        actions={
                            <span
                                className="muted num"
                                data-testid={T.statusScanned}
                                data-value={String(scanned.length)}
                            >
                                {scanned.length} scanned
                            </span>
                        }
                        flush
                    >
                        {scan.error && !scan.rows ? (
                            <ErrorState error={scan.error} retry={scan.refetch} />
                        ) : scan.loading ? (
                            <div className="mk-pad">
                                <Skeleton width={240} />
                            </div>
                        ) : (
                            <MarketList
                                rows={degraded}
                                testId={T.statusDegraded}
                                rowTestId={(m) => T.statusDegradedRow(m.mint)}
                                caption="Markets in degraded mode"
                                empty="No market is in degraded mode."
                            />
                        )}
                    </Panel>
                    <Panel title="Route broken" flush>
                        <div data-testid={T.statusRouteBroken}>
                            {brokenV.length > 0 && (
                                <ul className="mk-reasons mk-pad">
                                    {brokenV.map((b) => (
                                        <li
                                            key={b.key}
                                            className="mk-reason"
                                            data-testid={T.statusRouteBrokenRow(b.key)}
                                            data-kind="venue"
                                        >
                                            Venue {VENUE_BIT_LABEL[b.key] ?? b.key}: route marked broken
                                        </li>
                                    ))}
                                </ul>
                            )}
                            {scan.rows && (
                                <MarketList
                                    rows={broken}
                                    rowTestId={(m) => T.statusRouteBrokenRow(m.mint)}
                                    caption="Markets with a broken route"
                                    empty="No market has a broken route."
                                />
                            )}
                        </div>
                    </Panel>
                </div>
                <p className="mk-note">
                    These lists scan the {scanned.length} largest markets by TVL; markets with a running no-route chain
                    show it on their own page (the list endpoint has no route-chain filter yet).
                    {scan.hasMore && (
                        <>
                            {' '}
                            <Button size="sm" onClick={scan.loadMore}>
                                Scan more
                            </Button>
                        </>
                    )}
                </p>
            </div>
        </div>
    );
}
