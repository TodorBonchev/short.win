/**
 * The market page's indexer-backed tabs (FRONTEND.md §3.4): Positions (open), Lenders (largest
 * first, creator badges, the coin creator's share), Liquidations (kind, condition S1–S3, Q, buyback,
 * lender amount, keeper paid, borrower payout), Pools (current pool, candidates, the window,
 * adoption history), Coverage (gaps, pauses, observations) and Events.
 */
import type { Address } from '@solana/kit';
import { useQuery } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { useMemo } from 'react';
import { indexerQueries } from '../../../api/queries.ts';
import { big, type ExitEvent, type MarketDetail, type Position } from '../../../api/types.ts';
import { AddressText } from '../../../components/Address.tsx';
import { Badge, venueLabel } from '../../../components/Badge.tsx';
import { Countdown } from '../../../components/Countdown.tsx';
import { type Column, DataTable } from '../../../components/DataTable.tsx';
import { Disclosure } from '../../../components/Disclosure.tsx';
import { Bps, Price, Sol, Tokens } from '../../../components/Num.tsx';
import { Empty, ErrorState, Kv, Skeleton } from '../../../components/States.tsx';
import { BREAK_EVEN_DAYS } from '../../../config/disclosures.ts';
import { T } from '../../../config/testids.ts';
import { formatDuration, formatFixed, shortAddress } from '../../../lib/format.ts';
import { formatUtc } from '../../../lib/time.ts';
import { useNow } from '../../../shell/freshness.tsx';
import { useMarketPositionsLive } from '../../short/positions/data.ts';
import { CrValue, RiskCell } from '../../short/ui.tsx';
import { Age, SignatureLink } from '../shared/cells.tsx';
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
        { key: 'position', header: 'Position', render: (p) => <span className="num">{shortAddress(p.position)}</span> },
        { key: 'borrower', header: 'Borrower', render: (p) => <span className="num">{shortAddress(p.borrower)}</span> },
        {
            key: 'q',
            header: 'Q owed',
            align: 'right',
            render: (p) => <Tokens raw={big(p.q)} decimals={decimals} />,
        },
        { key: 'v', header: 'Vault', align: 'right', render: (p) => <Sol lamports={big(p.v)} /> },
        {
            key: 'm',
            header: 'Margin',
            align: 'right',
            render: (p) => <Sol lamports={big(p.m) + big(p.margin_added)} />,
        },
        {
            key: 'entry',
            header: 'Entry (SOL)',
            align: 'right',
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
            render: (p) => {
                const l = live.get(p.position as Address);
                return l ? (
                    <RiskCell bps={l.crBps} emaBps={l.crEmaBps} liqBps={l.crLiqBps} />
                ) : (
                    <span className="muted">—</span>
                );
            },
        },
        { key: 'opened', header: 'Open for', align: 'right', render: (p) => <Age ts={p.opened_ts} now={now} /> },
        { key: 'left', header: 'Time left', align: 'right', render: (p) => <Countdown deadline={p.deadline} /> },
        {
            key: 'state',
            header: 'State',
            render: (p) =>
                p.armed ? (
                    <Badge tone="danger" title="A SOL settlement is armed for this position">
                        Settlement armed
                    </Badge>
                ) : (
                    <Badge>Open</Badge>
                ),
        },
    ];
    if (q.isError) return <ErrorState error={q.error} retry={() => void q.refetch()} />;
    return (
        <>
            <DataTable
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

export function LendersTab({ mint, view, detail }: { mint: Address; view: MarketView; detail: MarketDetail }) {
    const q = useQuery(indexerQueries.marketLenders(mint));
    const now = useNowS();
    const decimals = view.decimals ?? 0;
    const exp = detail.creator_exposure;
    const loss = detail.lender_loss;
    if (q.isError) return <ErrorState error={q.error} retry={() => void q.refetch()} />;
    const rows = q.data?.data.data;
    return (
        <>
            <div className="row">
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
            <div className="tbl-wrap">
                <table className="tbl" data-testid={T.marketLenders} aria-busy={q.isPending || undefined}>
                    <caption className="visually-hidden">Lenders, largest share first</caption>
                    <thead>
                        <tr>
                            <th scope="col">Lender</th>
                            <th scope="col" data-align="right">
                                Share
                            </th>
                            <th scope="col" data-align="right">
                                Deposited
                            </th>
                            <th scope="col" data-align="right">
                                Withdrawn
                            </th>
                            <th scope="col" data-align="right">
                                SOL paid out
                            </th>
                            <th scope="col" data-align="right">
                                Locked until
                            </th>
                            <th scope="col" data-align="right">
                                Last activity
                            </th>
                        </tr>
                    </thead>
                    <tbody>
                        {q.isPending ? (
                            <tr>
                                <td colSpan={7}>
                                    <Skeleton width={240} />
                                </td>
                            </tr>
                        ) : (
                            rows?.map((l) => (
                                <tr
                                    key={l.owner}
                                    data-testid={T.marketLenderRow(l.owner)}
                                    data-coin-creator={String(l.is_coin_creator)}
                                    data-market-creator={String(l.is_market_creator)}
                                >
                                    <td>
                                        <AddressText address={l.owner} />
                                        <CreatorBadges l={l} />
                                    </td>
                                    <td data-align="right">
                                        {l.share_bps === null ? '—' : <Bps value={l.share_bps} />}
                                    </td>
                                    <td data-align="right">
                                        <Tokens raw={big(l.deposited_tokens)} decimals={decimals} />
                                    </td>
                                    <td data-align="right">
                                        <Tokens raw={big(l.withdrawn_tokens)} decimals={decimals} />
                                    </td>
                                    <td data-align="right">
                                        <Sol lamports={big(l.sol_paid)} />
                                    </td>
                                    <td data-align="right" className="num">
                                        {l.lock_until > now ? formatUtc(l.lock_until) : '—'}
                                    </td>
                                    <td data-align="right">
                                        <Age ts={l.last_ts} now={now} />
                                    </td>
                                </tr>
                            ))
                        )}
                    </tbody>
                </table>
                {rows && rows.length === 0 && <Empty>No lenders yet. Deposits appear here.</Empty>}
            </div>
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

export function LiquidationsTab({ mint, decimals }: { mint: Address; decimals: number }) {
    const q = useQuery(indexerQueries.marketLiquidations(mint));
    const now = useNowS();
    if (q.isError) return <ErrorState error={q.error} retry={() => void q.refetch()} />;
    const rows = q.data?.data.data;
    return (
        <div className="tbl-wrap">
            <table className="tbl" data-testid={T.marketLiquidations} aria-busy={q.isPending || undefined}>
                <caption className="visually-hidden">Recent liquidations, expiries and SOL settlements</caption>
                <thead>
                    <tr>
                        <th scope="col">When</th>
                        <th scope="col">Kind</th>
                        <th scope="col">Condition</th>
                        <th scope="col" data-align="right">
                            Q
                        </th>
                        <th scope="col" data-align="right">
                            Buyback
                        </th>
                        <th scope="col" data-align="right">
                            To lenders
                        </th>
                        <th scope="col" data-align="right">
                            Keeper paid
                        </th>
                        <th scope="col" data-align="right">
                            To borrower
                        </th>
                        <th scope="col">Position</th>
                        <th scope="col">Transaction</th>
                    </tr>
                </thead>
                <tbody>
                    {q.isPending ? (
                        <tr>
                            <td colSpan={10}>
                                <Skeleton width={240} />
                            </td>
                        </tr>
                    ) : (
                        rows?.map((x) => (
                            <tr
                                key={`${x.signature}-${x.ix_index}`}
                                data-testid={T.marketLiquidationRow(x.signature)}
                                data-kind={x.kind}
                                data-condition={x.settle_condition ?? undefined}
                            >
                                <td>
                                    <Age ts={x.ts} now={now} />
                                </td>
                                <td>
                                    {EXIT_TEXT[x.kind]}
                                    {x.expired && x.kind !== 'expiry' && <span className="muted"> (expired)</span>}
                                </td>
                                <td>
                                    {x.settle_condition ? (
                                        <Badge tone={x.settle_condition === 'S1' ? 'danger' : 'warn'}>
                                            {x.settle_condition}
                                        </Badge>
                                    ) : (
                                        <span className="muted">—</span>
                                    )}
                                </td>
                                <td data-align="right">
                                    <Tokens raw={big(x.q)} decimals={decimals} />
                                </td>
                                <td data-align="right">
                                    {x.buyback_cost === null ? (
                                        <span className="muted">—</span>
                                    ) : (
                                        <Sol lamports={big(x.buyback_cost)} />
                                    )}
                                </td>
                                <td data-align="right">
                                    {x.lender_amount === null ? (
                                        <span className="muted">—</span>
                                    ) : (
                                        <Sol lamports={big(x.lender_amount)} />
                                    )}
                                </td>
                                <td data-align="right">
                                    <Sol lamports={big(x.keeper_paid)} />
                                </td>
                                <td data-align="right">
                                    <Sol lamports={big(x.borrower_payout)} />
                                </td>
                                <td className="num">{shortAddress(x.position)}</td>
                                <td>
                                    <SignatureLink signature={x.signature} />
                                </td>
                            </tr>
                        ))
                    )}
                </tbody>
            </table>
            {rows && rows.length === 0 && (
                <Empty>No liquidations, expiries or SOL settlements on this market yet.</Empty>
            )}
        </div>
    );
}

// ---- Pools -------------------------------------------------------------------------------------

export function PoolsTab({ mint, showPoolSwitch }: { mint: Address; showPoolSwitch: boolean }) {
    const q = useQuery(indexerQueries.marketPools(mint));
    const now = useNowS();
    if (q.isError) return <ErrorState error={q.error} retry={() => void q.refetch()} />;
    const p = q.data?.data;
    return (
        <>
            {p ? (
                <Kv
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
                <div className="tbl-wrap">
                    <table className="tbl" data-testid={T.marketPools}>
                        <caption className="visually-hidden">Pools seen for this market</caption>
                        <thead>
                            <tr>
                                <th scope="col">Pool</th>
                                <th scope="col">Venue</th>
                                <th scope="col" data-align="right">
                                    Locked depth D
                                </th>
                                <th scope="col" data-align="right">
                                    Proposals
                                </th>
                                <th scope="col" data-align="right">
                                    First seen
                                </th>
                                <th scope="col">Adopted / retired</th>
                            </tr>
                        </thead>
                        <tbody>
                            {p.pools.map((x) => (
                                <tr key={x.pool}>
                                    <td>
                                        <AddressText address={x.pool} />
                                        {x.is_current && (
                                            <span className="mk-badges">
                                                <Badge tone="ok">Current</Badge>
                                            </span>
                                        )}
                                    </td>
                                    <td>{x.venue ? venueLabel(x.venue) : '—'}</td>
                                    <td data-align="right" className="num" data-value={x.last_d}>
                                        {x.last_d}
                                    </td>
                                    <td data-align="right" className="num">
                                        {x.proposals}
                                    </td>
                                    <td data-align="right">
                                        <Age ts={x.first_seen_ts} now={now} />
                                    </td>
                                    <td className="num">
                                        {x.adopted_ts ? formatUtc(x.adopted_ts) : '—'}
                                        {x.retired_ts ? ` → ${formatUtc(x.retired_ts)}` : ''}
                                    </td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                    {p.pools.length === 0 && <Empty>No pool proposals or adoptions yet.</Empty>}
                </div>
            )}
            {showPoolSwitch && <Disclosure id="market.poolSwitch" />}
        </>
    );
}

// ---- Coverage ----------------------------------------------------------------------------------

export function CoverageTab({ mint }: { mint: Address }) {
    const q = useQuery(indexerQueries.marketCoverage(mint));
    const now = useNowS();
    if (q.isError) return <ErrorState error={q.error} retry={() => void q.refetch()} />;
    const c = q.data?.data;
    if (!c) return <Skeleton width={240} />;
    const s = c.stats;
    return (
        <div className="stack" data-testid={T.marketCoverage} data-paused={String(s.paused_coverage)}>
            <div className="mk-grid-2">
                <Kv
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
                        ['Gaps (all time)', `${s.gaps} · ${formatDuration(s.gap_s_total)}`],
                        ['Longest gap', s.gap_s_max === null ? '—' : formatDuration(s.gap_s_max)],
                        ['Gaps (24 h)', `${s.gaps_24h} · ${formatDuration(s.gap_s_24h)}`],
                    ]}
                />
                <Kv
                    rows={[
                        ['Coverage pauses', `${s.coverage_pauses} · ${formatDuration(s.coverage_paused_s)}`],
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
                    <table className="tbl">
                        <caption className="visually-hidden">Recent coverage gaps</caption>
                        <thead>
                            <tr>
                                <th scope="col">When</th>
                                <th scope="col" data-align="right">
                                    Gap
                                </th>
                                <th scope="col" data-align="right">
                                    Open positions
                                </th>
                                <th scope="col">Transaction</th>
                            </tr>
                        </thead>
                        <tbody>
                            {c.recent_gaps.map((g) => (
                                <tr key={`${g.signature}-${g.ix_index}`}>
                                    <td>
                                        <Age ts={g.ts} now={now} />
                                    </td>
                                    <td data-align="right" className="num">
                                        {formatDuration(g.gap_s)}
                                    </td>
                                    <td data-align="right" className="num">
                                        {g.open_positions}
                                    </td>
                                    <td>
                                        <SignatureLink signature={g.signature} />
                                    </td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                )}
            </section>
            <section aria-label="Coverage pauses">
                <h3 className="mk-section-title">Pauses</h3>
                {c.pauses.length === 0 ? (
                    <p className="muted">New shorts were never paused for coverage here.</p>
                ) : (
                    <table className="tbl">
                        <caption className="visually-hidden">Coverage pauses</caption>
                        <thead>
                            <tr>
                                <th scope="col">Paused</th>
                                <th scope="col">Resumed</th>
                                <th scope="col" data-align="right">
                                    Duration
                                </th>
                            </tr>
                        </thead>
                        <tbody>
                            {c.pauses.map((x) => (
                                <tr key={x.paused_at}>
                                    <td className="num">{formatUtc(x.paused_at)}</td>
                                    <td className="num">
                                        {x.ongoing ? <Badge tone="warn">Ongoing</Badge> : formatUtc(x.resumed_at)}
                                    </td>
                                    <td data-align="right" className="num">
                                        {formatDuration(x.paused_s)}
                                    </td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                )}
            </section>
        </div>
    );
}

// ---- Events ------------------------------------------------------------------------------------

export function EventsTab({ mint }: { mint: Address }) {
    const q = useQuery(indexerQueries.marketEvents(mint));
    const now = useNowS();
    if (q.isError) return <ErrorState error={q.error} retry={() => void q.refetch()} />;
    const rows = q.data?.data.data;
    return (
        <div className="tbl-wrap">
            <table className="tbl" data-testid={T.marketEvents} aria-busy={q.isPending || undefined}>
                <caption className="visually-hidden">Events, newest first</caption>
                <thead>
                    <tr>
                        <th scope="col">When</th>
                        <th scope="col">Event</th>
                        <th scope="col" data-align="right">
                            Slot
                        </th>
                        <th scope="col">Transaction</th>
                    </tr>
                </thead>
                <tbody>
                    {q.isPending ? (
                        <tr>
                            <td colSpan={4}>
                                <Skeleton width={240} />
                            </td>
                        </tr>
                    ) : (
                        rows?.map((e) => (
                            <tr key={e.id} data-kind={e.kind}>
                                <td>
                                    <Age ts={e.ts} now={now} />
                                </td>
                                <td>
                                    {e.kind}
                                    {!e.finalized && <span className="muted"> · confirmed</span>}
                                </td>
                                <td data-align="right" className="num">
                                    {e.slot.toLocaleString('en-US')}
                                </td>
                                <td>
                                    <SignatureLink signature={e.signature} />
                                </td>
                            </tr>
                        ))
                    )}
                </tbody>
            </table>
            {rows && rows.length === 0 && <Empty>No events yet.</Empty>}
        </div>
    );
}
