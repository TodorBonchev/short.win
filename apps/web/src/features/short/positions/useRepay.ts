/** Token repayment has its own state read, independent of quote availability. */
import type { Address } from '@solana/kit';
import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { qk } from '../../../api/keys.ts';
import { useMath } from '../../../math/useMath.ts';
import { useAppClient } from '../../../rpc/context.tsx';
import { PlanError } from '../../../tx/errors.ts';
import type { TxPlan } from '../../../tx/types.ts';
import { useTxFlow } from '../../../tx/useTxFlow.ts';
import { useWallet } from '../../../wallet/useWallet.ts';
import { useChainNow } from '../hooks.ts';
import { planRepay, previewRepay, type RepayInputs, type RepayPreview, readRepayState } from './repay.ts';

export function useRepay(position: Address) {
    const client = useAppClient();
    const { connected } = useWallet();
    const { math, error: mathError } = useMath();
    const flow = useTxFlow();
    const [building, setBuilding] = useState(false);
    const [buildError, setBuildError] = useState<string | null>(null);
    const state = useQuery({
        queryKey: [...qk.positionAccount(position), 'repay', connected?.address ?? 'none'],
        enabled: !!connected?.address,
        queryFn: () => readRepayState(client.rpc, position, connected?.address ?? null),
        staleTime: 0,
        refetchInterval: 2_000,
        refetchOnWindowFocus: true,
        structuralSharing: false,
        retry: 1,
    });
    const now = useChainNow(state.data ? { now: state.data.now, fetchedAt: state.data.readAt } : null);
    let preview: RepayPreview | null = null;
    let previewError: string | null = null;
    if (math && state.data) {
        try {
            preview = previewRepay(math, state.data, now);
        } catch {
            previewError = 'Accrued rent could not be estimated from the current stored state.';
        }
    }
    const submit = async (source: Address, maxRent: bigint, symbol: string) => {
        if (!math || !connected?.signer) return;
        setBuilding(true);
        setBuildError(null);
        const inputs: RepayInputs = { signer: connected.signer, source, maxRent, symbol };
        // Rebuild re-reads position, rent and funds but NEVER raises the user's bound.
        const rebuild = async (): Promise<TxPlan> => {
            const fresh = await readRepayState(client.rpc, position, inputs.signer.address);
            if (fresh.position.borrower !== inputs.signer.address)
                throw new PlanError('This position belongs to another wallet');
            return { ...(await planRepay(math, fresh, inputs)), rebuild };
        };
        try {
            flow.start(await rebuild());
        } catch (error) {
            setBuildError(error instanceof Error ? error.message : 'The repayment could not be prepared.');
        } finally {
            setBuilding(false);
        }
    };
    return { state, math, mathError, preview, previewError, now, flow, building, buildError, submit };
}
