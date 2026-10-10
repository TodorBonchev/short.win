/**
 * SHORT track — what a trade's form tells the user around its submit (FRONTEND.md §7.6, D2, D5):
 * the one-line reason a submit is disabled (`FormFoot`), and a toast once a trade lands (the kind
 * word, the plan's title, Explorer and, for an open, View position).
 */
import { useEffect, useRef } from 'react';
import { type ToastLink, useToast } from '../../components/Toast.tsx';
import { explorerTx } from '../../config/env.ts';
import type { TxFlow } from '../../tx/useTxFlow.ts';

/** The first reason that applies, in the order the user would fix them; `null` when none does. */
export function firstReason(checks: readonly (readonly [boolean, string])[]): string | null {
    for (const [fails, reason] of checks) if (fails) return reason;
    return null;
}

/** Shared wording of the reasons every trade form gives. */
export const REASON = {
    connect: 'Connect a wallet first',
    signer: 'This wallet cannot sign transactions here',
    notMine: 'This position belongs to another wallet',
    stale: 'Waiting for a fresh read of the chain',
    building: 'Preparing the transaction…',
    inReview: 'A transaction is in review',
    slippage: 'Set a slippage between 0.1% and 50%',
} as const;

/** Pushes a success toast when the flow reaches `confirmed` (once per landing). */
export function useConfirmedToast(flow: TxFlow, extra?: () => readonly ToastLink[]): void {
    const toast = useToast();
    const { stage, plan, signature } = flow.state;
    const prev = useRef(stage);
    const links = useRef(extra);
    links.current = extra;
    useEffect(() => {
        const was = prev.current;
        prev.current = stage;
        if (stage !== 'confirmed' || was === 'confirmed' || !plan) return;
        toast.push({
            kind: 'success',
            title: plan.title,
            links: [
                ...(links.current?.() ?? []),
                ...(signature ? [{ href: explorerTx(signature), text: 'Explorer' }] : []),
            ],
        });
    }, [stage, plan, signature, toast]);
}
