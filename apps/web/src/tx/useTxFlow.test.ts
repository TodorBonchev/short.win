/**
 * The transaction flow (FRONTEND.md §5): the reducer's gates; and in the hook, the Sign path of
 * O-6 (§5.4 step 2 and (6); mobile.md §1.7, §1.10, §1.11): nothing awaited between the tap and the
 * wallet (the simulation's blockhash, refreshed while the review waits, and its lookup tables), the
 * Mobile Wallet Adapter's local-network step and notices, the way out of a sign that never settles
 * (R-S1), the end of a sign whose connected MWA account changes (R-S2), and the signer policy.
 */
import {
    address,
    type Blockhash,
    compileTransaction,
    getCompiledTransactionMessageDecoder,
    type Transaction,
} from '@solana/kit';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook, waitFor } from '@testing-library/react';
import { createElement, type ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { localNetwork } from '../wallet/localNetwork.ts';
import { mwaRegistered } from '../wallet/mwa.ts';
import { MWA_SIGN_CANCEL_AFTER_MS } from '../wallet/mwaCall.ts';
import { buildMessage } from './message.ts';
import { OutcomeUnknownError, type SignedInfo } from './send.ts';
import type { SimulationReport, TxPlan } from './types.ts';
import {
    canSign,
    LIFETIME_REFRESH_MS,
    requiredAcks,
    retryMode,
    signHold,
    type TxFlowState,
    txReducer,
    useTxFlow,
} from './useTxFlow.ts';

const plan: TxPlan = {
    kind: 'deposit',
    title: 'Deposit',
    instructions: [],
    watch: [],
    disclosures: ['lender.solRepayment', 'lender.risks'],
    invalidate: [],
};

const ok: SimulationReport = {
    ok: true,
    unitsConsumed: 1n,
    computeUnitLimit: 20_000,
    priorityFeeMicroLamports: 0n,
    feeLamports: 5_000n,
    sizeBytes: 500,
    deltas: [],
    logs: [],
    error: null,
    simulatedAt: 0,
};

const idle: TxFlowState = {
    stage: 'idle',
    plan: null,
    report: null,
    acks: new Set(),
    speed: 'normal',
    signature: null,
    error: null,
    refreshed: false,
};

describe('transaction flow', () => {
    it('gates signing on a successful simulation and every required acknowledgement', () => {
        expect(requiredAcks(plan)).toEqual(['lender.solRepayment']);
        let s = txReducer(idle, { t: 'start', plan });
        expect(s.stage).toBe('simulating');
        expect(canSign(s)).toBe(false);
        s = txReducer(s, { t: 'simulated', report: ok });
        expect(s.stage).toBe('review');
        expect(canSign(s)).toBe(false);
        s = txReducer(s, { t: 'ack', id: 'lender.solRepayment', v: true });
        expect(canSign(s)).toBe(true);
        s = txReducer(s, { t: 'ack', id: 'lender.solRepayment', v: false });
        expect(canSign(s)).toBe(false);
    });

    it('never offers Sign for a simulation the plan holds (a claim that pays 0: “Nothing to claim yet”)', () => {
        const claim: TxPlan = {
            ...plan,
            kind: 'claim_sol',
            disclosures: [],
            holdSign: (r) => (r.deltas.length === 0 ? 'Nothing to claim yet' : null),
        };
        let s = txReducer(idle, { t: 'start', plan: claim });
        expect(signHold(s)).toBeNull();
        s = txReducer(s, { t: 'simulated', report: ok });
        expect(s.stage).toBe('review');
        expect(signHold(s)).toBe('Nothing to claim yet');
        expect(canSign(s)).toBe(false);
        const paid = { key: 'v:lamports', label: 'Market SOL vault', before: 2n, after: 1n, unit: 'SOL', scale: 9 };
        s = txReducer(s, { t: 'simulated', report: { ...ok, deltas: [paid] } });
        expect(signHold(s)).toBeNull();
        expect(canSign(s)).toBe(true);
    });

    it('never offers Sign after a failed simulation', () => {
        let s = txReducer(idle, { t: 'start', plan });
        s = txReducer(s, { t: 'ack', id: 'lender.solRepayment', v: true });
        s = txReducer(s, {
            t: 'simulated',
            report: { ...ok, ok: false, error: { kind: 'program', code: 6051, message: 'paused', action: 'wait' } },
        });
        expect(s.stage).toBe('failed');
        expect(s.error?.code).toBe(6051);
        expect(canSign(s)).toBe(false);
    });

    it('moves signing → confirming → confirmed and resets on close, keeping the fee speed', () => {
        let s = txReducer(idle, { t: 'speed', speed: 'fast' });
        s = txReducer(s, { t: 'start', plan });
        s = txReducer(s, { t: 'simulated', report: ok });
        s = txReducer(s, { t: 'signing' });
        expect(s.stage).toBe('signing');
        s = txReducer(s, { t: 'signed', signature: 'sig' as never });
        expect(s.stage).toBe('confirming');
        s = txReducer(s, { t: 'confirmed' });
        expect(s.stage).toBe('confirmed');
        s = txReducer(s, { t: 'close' });
        expect(s).toEqual({ ...idle, speed: 'fast' });
    });

    it('O-6: a notice keeps the review open; the way out only while signing', () => {
        let s = txReducer(idle, { t: 'start', plan: { ...plan, disclosures: [] } });
        s = txReducer(s, { t: 'simulated', report: ok });
        s = txReducer(s, { t: 'signing' });
        expect(s.signCancellable).toBe(false);
        s = txReducer(s, { t: 'signCancellable' });
        expect(s.signCancellable).toBe(true);
        s = txReducer(s, { t: 'signNotice', notice: 'launch' });
        expect(s).toMatchObject({ stage: 'review', signNotice: 'launch', signCancellable: false });
        expect(canSign(s)).toBe(true);
        expect(txReducer(s, { t: 'signCancellable' })).toBe(s); // not while reviewing
        s = txReducer(s, { t: 'simulating', refreshed: true });
        expect(s.signNotice).toBeNull();
        s = txReducer(s, { t: 'simulated', report: ok });
        s = txReducer(s, { t: 'signNotice', notice: 'local-network' });
        expect(s.signNotice).toBe('local-network');
        // A failure straight from the review (a refresh with no signer, the policy refusal) does
        // not carry the review's notice.
        const refused = txReducer(s, { t: 'failed', error: { kind: 'wallet', action: 'blocked', message: 'x' } });
        expect(refused).toMatchObject({ stage: 'failed', signNotice: null });
        s = txReducer(s, { t: 'signing' });
        expect(s.signNotice).toBeNull();
        // Only from review or signing.
        const failed = txReducer(s, { t: 'failed', error: { kind: 'wallet', action: 'retry', message: 'x' } });
        expect(failed.signCancellable).toBe(false);
        expect(txReducer(failed, { t: 'signNotice', notice: 'launch' })).toBe(failed);
        expect(retryMode({ ...failed, error: { kind: 'wallet-changed', action: 'blocked', message: '' } })).toBe(
            'close',
        );
    });
});

// ---- the hook ----------------------------------------------------------------------------------

const WALLET = address('9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin');
const OTHER = address('BPFLoaderUpgradeab1e11111111111111111111111');
const ALT = address('AddressLookupTab1e1111111111111111111111111');
const BH_A = 'EkSnNWid2cvwEVnVx9aBqawnmiCNiDgp3gUdkDPTKN1N' as Blockhash;
const BH_B = '4uQeVj5tqViQh7yWWGStvkEG1Zmhx6uasJtWCJziofM' as Blockhash;
const MWA_NAME = 'Mobile Wallet Adapter';

const h = vi.hoisted(() => {
    const counts = { getLatestBlockhash: 0, alt: 0, sendTransaction: 0, isBlockhashValid: 0 };
    const state = {
        blockhash: 'EkSnNWid2cvwEVnVx9aBqawnmiCNiDgp3gUdkDPTKN1N',
        /** Whether the simulation reports its blockhash (prepared for Sign). */
        simLifetime: true,
        isValid: null as null | (() => Promise<{ value: boolean }>),
    };
    const rpc = {
        getLatestBlockhash: () => ({
            send: async () => {
                counts.getLatestBlockhash++;
                return { value: { blockhash: state.blockhash, lastValidBlockHeight: 100n } };
            },
        }),
        isBlockhashValid: () => ({
            send: () => {
                counts.isBlockhashValid++;
                return state.isValid ? state.isValid() : Promise.resolve({ value: true });
            },
        }),
        getBlockHeight: () => ({ send: async () => 50n }),
        sendTransaction: () => ({
            send: async () => {
                counts.sendTransaction++;
                return 'sig';
            },
        }),
    };
    return {
        counts,
        state,
        client: { rpc, rpcSubscriptions: {} },
        connected: null as unknown,
        listeners: new Set<() => void>(),
        /** Replaces signSendConfirm when set (the phase-gap and post-onSigned cases). */
        send: null as null | ((a: Record<string, unknown>) => Promise<unknown>),
        /** Replaces useWallet when set (a signer the real useWallet would never hand over). */
        fakeWallet: null as null | (() => unknown),
    };
});

vi.mock('../rpc/context.tsx', () => ({ useAppClient: () => h.client }));
vi.mock('@solana/kit-plugin-wallet/react', async () => {
    const { useSyncExternalStore } = await import('react');
    const subscribe = (l: () => void) => {
        h.listeners.add(l);
        return () => h.listeners.delete(l);
    };
    return {
        useConnectedWallet: () => useSyncExternalStore(subscribe, () => h.connected),
        useIsWalletReady: () => true,
        useWalletStatus: () => 'connected',
    };
});
vi.mock('./priorityFee.ts', () => ({ estimatePriorityFee: async () => 0n }));
vi.mock('./simulate.ts', () => ({
    simulatePlan: async (a: { rpc: { getLatestBlockhash(): { send(): Promise<{ value: unknown }> } } }) => {
        const { value } = await a.rpc.getLatestBlockhash().send();
        const report: SimulationReport = {
            ok: true,
            unitsConsumed: 1_000n,
            computeUnitLimit: 20_000,
            priorityFeeMicroLamports: 0n,
            feeLamports: 5_000n,
            sizeBytes: 300,
            deltas: [],
            logs: [],
            error: null,
            simulatedAt: Date.now(),
        };
        return { report, lifetime: h.state.simLifetime ? value : null };
    },
}));
vi.mock('@solana/kit', async (orig) => ({
    ...(await orig<typeof import('@solana/kit')>()),
    fetchAddressesForLookupTables: async (list: readonly string[]) => {
        h.counts.alt++;
        return Object.fromEntries(list.map((a) => [a, []]));
    },
}));
vi.mock('./send.ts', async (orig) => {
    const real = await orig<typeof import('./send.ts')>();
    return {
        ...real,
        signSendConfirm: (a: Parameters<typeof real.signSendConfirm>[0]) =>
            h.send ? h.send(a as unknown as Record<string, unknown>) : real.signSendConfirm(a),
    };
});
vi.mock('../wallet/useWallet.ts', async (orig) => {
    const real = await orig<typeof import('../wallet/useWallet.ts')>();
    return {
        useWallet: () => {
            const wallet = real.useWallet();
            return h.fakeWallet ? h.fakeWallet() : wallet;
        },
    };
});

const flowPlan: TxPlan = { ...plan, disclosures: [], lookupTables: [ALT] };

/**
 * A modifying wallet (`solana:signTransaction`) whose answer the test controls; `signer` is what the
 * plugin hands over (Kit freezes it, so the record lives beside it).
 */
function walletSigner() {
    const s = {
        calls: 0,
        countsAtCall: null as null | typeof h.counts,
        txs: [] as Transaction[],
        aborted: false,
        onCall: null as null | (() => void),
        /** What the wallet does: never answer (but honour the abort), answer, or throw. */
        mode: 'never' as 'never' | 'echo' | 'otherBlockhash' | Error,
        release: null as null | (() => void),
        signer: null as unknown,
    };
    s.signer = {
        address: WALLET,
        modifyAndSignTransactions(txs: readonly Transaction[], config?: { abortSignal?: AbortSignal }) {
            s.calls++;
            s.countsAtCall = { ...h.counts };
            s.txs.push(...txs);
            s.onCall?.();
            const signal = config?.abortSignal;
            signal?.addEventListener('abort', () => {
                s.aborted = true;
            });
            if (s.mode instanceof Error) return Promise.reject(s.mode);
            const answer = () =>
                txs.map((tx) =>
                    s.mode === 'otherBlockhash'
                        ? {
                              messageBytes: compileTransaction(
                                  buildMessage({
                                      instructions: flowPlan.instructions,
                                      feePayer: { address: WALLET } as never,
                                      lifetime: { blockhash: BH_B, lastValidBlockHeight: 100n },
                                      computeUnitLimit: 20_000,
                                      computeUnitPrice: 0n,
                                      lookupTables: { [ALT]: [] },
                                      version: 0,
                                  }),
                              ).messageBytes,
                              signatures: { [WALLET]: new Uint8Array(64).fill(3) },
                              lifetimeConstraint: { blockhash: BH_B, lastValidBlockHeight: 100n },
                          }
                        : { ...tx, signatures: { ...tx.signatures, [WALLET]: new Uint8Array(64).fill(3) } },
                );
            // Like wallet-account-signer's getAbortablePromise: an abort rejects at once.
            return new Promise((resolve, reject) => {
                signal?.addEventListener('abort', () => reject(signal.reason));
                if (s.mode === 'echo' || s.mode === 'otherBlockhash') {
                    s.release = () => resolve(answer());
                }
            });
        },
    };
    return s;
}

/** A sending-only signer (`solana:signAndSendTransaction`). */
const sendingOnly = () => ({
    address: WALLET,
    signAndSendTransactions: vi.fn(async (txs: readonly unknown[]) => txs.map(() => new Uint8Array(64))),
});

/** The plugin's connected wallet. */
function plugged(name: string, signer: unknown, at: string = WALLET) {
    return {
        account: { address: at },
        signer,
        wallet: { name, icon: undefined },
        supportedTransactionVersions: new Set([0, 'legacy']),
    };
}
function setConnected(c: unknown) {
    h.connected = c;
    for (const l of [...h.listeners]) l();
}

/** A QueryClient per hook (the lookup tables are cached in it). */
function harness() {
    const qc = new QueryClient();
    return ({ children }: { children: ReactNode }) => createElement(QueryClientProvider, { client: qc }, children);
}

/** A flow at review (simulated once). */
async function reviewed(p: TxPlan = flowPlan) {
    const r = renderHook(() => useTxFlow(), { wrapper: harness() });
    await act(async () => {
        r.result.current.start(p);
    });
    expect(r.result.current.state.stage).toBe('review');
    return r;
}

const blockhashOf = (tx: Transaction | undefined) =>
    tx ? getCompiledTransactionMessageDecoder().decode(tx.messageBytes).lifetimeToken : null;

/** The library's errors as it throws them: its own error under `new Error(message, { cause })`. */
const mwaError = (name: string, code: string | number, data?: unknown) =>
    new Error('wrapped', {
        cause: Object.assign(new Error(String(code)), { name, code, data }),
    });

beforeEach(() => {
    for (const k of Object.keys(h.counts) as (keyof typeof h.counts)[]) h.counts[k] = 0;
    h.state.blockhash = BH_A;
    h.state.simLifetime = true;
    h.state.isValid = null;
    h.send = null;
    h.fakeWallet = null;
    h.connected = null;
});
afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
    act(() => {
        mwaRegistered.set(false);
        localNetwork.set('ready');
    });
});

describe('Sign: nothing awaited between the tap and the wallet (§5.4 step 2)', () => {
    it('the wallet is called within the tap’s microtasks, with no network request since the review', async () => {
        const signer = walletSigner();
        setConnected(plugged('Phantom', signer.signer));
        const r = await reviewed();
        // The review fetched one blockhash (the simulation) and the lookup tables once.
        expect(h.counts).toMatchObject({ getLatestBlockhash: 1, alt: 1 });
        const before = { ...h.counts };
        const order: string[] = [];
        signer.onCall = () => order.push('wallet');
        act(() => {
            r.result.current.sign();
            setTimeout(() => order.push('timeout'), 0);
        });
        await waitFor(() => expect(order).toContain('timeout'));
        expect(order[0]).toBe('wallet');
        expect(signer.countsAtCall).toEqual(before);
        expect(blockhashOf(signer.txs[0])).toBe(BH_A);
        expect(r.result.current.state.stage).toBe('signing');
    });

    it('the blockhash is refreshed every 10 s while the review waits, and only then', async () => {
        vi.useFakeTimers();
        const signer = walletSigner();
        setConnected(plugged('Phantom', signer.signer));
        const r = await reviewed();
        expect(h.counts.getLatestBlockhash).toBe(1);
        h.state.blockhash = BH_B;
        await act(async () => {
            await vi.advanceTimersByTimeAsync(LIFETIME_REFRESH_MS - 1);
        });
        expect(h.counts.getLatestBlockhash).toBe(1);
        await act(async () => {
            await vi.advanceTimersByTimeAsync(1);
        });
        expect(h.counts.getLatestBlockhash).toBe(2);
        // Not while the page is hidden.
        Object.defineProperty(document, 'hidden', { configurable: true, value: true });
        await act(async () => {
            await vi.advanceTimersByTimeAsync(LIFETIME_REFRESH_MS);
        });
        delete (document as unknown as Record<string, unknown>).hidden;
        expect(h.counts.getLatestBlockhash).toBe(2);
        // Sign uses the refreshed one.
        act(() => r.result.current.sign());
        await act(async () => {
            await vi.advanceTimersByTimeAsync(0);
        });
        expect(blockhashOf(signer.txs[0])).toBe(BH_B);
        expect(h.counts.getLatestBlockhash).toBe(2);
        // Outside the review the interval is gone.
        await act(async () => {
            await vi.advanceTimersByTimeAsync(3 * LIFETIME_REFRESH_MS);
        });
        expect(h.counts.getLatestBlockhash).toBe(2);
    });

    it('a review without its prepared blockhash is simulated again, without asking the wallet', async () => {
        const signer = walletSigner();
        setConnected(plugged('Phantom', signer.signer));
        h.state.simLifetime = false;
        const r = await reviewed();
        await act(async () => {
            r.result.current.sign();
        });
        expect(signer.calls).toBe(0);
        expect(h.counts.getLatestBlockhash).toBe(2);
        expect(r.result.current.state).toMatchObject({ stage: 'review', refreshed: true });
    });
});

describe('Sign with the Mobile Wallet Adapter (§5.4 (6))', () => {
    beforeEach(() => act(() => mwaRegistered.set(true)));

    it('local network "prompt": asks Chrome, does not call the wallet, keeps the review with the notice', async () => {
        const signer = walletSigner();
        setConnected(plugged(MWA_NAME, signer.signer));
        const fetch = vi.fn(async () => new Response(null));
        vi.stubGlobal('fetch', fetch);
        const r = await reviewed();
        act(() => localNetwork.set('prompt'));
        act(() => r.result.current.sign());
        expect(signer.calls).toBe(0);
        expect(fetch).toHaveBeenCalledOnce();
        expect(r.result.current.state).toMatchObject({ stage: 'review', signNotice: 'local-network' });
        expect(canSign(r.result.current.state)).toBe(true);
        // "denied": the notice, no request.
        act(() => localNetwork.set('denied'));
        act(() => r.result.current.sign());
        expect(fetch).toHaveBeenCalledOnce();
        expect(signer.calls).toBe(0);
    });

    it('a review with a notice that then fails (Refresh after the wallet left) drops the notice', async () => {
        const signer = walletSigner();
        setConnected(plugged(MWA_NAME, signer.signer));
        const r = await reviewed();
        act(() => localNetwork.set('denied'));
        act(() => r.result.current.sign());
        expect(r.result.current.state).toMatchObject({ stage: 'review', signNotice: 'local-network' });
        act(() => setConnected(null));
        await act(async () => {
            r.result.current.refresh();
        });
        expect(r.result.current.state).toMatchObject({ stage: 'failed', signNotice: null });
        expect(r.result.current.state.error?.message).toBe('Connect a wallet that can sign.');
        expect(signer.calls).toBe(0);
    });

    it.each([
        ['ERROR_WALLET_NOT_FOUND', { stage: 'review', signNotice: 'launch' }],
        ['ERROR_LOOPBACK_ACCESS_BLOCKED', { stage: 'review', signNotice: 'local-network' }],
    ] as const)('%s: the review stays, Sign enabled, with its notice', async (code, expected) => {
        const signer = walletSigner();
        signer.mode = mwaError('SolanaMobileWalletAdapterError', code);
        setConnected(plugged(MWA_NAME, signer.signer));
        const r = await reviewed();
        await act(async () => {
            r.result.current.sign();
        });
        expect(signer.calls).toBe(1);
        expect(r.result.current.state).toMatchObject({ ...expected, error: null });
        expect(canSign(r.result.current.state)).toBe(true);
    });

    it.each([
        ['SolanaMobileWalletAdapterProtocolError', -3, 'wallet-rejected'],
        ['SolanaMobileWalletAdapterError', 'ERROR_SESSION_CLOSED', 'wallet'],
    ] as const)('%s %s: failed, %s', async (name, code, kind) => {
        const signer = walletSigner();
        signer.mode = mwaError(name, code);
        setConnected(plugged(MWA_NAME, signer.signer));
        const r = await reviewed();
        await act(async () => {
            r.result.current.sign();
        });
        expect(r.result.current.state.stage).toBe('failed');
        expect(r.result.current.state.error?.kind).toBe(kind);
    });

    it('R-S1: a sign that never settles can be cancelled after 5 s; nothing is reported after', async () => {
        vi.useFakeTimers();
        const signer = walletSigner();
        setConnected(plugged(MWA_NAME, signer.signer));
        const r = await reviewed();
        act(() => r.result.current.sign());
        await act(async () => {
            await vi.advanceTimersByTimeAsync(MWA_SIGN_CANCEL_AFTER_MS - 1);
        });
        expect(signer.calls).toBe(1);
        expect(r.result.current.state).toMatchObject({ stage: 'signing', signCancellable: false });
        act(() => r.result.current.close());
        expect(r.result.current.state.stage).toBe('signing');
        expect(signer.aborted).toBe(false);
        await act(async () => {
            await vi.advanceTimersByTimeAsync(1);
        });
        expect(r.result.current.state.signCancellable).toBe(true);
        act(() => r.result.current.close());
        expect(r.result.current.state.stage).toBe('idle');
        expect(signer.aborted).toBe(true);
        await act(async () => {
            await vi.advanceTimersByTimeAsync(60_000);
        });
        // No "You rejected…": the abort is not a failure.
        expect(r.result.current.state).toMatchObject({ stage: 'idle', error: null });
        expect(h.counts.sendTransaction).toBe(0);
    });

    it('R-S1: another wallet never becomes cancellable (unchanged)', async () => {
        vi.useFakeTimers();
        act(() => mwaRegistered.set(true));
        const signer = walletSigner();
        setConnected(plugged('Phantom', signer.signer));
        const r = await reviewed();
        act(() => r.result.current.sign());
        await act(async () => {
            await vi.advanceTimersByTimeAsync(30_000);
        });
        expect(r.result.current.state.stage).toBe('signing');
        expect(r.result.current.state.signCancellable).toBeFalsy();
        act(() => r.result.current.close());
        expect(r.result.current.state.stage).toBe('signing');
        expect(signer.aborted).toBe(false);
    });

    it('an abort after the wallet answered (its blockhash still being checked): nothing sent, no onSigned', async () => {
        vi.useFakeTimers();
        const signer = walletSigner();
        signer.mode = 'otherBlockhash';
        let valid: (v: { value: boolean }) => void = () => {};
        h.state.isValid = () =>
            new Promise((resolve) => {
                valid = resolve;
            });
        setConnected(plugged(MWA_NAME, signer.signer));
        const r = await reviewed();
        act(() => r.result.current.sign());
        await act(async () => {
            await vi.advanceTimersByTimeAsync(MWA_SIGN_CANCEL_AFTER_MS);
        });
        expect(r.result.current.state.signCancellable).toBe(true);
        // The wallet answers; the app checks the blockhash it chose (pending).
        await act(async () => {
            signer.release?.();
            await vi.advanceTimersByTimeAsync(0);
        });
        expect(h.counts.isBlockhashValid).toBe(1);
        expect(r.result.current.state.stage).toBe('signing');
        act(() => r.result.current.close());
        expect(r.result.current.state.stage).toBe('idle');
        await act(async () => {
            valid({ value: true });
            await vi.advanceTimersByTimeAsync(1_000);
        });
        expect(h.counts.sendTransaction).toBe(0);
        expect(r.result.current.state).toMatchObject({ stage: 'idle', signature: null, error: null });
    });

    it('an unknown outcome is still reported after the way out was used (a wallet that had sent it)', async () => {
        // send.ts's sending branch after an abort that landed once the wallet answered: onSigned,
        // then OutcomeUnknownError. The review comes back with the signature, no retry (R6).
        vi.useFakeTimers();
        const signer = walletSigner();
        setConnected(plugged(MWA_NAME, signer.signer));
        const r = await reviewed();
        let answer: () => void = () => {};
        h.send = (a) =>
            new Promise((_, reject) => {
                answer = () => {
                    (a.onSigned as (i: SignedInfo) => void)({
                        signature: 'sig' as never,
                        finalFeeLamports: null,
                        sentByWallet: true,
                    });
                    reject(new OutcomeUnknownError('sig' as never, 'It may still land.'));
                };
            });
        act(() => r.result.current.sign());
        await act(async () => {
            await vi.advanceTimersByTimeAsync(MWA_SIGN_CANCEL_AFTER_MS);
        });
        act(() => r.result.current.close());
        expect(r.result.current.state.stage).toBe('idle');
        await act(async () => {
            answer();
            await vi.advanceTimersByTimeAsync(0);
        });
        expect(r.result.current.state).toMatchObject({ stage: 'failed', signature: 'sig', sentByWallet: true });
        expect(r.result.current.state.error?.kind).toBe('unknown-outcome');
        expect(retryMode(r.result.current.state)).toBe('none');
    });

    it('the phase gap: a close() inside onSigned (before the render) is refused', async () => {
        vi.useFakeTimers();
        const signer = walletSigner();
        setConnected(plugged(MWA_NAME, signer.signer));
        const r = await reviewed();
        let abortedAtClose: boolean | null = null;
        h.send = (a) =>
            new Promise(() => {
                setTimeout(() => {
                    (a.onSigned as (i: SignedInfo) => void)({
                        signature: 'sig' as never,
                        finalFeeLamports: null,
                        sentByWallet: false,
                    });
                    r.result.current.close(); // the same tick: state still says signing + cancellable
                    abortedAtClose = (a.abortSignal as AbortSignal).aborted;
                }, MWA_SIGN_CANCEL_AFTER_MS + 1_000);
            });
        act(() => r.result.current.sign());
        await act(async () => {
            await vi.advanceTimersByTimeAsync(MWA_SIGN_CANCEL_AFTER_MS);
        });
        expect(r.result.current.state.signCancellable).toBe(true);
        await act(async () => {
            await vi.advanceTimersByTimeAsync(1_000);
        });
        expect(abortedAtClose).toBe(false);
        expect(r.result.current.state).toMatchObject({ stage: 'confirming', signature: 'sig' });
    });

    it('R-S2: the connected wallet drops (or switches) mid-sign: failed wallet-changed, retried by closing', async () => {
        for (const next of [null, 'switch'] as const) {
            const signer = walletSigner();
            setConnected(plugged(MWA_NAME, signer.signer));
            const r = await reviewed();
            act(() => r.result.current.sign());
            await waitFor(() => expect(signer.calls).toBe(1));
            act(() => setConnected(next === null ? null : plugged(MWA_NAME, walletSigner().signer, OTHER)));
            expect(r.result.current.state.stage).toBe('failed');
            expect(r.result.current.state.error).toEqual({
                kind: 'wallet-changed',
                action: 'blocked',
                message: 'Connect a wallet that can sign.',
            });
            expect(retryMode(r.result.current.state)).toBe('close');
            expect(signer.aborted).toBe(true);
            await act(async () => {
                await Promise.resolve();
            });
            expect(r.result.current.state.error?.kind).toBe('wallet-changed'); // not overwritten
            r.unmount();
        }
    });

    it('R-S2: no effect once the transaction was handed over, nor for another wallet', async () => {
        const signer = walletSigner();
        setConnected(plugged(MWA_NAME, signer.signer));
        h.send = async (a) => {
            (a.onSigned as (i: SignedInfo) => void)({
                signature: 'sig' as never,
                finalFeeLamports: null,
                sentByWallet: false,
            });
            return new Promise(() => {});
        };
        const r = await reviewed();
        await act(async () => {
            r.result.current.sign();
        });
        expect(r.result.current.state.stage).toBe('confirming');
        act(() => setConnected(null));
        expect(r.result.current.state.stage).toBe('confirming');
        r.unmount();
        h.send = null;
        const other = walletSigner();
        setConnected(plugged('Phantom', other.signer));
        const r2 = await reviewed();
        act(() => r2.result.current.sign());
        await waitFor(() => expect(other.calls).toBe(1));
        act(() => setConnected(null));
        expect(r2.result.current.state.stage).toBe('signing');
        expect(other.aborted).toBe(false);
    });
});

describe('the signer policy: the adapter signs only through solana:signTransaction (§1.11)', () => {
    beforeEach(() => act(() => mwaRegistered.set(true)));

    it('an MWA wallet whose plugin signer is sending-only gets no signer: “Connect a wallet that can sign.”', async () => {
        const send = sendingOnly();
        setConnected(plugged(MWA_NAME, send));
        const r = renderHook(() => useTxFlow(), { wrapper: harness() });
        expect(r.result.current.wallet).toMatchObject({ name: MWA_NAME, sendsItself: false });
        await act(async () => {
            r.result.current.start(flowPlan);
        });
        expect(r.result.current.state.stage).toBe('failed');
        expect(r.result.current.state.error?.message).toBe('Connect a wallet that can sign.');
        expect(send.signAndSendTransactions).not.toHaveBeenCalled();
        // Another wallet with the same signer keeps it (FE-21 unchanged).
        setConnected(plugged('Sender', send));
        const r2 = renderHook(() => useTxFlow(), { wrapper: harness() });
        expect(r2.result.current.wallet?.sendsItself).toBe(true);
    });

    it('sign() refuses a sending-only signer for an MWA wallet even if handed one', async () => {
        const send = sendingOnly();
        h.fakeWallet = () => ({
            ready: true,
            status: 'connected',
            connected: { address: WALLET, signer: send, walletName: MWA_NAME, supportsV0: true },
        });
        const r = await reviewed();
        await act(async () => {
            r.result.current.sign();
        });
        expect(r.result.current.state.stage).toBe('failed');
        expect(r.result.current.state.error?.message).toBe('Connect a wallet that can sign.');
        expect(send.signAndSendTransactions).not.toHaveBeenCalled();
    });
});
