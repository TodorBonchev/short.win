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
 * Nothing awaits the network between the Sign tap and the wallet (§5.4 step 2; an Android wallet
 * app opens only within the tap's user activation): the blockhash is the simulation's, refreshed
 * every `LIFETIME_REFRESH_MS` while the review waits, and the lookup tables are the simulation's.
 *
 * The Mobile Wallet Adapter (§5.4 (6)): Sign asks Chrome's local-network permission first (the
 * review stays, with a notice); a launch the phone refused keeps the review open with a retry
 * notice; the adapter signs only through `solana:signTransaction` (the signer policy); and its calls
 * that never settle have a way out (FE-35): Cancel works `MWA_SIGN_CANCEL_AFTER_MS` after the wallet
 * call starts (R-S1), and a sign whose connected MWA account changes ends at once (R-S2). A signed
 * transaction is never sent after an abort (`send.ts` checks it before `onSigned`), and the way out
 * is armed only where that holds; one a wallet sent itself is never dropped silently.
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
import { useCallback, useEffect, useReducer, useRef } from 'react';
import { qk } from '../api/keys.ts';
import { DISCLOSURES, type DisclosureId } from '../config/disclosures.ts';
import { config } from '../config/env.ts';
import { useAppClient } from '../rpc/context.tsx';
import { localNetwork, requestLocalNetwork } from '../wallet/localNetwork.ts';
import { isMwaWallet, mwaSignerOk } from '../wallet/mwa.ts';
import { MWA_SIGN_CANCEL_AFTER_MS } from '../wallet/mwaCall.ts';
import { useWallet } from '../wallet/useWallet.ts';
import { mapThrown } from './errors.ts';
import type { Lifetime } from './message.ts';
import { estimatePriorityFee, type FeeSpeed } from './priorityFee.ts';
import { OutcomeUnknownError, type SignedInfo, sendsItself, signSendConfirm } from './send.ts';
import { simulatePlan } from './simulate.ts';
import { TOKEN_ERROR_CPI_GUARD_TRANSFER_BLOCKED, TOKEN_ERROR_NO_MEMO } from './tokenAccount.ts';
import type { SimulationReport, TxError, TxPlan, TxStage } from './types.ts';

export const STALE_REVIEW_MS = 20_000;
/** How often the review's blockhash is refreshed while it waits for Sign. */
export const LIFETIME_REFRESH_MS = 10_000;

/** The notice beside Sign that keeps the review open (the Mobile Wallet Adapter, §5.4 (6)). */
export type SignNotice = 'local-network' | 'launch';

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
    /**
     * Beside Sign, the review kept open: Chrome's local-network permission is missing, or the
     * wallet app could not be launched (Sign again).
     */
    readonly signNotice?: SignNotice | null;
    /** Signing with an MWA wallet for `MWA_SIGN_CANCEL_AFTER_MS`: Cancel closes the review (R-S1). */
    readonly signCancellable?: boolean;
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
    | { t: 'signNotice'; notice: SignNotice }
    | { t: 'signCancellable' }
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
            return { ...s, stage: 'simulating', error: null, refreshed: false, rebuilding: true, signNotice: null };
        case 'rebuilt':
            // A new plan: nothing acknowledged or simulated for it yet.
            return { ...initial, speed: s.speed, plan: a.plan, stage: 'simulating', rebuilt: true };
        case 'simulating':
            return {
                ...s,
                stage: 'simulating',
                error: null,
                refreshed: !!a.refreshed,
                rebuilding: false,
                signNotice: null,
            };
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
            return { ...s, stage: 'signing', error: null, signNotice: null, signCancellable: false };
        case 'signed':
            return {
                ...s,
                stage: 'confirming',
                signature: a.signature,
                signCancellable: false,
                ...(a.finalFeeLamports != null ? { finalFeeLamports: a.finalFeeLamports } : {}),
                ...(a.sentByWallet ? { sentByWallet: true } : {}),
            };
        case 'confirmed':
            return { ...s, stage: 'confirmed' };
        case 'failed':
            // A notice belongs to the review it kept open (a refresh or a refusal can fail straight
            // from it, with no `simulating` in between).
            return {
                ...s,
                stage: 'failed',
                error: a.error,
                rebuilding: false,
                signCancellable: false,
                signNotice: null,
            };
        case 'signNotice':
            // Back to the review, Sign enabled, with the notice beside it.
            if (s.stage !== 'review' && s.stage !== 'signing') return s;
            return { ...s, stage: 'review', signNotice: a.notice, signCancellable: false };
        case 'signCancellable':
            return s.stage === 'signing' ? { ...s, signCancellable: true } : s;
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
    // The connected MWA account changed mid-sign (R-S2): the form re-reads with whatever is
    // connected now; the old plan is never re-simulated for another account.
    if (e?.kind === 'wallet-changed') return 'close';
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

/** What the review's simulation used, kept for Sign (no network await between the tap and the wallet). */
interface Prepared {
    readonly plan: TxPlan;
    readonly lookupTables: AddressesByLookupTableAddress;
    readonly lifetime: Lifetime;
    readonly at: number;
}

/** Where the current sign is: asking the wallet, handed to the network, or over. */
type SignPhase = 'idle' | 'wallet' | 'sent' | 'ended';

const CANNOT_SIGN: TxError = { kind: 'wallet', action: 'blocked', message: 'Connect a wallet that can sign.' };

export function useTxFlow(): TxFlow {
    const client = useAppClient();
    const qc = useQueryClient();
    const { connected } = useWallet();
    const [state, dispatch] = useReducer(txReducer, initial);
    const live = useRef(state);
    live.current = state;
    const abort = useRef<AbortController | null>(null);
    const prepared = useRef<Prepared | null>(null);
    const phase = useRef<SignPhase>('idle');
    /** The MWA account the current sign is for (R-S2); null for other wallets. */
    const signingAs = useRef<string | null>(null);
    const cancelTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

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
                prepared.current = null;
                dispatch({ t: 'failed', error: CANNOT_SIGN });
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
                const { report, lifetime } = await simulatePlan({
                    rpc: client.rpc,
                    plan,
                    feePayer: connected.signer,
                    lookupTables,
                    version: connected.supportsV0 ? 0 : 'legacy',
                    priorityFeeMicroLamports: fee,
                });
                // Kept for Sign (a ref: no re-render): the blockhash simulated with, the tables.
                prepared.current = report.ok && lifetime ? { plan, lookupTables, lifetime, at: Date.now() } : null;
                const refined =
                    report.error && plan.refineError ? { ...report, error: plan.refineError(report.error) } : report;
                dispatch({ t: 'simulated', report: refined });
            } catch (e) {
                prepared.current = null;
                dispatch({ t: 'failed', error: mapThrown(e) });
            }
        },
        [client, connected, tables],
    );

    // While the review waits, its blockhash stays fresh (at most 10 s + a round trip old at Sign).
    const stage = state.stage;
    useEffect(() => {
        if (stage !== 'review') return;
        const id = setInterval(() => {
            if (typeof document !== 'undefined' && document.hidden) return;
            const p = prepared.current;
            if (!p) return;
            try {
                client.rpc
                    .getLatestBlockhash({ commitment: 'confirmed' })
                    .send()
                    .then(
                        ({ value }) => {
                            const now = prepared.current;
                            if (now && now.plan === p.plan) {
                                prepared.current = { ...now, lifetime: value as Lifetime, at: Date.now() };
                            }
                        },
                        () => {},
                    );
            } catch {
                // The simulation's blockhash stays; Sign's stale-review rule bounds its age.
            }
        }, LIFETIME_REFRESH_MS);
        return () => clearInterval(id);
    }, [stage, client]);

    // R-S2: the connected MWA account changed (or the wallet dropped it) while it was asked to sign.
    const connectedAddress = connected?.address ?? null;
    useEffect(() => {
        const as = signingAs.current;
        if (phase.current !== 'wallet' || as === null || connectedAddress === as) return;
        phase.current = 'ended';
        clearTimeout(cancelTimer.current);
        abort.current?.abort(new DOMException('The connected wallet changed', 'AbortError'));
        dispatch({ t: 'failed', error: { ...CANNOT_SIGN, kind: 'wallet-changed' } });
    }, [connectedAddress]);

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
        const s = live.current;
        // Never orphan an in-flight transaction: refused while confirming, and while signing unless
        // the MWA way out is open (R-S1) and nothing has been handed to the network yet.
        if (s.stage === 'confirming') return;
        if (s.stage === 'signing' && (!s.signCancellable || phase.current === 'sent')) return;
        if (s.stage === 'signing') {
            phase.current = 'ended';
            clearTimeout(cancelTimer.current);
        }
        abort.current?.abort(new DOMException('Cancelled at the review', 'AbortError'));
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
        const mwa = isMwaWallet(connected.walletName);
        // The signer policy, again before any wallet call: MWA's sending path is never used.
        if (mwa && !mwaSignerOk(connected.signer)) {
            dispatch({ t: 'failed', error: CANNOT_SIGN });
            return;
        }
        if (Date.now() - s.report.simulatedAt > STALE_REVIEW_MS) {
            void simulate(s.plan, true);
            return;
        }
        const p = prepared.current;
        if (!p || p.plan !== s.plan) {
            void simulate(s.plan, true);
            return;
        }
        // Chrome's local-network permission before the wallet (read synchronously: no await).
        if (mwa) {
            const lna = localNetwork.get();
            if (lna !== 'ready') {
                if (lna === 'prompt') requestLocalNetwork();
                dispatch({ t: 'signNotice', notice: 'local-network' });
                return;
            }
        }
        const plan = s.plan;
        const report = s.report;
        const signer = connected.signer;
        // The way out (R-S1 Cancel, R-S2) aborts the wallet call. That is safe only on send.ts's
        // signing branch, where nothing is sent before `onSigned`; a sending wallet may have sent
        // already. The policy above keeps MWA there; this keeps the way out there if it changes.
        const wayOut = mwa && mwaSignerOk(signer);
        const ctl = new AbortController();
        abort.current = ctl;
        phase.current = 'wallet';
        signingAs.current = wayOut ? connected.address : null;
        dispatch({ t: 'signing' });
        clearTimeout(cancelTimer.current);
        const timer = wayOut
            ? setTimeout(() => dispatch({ t: 'signCancellable' }), MWA_SIGN_CANCEL_AFTER_MS)
            : undefined;
        cancelTimer.current = timer;
        void (async () => {
            try {
                await signSendConfirm({
                    client,
                    plan,
                    feePayer: signer,
                    lookupTables: p.lookupTables,
                    lifetime: p.lifetime,
                    version: connected.supportsV0 ? 0 : 'legacy',
                    computeUnitLimit: report.computeUnitLimit,
                    priorityFeeMicroLamports: report.priorityFeeMicroLamports,
                    maxComputeUnitPrice: config.priorityFeeMaxMicroLamports,
                    onSigned: (info: SignedInfo) => {
                        clearTimeout(timer);
                        // Before the dispatch: a Cancel in the render gap is refused.
                        if (abort.current === ctl) phase.current = 'sent';
                        dispatch({ t: 'signed', ...info });
                    },
                    abortSignal: ctl.signal,
                });
                if (abort.current === ctl) phase.current = 'ended';
                dispatch({ t: 'confirmed' });
                for (const key of plan.invalidate) void qc.invalidateQueries({ queryKey: key });
                void qc.invalidateQueries({ queryKey: qk.balance(connected.address) });
            } catch (e) {
                clearTimeout(timer);
                // Cancel closed the review, or R-S2 already reported: never "You rejected…". A
                // transaction the wallet already sent is still reported (unknown outcome, no retry).
                if (ctl.signal.aborted && !(e instanceof OutcomeUnknownError)) return;
                if (abort.current === ctl) phase.current = 'ended';
                const err = mapThrown(e);
                if (err.kind === 'wallet-launch' || err.kind === 'local-network') {
                    dispatch({ t: 'signNotice', notice: err.kind === 'local-network' ? 'local-network' : 'launch' });
                } else {
                    dispatch({ t: 'failed', error: err });
                }
            }
        })();
    }, [client, connected, qc, simulate]);

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
