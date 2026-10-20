/**
 * Lend → Claim (FRONTEND.md §3.6): what `claim_sol` pays now (`previewClaim`, pot draw included),
 * the credited entitlement, the vesting income, the pot and a deficit. When the pot cannot pay the
 * entitlement, other lenders whose sync moves a surplus to the pot are synced first in the same
 * transaction (`quote.ts` `gatherPotSyncs`, ≤ 4; unsigned, so the pot-only touch with its forward
 * chain for a lender behind a rebase record: KI-66 full, KI66-R4). A claim that leaves the position
 * empty closes it (KI-67; the review says so). A negative balance is covered with `cover_deficit`.
 *
 * The button says what the claim does (the owner's thirteenth round, O-5; audit finding D-21): what
 * it pays now and, when the syncs can add to it, how much — {g} = min(shortfall, what the syncs the
 * search picked move to the pot), never more than is credited (= claimable + shortfall). The owner's
 * decision of 2026-10-11 ("only when syncs can pay"): the search the review runs runs first, in the
 * background (`syncSearch.ts`); with nothing claimable now the claim opens only when the syncs add
 * `CLAIM_DUST_LAMPORTS` (10,000 lamports) or more — a shortfall no sync can fund (rent the open
 * shorts have not paid in; a market whose only lender is you) leaves the disabled "Review claim ·
 * 0 SOL" with "Nothing to claim yet". While the search runs, nothing more than is known is shown:
 * the full label when something is claimable now, else the disabled, busy button without a reason.
 * A shortfall under the threshold is rounding dust and counts as none (`previews.ts`). The note
 * keeps the whole shortfall (what is out of reach, and why). Text: `config/copy.ts`; the label:
 * `ClaimLabel.tsx` (the Lending page's rows use it too).
 */
import { useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { Button } from '../../../components/Button.tsx';
import { FormFoot } from '../../../components/FormFoot.tsx';
import { Sol } from '../../../components/Num.tsx';
import { Kv } from '../../../components/States.tsx';
import { copy } from '../../../config/copy.ts';
import { lenderRiskVars } from '../../../config/disclosureVars.ts';
import { T } from '../../../config/testids.ts';
import { formatSol } from '../../../lib/format.ts';
import { ConnectCta } from '../../../wallet/ConnectCta.tsx';
import { claimPlanFrom, coverPlanFrom } from '../build.ts';
import { type LendCtx, lendEnvOf, lendStateOf, nowFor } from '../ctx.ts';
import { previewClaimView } from '../previews.ts';
import { Blocked, RefusalLine, solFrac } from '../ui.tsx';
import { ClaimLabel } from './ClaimLabel.tsx';
import { indexerLenders } from './otherLenders.ts';
import { claimOffer, SEARCH_OFF, useClaimSyncSearches } from './syncSearch.ts';

/** SOL kept back in the wallet for fees when covering a deficit. */
const FEE_RESERVE = 10_000_000n;

export function ClaimForm({ ctx }: { ctx: LendCtx }) {
    const { chain, math, signer, flow } = ctx;
    const qc = useQueryClient();
    const [building, setBuilding] = useState<'claim' | 'cover' | null>(null);
    const [buildError, setBuildError] = useState<string | null>(null);
    const now = nowFor(ctx, 'clock');
    const view = previewClaimView(math, lendStateOf(chain), now);
    // What the syncs can add, searched before the button offers them (`syncSearch.ts`).
    const [search] = useClaimSyncSearches(
        [{ chain, shortfall: view.ok ? view.value.shortfall : 0n, claimable: view.ok ? view.value.claimable : 0n }],
        math,
        signer?.address ?? null,
        qc,
    );

    if (!signer) {
        return (
            <div className="stack lend-form">
                <div className="muted">Connect a wallet to see what you can claim in this market.</div>
                <ConnectCta surface="lend" />
            </div>
        );
    }
    if (!chain.lender) return <div className="muted lend-form">You have no lender position in this market.</div>;
    if (!view.ok) return <RefusalLine refusal={view.refusal} />;
    const v = view.value;
    const offer = claimOffer(v, search ?? SEARCH_OFF);

    const claim = async () => {
        if (!signer) return;
        setBuilding('claim');
        setBuildError(null);
        try {
            // Other lenders whose touch can fund the pot come from the indexer's list (§3.6): the
            // button's own pick while it is fresh, else the review searches again.
            const pick = search?.status === 'done' && search.fresh && search.pick ? search.pick : undefined;
            flow.start(
                await claimPlanFrom(lendEnvOf(ctx, signer), chain, ctx.nowS, indexerLenders(qc, ctx.mint), pick),
            );
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
    // Why the claim cannot open yet: the same predicates as its `disabled` (§7.6). While the search
    // for syncs runs with nothing claimable now, the button waits (busy) without a reason yet.
    const reason = ctx.stale
        ? 'Waiting for a fresh chain read'
        : !offer.open && !offer.waiting
          ? 'Nothing to claim yet'
          : null;
    return (
        <div className="stack lend-form">
            <Kv
                rows={[
                    [
                        'Claimable now',
                        <Sol
                            key="c"
                            lamports={v.claimable}
                            frac={solFrac(v.claimable)}
                            testId={T.claimValue('claimable')}
                        />,
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
                Rent is credited continuously, but credited income may not be payable yet. Claims use the SOL currently
                available to your position and the shared pot; earlier claims can leave less for later claimants. Some
                credited income may never be paid if the shortfall is never covered. One-off income, and rent paid above
                what accrued (the minimum rent of an early exit), vests over{' '}
                {lenderRiskVars(chain.config?.params).vestPeriod}.
                {v.shortfall > 0n &&
                    ` ${copy('claim.note.shortfall', { g: formatSol(v.shortfall, solFrac(v.shortfall)) })}`}
            </div>
            {buildError && (
                <div className="field-error" role="alert">
                    {buildError}
                </div>
            )}
            <FormFoot reason={reason} reasonTestId={T.lendClaimReason}>
                <Button
                    variant="primary"
                    block
                    busy={building === 'claim' || (offer.waiting && !ctx.stale)}
                    disabled={reason !== null || !offer.open || building !== null}
                    onClick={() => void claim()}
                    data-testid={T.claimSubmit}
                    className="lend-claim-submit"
                    data-lines={offer.syncs > 0n ? '2' : undefined}
                >
                    <ClaimLabel claimable={offer.claimable} syncs={offer.syncs} />
                </Button>
            </FormFoot>
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
