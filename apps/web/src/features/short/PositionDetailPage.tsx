/**
 * SHORT track — one position (FRONTEND.md §3.8): the dashboard fields live from the chain (`Q`
 * owed, vault, CR on both legs, liquidation price, rent, P&L if closed now, time left), the terms
 * locked at open, the settlement state and how the position can end, the event timeline from the
 * indexer; add margin (CR preview) and close (buyback, rent, payout, P&L; the straddle notice on a
 * completing curve, FE-3). A closed position shows its exit from the indexer.
 */
import { fetchMaybeMarket } from '@shortwin/client';
import type { Address } from '@solana/kit';
import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { useLiveChannels } from '../../api/live.tsx';
import { indexerQueries } from '../../api/queries.ts';
import { big, type IndexerEvent, type Position, type WsChannel } from '../../api/types.ts';
import { Badge, RiskBadge, riskLevel } from '../../components/Badge.tsx';
import { Countdown } from '../../components/Countdown.tsx';
import { Pnl, Price, Sol, Tokens } from '../../components/Num.tsx';
import { Panel } from '../../components/Panel.tsx';
import { Empty, ErrorState, Kv, Skeleton } from '../../components/States.tsx';
import { TokenId } from '../../components/TokenId.tsx';
import { explorerTx } from '../../config/env.ts';
import { T } from '../../config/testids.ts';
import { formatBps, formatDuration, shortAddress } from '../../lib/format.ts';
import { formatUtc } from '../../lib/time.ts';
import { useAppClient } from '../../rpc/context.tsx';
import { FreshnessBadge } from '../../shell/freshness.tsx';
import { ReadAgeView, useReadAge } from '../../shell/SlotAge.tsx';
import { useChainNow, usePositionAccount, useShortSnapshot, useTokenLabel } from './hooks.ts';
import { AddMarginForm } from './positions/AddMarginForm.tsx';
import { CloseForm } from './positions/CloseForm.tsx';
import { SettlementRisk } from './positions/SettlementRisk.tsx';
import { positionMetrics, TIER_NAMES } from './previews.ts';
import { CrValue } from './ui.tsx';
import './short.css';

/**
 * The mint of the market an open position names, from the chain (M9-SEC-2): the `Market` account
 * at `position.market` — always read, whatever the indexer says; a market's mint never changes.
 */
function useChainMintOf(market: Address | null): Address | null {
    const client = useAppClient();
    const q = useQuery({
        queryKey: ['chain', 'market-by-address', market ?? 'none'] as const,
        queryFn: async () => {
            if (!market) return null;
            const m = await fetchMaybeMarket(client.rpc, market, { commitment: 'confirmed' });
            return m.exists ? m.data.mint : null;
        },
        enabled: !!market,
        staleTime: Number.POSITIVE_INFINITY,
    });
    return q.data ?? null;
}

function Timeline({ events }: { events: readonly IndexerEvent[] }) {
    if (!events.length) return <Empty>No events yet.</Empty>;
    return (
        <ol className="timeline">
            {events.map((e) => (
                <li key={e.id}>
                    <span className="num muted">{formatUtc(e.ts)}</span>
                    <span>{e.kind}</span>
                    <a className="num" href={explorerTx(e.signature)} target="_blank" rel="noopener noreferrer">
                        {shortAddress(e.signature, 8, 8)}
                    </a>
                </li>
            ))}
        </ol>
    );
}

function ClosedSummary({ p, decimals, symbol }: { p: Position; decimals: number | null; symbol: string }) {
    const e = p.exit;
    return (
        <Kv
            rows={[
                [
                    'Status',
                    <Badge key="s" tone={p.status === 'closed' ? 'neutral' : 'danger'}>
                        {p.status}
                        {e?.settle_condition ? ` ${e.settle_condition}` : ''}
                    </Badge>,
                ],
                ['Q', decimals === null ? p.q : <Tokens key="q" raw={big(p.q)} decimals={decimals} symbol={symbol} />],
                ['Buyback', e?.buyback_cost ? <Sol key="b" lamports={big(e.buyback_cost)} /> : '—'],
                ['Rent paid', e?.rent_paid ? <Sol key="r" lamports={big(e.rent_paid)} frac={6} /> : '—'],
                ['Penalty', e?.penalty ? <Sol key="pe" lamports={big(e.penalty)} /> : '—'],
                ['Expiry fee', e?.expiry_fee ? <Sol key="ef" lamports={big(e.expiry_fee)} /> : '—'],
                ['Keeper paid', e?.keeper_paid ? <Sol key="k" lamports={big(e.keeper_paid)} /> : '—'],
                ['You received', e ? <Sol key="pay" lamports={big(e.borrower_payout)} /> : '—'],
                ['P&L', e ? <Pnl key="pnl" lamports={big(e.borrower_pnl)} /> : '—'],
                ['Duration', e ? formatDuration(e.duration_s) : '—'],
            ]}
        />
    );
}

export function PositionDetailPage({ position }: { position: Address }) {
    const chain = usePositionAccount(position);
    const detail = useQuery({ ...indexerQueries.position(position), retry: false });
    const api = detail.data?.data.position ?? null;
    const pos = chain.data ?? null;
    // An open position is priced on the chain's market only; the indexer's mint labels a closed one.
    const chainMint = useChainMintOf(pos?.market ?? null);
    const mint = pos ? chainMint : ((api?.mint as Address | undefined) ?? null);
    const mismatch = !!pos && !!api && (api.market !== pos.market || (chainMint !== null && api.mint !== chainMint));
    const snap = useShortSnapshot(pos ? mint : null);
    const { symbol } = useTokenLabel(mint);
    const now = useChainNow(snap.data ?? null);
    useLiveChannels([`position:${position}` as WsChannel]);
    const age = useReadAge(snap.data?.slot ?? null, snap.data?.fetchedAt ?? null);

    const fetched = snap.data ?? null;
    // The snapshot must be of the position's own market (a defence in depth over the chain's mint).
    const s = fetched && pos && fetched.marketAddress === pos.market ? fetched : null;
    const math = snap.math;
    const metrics = pos && s && math ? positionMetrics(math, { ...s, now }, pos) : null;
    const decimals = s?.decimals ?? null;
    const tier = pos ? s?.config.tiers[pos.tier] : undefined;
    const open = !!pos;
    const loading = chain.isLoading || (open && !s && !snap.error);

    return (
        <div className="page" data-testid={T.page('position')}>
            <div className="stack" data-testid={T.positionDetail} data-value={position}>
                <div className="page-head">
                    <h1>
                        Short <bdi>{symbol}</bdi>{' '}
                        <span className="muted num" title={position}>
                            {shortAddress(position, 6, 6)}
                        </span>
                    </h1>
                    {mint && (
                        <Link to="/m/$mint" params={{ mint }}>
                            Market <TokenId symbol={null} mint={mint} />
                        </Link>
                    )}
                    {open ? <Badge tone="info">open</Badge> : api ? <Badge>{api.status}</Badge> : null}
                    {open && metrics && pos && metrics.crBps !== null && (
                        <RiskBadge level={riskLevel(metrics.crBps, metrics.crEmaBps, pos.crLiqBps)} />
                    )}
                    {open && <ReadAgeView age={age} />}
                </div>
                {mismatch && (
                    <div className="banner" data-tone="danger" role="alert" data-testid={T.positionDataMismatch}>
                        The indexer’s record of this position names another market or token than the chain does. The
                        numbers below are the chain’s, for market <span className="num">{pos?.market}</span>.
                    </div>
                )}
                {chain.error ? (
                    <ErrorState error={chain.error} retry={() => void chain.refetch()} />
                ) : loading ? (
                    <Panel>
                        <Skeleton width={240} />
                    </Panel>
                ) : !open ? (
                    api ? (
                        <Panel title="Closed">
                            <ClosedSummary p={api} decimals={decimals} symbol={symbol} />
                        </Panel>
                    ) : (
                        <Empty>
                            No open position at this address
                            {detail.isError ? ' (the indexer is unreachable for its history)' : ''}.
                        </Empty>
                    )
                ) : pos && s && math && metrics ? (
                    <div className="position-layout">
                        <div className="stack">
                            <Panel title="Position">
                                <Kv
                                    rows={[
                                        [
                                            'Q owed',
                                            decimals === null ? (
                                                pos.q.toString()
                                            ) : (
                                                <Tokens
                                                    key="q"
                                                    raw={pos.q}
                                                    decimals={decimals}
                                                    symbol={symbol}
                                                    testId={T.positionCell('q')}
                                                />
                                            ),
                                        ],
                                        ['Vault', <Sol key="v" lamports={pos.v} testId={T.positionCell('vault')} />],
                                        ['Sale proceeds', <Sol key="s" lamports={pos.s} />],
                                        ['Margin (incl. added)', <Sol key="m" lamports={pos.m} />],
                                        ['Open fee', <Sol key="f" lamports={pos.feeOpen} />],
                                        [
                                            'CR (spot)',
                                            <CrValue
                                                key="cr"
                                                bps={metrics.crBps}
                                                liqBps={pos.crLiqBps}
                                                testId={T.positionCell('cr')}
                                            />,
                                        ],
                                        [
                                            'CR (EMA)',
                                            <CrValue key="cre" bps={metrics.crEmaBps} liqBps={pos.crLiqBps} />,
                                        ],
                                        [
                                            'Liquidation price',
                                            metrics.liqPriceWad !== null && decimals !== null ? (
                                                <Price
                                                    key="lp"
                                                    wad={metrics.liqPriceWad}
                                                    decimals={decimals}
                                                    testId={T.positionCell('liq-price')}
                                                />
                                            ) : (
                                                '—'
                                            ),
                                        ],
                                        [
                                            'Rent due',
                                            <Sol
                                                key="r"
                                                lamports={metrics.rentDue}
                                                frac={6}
                                                testId={T.positionCell('rent')}
                                            />,
                                        ],
                                        ['Minimum rent', <Sol key="rm" lamports={pos.rMin} frac={6} />],
                                        [
                                            'P&L if closed now',
                                            metrics.pnl === null ? (
                                                '—'
                                            ) : (
                                                <Pnl key="p" lamports={metrics.pnl} testId={T.positionCell('pnl')} />
                                            ),
                                        ],
                                        [
                                            'Opened',
                                            <span key="o" className="num">
                                                {formatUtc(pos.tOpen)}
                                            </span>,
                                        ],
                                        [
                                            'Deadline',
                                            <span key="d" className="num">
                                                {formatUtc(pos.deadline)}
                                            </span>,
                                        ],
                                        [
                                            'Time left',
                                            <Countdown
                                                key="t"
                                                deadline={Number(pos.deadline)}
                                                testId={T.positionCell('time-left')}
                                            />,
                                        ],
                                    ]}
                                />
                            </Panel>
                            <SettlementRisk
                                pos={pos}
                                metrics={metrics}
                                venue={s.venue}
                                market={s.market}
                                now={now}
                                tier={tier}
                            />
                            <Panel title={`Terms locked at open · Tier ${TIER_NAMES[pos.tier] ?? pos.tier}`}>
                                <Kv
                                    rows={[
                                        ['Liquidation CR', formatBps(pos.crLiqBps, 0)],
                                        ['Penalty', formatBps(pos.penaltyBps)],
                                        ['Max liquidation slippage', formatBps(pos.maxLiqSlipBps)],
                                        ['Expiry fee', formatBps(pos.expiryFeeBps)],
                                        [
                                            'Rent: base / slope 1 / slope 2',
                                            `${formatBps(pos.rateBaseBps)} / ${formatBps(pos.rateSlope1Bps)} / ${formatBps(pos.rateSlope2Bps)}`,
                                        ],
                                        ['Rent to the protocol', formatBps(pos.rentProtocolBps)],
                                        ['Venue fee hard cap', formatBps(pos.venueFeeHardCapBps)],
                                        [
                                            'Settlement keeper reward',
                                            <span key="k" className="num">
                                                <Sol lamports={pos.settleKeeperFixed} unit={false} /> SOL +{' '}
                                                {formatBps(pos.settleKeeperBps)}
                                            </span>,
                                        ],
                                        [
                                            'Settlement watch',
                                            pos.sArmed ? `armed · ${pos.sCount.toString()} observations` : 'not armed',
                                        ],
                                    ]}
                                />
                            </Panel>
                            <Panel title="Timeline" actions={<FreshnessBadge />}>
                                {detail.data ? (
                                    <Timeline events={detail.data.data.events} />
                                ) : (
                                    <Empty>The indexer is unreachable; the history will appear when it is back.</Empty>
                                )}
                            </Panel>
                        </div>
                        <Panel title="Manage">
                            <div className="stack">
                                <AddMarginForm
                                    math={math}
                                    snapshot={s}
                                    position={position}
                                    pos={pos}
                                    symbol={symbol}
                                    stale={age.state === 'stale'}
                                />
                                <CloseForm
                                    math={math}
                                    snapshot={{ ...s, now }}
                                    position={position}
                                    pos={pos}
                                    symbol={symbol}
                                    stale={age.state === 'stale'}
                                />
                            </div>
                        </Panel>
                    </div>
                ) : snap.error ? (
                    <ErrorState error={snap.error} retry={() => void snap.refetch()} />
                ) : (
                    <Skeleton width={240} />
                )}
            </div>
        </div>
    );
}
