/**
 * The transaction lifecycle as a React state machine (FRONTEND.md §5):
 *
 *   idle → simulating → review ⇄ simulating (refresh, or a stale quote at Sign)
 *                         ↓ Sign (only with a successful simulation the plan does not hold
 *                           (`holdSign`) and every ack ticked)
 *                      signing → confirming → confirmed
 *   any step → failed (mapped error; Retry per `retryMode`)
 *
 * The wallet is never asked to sign before the user has seen a successful simulation of the
 * same instructions; a review older than `STALE_REVIEW_MS` is simulated again first.
 *
 * Retry after a failure (M9-UX-1): re-simulating the same plan only helps when the failure was
 * transient (RPC, an expired blockhash, a wallet rejection). A refusal that a fresh plan fixes — a
 * moved quote under its `min_*` / `max_*` bounds, a step the plan lacked (memo, epoch records,
 * the sweep bundle), changed venue accounts — is retried by `plan.rebuild()` (a fresh chain read,
 * a new quote, steps re-derived; the review then says so), or, for a plan without one, by closing
 * the review and re-reading its data so the form builds a new one. A transaction that may still
 * land (`unknown-outcome`) or has landed (a post hoc integrity incident) is never retried.
 */
import {
    type AddressesByLookupTableAddress,
    fetchAddressesForLookupTables,
    isWritableRole,
    type Signature,
} from '@solana/kit';
import { useQueryClient } from '@tanstack/react-query';
import { useCallback, useReducer, useRef } from 'react';
import { qk } from '../api/keys.ts';
import { DISCLOSURES, type DisclosureId } from '../config/disclosures.ts';
import { config } from '../config/env.ts';
import { useAppClient } from '../rpc/context.tsx';
import { useWallet } from '../wallet/useWallet.ts';
import { mapThrown } from './errors.ts';
import { estimatePriorityFee, type FeeSpeed } from './priorityFee.ts';
import { type SignedInfo, sendsItself, signSendConfirm } from './send.ts';
import { simulatePlan } from './simulate.ts';
import { TOKEN_ERROR_CPI_GUARD_TRANSFER_BLOCKED, TOKEN_ERROR_NO_MEMO } from './tokenAccount.ts';
import type { SimulationReport, TxError, TxPlan, TxStage } from './types.ts';

export const STALE_REVIEW_MS = 20_000;

export interface TxFlowState {
    readonly stage: TxStage;
    readonly plan: TxPlan | null;
    readonly report: SimulationReport | null;
    readonly acks: ReadonlySet<DisclosureId>;
    readonly speed: FeeSpeed;
    readonly signature: Signature | null;
    readonly error: TxError | null;
    /** True when the last simulation was triggered by a stale review at Sign. */
    readonly refreshed: boolean;
    /** The plan was rebuilt from fresh chain state by Retry (a new quote; the review says so). */
    readonly rebuilt?: boolean;
    /** Retry is rebuilding the plan (stage `simulating`). */
    readonly rebuilding?: boolean;
    /** The fee as the wallet signed it, when the wallet changed the compute budget (FE-2). */
    readonly finalFeeLamports?: bigint | null;
    /** The wallet sent the transaction itself (checked only after it landed, §5.4). */
    readonly sentByWallet?: boolean;
}

type Action =
    | { t: 'start'; plan: TxPlan }
    | { t: 'rebuilding' }
    | { t: 'rebuilt'; plan: TxPlan }
    | { t: 'simulating'; refreshed?: boolean }
    | { t: 'simulated'; report: SimulationReport }
    | { t: 'ack'; id: DisclosureId; v: boolean }
    | { t: 'speed'; speed: FeeSpeed }
    | { t: 'signing' }
    | { t: 'signed'; signature: Signature; finalFeeLamports?: bigint | null; sentByWallet?: boolean }
    | { t: 'confirmed' }
    | { t: 'failed'; error: TxError }
    | { t: 'close' };

const initial: TxFlowState = {
    stage: 'idle',
    plan: null,
    report: null,
    acks: new Set(),
    speed: 'normal',
    signature: null,
    error: null,
    refreshed: false,
};

export function txReducer(s: TxFlowState, a: Action): TxFlowState {
    switch (a.t) {
        case 'start':
            return { ...initial, speed: s.speed, plan: a.plan, stage: 'simulating' };
        case 'rebuilding':
            return { ...s, stage: 'simulating', error: null, refreshed: false, rebuilding: true };
        case 'rebuilt':
            // A new plan: nothing acknowledged or simulated for it yet.
            return { ...initial, speed: s.speed, plan: a.plan, stage: 'simulating', rebuilt: true };
        case 'simulating':
            return { ...s, stage: 'simulating', error: null, refreshed: !!a.refreshed, rebuilding: false };
        case 'simulated':
            return { ...s, stage: a.report.ok ? 'review' : 'failed', report: a.report, error: a.report.error };
        case 'ack': {
            const acks = new Set(s.acks);
            if (a.v) acks.add(a.id);
            else acks.delete(a.id);
            return { ...s, acks };
        }
        case 'speed':
            return { ...s, speed: a.speed };
        case 'signing':
            return { ...s, stage: 'signing', error: null };
        case 'signed':
            return {
                ...s,
                stage: 'confirming',
                signature: a.signature,
                ...(a.finalFeeLamports != null ? { finalFeeLamports: a.finalFeeLamports } : {}),
                ...(a.sentByWallet ? { sentByWallet: true } : {}),
            };
        case 'confirmed':
            return { ...s, stage: 'confirmed' };
        case 'failed':
            return { ...s, stage: 'failed', error: a.error, rebuilding: false };
        case 'close':
            return { ...initial, speed: s.speed };
    }
}

/** The disclosures of a plan that need a ticked checkbox before signing. */
export function requiredAcks(plan: TxPlan | null): DisclosureId[] {
    return (plan?.disclosures ?? []).filter((id) => DISCLOSURES[id].ack !== null);
}

/**
 * Why the plan's successful simulation must not be signed (`TxPlan.holdSign`, e.g. a claim that
 * would pay 0 lamports), or `null`.
 */
export function signHold(s: Pick<TxFlowState, 'stage' | 'plan' | 'report'>): string | null {
    if (s.stage !== 'review' || !s.report?.ok || !s.plan?.holdSign) return null;
    return s.plan.holdSign(s.report);
}

export function canSign(s: TxFlowState): boolean {
    return (
        s.stage === 'review' &&
        !!s.report?.ok &&
        signHold(s) === null &&
        requiredAcks(s.plan).every((id) => s.acks.has(id))
    );
}

/**
 * Whether a fresh plan (not a re-simulation of this one) is what fixes `error`: a short.win
 * refusal whose action is `retry` (a moved quote under its bounds, changed venue accounts or epoch
 * records, a missing memo / sync / sweep step), Token-2022's memo (36) and CPI Guard (42) refusals
 * (the destination or source changed since the read), or a rebuild that itself failed.
 */
export function needsNewPlan(error: TxError | null): boolean {
    if (!error) return false;
    if (error.kind === 'program') return error.action === 'retry';
    if (error.kind === 'token') {
        return error.code === TOKEN_ERROR_NO_MEMO || error.code === TOKEN_ERROR_CPI_GUARD_TRANSFER_BLOCKED;
    }
    return error.kind === 'plan';
}

/**
 * What the review's Retry / Refresh button does now: re-simulate the same plan, rebuild it
 * (`plan.rebuild`), close the review so its form re-reads and builds a new one, or nothing (the
 * transaction may still land or has landed).
 */
export type RetryMode = 'resimulate' | 'rebuild' | 'close' | 'none';

export function retryMode(s: TxFlowState): RetryMode {
    if (s.stage === 'review') return 'resimulate';
    if (s.stage !== 'failed' || !s.plan) return 'none';
    const e = s.error;
    if (e?.kind === 'unknown-outcome') return 'none';
    if (e?.kind === 'integrity' && s.signature) return 'none';
    if (needsNewPlan(e)) return s.plan.rebuild ? 'rebuild' : 'close';
    return 'resimulate';
}

/** Who will sign (shown in the review, M9-SEC-8). */
export interface SigningWallet {
    readonly name: string;
    readonly icon: string | null;
    readonly address: string;
    /** Only `solana:signAndSendTransaction`: nothing can be checked before it is sent (§5.4). */
    readonly sendsItself: boolean;
}

export interface TxFlow {
    readonly state: TxFlowState;
    start(plan: TxPlan): void;
    refresh(): void;
    setAck(id: DisclosureId, v: boolean): void;
    /** Changes the priority-fee speed and re-simulates an open review with it. */
    setSpeed(speed: FeeSpeed): void;
    sign(): void;
    close(): void;
    /** The connected wallet and account that will sign (absent in some test stubs). */
    readonly wallet?: SigningWallet | null;
}

export function useTxFlow(): TxFlow {
    const client = useAppClient();
    const qc = useQueryClient();
    const { connected } = useWallet();
    const [state, dispatch] = useReducer(txReducer, initial);
    const live = useRef(state);
    live.current = state;
    const abort = useRef<AbortController | null>(null);

    const tables = useCallback(
        async (plan: TxPlan): Promise<AddressesByLookupTableAddress> => {
            const list = [...(config.protocolAlt ? [config.protocolAlt] : []), ...(plan.lookupTables ?? [])];
            if (!list.length || !connected?.supportsV0) return {};
            return qc.fetchQuery({
                queryKey: ['chain', 'alt', list.join(',')],
                queryFn: () => fetchAddressesForLookupTables(list, client.rpc),
                staleTime: 10 * 60_000,
            });
        },
        [client, qc, connected?.supportsV0],
    );

    const simulate = useCallback(
        async (plan: TxPlan, refreshed = false, speed: FeeSpeed = live.current.speed) => {
            if (!connected?.signer) {
                dispatch({
                    t: 'failed',
                    error: { kind: 'wallet', action: 'blocked', message: 'Connect a wallet that can sign.' },
                });
                return;
            }
            dispatch({ t: 'simulating', refreshed });
            try {
                const writable = plan.instructions.flatMap((ix) =>
                    (ix.accounts ?? []).filter((m) => isWritableRole(m.role)).map((m) => m.address),
                );
                const [lookupTables, fee] = await Promise.all([
                    tables(plan),
                    estimatePriorityFee(
                        client.rpc,
                        [...new Set(writable)],
                        speed,
                        config.priorityFeeMaxMicroLamports,
                        config.cluster === 'localnet',
                    ),
                ]);
                const { report } = await simulatePlan({
                    rpc: client.rpc,
                    plan,
                    feePayer: connected.signer,
                    lookupTables,
                    version: connected.supportsV0 ? 0 : 'legacy',
                    priorityFeeMicroLamports: fee,
                });
                const refined =
                    report.error && plan.refineError ? { ...report, error: plan.refineError(report.error) } : report;
                dispatch({ t: 'simulated', report: refined });
            } catch (e) {
                dispatch({ t: 'failed', error: mapThrown(e) });
            }
        },
        [client, connected, tables],
    );

    const start = useCallback(
        (plan: TxPlan) => {
            dispatch({ t: 'start', plan });
            void simulate(plan);
        },
        [simulate],
    );

    const rebuild = useCallback(
        async (plan: TxPlan) => {
            if (!plan.rebuild) return;
            dispatch({ t: 'rebuilding' });
            let next: TxPlan;
            try {
                next = await plan.rebuild();
            } catch (e) {
                dispatch({ t: 'failed', error: mapThrown(e) });
                return;
            }
            dispatch({ t: 'rebuilt', plan: next });
            await simulate(next);
        },
        [simulate],
    );

    const close = useCallback(() => {
        const st = live.current.stage;
        if (st === 'signing' || st === 'confirming') return; // never orphan an in-flight transaction
        abort.current?.abort();
        dispatch({ t: 'close' });
    }, []);

    const refresh = useCallback(() => {
        const s = live.current;
        const plan = s.plan;
        if (!plan) return;
        switch (retryMode(s)) {
            case 'resimulate':
                void simulate(plan);
                return;
            case 'rebuild':
                void rebuild(plan);
                return;
            case 'close':
                // The form re-reads what the plan was built from, then builds a new one on Review.
                for (const key of plan.invalidate) void qc.invalidateQueries({ queryKey: key });
                close();
                return;
            case 'none':
                return;
        }
    }, [simulate, rebuild, close, qc]);

    const sign = useCallback(() => {
        const s = live.current;
        if (!canSign(s) || !s.plan || !s.report || !connected?.signer) return;
        if (Date.now() - s.report.simulatedAt > STALE_REVIEW_MS) {
            void simulate(s.plan, true);
            return;
        }
        const plan = s.plan;
        const report = s.report;
        const signer = connected.signer;
        dispatch({ t: 'signing' });
        abort.current = new AbortController();
        void (async () => {
            try {
                const lookupTables = await tables(plan);
                await signSendConfirm({
                    client,
                    plan,
                    feePayer: signer,
                    lookupTables,
                    version: connected.supportsV0 ? 0 : 'legacy',
                    computeUnitLimit: report.computeUnitLimit,
                    priorityFeeMicroLamports: report.priorityFeeMicroLamports,
                    maxComputeUnitPrice: config.priorityFeeMaxMicroLamports,
                    onSigned: (info: SignedInfo) => dispatch({ t: 'signed', ...info }),
                    abortSignal: abort.current?.signal,
                });
                dispatch({ t: 'confirmed' });
                for (const key of plan.invalidate) void qc.invalidateQueries({ queryKey: key });
                void qc.invalidateQueries({ queryKey: qk.balance(connected.address) });
            } catch (e) {
                dispatch({ t: 'failed', error: mapThrown(e) });
            }
        })();
    }, [client, connected, qc, simulate, tables]);

    const wallet: SigningWallet | null = connected
        ? {
              name: connected.walletName,
              icon: connected.walletIcon ?? null,
              address: connected.address,
              sendsItself: sendsItself(connected.signer),
          }
        : null;

    return {
        state,
        start,
        refresh,
        setAck: (id, v) => dispatch({ t: 'ack', id, v }),
        setSpeed: (speed) => {
            dispatch({ t: 'speed', speed });
            const plan = live.current.plan;
            const st = live.current.stage;
            if (plan && (st === 'review' || st === 'failed')) void simulate(plan, false, speed);
        },
        sign,
        close,
        wallet,
    };
}
