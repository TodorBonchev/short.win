/**
 * The Claim button's label (the owner's thirteenth round, O-5; audit finding D-21; FRONTEND.md §3.6,
 * §3.8, §4.3): "Review claim · {c} SOL", or, when the syncs can add to it (`syncs`, {g}: min(shortfall,
 * the picked syncs' gain), 10,000 lamports or more — the owner's decision of 2026-10-11, "only when
 * syncs can pay"; `syncSearch.ts` `claimOffer`), "Review claim · {c} SOL now · syncs may add up to
 * {g} SOL" on two lines (the second from "syncs"; one accessible name). Lend → Claim and each row of
 * the Lending page show it: the same claim, the same words.
 */
import { copy } from '../../../config/copy.ts';
import { T } from '../../../config/testids.ts';
import { formatSol } from '../../../lib/format.ts';
import { solFrac } from '../ui.tsx';

export function ClaimLabel({ claimable, syncs }: { claimable: bigint; syncs: bigint }) {
    const c = formatSol(claimable, solFrac(claimable));
    if (syncs === 0n) return <>{copy('claim.label.full', { c })}</>;
    const full = copy('claim.label.syncs', { c, g: formatSol(syncs, solFrac(syncs)) });
    const cut = full.indexOf(' · syncs');
    return (
        <span className="claim-label">
            <span>{full.slice(0, cut)}</span>
            <span className="visually-hidden"> · </span>
            <span className="claim-label-sub" data-testid={T.claimSubmitSyncs}>
                {full.slice(cut + 3)}
            </span>
        </span>
    );
}
