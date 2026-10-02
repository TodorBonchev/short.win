/**
 * SHORT track — the positions dashboard (FRONTEND.md §3.8; SPEC §8): one row per position of the
 * connected wallet — market, `Q` owed, vault SOL, CR now, liquidation price, P&L if closed now,
 * rent due, time left, risk, status — with add margin and close; closed positions show their exit
 * (kind, condition, payout, P&L). Open rows are live from the chain (FE-D1; `positions/data.ts`)
 * and subscribe their `position:<pubkey>` channel (≤ 40).
 */
import type { Address } from '@solana/kit';
import { useQueries } from '@tanstack/react-query';
import { Link, useNavigate, useSearch } from '@tanstack/react-router';
import { useLiveChannels } from '../../api/live.tsx';
import { indexerQueries } from '../../api/queries.ts';
import { big, type WsChannel } from '../../api/types.ts';
import { Badge, type Tone } from '../../components/Badge.tsx';
import { Countdown } from '../../components/Countdown.tsx';
import { type Column, DataTable } from '../../components/DataTable.tsx';
import { Select } from '../../components/Field.tsx';
import { Pnl, Price, Sol, Tokens } from '../../components/Num.tsx';
import { Panel } from '../../components/Panel.tsx';
import { Empty } from '../../components/States.tsx';
import { TokenId } from '../../components/TokenId.tsx';
import { T } from '../../config/testids.ts';
import { shortAddress } from '../../lib/format.ts';
import { FreshnessBadge } from '../../shell/freshness.tsx';
import { ReadAgeBadge } from '../../shell/SlotAge.tsx';
import { useWallet } from '../../wallet/useWallet.ts';
import { type PositionRowData, type StatusFilter, usePositions } from './positions/data.ts';
import { exitShort, exitState } from './positions/SettlementRisk.tsx';
import { CrValue, RiskCell } from './ui.tsx';
import './short.css';

const STATUS_OPTIONS: readonly { value: StatusFilter; label: string }[] = [
    { value: 'open', label: 'Open' },
    { value: 'closed', label: 'Closed' },
    { value: 'liquidated', label: 'Liquidated' },
    { value: 'expired', label: 'Expired' },
    { value: 'settled', label: 'Settled in SOL' },
    { value: 'all', label: 'All' },
];

const STATUS_TONE: Record<string, Tone> = {
    open: 'info',
    closed: 'neutral',
    liquidated: 'danger',
    expired: 'warn',
    settled: 'danger',
};

/** The live channels of the open rows (≤ 40, FRONTEND.md §6.4). */
export function liveChannels(rows: readonly PositionRowData[]): WsChannel[] {
    return rows
        .filter((r) => r.status === 'open')
        .slice(0, 40)
        .map((r) => `position:${r.position}` as WsChannel)
        .sort();
}

/** The exit state of an open row in one line (tooltip of its risk badge). */
function exitTitle(r: PositionRowData): string | undefined {
    if (!r.chain || !r.metrics || !r.snapshot) return undefined;
    return exitShort(exitState(r.chain, r.metrics, r.snapshot.venue, r.snapshot.now), r.chain);
}

interface Labels {
    readonly symbol: (mint: Address | null) => string | null;
    readonly decimals: (r: PositionRowData) => number | null;
}

function columns(labels: Labels): Column<PositionRowData>[] {
    const cell = (field: Parameters<typeof T.positionCell>[0]) => T.positionCell(field);
    return [
        {
            key: 'market',
            header: 'Market',
            render: (r) => (
                <span data-testid={cell('market')} data-value={r.mint ?? r.market}>
                    {r.mint ? (
                        <Link to="/m/$mint" params={{ mint: r.mint }} onClick={(e) => e.stopPropagation()}>
                            <TokenId symbol={labels.symbol(r.mint)} mint={r.mint} />
                        </Link>
                    ) : (
                        shortAddress(r.market)
                    )}
                    {r.mismatch && (
                        <span
                            className="badge"
                            data-tone="danger"
                            data-testid={T.positionDataMismatch}
                            title="The indexer names another market or token for this position than the chain does. Live numbers are shown only for the chain's market."
                        >
                            data mismatch
                        </span>
                    )}
                </span>
            ),
        },
        {
            key: 'q',
            header: 'Q owed',
            align: 'right',
            render: (r) => {
                const q = r.chain?.q ?? big(r.api?.q);
                const d = labels.decimals(r);
                return d === null ? (
                    <span className="num" data-testid={cell('q')} data-value={q.toString()}>
                        {q.toString()}
                    </span>
                ) : (
                    <Tokens raw={q} decimals={d} testId={cell('q')} />
                );
            },
        },
        {
            key: 'vault',
            header: 'Vault',
            align: 'right',
            render: (r) => <Sol lamports={r.chain?.v ?? big(r.api?.v)} testId={cell('vault')} />,
        },
        {
            key: 'cr',
            header: 'CR',
            align: 'right',
            render: (r) =>
                r.metrics && r.chain ? (
                    <CrValue bps={r.metrics.crBps} liqBps={r.chain.crLiqBps} testId={cell('cr')} />
                ) : (
                    <span className="muted" data-testid={cell('cr')}>
                        —
                    </span>
                ),
        },
        {
            key: 'liq',
            header: 'Liq. price (SOL)',
            align: 'right',
            render: (r) => {
                const d = labels.decimals(r);
                return r.metrics?.liqPriceWad != null && d !== null ? (
                    <Price wad={r.metrics.liqPriceWad} decimals={d} testId={cell('liq-price')} />
                ) : (
                    <span className="muted" data-testid={cell('liq-price')}>
                        —
                    </span>
                );
            },
        },
        {
            key: 'pnl',
            header: 'P&L',
            align: 'right',
            render: (r) => {
                const pnl =
                    r.status === 'open' ? (r.metrics?.pnl ?? null) : r.api?.exit ? big(r.api.exit.borrower_pnl) : null;
                return pnl === null ? (
                    <span className="muted" data-testid={cell('pnl')}>
                        —
                    </span>
                ) : (
                    <Pnl lamports={pnl} testId={cell('pnl')} />
                );
            },
        },
        {
            key: 'rent',
            header: 'Rent',
            align: 'right',
            render: (r) => {
                const rent = r.metrics?.rentDue ?? (r.api?.exit?.rent_paid ? big(r.api.exit.rent_paid) : null);
                return rent === null ? (
                    <span className="muted" data-testid={cell('rent')}>
                        —
                    </span>
                ) : (
                    <Sol lamports={rent} frac={6} testId={cell('rent')} />
                );
            },
        },
        {
            key: 'left',
            header: 'Time left',
            align: 'right',
            render: (r) => {
                const deadline = r.chain ? Number(r.chain.deadline) : (r.api?.deadline ?? 0);
                return r.status === 'open' ? (
                    <Countdown deadline={deadline} testId={cell('time-left')} />
                ) : (
                    <span className="muted" data-testid={cell('time-left')}>
                        —
                    </span>
                );
            },
        },
        {
            key: 'risk',
            header: 'Risk',
            render: (r) =>
                r.status === 'open' && r.chain ? (
                    <span title={exitTitle(r)}>
                        <RiskCell
                            bps={r.metrics?.crBps ?? null}
                            emaBps={r.metrics?.crEmaBps ?? null}
                            liqBps={r.chain.crLiqBps}
                            testId={cell('risk')}
                        />
                    </span>
                ) : (
                    <span className="muted" data-testid={cell('risk')}>
                        —
                    </span>
                ),
        },
        {
            key: 'status',
            header: 'Status',
            render: (r) => (
                <span data-testid={cell('status')} data-value={r.status}>
                    <Badge tone={STATUS_TONE[r.status] ?? 'neutral'}>
                        {r.status === 'settled' && r.api?.exit?.settle_condition
                            ? `Settled ${r.api.exit.settle_condition}`
                            : r.status}
                    </Badge>
                </span>
            ),
        },
        {
            key: 'actions',
            header: <span className="visually-hidden">Actions</span>,
            render: (r) =>
                r.status === 'open' ? (
                    <span className="position-actions">
                        <Link
                            to="/positions/$position"
                            params={{ position: r.position }}
                            hash="add-margin"
                            className="btn"
                            data-size="sm"
                            data-testid={T.positionAddMargin}
                            onClick={(e) => e.stopPropagation()}
                        >
                            Add margin
                        </Link>
                        <Link
                            to="/positions/$position"
                            params={{ position: r.position }}
                            hash="close"
                            className="btn"
                            data-size="sm"
                            data-variant="danger"
                            data-testid={T.positionClose}
                            onClick={(e) => e.stopPropagation()}
                        >
                            Close
                        </Link>
                    </span>
                ) : null,
        },
    ];
}

export function PositionsPage() {
    const { connected } = useWallet();
    const owner = connected?.address ?? null;
    const search = useSearch({ from: '/positions' });
    const navigate = useNavigate({ from: '/positions' });
    const status: StatusFilter = search.status ?? 'open';
    const { rows, usingChain, loading, error, readAt } = usePositions(owner, status);
    useLiveChannels(liveChannels(rows));
    const mismatched = rows.filter((r) => r.mismatch).length;

    const mints = [...new Set(rows.map((r) => r.mint).filter((m): m is Address => m !== null))];
    const details = useQueries({ queries: mints.map((m) => ({ ...indexerQueries.market(m), retry: false })) });
    const info = new Map(mints.map((m, i) => [m, details[i]?.data?.data ?? null] as const));
    const labels: Labels = {
        // Display only: the indexer's (normalised) symbol for the CHAIN's mint of the row, else none.
        symbol: (mint) => {
            const d = mint ? info.get(mint) : null;
            return d && d.mint === mint ? d.symbol : null;
        },
        decimals: (r) => r.snapshot?.decimals ?? (r.mint ? (info.get(r.mint)?.decimals ?? null) : null),
    };

    return (
        <div className="page" data-testid={T.page('positions')}>
            <div className="page-head">
                <h1>Positions</h1>
                <div className="positions-toolbar grow">
                    <Select<StatusFilter>
                        label="Status"
                        value={status}
                        options={STATUS_OPTIONS}
                        onChange={(v) => void navigate({ search: { status: v } })}
                        testId={T.positionsFilter}
                    />
                    {usingChain ? (
                        <span
                            className="badge"
                            data-tone="warn"
                            title="The indexer is unreachable; open positions are read from the chain."
                        >
                            Chain view (open only)
                        </span>
                    ) : (
                        // The list, statuses and closed exits come from the indexer (§6.6).
                        <FreshnessBadge />
                    )}
                    {owner && <ReadAgeBadge readAt={readAt} />}
                </div>
            </div>
            {mismatched > 0 && (
                <div className="banner" data-tone="danger" role="alert">
                    The indexer’s record of {mismatched} position{mismatched === 1 ? '' : 's'} names another market or
                    token than the chain does. Their live columns are computed only on the chain’s market; check the
                    position before acting.
                </div>
            )}
            <Panel flush>
                {!owner ? (
                    <Empty>Connect a wallet to see your positions.</Empty>
                ) : error ? (
                    <Empty>
                        Your positions could not be read: {error instanceof Error ? error.message : String(error)}
                    </Empty>
                ) : (
                    <DataTable
                        testId={T.positionsTable}
                        caption="Your short positions"
                        columns={columns(labels)}
                        rows={rows}
                        loading={loading}
                        rowKey={(r) => r.position}
                        rowTestId={(r) => T.positionRow(r.position)}
                        rowAttrs={(r) => ({ 'data-status': r.status })}
                        skeletonRows={4}
                        onRowActivate={(r) =>
                            void navigate({ to: '/positions/$position', params: { position: r.position } })
                        }
                        empty={
                            status === 'open' ? (
                                <>
                                    No open positions. Open a short from a <Link to="/markets">market</Link>.
                                </>
                            ) : (
                                'No positions with this status.'
                            )
                        }
                    />
                )}
            </Panel>
            <p className="short-note">
                Risk compares each position’s CR — vault SOL ÷ the SOL to buy the tokens back now — with its liquidation
                CR: Safe beyond 30% above it, Watch within 30%, At risk within 10%, At risk (live price below) when only
                the live price puts it below, and Liquidatable when it is below on both the live and the smoothed price,
                as the program checks. A position that cannot be bought back settles in SOL (S1–S3); open a position for
                its exit state and terms.
            </p>
        </div>
    );
}
