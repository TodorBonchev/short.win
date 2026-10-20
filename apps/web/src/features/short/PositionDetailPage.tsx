/**
 * SHORT track — one position (FRONTEND.md §3.8): the dashboard fields live from the chain (`Q`
 * owed, vault, CR on both legs, liquidation price, rent, P&L if closed now, time left), the terms
 * locked at open, the settlement state and how the position can end, the event timeline from the
 * indexer; add margin (CR preview) and close (buyback, rent, payout, P&L; past a live curve's `rT`
 * at `B̂_v3`, the close completing the curve: FE-3 closed). A closed position shows its exit from
 * the indexer.
 */
import { fetchMaybeMarket } from '@shortwin/client';
import type { Address } from '@solana/kit';
import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { type ReactNode, useId, useRef } from 'react';
import { ApiError } from '../../api/client.ts';
import { useLiveChannels } from '../../api/live.tsx';
import { indexerQueries } from '../../api/queries.ts';
import { big, type IndexerEvent, type Position, type WsChannel } from '../../api/types.ts';
import { ActionBar } from '../../components/ActionBar.tsx';
import { Badge, RiskBadge, riskLevel } from '../../components/Badge.tsx';
import { Button } from '../../components/Button.tsx';
import { Countdown } from '../../components/Countdown.tsx';
import { Term } from '../../components/Glossary.tsx';
import { Pnl, Price, Sol, Tokens } from '../../components/Num.tsx';
import { Panel } from '../../components/Panel.tsx';
import { Scrim, SheetHead, useSheetMode } from '../../components/Sheet.tsx';
import { Empty, ErrorState, Kv, Skeleton } from '../../components/States.tsx';
import { TokenId } from '../../components/TokenId.tsx';
import { explorerTx } from '../../config/env.ts';
import { T } from '../../config/testids.ts';
import { formatBps, formatDuration, shortAddress } from '../../lib/format.ts';
import { formatUtc } from '../../lib/time.ts';
import { useAppClient } from '../../rpc/context.tsx';
import { FreshnessBadge } from '../../shell/freshness.tsx';
import { ReadAgeView, useReadAge } from '../../shell/SlotAge.tsx';
import { useSheetParam } from '../../shell/sheetParam.ts';
import { TxDoneContext } from '../../tx/TxReviewModal.tsx';
import { ConnectCta } from '../../wallet/ConnectCta.tsx';
import { useWallet } from '../../wallet/useWallet.ts';
import { useChainNow, usePositionAccount, useShortSnapshot, useTokenLabel } from './hooks.ts';
import { AddMarginForm } from './positions/AddMarginForm.tsx';
import { CloseForm } from './positions/CloseForm.tsx';
import { HealthBanner, SettlementRisk } from './positions/SettlementRisk.tsx';
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
    if (!events.length) return <p className="short-note">No events yet.</p>;
    return (
        <ol className="timeline">
            {events.map((e) => (
                <li key={e.id}>
                    <span className="timeline-time num muted">{formatUtc(e.ts)}</span>
                    <span className="timeline-kind">{e.kind}</span>
                    <a
                        className="timeline-sig num"
                        href={explorerTx(e.signature)}
                        target="_blank"
                        rel="noopener noreferrer"
                    >
                        {shortAddress(e.signature, 8, 8)}
                    </a>
                </li>
            ))}
        </ol>
    );
}

/**
 * A position's facts as hairline key / value rows that flow, top to bottom, into as many columns as
 * the width allows (up to three; one on a phone). Callers order the rows in groups (size, risk now,
 * rent and dates) so a column reads as one group.
 */
function Facts({ rows }: { rows: readonly (readonly [string, ReactNode, boolean?])[] }) {
    return (
        <dl className="kv position-facts">
            {rows.map(([k, v, quiet]) => (
                <div key={k} className="kv-row">
                    <dt>
                        <Term>{k}</Term>
                    </dt>
                    <dd data-zero={quiet ? '' : undefined}>{v}</dd>
                </div>
            ))}
        </dl>
    );
}

function ClosedSummary({ p, decimals, symbol }: { p: Position; decimals: number | null; symbol: string }) {
    const e = p.exit;
    return (
        <Kv
            rows={[
                [
                    'Status',
                    <Badge key="s" tone={p.status === 'closed' ? 'neutral' : 'danger'} className="badge-status">
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
    const { connected } = useWallet();
    const chain = usePositionAccount(position);
    const detail = useQuery({ ...indexerQueries.position(position), retry: false });
    const api = detail.data?.data.position ?? null;
    // Confirmed history may arrive before this tab's next chain poll. Do not show cached
    // pre-exit metrics or actions after the indexer has already recorded the exit.
    const closed = api !== null && api.status !== 'open';
    const pos = closed ? null : (chain.data ?? null);
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
    const loading = (chain.isLoading && !closed) || (open && !s && !snap.error);
    const ready = !!(pos && s && math && metrics);

    // Under 1024 px the Manage panel is a sheet (FRONTEND.md §7.6): the action bar opens it at a
    // form (`?sheet=add-margin|close`), Back / Escape / ✕ close it.
    const { open: sheet, openSheet, closeSheet } = useSheetParam('/positions/$position');
    const manage = useRef<HTMLElement>(null);
    const sheetTitle = useId();
    const inSheet = useSheetMode(manage, {
        open: !!sheet && ready,
        onClose: closeSheet,
        labelledBy: sheetTitle,
        // At the chosen form: its heading scrolled to the top and focused.
        initialFocus: () => {
            const h = document.getElementById(sheet === 'close' ? 'close-title' : 'add-margin-title');
            h?.scrollIntoView?.({ block: 'start' });
            return h;
        },
    });
    const risk =
        metrics && pos && metrics.crBps !== null ? riskLevel(metrics.crBps, metrics.crEmaBps, pos.crLiqBps) : null;

    return (
        <div className="page" data-testid={T.page('position')}>
            <div className="stack" data-testid={T.positionDetail} data-value={position}>
                <div className="page-head position-head">
                    <h1>
                        {mint ? (
                            <>
                                Short <bdi>{symbol}</bdi>
                            </>
                        ) : (
                            'Position'
                        )}{' '}
                        <span className="muted num position-id" title={position}>
                            {shortAddress(position, 6, 6)}
                        </span>
                    </h1>
                    {mint && (
                        <Link to="/m/$mint" params={{ mint }} className="target-link">
                            Market <TokenId symbol={null} mint={mint} />
                        </Link>
                    )}
                    {open ? (
                        <Badge tone="info" className="badge-status">
                            open
                        </Badge>
                    ) : closed && api ? (
                        <Badge className="badge-status">{api.status}</Badge>
                    ) : api ? (
                        <Badge className="badge-status">History updating</Badge>
                    ) : null}
                    {open && risk && <RiskBadge level={risk} />}
                    {open && <ReadAgeView age={age} />}
                </div>
                {ready && (
                    <ActionBar label="Manage this position">
                        <Button
                            variant="primary"
                            aria-haspopup="dialog"
                            onClick={() => openSheet('add-margin')}
                            data-testid={T.positionActionAddMargin}
                        >
                            Add margin
                        </Button>
                        <Button
                            variant="danger"
                            aria-haspopup="dialog"
                            onClick={() => openSheet('close')}
                            data-testid={T.positionActionClose}
                        >
                            Close position
                        </Button>
                    </ActionBar>
                )}
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
                    closed && api ? (
                        <Panel title="Closed">
                            <ClosedSummary p={api} decimals={decimals} symbol={symbol} />
                        </Panel>
                    ) : api ? (
                        <Panel title="Updating position history">
                            <p role="status">
                                There is no open position on the chain. Waiting for the indexer to record its exit.
                            </p>
                        </Panel>
                    ) : detail.isLoading ? (
                        <Panel>
                            <Skeleton width={240} />
                        </Panel>
                    ) : (
                        <Empty
                            title="No position at this address"
                            action={
                                <Link
                                    to="/positions"
                                    className="btn"
                                    data-variant="primary"
                                    data-testid={T.positionNotFoundCta}
                                >
                                    Back to positions
                                </Link>
                            }
                        >
                            {/* A 404 is the indexer's answer (no history); any other error is no answer. */}
                            {detail.isError && !(detail.error instanceof ApiError && detail.error.status === 404)
                                ? 'There is no open position here, and the indexer is unreachable for its history.'
                                : 'There is no open position here and no closed one in the history.'}
                        </Empty>
                    )
                ) : pos && s && math && metrics ? (
                    <>
                        <HealthBanner pos={pos} metrics={metrics} venue={s.venue} market={s.market} now={now} />
                        <dl className="position-hero">
                            <div className="position-hero-item">
                                <dt className="position-hero-label">
                                    <Term>CR now</Term>
                                </dt>
                                <dd className="position-hero-value position-hero-cr">
                                    <CrValue bps={metrics.crBps} liqBps={pos.crLiqBps} />
                                    <span className="position-hero-sub">
                                        liquidation at {formatBps(pos.crLiqBps, 0)}
                                    </span>
                                </dd>
                            </div>
                            <div className="position-hero-item">
                                <dt className="position-hero-label">
                                    <Term>P&L if closed now</Term>
                                </dt>
                                <dd className="position-hero-value">
                                    {metrics.pnl === null ? (
                                        <span className="muted">—</span>
                                    ) : (
                                        <Pnl lamports={metrics.pnl} />
                                    )}
                                </dd>
                            </div>
                            <div className="position-hero-item">
                                <dt className="position-hero-label">
                                    <Term>Time left</Term>
                                </dt>
                                <dd className="position-hero-value">
                                    <Countdown deadline={Number(pos.deadline)} />
                                </dd>
                            </div>
                        </dl>
                        <div className="position-layout layout-side">
                            <div className="stack">
                                <Panel title="Position">
                                    <Facts
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
                                            [
                                                'Vault',
                                                <Sol key="v" lamports={pos.v} testId={T.positionCell('vault')} />,
                                            ],
                                            ['Sale proceeds', <Sol key="s" lamports={pos.s} />],
                                            ['Margin (incl. added)', <Sol key="m" lamports={pos.m} />],
                                            ['Open fee', <Sol key="f" lamports={pos.feeOpen} />, pos.feeOpen === 0n],
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
                                                metrics.liqPriceWad === null,
                                            ],
                                            [
                                                'P&L if closed now',
                                                metrics.pnl === null ? (
                                                    '—'
                                                ) : (
                                                    <Pnl
                                                        key="p"
                                                        lamports={metrics.pnl}
                                                        testId={T.positionCell('pnl')}
                                                    />
                                                ),
                                                metrics.pnl === null,
                                            ],
                                            [
                                                'Time left',
                                                <Countdown
                                                    key="t"
                                                    deadline={Number(pos.deadline)}
                                                    testId={T.positionCell('time-left')}
                                                />,
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
                                        ]}
                                    />
                                </Panel>
                                <Panel title={`Terms locked at open · Tier ${TIER_NAMES[pos.tier] ?? pos.tier}`}>
                                    <Facts
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
                                                pos.sArmed
                                                    ? `armed · ${pos.sCount.toString()} observations`
                                                    : 'not armed',
                                                !pos.sArmed,
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
                                    banner={false}
                                />
                                <Panel title="Timeline" actions={<FreshnessBadge />}>
                                    {detail.data ? (
                                        <Timeline events={detail.data.data.events} />
                                    ) : (
                                        <p className="short-note">
                                            The indexer is unreachable; the history will appear when it is back.
                                        </p>
                                    )}
                                </Panel>
                            </div>
                            <Panel
                                title="Manage"
                                className="sheet-host manage-panel side-sticky"
                                ref={manage}
                                head={
                                    <SheetHead
                                        title={
                                            <>
                                                Manage short <bdi>{symbol}</bdi>
                                            </>
                                        }
                                        titleId={sheetTitle}
                                        onClose={closeSheet}
                                        testId={T.manageSheetClose}
                                    />
                                }
                            >
                                {/* In the sheet, closing a confirmed review (Done, ✕, Escape) also closes
                                    the sheet. */}
                                <TxDoneContext.Provider value={inSheet ? closeSheet : null}>
                                    <div className="stack">
                                        {!connected && <ConnectCta surface="position" />}
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
                                </TxDoneContext.Provider>
                            </Panel>
                            <Scrim onClick={closeSheet} />
                        </div>
                    </>
                ) : snap.error ? (
                    <ErrorState error={snap.error} retry={() => void snap.refetch()} />
                ) : (
                    <Skeleton width={240} />
                )}
            </div>
        </div>
    );
}
