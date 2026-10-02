/**
 * LEND track — the lender dashboard (`/lend`; FRONTEND.md §3.8): one row per market the wallet
 * lends in — shares and share of the pool, token value, SOL claimable, vesting, lock and
 * `lock_until`, the share-epoch standing (current / behind / parked on a wipe record), whether the
 * indexer's view is in sync (unsynced / quarantined), creator badges — with a claim per row and
 * "claim all" (one reviewed transaction per market, in turn). Numbers come from the chain through
 * the WASM previews (FE-D1); the indexer lists the markets (or `getProgramAccounts` while it is
 * unavailable).
 */
import './lend.css';
import { Link } from '@tanstack/react-router';
import { useEffect, useRef, useState } from 'react';
import { useLiveChannels } from '../../api/live.tsx';
import { Badge } from '../../components/Badge.tsx';
import { Button } from '../../components/Button.tsx';
import { type Column, DataTable } from '../../components/DataTable.tsx';
import { Disclosure } from '../../components/Disclosure.tsx';
import { Bps, Sol, Tokens } from '../../components/Num.tsx';
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
import { useWallet } from '../../wallet/useWallet.ts';
import { claimPlanFrom, syncPlanFrom } from './build.ts';
import { chainNowSeconds } from './chain.ts';
import { planContextFor } from './ctx.ts';
import { type IndexerState, type LenderRow, useLenderRows } from './dashboard/rows.ts';
import { useNowSeconds } from './hooks.ts';
import { buildClaimPlan, buildSyncPlan } from './plans.ts';

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
                // The builders the review's Retry re-runs on a fresh read (`build.ts`, M9-UX-1).
                const env = { rpc: client.rpc, math, mint: row.mint, signer, symbol: row.symbol };
                plan =
                    make === 'claim'
                        ? await claimPlanFrom(env, row.chain, chainNowSeconds(row.chain, Date.now()), null)
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

    const columns: Column<LenderRow>[] = [
        {
            key: 'market',
            header: 'Market',
            render: (r) => (
                <Link to="/m/$mint" params={{ mint: r.mint }} search={{ panel: 'lend', action: 'withdraw' }}>
                    <TokenId symbol={r.symbol} mint={r.mint} />
                </Link>
            ),
        },
        {
            key: 'shares',
            header: 'Shares',
            align: 'right',
            render: (r) =>
                r.position ? (
                    <span data-testid={T.lenderCell('shares')} data-value={r.position.shares.toString()}>
                        <Tokens raw={r.position.shares} decimals={r.decimals} />{' '}
                        <Bps value={r.position.shareBps} className="muted" />
                    </span>
                ) : (
                    '—'
                ),
        },
        {
            key: 'value',
            header: 'Value',
            align: 'right',
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
            align: 'right',
            render: (r) =>
                r.position ? <Sol lamports={r.position.claimable} frac={6} testId={T.lenderCell('claimable')} /> : '—',
        },
        {
            key: 'vesting',
            header: 'Vesting',
            align: 'right',
            render: (r) =>
                r.position ? <Sol lamports={r.position.vesting} frac={6} testId={T.lenderCell('vesting')} /> : '—',
        },
        {
            key: 'lock',
            header: 'Locked',
            align: 'right',
            render: (r) =>
                r.position ? <Tokens raw={r.position.lock} decimals={r.decimals} testId={T.lenderCell('lock')} /> : '—',
        },
        {
            key: 'lock-until',
            header: 'Lock ends',
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
            key: 'epochs',
            header: 'Epoch',
            render: (r) => {
                const s = r.standing;
                const state = s.kind === 'behind' ? 'behind' : s.kind === 'parked' ? 'parked' : 'current';
                return (
                    <span
                        data-testid={T.lenderEpochs(r.market)}
                        data-state={state}
                        data-value={String(s.kind === 'behind' ? s.epochs : 0)}
                    >
                        {s.kind === 'behind' ? (
                            <Badge tone="warn">{`${s.epochs} behind`}</Badge>
                        ) : s.kind === 'parked' ? (
                            <Badge tone="danger" title="A full settlement wiped the pool out">
                                Wiped
                            </Badge>
                        ) : (
                            <Badge tone="ok">Current</Badge>
                        )}
                    </span>
                );
            },
        },
        {
            key: 'indexer',
            header: 'Indexer',
            render: (r) => (
                <span data-testid={T.lenderIndexer(r.market)} data-state={r.indexer}>
                    <Badge tone={r.indexer === 'ok' ? 'neutral' : r.indexer === 'quarantined' ? 'danger' : 'warn'}>
                        {INDEXER_TEXT[r.indexer]}
                    </Badge>
                </span>
            ),
        },
        {
            key: 'creator',
            header: 'Creator',
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
            align: 'right',
            render: (r) =>
                r.standing.kind === 'behind' && r.chain && !r.chain.recordsComplete ? (
                    <Button
                        size="sm"
                        disabled={!signer || flow.state.stage !== 'idle'}
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
                        disabled={!signer || !r.position || r.position.claimable === 0n || flow.state.stage !== 'idle'}
                        busy={busy === `claim:${r.market}`}
                        onClick={() => void start(r, 'claim')}
                        data-testid={T.lenderClaim(r.market)}
                    >
                        Claim
                    </Button>
                ),
        },
    ];

    return (
        <div className="page lend-dashboard" data-testid={T.page('lend')}>
            <div className="page-head">
                <h1>Lending</h1>
                {/* The markets list comes from the indexer (FRONTEND.md §6.6): stale data is never silent. */}
                {!indexerDown && <FreshnessBadge />}
                <span className="grow" />
                {owner && (
                    <Button
                        variant="primary"
                        disabled={!signer || claimable.length === 0 || flow.state.stage !== 'idle'}
                        onClick={claimAll}
                        data-testid={T.lenderClaimAll}
                    >
                        {claimable.length > 1 ? `Claim all (${claimable.length} markets)` : 'Claim all'}
                    </Button>
                )}
            </div>
            {!owner ? (
                <Empty testId={T.lenderEmpty}>Connect a wallet to see your lending positions.</Empty>
            ) : (
                <>
                    <div className="stat-strip">
                        <Stat label="Markets">{rows.length}</Stat>
                        <Stat label="SOL claimable">
                            <Sol lamports={totalClaimable} frac={6} />
                        </Stat>
                        <Stat label="Data">{indexerDown ? 'Chain only (indexer unavailable)' : 'Indexer + chain'}</Stat>
                    </div>
                    {planError && (
                        <div className="banner" data-tone="danger" role="alert">
                            {planError}
                        </div>
                    )}
                    {error ? (
                        <ErrorState error={error} />
                    ) : (
                        <DataTable
                            caption="Your lending positions"
                            columns={columns}
                            rows={rows}
                            rowKey={(r) => r.market}
                            rowTestId={(r) => T.lenderRow(r.market)}
                            loading={loading}
                            skeletonRows={3}
                            testId={T.lenderDashboard}
                            empty={
                                <span data-testid={T.lenderEmpty}>
                                    You do not lend in any market yet. Open a market and use its Lend tab.
                                </span>
                            }
                        />
                    )}
                    <Disclosure id="lender.risks" vars={lenderRiskVars(config)} collapsible />
                </>
            )}
            <TxReviewModal flow={flow} />
        </div>
    );
}
