/**
 * LEND track — the market page's Lend tab (FRONTEND.md §3.6): Deposit | Withdraw | Claim
 * (`?action=`), over the chain state of the market (FE-D1) and the WASM previews. Every action goes
 * through the transaction pipeline (simulate → review with the disclosures → sign; FRONTEND.md §5).
 * The props are the frozen contract: `MarketPage` renders `LendPanel({ mint, action })`.
 */
import './lend.css';
import type { Address } from '@solana/kit';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { type ReactNode, useEffect, useRef, useState } from 'react';
import { useLiveChannels } from '../../api/live.tsx';
import { indexerQueries } from '../../api/queries.ts';
import { Button } from '../../components/Button.tsx';
import { Bps, Sol, Tokens } from '../../components/Num.tsx';
import { Empty, ErrorState, Kv, Skeleton } from '../../components/States.tsx';
import { TabPanel, Tabs } from '../../components/Tabs.tsx';
import { useToast } from '../../components/Toast.tsx';
import { TokenId } from '../../components/TokenId.tsx';
import { T } from '../../config/testids.ts';
import { cleanSymbol } from '../../lib/displayText.ts';
import { shortAddress } from '../../lib/format.ts';
import { formatUtc } from '../../lib/time.ts';
import { useMath } from '../../math/useMath.ts';
import type { LendAction } from '../../router.tsx';
import { useAppClient } from '../../rpc/context.tsx';
import { ReadAgeView, useReadAge } from '../../shell/SlotAge.tsx';
import { TxReviewModal } from '../../tx/TxReviewModal.tsx';
import { useTxFlow } from '../../tx/useTxFlow.ts';
import { useWallet } from '../../wallet/useWallet.ts';
import { syncPlanFrom } from './build.ts';
import { chainNowSeconds, lendKeys, MAX_RECORDS } from './chain.ts';
import { ClaimForm } from './claim/ClaimForm.tsx';
import { type LendCtx, type LoadedChain, lendEnvOf, lendStateOf, nowFor } from './ctx.ts';
import { DepositForm } from './deposit/DepositForm.tsx';
import { useLendState, useNowSeconds } from './hooks.ts';
import { previewPosition, standingOf } from './previews.ts';
import { Blocked } from './ui.tsx';
import { WithdrawForm } from './withdraw/WithdrawForm.tsx';

const TABS = [
    { key: 'deposit', label: 'Deposit', testId: T.lendTab('deposit') },
    { key: 'withdraw', label: 'Withdraw', testId: T.lendTab('withdraw') },
    { key: 'claim', label: 'Claim', testId: T.lendTab('claim') },
] as const;

/** The routed panel: the tab is `?action=` on the market page. */
export function LendPanel({ mint, action }: { mint: Address; action?: LendAction | undefined }) {
    const navigate = useNavigate({ from: '/m/$mint' });
    return (
        <LendPanelView
            mint={mint}
            action={action ?? 'deposit'}
            onAction={(a) => void navigate({ search: (s) => ({ ...s, panel: 'lend', action: a }), replace: true })}
        />
    );
}

export function LendPanelView({
    mint,
    action,
    onAction,
}: {
    mint: Address;
    action: LendAction;
    onAction: (a: LendAction) => void;
}) {
    const client = useAppClient();
    const qc = useQueryClient();
    const toast = useToast();
    const { connected } = useWallet();
    const owner = connected?.address ?? null;
    const signer = connected?.signer ?? null;
    const lend = useLendState(mint, owner);
    const { math, error: mathError } = useMath();
    const detail = useQuery(indexerQueries.market(mint));
    useLiveChannels(owner ? [`lender:${owner}`] : []);
    const flow = useTxFlow();
    const wallS = useNowSeconds();
    // The age of the chain read behind every preview (FRONTEND.md §6.6): shown, and blocking past stale.
    const age = useReadAge(lend.data?.slot ?? null, lend.data?.readAt ?? null, !!(lend as { live?: boolean }).live);

    // A landed transaction is announced; after any review closes (landed, failed or cancelled) the
    // accounts are read again, so a changed token account (CPI Guard or memo flag, FRONTEND.md
    // §10.3) shows on re-entry.
    const stage = flow.state.stage;
    const title = flow.state.plan?.title;
    const prev = useRef(stage);
    useEffect(() => {
        const was = prev.current;
        prev.current = stage;
        if (stage === 'confirmed' && was !== 'confirmed' && title) toast.push({ kind: 'success', title });
        if (stage === 'idle' && was !== 'idle') void qc.invalidateQueries({ queryKey: lendKeys.state(mint, owner) });
    }, [stage, title, toast, qc, mint, owner]);

    let body: ReactNode;
    if (lend.isPending) {
        body = (
            <div className="stack lend-form" aria-busy="true">
                <Skeleton width={220} />
                <Skeleton width={160} />
                <Skeleton width={260} />
            </div>
        );
    } else if (lend.isError) {
        body = <ErrorState error={lend.error} retry={() => void lend.refetch()} />;
    } else if (!lend.data.market || !lend.data.vault) {
        body = (
            <Empty>
                No short.win market for this token yet.{' '}
                <a href={`/markets/new?mint=${encodeURIComponent(mint)}`}>Create it</a>
            </Empty>
        );
    } else if (mathError) {
        body = <ErrorState error={mathError} />;
    } else if (!math) {
        body = <Skeleton width={220} />;
    } else {
        const chain = lend.data as LoadedChain;
        const ctx: LendCtx = {
            mint,
            chain,
            math,
            rpc: client.rpc,
            nowS: chainNowSeconds(chain, wallS * 1000),
            // Display only, and only a detail about this very mint (M9-SEC-3); normalised at the API.
            symbol:
                (detail.data?.data.mint === mint ? cleanSymbol(detail.data.data.symbol) : null) ?? shortAddress(mint),
            decimals: chain.market.decimals,
            signer,
            detail: detail.data?.data && detail.data.data.mint === mint ? detail.data.data : undefined,
            flow,
            stale: age.state === 'stale',
        };
        body = (
            <>
                <div className="row lend-token">
                    <TokenId symbol={ctx.symbol} mint={mint} />
                    <ReadAgeView age={age} />
                </div>
                <PositionSummary ctx={ctx} />
                <SyncNotice ctx={ctx} />
                {action === 'deposit' && <DepositForm ctx={ctx} />}
                {action === 'withdraw' && <WithdrawForm ctx={ctx} onCoverDeficit={() => onAction('claim')} />}
                {action === 'claim' && <ClaimForm ctx={ctx} />}
            </>
        );
    }

    return (
        <div className="lend" data-testid={T.lendPanel} data-mint={mint} data-action={action}>
            <Tabs label="Lend" idPrefix="lend" value={action} onChange={onAction} tabs={TABS} />
            <TabPanel idPrefix="lend" tabKey={action}>
                <div className="stack lend-body">{body}</div>
            </TabPanel>
            <TxReviewModal flow={flow} />
        </div>
    );
}

/** The lender's position in this market (chain state through the WASM previews). */
function PositionSummary({ ctx }: { ctx: LendCtx }) {
    const { chain, math, decimals, symbol } = ctx;
    if (!chain.lender) return null;
    const p = previewPosition(math, lendStateOf(chain), nowFor(ctx, 'clock'));
    if (!p.ok) return null;
    const v = p.value;
    return (
        <div className="lend-position">
            <Kv
                rows={[
                    [
                        'Your shares',
                        <Tokens key="s" raw={v.shares} decimals={decimals} testId={T.lendPosition('shares')} />,
                    ],
                    ['Share of pool', <Bps key="b" value={v.shareBps} testId={T.lendPosition('share-bps')} />],
                    [
                        'Token value',
                        <Tokens
                            key="v"
                            raw={v.value}
                            decimals={decimals}
                            symbol={symbol}
                            testId={T.lendPosition('value')}
                        />,
                    ],
                    [
                        'SOL claimable',
                        <Sol key="c" lamports={v.claimable} frac={6} testId={T.lendPosition('claimable')} />,
                    ],
                    [
                        'Locked',
                        <span key="l">
                            <Tokens raw={v.lock} decimals={decimals} testId={T.lendPosition('lock')} />
                            {v.lock > 0n && <span className="muted"> until {formatUtc(v.lockUntil)}</span>}
                        </span>,
                    ],
                ]}
            />
            {v.standing.kind === 'parked' && (
                <div className="muted lend-note">
                    A full settlement wiped this pool out. Your old shares hold no tokens; claim the SOL the settlement
                    left you.
                </div>
            )}
        </div>
    );
}

/** A lender behind more share epochs than one transaction carries syncs first (`sync_lender`). */
function SyncNotice({ ctx }: { ctx: LendCtx }) {
    const { chain, signer, flow } = ctx;
    const [building, setBuilding] = useState(false);
    const [error, setError] = useState<string | null>(null);
    if (!chain.lender || chain.recordsComplete) return null;
    const standing = standingOf(chain.market, chain.lender);
    const behind = standing.kind === 'behind' ? standing.epochs : 0;
    const sync = async () => {
        if (!signer) return;
        setBuilding(true);
        setError(null);
        try {
            flow.start(await syncPlanFrom(lendEnvOf(ctx, signer), ctx.chain));
        } catch (e) {
            setError(e instanceof Error ? e.message : String(e));
        } finally {
            setBuilding(false);
        }
    };
    return (
        <Blocked code="LenderBehindEpochs">
            Your position is {behind} share epochs behind this market. Sync it first ({MAX_RECORDS} epochs per
            transaction); then deposit, withdraw and claim work as usual.
            <div>
                <Button
                    size="sm"
                    busy={building}
                    disabled={!signer}
                    onClick={() => void sync()}
                    data-testid={T.lendSyncSubmit}
                >
                    Sync position
                </Button>
            </div>
            {error && <div className="field-error">{error}</div>}
        </Blocked>
    );
}
