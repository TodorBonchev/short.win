/**
 * LEND track — the lender dashboard (`/lend`; FRONTEND.md §3.8): one row per market the wallet
 * lends in — shares and share of the pool, token value, SOL claimable, vesting, lock and
 * `lock_until`, the share-epoch standing (current / behind / parked on a wipe record), whether the
 * indexer's view is in sync (unsynced / quarantined), creator badges — with a claim per row and
 * "claim all" (one reviewed transaction per market, in turn; each claim syncs the other lenders
 * whose touch funds the pot first, as Lend → Claim does, KI66-R4). Numbers come from the chain through
 * the WASM previews (FE-D1); the indexer lists the markets (or `getProgramAccounts` while it is
 * unavailable).
 */
import './lend.css';
import { useQueryClient } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { useEffect, useRef, useState } from 'react';
import { useLiveChannels } from '../../api/live.tsx';
import { ActionBar } from '../../components/ActionBar.tsx';
import { Badge } from '../../components/Badge.tsx';
import { Button } from '../../components/Button.tsx';
import { type Column, DataTable } from '../../components/DataTable.tsx';
import { Disclosure } from '../../components/Disclosure.tsx';
import { Bps, Sol, Tokens } from '../../components/Num.tsx';
import { Panel } from '../../components/Panel.tsx';
import { Empty, ErrorState, Stat } from '../../components/States.tsx';
import { useToast } from '../../components/Toast.tsx';
import { TokenId } from '../../components/TokenId.tsx';
import { lenderRiskVars } from '../../config/disclosureVars.ts';
import { T } from '../../config/testids.ts';
import { formatUtc } from '../../lib/time.ts';
import { useMath } from '../../math/useMath.ts';
import { useAppClient } from '../../rpc/context.tsx';
import { useChainConfig } from '../../rpc/queries.ts';
import { FreshnessBadge } from '../../shell/freshness.tsx';
import { TxReviewModal } from '../../tx/TxReviewModal.tsx';
import type { TxPlan } from '../../tx/types.ts';
import { useTxFlow } from '../../tx/useTxFlow.ts';
import { ConnectCta } from '../../wallet/ConnectCta.tsx';
import { useWallet } from '../../wallet/useWallet.ts';
import { claimPlanFrom, syncPlanFrom } from './build.ts';
import { chainNowSeconds } from './chain.ts';
import { indexerLenders } from './claim/otherLenders.ts';
import { planContextFor } from './ctx.ts';
import { type IndexerState, type LenderRow, totalValueLamports, useLenderRows } from './dashboard/rows.ts';
import { useNowSeconds } from './hooks.ts';
import { buildClaimPlan, buildSyncPlan } from './plans.ts';
import { solFrac } from './ui.tsx';

const INDEXER_TEXT: Readonly<Record<IndexerState, string>> = {
    ok: 'In sync',
    unsynced: 'Indexer behind',
    quarantined: 'Quarantined events',
    missing: 'Chain only',
};

export function LenderDashboardPage() {
    const client = useAppClient();
    const configQ = useChainConfig();
    const config = configQ.data?.exists ? configQ.data.data.params : null;
    const { connected } = useWallet();
    const owner = connected?.address ?? null;
    const signer = connected?.signer ?? null;
    const { math } = useMath();
    const wallS = useNowSeconds();
    const { rows, loading, indexerDown, error } = useLenderRows(owner, math, wallS * 1000);
    useLiveChannels(owner ? [`lender:${owner}`] : []);
    const flow = useTxFlow();
    const qc = useQueryClient();
    const toast = useToast();
    const [busy, setBusy] = useState<string | null>(null);
    const [planError, setPlanError] = useState<string | null>(null);
    /** "Claim all": the markets still to claim, by market address. */
    const queue = useRef<string[]>([]);

    const start = async (row: LenderRow, make: 'claim' | 'sync') => {
        if (!signer || !row.chain) return;
        setBusy(`${make}:${row.market}`);
        setPlanError(null);
        try {
            let plan: TxPlan;
            if (math) {
                // The builders the review's Retry re-runs on a fresh read (`build.ts`, M9-UX-1). A
                // claim syncs the other lenders it needs first, as Lend → Claim does (KI66-R4).
                const env = { rpc: client.rpc, math, mint: row.mint, signer, symbol: row.symbol };
                plan =
                    make === 'claim'
                        ? await claimPlanFrom(
                              env,
                              row.chain,
                              chainNowSeconds(row.chain, Date.now()),
                              indexerLenders(qc, row.mint),
                          )
                        : await syncPlanFrom(env, row.chain);
            } else {
                const c = planContextFor(row.chain, signer, row);
                plan =
                    make === 'claim'
                        ? await buildClaimPlan({ ...c, syncs: [], expected: row.position?.claimable ?? 0n })
                        : await buildSyncPlan(c);
            }
            flow.start(plan);
        } catch (e) {
            setPlanError(e instanceof Error ? e.message : String(e));
            queue.current = [];
        } finally {
            setBusy(null);
        }
    };
    const latest = useRef({ rows, start });
    latest.current = { rows, start };

    // One review per market; the next opens when the previous one is done, and a review closed
    // without landing stops the queue.
    const stage = flow.state.stage;
    const title = flow.state.plan?.title;
    const prev = useRef(stage);
    useEffect(() => {
        const was = prev.current;
        prev.current = stage;
        if (stage === 'confirmed' && was !== 'confirmed' && title) toast.push({ kind: 'success', title });
        if (stage !== 'idle' || was === 'idle') return;
        if (was !== 'confirmed') {
            queue.current = [];
            return;
        }
        const next = queue.current.shift();
        const row = next ? latest.current.rows.find((r) => r.market === next) : undefined;
        if (row) void latest.current.start(row, 'claim');
    }, [stage, title, toast]);

    const claimable = rows.filter((r) => (r.position?.claimable ?? 0n) > 0n && r.standing.kind !== 'behind');
    const totalClaimable = claimable.reduce((s, r) => s + (r.position?.claimable ?? 0n), 0n);

    const claimAll = () => {
        const [first, ...rest] = claimable;
        if (!first) return;
        queue.current = rest.map((r) => r.market);
        void start(first, 'claim');
    };

    const busyFlow = flow.state.stage !== 'idle';
    const withPosition = rows.filter((r) => r.position !== null).length;
    const total = totalValueLamports(rows);
    const credited = rows.reduce((s, r) => s + (r.position?.pending ?? 0n), 0n);

    const columns: Column<LenderRow>[] = [
        {
            key: 'market',
            header: 'Market',
            card: 'title',
            render: (r) => (
                <Link to="/m/$mint" params={{ mint: r.mint }} search={{ panel: 'lend', action: 'withdraw' }}>
                    <TokenId symbol={r.symbol} mint={r.mint} />
                </Link>
            ),
        },
        {
            key: 'value',
            header: 'Value',
            align: 'right',
            card: 'aside',
            render: (r) =>
                r.position ? (
                    <Tokens
                        raw={r.position.value}
                        decimals={r.decimals}
                        symbol={r.symbol}
                        testId={T.lenderCell('value')}
                    />
                ) : (
                    '—'
                ),
        },
        {
            key: 'claimable',
            header: 'SOL claimable',
            label: 'Claimable',
            align: 'right',
            render: (r) =>
                r.position ? (
                    <Sol
                        lamports={r.position.claimable}
                        frac={solFrac(r.position.claimable)}
                        testId={T.lenderCell('claimable')}
                    />
                ) : (
                    '—'
                ),
        },
        {
            // The share of the pool; the raw share count is the market lend panel's (`data-value`).
            key: 'shares',
            header: 'Pool share',
            align: 'right',
            priority: 2,
            render: (r) =>
                r.position ? (
                    <span data-testid={T.lenderCell('shares')} data-value={r.position.shares.toString()}>
                        <Bps value={r.position.shareBps} />
                    </span>
                ) : (
                    '—'
                ),
        },
        {
            key: 'vesting',
            header: 'Vesting',
            align: 'right',
            priority: 2,
            render: (r) =>
                r.position ? <Sol lamports={r.position.vesting} frac={6} testId={T.lenderCell('vesting')} /> : '—',
        },
        {
            key: 'lock',
            header: 'Locked',
            align: 'right',
            priority: 2,
            render: (r) =>
                r.position ? <Tokens raw={r.position.lock} decimals={r.decimals} testId={T.lenderCell('lock')} /> : '—',
        },
        {
            key: 'lock-until',
            header: 'Lock ends',
            priority: 3,
            render: (r) =>
                r.position ? (
                    <span
                        className="num"
                        data-testid={T.lenderCell('lock-until')}
                        data-value={r.position.lockUntil.toString()}
                    >
                        {r.position.lock > 0n ? formatUtc(r.position.lockUntil) : '—'}
                    </span>
                ) : (
                    '—'
                ),
        },
        {
            // The share-epoch standing and the indexer's view in one cell: a badge only when one is
            // not current (§3.8). Priority 2, not 3: a wiped or quarantined position is never hidden.
            key: 'sync',
            header: 'Sync',
            priority: 2,
            render: (r) => <SyncCell row={r} />,
        },
        {
            key: 'creator',
            header: 'Creator',
            priority: 3,
            render: (r) => (
                <span data-testid={T.lenderCell('creator')} data-value={String(r.isCoinCreator || r.isMarketCreator)}>
                    {r.isCoinCreator && <Badge tone="warn">Coin creator</Badge>}{' '}
                    {r.isMarketCreator && <Badge tone="info">Market creator</Badge>}
                </span>
            ),
        },
        {
            key: 'actions',
            header: <span className="visually-hidden">Actions</span>,
            label: '',
            align: 'right',
            card: 'actions',
            render: (r) => {
                const sync = r.standing.kind === 'behind' && r.chain && !r.chain.recordsComplete;
                const reason = sync ? null : claimReason(r);
                const reasonId = `lender-claim-reason-${r.market}`;
                return (
                    <>
                        <Link
                            to="/m/$mint"
                            params={{ mint: r.mint }}
                            search={{ panel: 'lend', action: 'withdraw', sheet: 'trade' }}
                            className="btn hit"
                            data-size="sm"
                            data-testid={T.lenderWithdraw(r.market)}
                            aria-label={`Withdraw from ${r.symbol}`}
                        >
                            Withdraw
                        </Link>
                        <Link
                            to="/m/$mint"
                            params={{ mint: r.mint }}
                            search={{ panel: 'lend', action: 'deposit', sheet: 'trade' }}
                            className="btn hit"
                            data-size="sm"
                            data-testid={T.lenderDeposit(r.market)}
                            aria-label={`Deposit into ${r.symbol}`}
                        >
                            Deposit
                        </Link>
                        {sync ? (
                            <Button
                                size="sm"
                                disabled={!signer || busyFlow}
                                busy={busy === `sync:${r.market}`}
                                onClick={() => void start(r, 'sync')}
                                data-testid={T.lenderSync(r.market)}
                            >
                                Sync
                            </Button>
                        ) : (
                            <Button
                                size="sm"
                                variant="primary"
                                disabled={!signer || reason !== null || busyFlow}
                                busy={busy === `claim:${r.market}`}
                                onClick={() => void start(r, 'claim')}
                                data-testid={T.lenderClaim(r.market)}
                                aria-describedby={reason ? reasonId : undefined}
                            >
                                Claim
                            </Button>
                        )}
                        {reason && (
                            <span id={reasonId} className="lend-row-reason" data-testid={T.lenderClaimReason(r.market)}>
                                {reason}
                            </span>
                        )}
                    </>
                );
            },
        },
    ];

    return (
        <div className="page lend-dashboard" data-testid={T.page('lend')}>
            <div className="page-head">
                <h1>Lending</h1>
                {/* The markets list comes from the indexer (FRONTEND.md §6.6): stale data is never silent. */}
                {owner &&
                    (indexerDown ? (
                        <Badge tone="warn" testId={T.lenderSource}>
                            Chain only (indexer unavailable)
                        </Badge>
                    ) : (
                        <FreshnessBadge />
                    ))}
                <span className="grow" />
                {/* "Claim all" only when something is claimable: in the header on a desktop, in the
                    action bar within thumb reach under 1024 px (one element; lend.css). */}
                {owner && claimable.length > 0 && (
                    <ActionBar label="Lending actions">
                        <Button
                            variant="primary"
                            disabled={!signer || busyFlow}
                            onClick={claimAll}
                            data-testid={T.lenderClaimAll}
                        >
                            {claimable.length > 1 ? `Claim all (${claimable.length} markets)` : 'Claim all'}
                            <span className="lend-claim-all-sum">
                                {' · '}
                                <Sol lamports={totalClaimable} frac={solFrac(totalClaimable)} />
                            </span>
                        </Button>
                    </ActionBar>
                )}
            </div>
            {!owner ? (
                <Empty
                    title="Connect a wallet to see your lending positions."
                    testId={T.lenderEmpty}
                    action={<ConnectCta surface="lend-dashboard" />}
                />
            ) : (
                <>
                    <div className="stat-strip lend-kpis">
                        <Stat label="Markets">
                            <span className="num" data-testid={T.lenderKpi('markets')} data-value={rows.length}>
                                {rows.length}
                            </span>
                        </Stat>
                        <Stat label="Total value">
                            {total.priced > 0 ? (
                                <span
                                    className="lend-kpi-approx"
                                    data-testid={T.lenderKpi('value')}
                                    data-value={total.lamports.toString()}
                                    data-priced={total.priced}
                                >
                                    ≈ <Sol lamports={total.lamports} frac={total.lamports >= 10n ** 9n ? 2 : 6} />
                                    {total.priced < withPosition && (
                                        <span className="lend-kpi-note">
                                            {total.priced} of {withPosition} priced
                                        </span>
                                    )}
                                </span>
                            ) : (
                                <span className="lend-kpi-note" data-testid={T.lenderKpi('value')} data-priced={0}>
                                    {withPosition > 0 ? 'No price yet' : '—'}
                                </span>
                            )}
                        </Stat>
                        <Stat label="SOL claimable">
                            <Sol
                                lamports={totalClaimable}
                                frac={solFrac(totalClaimable)}
                                testId={T.lenderKpi('claimable')}
                            />
                        </Stat>
                        <Stat label="Rent credited">
                            <Sol lamports={credited} frac={solFrac(credited)} testId={T.lenderKpi('credited')} />
                        </Stat>
                    </div>
                    {planError && (
                        <div className="banner" data-tone="danger" role="alert">
                            {planError}
                        </div>
                    )}
                    {error ? (
                        <ErrorState error={error} />
                    ) : (
                        <Panel flush>
                            <DataTable
                                caption="Your lending positions"
                                columns={columns}
                                rows={rows}
                                rowKey={(r) => r.market}
                                rowTestId={(r) => T.lenderRow(r.market)}
                                loading={loading}
                                skeletonRows={3}
                                testId={T.lenderDashboard}
                                className="lend-table"
                                empty={
                                    <div className="lend-empty">
                                        <span data-testid={T.lenderEmpty}>
                                            You do not lend in any market yet. Open a market and use its Lend tab.
                                        </span>
                                        <Link
                                            to="/markets"
                                            className="btn"
                                            data-variant="primary"
                                            data-testid={T.lendEmptyCta}
                                        >
                                            Browse markets
                                        </Link>
                                    </div>
                                }
                            />
                        </Panel>
                    )}
                    <Disclosure id="lender.risks" vars={lenderRiskVars(config)} collapsible />
                </>
            )}
            <TxReviewModal flow={flow} />
        </div>
    );
}

/** Why a row's Claim is disabled, in text (null when it is enabled, or only busy). */
function claimReason(r: LenderRow): string | null {
    if (r.chainError) return 'Chain read failed';
    if (!r.position) return r.chain ? 'Nothing to claim' : 'Reading the chain…';
    if (r.position.claimable === 0n) return 'Nothing to claim yet';
    return null;
}

/** The share-epoch standing and the indexer's view of a row; quiet while both are current. */
function SyncCell({ row: r }: { row: LenderRow }) {
    const s = r.standing;
    const epoch = s.kind === 'behind' ? 'behind' : s.kind === 'parked' ? 'parked' : 'current';
    const fine = epoch === 'current' && r.indexer === 'ok';
    return (
        <span className="lend-sync">
            <span
                data-testid={T.lenderEpochs(r.market)}
                data-state={epoch}
                data-value={String(s.kind === 'behind' ? s.epochs : 0)}
            >
                {s.kind === 'behind' ? (
                    <Badge tone="warn">{`${s.epochs} epochs behind`}</Badge>
                ) : s.kind === 'parked' ? (
                    <Badge tone="danger">Pool wiped out</Badge>
                ) : null}
            </span>
            <span data-testid={T.lenderIndexer(r.market)} data-state={r.indexer}>
                {r.indexer !== 'ok' && (
                    <Badge tone={r.indexer === 'quarantined' ? 'danger' : 'warn'}>{INDEXER_TEXT[r.indexer]}</Badge>
                )}
            </span>
            {fine && <span className="muted">Current</span>}
        </span>
    );
}
