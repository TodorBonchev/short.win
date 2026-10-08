/**
 * Lend → Claim (FRONTEND.md §3.6): what `claim_sol` pays now (`previewClaim`, pot draw included),
 * the credited entitlement, the vesting income, the pot and a deficit. When the pot cannot pay the
 * entitlement, other lenders whose sync moves a surplus to the pot are synced first in the same
 * transaction (`quote.ts` `pickPotSyncs`, ≤ 4; unsigned, so the pot-only touch with its forward
 * chain for a lender behind a rebase record: KI-66 full, KI66-R4). A claim that leaves the position
 * empty closes it (KI-67; the review says so). A negative balance is covered with `cover_deficit`.
 */
import { type Address, isAddress } from '@solana/kit';
import { useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { indexerQueries } from '../../../api/queries.ts';
import { Button } from '../../../components/Button.tsx';
import { Sol } from '../../../components/Num.tsx';
import { Kv } from '../../../components/States.tsx';
import { lenderRiskVars } from '../../../config/disclosureVars.ts';
import { T } from '../../../config/testids.ts';
import { formatSol } from '../../../lib/format.ts';
import { claimPlanFrom, coverPlanFrom } from '../build.ts';
import { type LendCtx, lendEnvOf, lendStateOf, nowFor } from '../ctx.ts';
import { previewClaimView } from '../previews.ts';
import { Blocked, RefusalLine } from '../ui.tsx';

/** SOL kept back in the wallet for fees when covering a deficit. */
const FEE_RESERVE = 10_000_000n;

export function ClaimForm({ ctx }: { ctx: LendCtx }) {
    const { chain, math, signer, flow } = ctx;
    const qc = useQueryClient();
    const [building, setBuilding] = useState<'claim' | 'cover' | null>(null);
    const [buildError, setBuildError] = useState<string | null>(null);
    const now = nowFor(ctx, 'clock');
    const view = previewClaimView(math, lendStateOf(chain), now);

    if (!chain.lender) return <div className="muted lend-form">You have no lender position in this market.</div>;
    if (!view.ok) return <RefusalLine refusal={view.refusal} />;
    const v = view.value;

    const claim = async () => {
        if (!signer) return;
        setBuilding('claim');
        setBuildError(null);
        try {
            // Other lenders whose touch can fund the pot come from the indexer's list (§3.6).
            const otherLenders = async (): Promise<Address[]> => {
                const page = await qc.fetchQuery(indexerQueries.marketLenders(ctx.mint));
                return page.data.data.map((l) => l.owner).filter((o): o is Address => isAddress(o));
            };
            flow.start(await claimPlanFrom(lendEnvOf(ctx, signer), chain, ctx.nowS, otherLenders));
        } catch (e) {
            setBuildError(e instanceof Error ? e.message : String(e));
        } finally {
            setBuilding(null);
        }
    };

    const cover = async () => {
        if (!signer) return;
        setBuilding('cover');
        setBuildError(null);
        try {
            flow.start(await coverPlanFrom(lendEnvOf(ctx, signer), chain, ctx.nowS));
        } catch (e) {
            setBuildError(e instanceof Error ? e.message : String(e));
        } finally {
            setBuilding(null);
        }
    };

    const canCover = chain.walletLamports >= v.deficit + FEE_RESERVE;
    return (
        <div className="stack lend-form">
            <Kv
                rows={[
                    [
                        'Claimable now',
                        <Sol key="c" lamports={v.claimable} frac={6} testId={T.claimValue('claimable')} />,
                    ],
                    ['Credited to you', <Sol key="e" lamports={v.entitled} frac={6} />],
                    [
                        'Still vesting (your share)',
                        <Sol key="v" lamports={v.vesting} frac={6} testId={T.claimValue('vesting')} />,
                    ],
                    ['Market pot', <Sol key="p" lamports={v.pot} frac={6} testId={T.claimValue('pot')} />],
                    ['Deficit', <Sol key="d" lamports={v.deficit} frac={9} testId={T.claimValue('deficit')} />],
                ]}
            />
            <div className="muted lend-note">
                Rent is credited continuously and becomes claimable once borrowers have paid it in; one-off income, and
                rent paid above what accrued (the minimum rent of an early exit), vests over{' '}
                {lenderRiskVars(chain.config?.params).vestPeriod}.
                {v.shortfall > 0n &&
                    ` ${formatSol(v.shortfall, 6)} SOL credited to you is not paid in yet. A claim first syncs other lenders whose surplus can fund it, where there are any.`}
            </div>
            {buildError && (
                <div className="field-error" role="alert">
                    {buildError}
                </div>
            )}
            <Button
                variant="primary"
                block
                busy={building === 'claim'}
                disabled={!signer || building !== null || !!ctx.stale || (v.claimable === 0n && v.shortfall === 0n)}
                onClick={() => void claim()}
                data-testid={T.claimSubmit}
            >
                {`Review claim · ${formatSol(v.claimable, 6)} SOL`}
            </Button>
            {v.deficit > 0n && (
                <>
                    <Blocked code="NegativePending">
                        A shortfall left your SOL balance at −{formatSol(v.deficit, 9)} SOL. Withdrawals wait until it
                        is covered.
                    </Blocked>
                    <Button
                        variant="danger"
                        block
                        busy={building === 'cover'}
                        disabled={!signer || building !== null || !!ctx.stale || !canCover}
                        onClick={() => void cover()}
                        data-testid={T.coverDeficitSubmit}
                    >
                        {`Cover deficit · ${formatSol(v.deficit, 9)} SOL`}
                    </Button>
                    {!canCover && <div className="field-error">Not enough SOL in the wallet to cover it.</div>}
                </>
            )}
        </div>
    );
}
