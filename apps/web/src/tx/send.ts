/**
 * Sign, send, confirm (FRONTEND.md §5.4–§5.5). The message is rebuilt with the measured compute
 * limit and the review's blockhash (fetched with the simulation, refreshed while the review waits;
 * `lifetime`) — nothing awaits the network between the Sign tap and the wallet — and signed through
 * Wallet Standard:
 *
 * - **A signing wallet** (`solana:signTransaction`, Kit's `TransactionModifyingSigner`): the signed
 *   bytes are checked by `assertSameIntent` before anything is sent. A wallet may change the
 *   compute budget (the final fee is reported, FE-2) and the blockhash — a changed blockhash is
 *   accepted only while `isBlockhashValid` says so, and its confirmation is then bounded by the
 *   current block height + 151 (no blockhash outlives that), never by Kit's open-ended bound.
 *   Kit's `sendAndConfirmTransactionFactory` sends with preflight and confirms at `confirmed`.
 * - **A sending-only wallet** (`solana:signAndSendTransaction`): it sends itself, so nothing can be
 *   checked before; the app waits for the signature, then reads the landed transaction back and
 *   runs the same intent check on it — a mismatch is reported as an integrity incident (it has
 *   landed). The app cannot see which blockhash such a wallet used, so a signature not seen in time
 *   ends as "outcome unknown", never as "expired" (a retry could otherwise send a second one).
 *
 * Every wallet call is wrapped (`WalletCallError`), so only what the wallet itself throws can read
 * as a rejection (`errors.ts`), and confirmation has a hard timeout (`OutcomeUnknownError`).
 */
import {
    type AddressesByLookupTableAddress,
    type Blockhash,
    compileTransaction,
    getBase58Decoder,
    getBase64Encoder,
    getSignatureFromTransaction,
    getTransactionDecoder,
    isTransactionModifyingSigner,
    isTransactionPartialSigner,
    isTransactionSendingSigner,
    type Signature,
    sendAndConfirmTransactionFactory,
    signAndSendTransactionMessageWithSigners,
    signTransactionMessageWithSigners,
    type TransactionSigner,
} from '@solana/kit';
import type { AppClient } from '../rpc/client.ts';
import { assertSameIntent, IntegrityError, type IntentCheck } from './integrity.ts';
import { buildMessage, feeLamports, type Lifetime } from './message.ts';
import type { TxPlan } from './types.ts';

/** No blockhash is valid more than 150 blocks after the block it names; one more for the boundary. */
export const MAX_BLOCKHASH_AGE = 151n;
/** The hard bound on waiting for a confirmation (the review becomes dismissible after it). */
export const CONFIRM_TIMEOUT_MS = 120_000;
/** How long the landed transaction of a sending-only wallet is polled for (post hoc check). */
const LANDED_READ_TRIES = 10;
const POLL_MS = 1_000;
/** A sending-only wallet's transaction whose landing the app has not seen. */
const SENT_BY_WALLET_UNSEEN =
    'Your wallet sent this transaction itself and it has not been seen on chain. It may still land.';

/** Something the wallet threw (rejection, a wallet bug, its own RPC): never an RPC error of the app. */
export class WalletCallError extends Error {
    override readonly cause: unknown;
    /**
     * The call was the sending path (`solana:signAndSendTransaction`): the wallet may already have
     * sent the transaction when it failed (`errors.ts` maps a Mobile Wallet Adapter transport error
     * there to "outcome unknown").
     */
    readonly sending: boolean;
    constructor(cause: unknown, opts: { readonly sending?: boolean } = {}) {
        super((cause as { message?: string } | null)?.message ?? String(cause));
        this.name = 'WalletCallError';
        this.cause = cause;
        this.sending = opts.sending ?? false;
    }
}

/**
 * The transaction was handed to the network (or to a sending wallet) but its outcome is not
 * known: it may still land. The review must not offer a blind retry.
 */
export class OutcomeUnknownError extends Error {
    readonly signature: Signature;
    constructor(signature: Signature, reason: string) {
        super(reason);
        this.name = 'OutcomeUnknownError';
        this.signature = signature;
    }
}

/** An integrity incident found after the transaction landed (sending-only wallets, §5.4). */
export class LandedIntegrityError extends IntegrityError {
    readonly signature: Signature;
    constructor(signature: Signature, reason: string) {
        super(reason);
        this.name = 'LandedIntegrityError';
        this.signature = signature;
    }
}

/** What the app learns at signing: the signature and, when the wallet changed it, the final fee. */
export interface SignedInfo {
    readonly signature: Signature;
    /** The fee of the transaction as signed, lamports, when the wallet changed the compute budget (FE-2). */
    readonly finalFeeLamports: bigint | null;
    /** The wallet sends itself: the transaction is checked only after it lands. */
    readonly sentByWallet: boolean;
}

export interface SendArgs {
    readonly client: AppClient;
    readonly plan: TxPlan;
    readonly feePayer: TransactionSigner;
    readonly lookupTables: AddressesByLookupTableAddress;
    readonly version: 0 | 'legacy';
    readonly computeUnitLimit: number;
    readonly priorityFeeMicroLamports: bigint;
    /** The highest compute-unit price a wallet may set when it edits the fee (FE-2). */
    readonly maxComputeUnitPrice: bigint;
    /**
     * The blockhash to build with: the review's, fetched with the simulation and refreshed while it
     * waits (`useTxFlow`), so nothing awaits the network between the Sign tap and the wallet (an
     * Android wallet app opens only within the tap's user activation). Without it one is fetched.
     */
    readonly lifetime?: Lifetime;
    /**
     * Called once the signature is known, before confirmation. A signing wallet's: never after an
     * abort. A sending wallet's: always once it answered (it has sent the transaction).
     */
    readonly onSigned?: (info: SignedInfo) => void;
    /**
     * A signing wallet: aborting before `onSigned` drops the transaction, nothing is sent. A sending
     * wallet: an abort before its call sends nothing; its call is not aborted (it sends itself), and
     * an abort meanwhile ends in `OutcomeUnknownError` with the signature, never a drop.
     */
    readonly abortSignal?: AbortSignal;
    /** Overrides `CONFIRM_TIMEOUT_MS` (tests). */
    readonly confirmTimeoutMs?: number;
    /** Overrides the signature / landed-transaction polling interval (tests). */
    readonly pollMs?: number;
}

/** A signer that can only sign-and-send (no `solana:signTransaction`). */
export function sendsItself(signer: TransactionSigner | null | undefined): boolean {
    return (
        !!signer &&
        isTransactionSendingSigner(signer) &&
        !isTransactionModifyingSigner(signer) &&
        !isTransactionPartialSigner(signer)
    );
}

async function walletCall<T>(f: () => Promise<T>, opts: { readonly sending?: boolean } = {}): Promise<T> {
    try {
        return await f();
    } catch (e) {
        throw new WalletCallError(e, opts);
    }
}

const sleep = (ms: number, signal?: AbortSignal) =>
    new Promise<void>((resolve, reject) => {
        const t = setTimeout(resolve, ms);
        signal?.addEventListener(
            'abort',
            () => {
                clearTimeout(t);
                reject(signal.reason);
            },
            { once: true },
        );
    });

/** The fee of a signed message from its compute budget (FE-2): base fee + ⌈limit · price / 10⁶⌉. */
export function signedFee(check: IntentCheck, instructionCount: number): bigint {
    // Without a limit instruction the runtime grants 200,000 CU per non-compute-budget instruction.
    const limit = check.computeUnitLimit ?? Math.min(1_400_000, 200_000 * Math.max(1, instructionCount));
    return feeLamports(1, limit, check.computeUnitPrice);
}

/**
 * The lifetime the signed transaction is confirmed against (M9-SEC-4): ours when the wallet kept
 * our blockhash; when it chose its own, that one only while `isBlockhashValid` says so, bounded by
 * the current block height + `MAX_BLOCKHASH_AGE` — never Kit's open-ended bound for a changed
 * message (`lastValidBlockHeight` = 2⁶⁴ − 1), which would leave the review confirming forever.
 */
export async function confirmationLifetime(
    rpc: Pick<AppClient['rpc'], 'isBlockhashValid' | 'getBlockHeight'>,
    check: Pick<IntentCheck, 'lifetimeChanged' | 'lifetimeToken'>,
    ours: Lifetime,
): Promise<Lifetime> {
    if (!check.lifetimeChanged) return ours;
    const blockhash = check.lifetimeToken as Blockhash;
    const { value: valid } = await rpc.isBlockhashValid(blockhash, { commitment: 'confirmed' }).send();
    if (!valid) throw new IntegrityError('the wallet set a blockhash that is no longer valid');
    const height = await rpc.getBlockHeight({ commitment: 'confirmed' }).send();
    return { blockhash, lastValidBlockHeight: height + MAX_BLOCKHASH_AGE };
}

export async function signSendConfirm(a: SendArgs): Promise<Signature> {
    const lifetime: Lifetime =
        a.lifetime ?? ((await a.client.rpc.getLatestBlockhash({ commitment: 'confirmed' }).send()).value as Lifetime);
    const message = buildMessage({
        instructions: a.plan.instructions,
        feePayer: a.feePayer,
        lifetime,
        computeUnitLimit: a.computeUnitLimit,
        computeUnitPrice: a.priorityFeeMicroLamports,
        lookupTables: a.lookupTables,
        version: a.version,
    });
    const expected = compileTransaction(message).messageBytes;
    const timeoutMs = a.confirmTimeoutMs ?? CONFIRM_TIMEOUT_MS;
    const signer = a.feePayer;
    const opts = a.abortSignal ? { abortSignal: a.abortSignal } : {};

    if (isTransactionModifyingSigner(signer) || isTransactionPartialSigner(signer)) {
        const signed = await walletCall(() => signTransactionMessageWithSigners(message, opts));
        const check = assertSameIntent(expected, signed.messageBytes, a.lookupTables, {
            maxComputeUnitPrice: a.maxComputeUnitPrice,
            allowLifetimeChange: true,
        });
        const confirmLifetime = await confirmationLifetime(a.client.rpc, check, lifetime);
        const feeChanged =
            check.modified &&
            (check.computeUnitPrice !== a.priorityFeeMicroLamports || check.computeUnitLimit !== a.computeUnitLimit);
        const signature = getSignatureFromTransaction(signed);
        // A Cancel that landed after the wallet answered (the review's way out of an MWA call that
        // never settles): the signed transaction is dropped, nothing is sent.
        a.abortSignal?.throwIfAborted();
        a.onSigned?.({
            signature,
            finalFeeLamports: feeChanged ? signedFee(check, a.plan.instructions.length) : null,
            sentByWallet: false,
        });
        const transaction = { ...signed, lifetimeConstraint: confirmLifetime };
        const sendAndConfirm = sendAndConfirmTransactionFactory({
            rpc: a.client.rpc,
            rpcSubscriptions: a.client.rpcSubscriptions,
        } as Parameters<typeof sendAndConfirmTransactionFactory>[0]);
        const inner = new AbortController();
        const onAbort = () => inner.abort(a.abortSignal?.reason);
        a.abortSignal?.addEventListener('abort', onAbort, { once: true });
        let timer: ReturnType<typeof setTimeout> | undefined;
        try {
            await Promise.race([
                sendAndConfirm(transaction as Parameters<typeof sendAndConfirm>[0], {
                    commitment: 'confirmed',
                    abortSignal: inner.signal,
                }),
                new Promise<never>((_, reject) => {
                    timer = setTimeout(() => {
                        inner.abort();
                        reject(
                            new OutcomeUnknownError(
                                signature,
                                'No confirmation within the time limit. The transaction may still land.',
                            ),
                        );
                    }, timeoutMs);
                }),
            ]);
        } finally {
            clearTimeout(timer);
            a.abortSignal?.removeEventListener('abort', onAbort);
        }
        return signature;
    }

    if (isTransactionSendingSigner(signer)) {
        // An abort before the call sends nothing. The call itself gets no abort signal: a sending
        // wallet cannot be stopped once asked, and Kit checks the signal again after the wallet
        // answered and would then throw its signature away.
        a.abortSignal?.throwIfAborted();
        const bytes = await walletCall(() => signAndSendTransactionMessageWithSigners(message), {
            sending: true,
        });
        const signature = getBase58Decoder().decode(bytes) as Signature;
        // The wallet has sent it: an abort that landed meanwhile cannot unsend it, so the signature
        // is reported and the outcome is unknown (R6: never dropped silently, never a blind
        // retry). Only the signing branch drops a transaction after an abort.
        a.onSigned?.({ signature, finalFeeLamports: null, sentByWallet: true });
        if (a.abortSignal?.aborted) throw new OutcomeUnknownError(signature, SENT_BY_WALLET_UNSEEN);
        // The wallet chose the blockhash it sent with; ours bounds it only loosely (it fetched its own
        // no earlier than we did, but possibly later), so running out of time is "unknown", not "expired".
        await waitForSignature(a.client, signature, {
            giveUpAfterHeight: lifetime.lastValidBlockHeight + MAX_BLOCKHASH_AGE,
            timeoutMs,
            ...(a.pollMs !== undefined ? { pollMs: a.pollMs } : {}),
            ...(a.abortSignal ? { abortSignal: a.abortSignal } : {}),
        });
        await checkLanded(a.client, signature, expected, a.lookupTables, a.maxComputeUnitPrice, a.pollMs);
        return signature;
    }
    throw new Error('the connected wallet cannot sign transactions');
}

/**
 * Polls signature status until `confirmed` or a failure. Past `giveUpAfterHeight` or `timeoutMs`
 * it ends in `OutcomeUnknownError`: the app does not know the sent transaction's own lifetime.
 */
export async function waitForSignature(
    client: AppClient,
    signature: Signature,
    opts: {
        readonly giveUpAfterHeight: bigint;
        readonly timeoutMs: number;
        readonly pollMs?: number;
        readonly abortSignal?: AbortSignal;
    },
): Promise<void> {
    const until = Date.now() + opts.timeoutMs;
    for (;;) {
        opts.abortSignal?.throwIfAborted();
        const { value } = await client.rpc.getSignatureStatuses([signature]).send();
        const s = value[0];
        if (s?.err) throw Object.assign(new Error('transaction failed'), { context: { transactionError: s.err } });
        if (s && (s.confirmationStatus === 'confirmed' || s.confirmationStatus === 'finalized')) return;
        const height = await client.rpc.getBlockHeight({ commitment: 'confirmed' }).send();
        if (height > opts.giveUpAfterHeight || Date.now() > until) {
            throw new OutcomeUnknownError(signature, SENT_BY_WALLET_UNSEEN);
        }
        await sleep(opts.pollMs ?? POLL_MS, opts.abortSignal);
    }
}

/**
 * The post hoc check of a sending-only wallet's transaction (FRONTEND.md §5.4, FE-11): reads it
 * back at `confirmed` and runs the intent check on what landed (any lifetime; the fee ceiling).
 */
export async function checkLanded(
    client: AppClient,
    signature: Signature,
    expected: Parameters<typeof assertSameIntent>[0],
    tables: AddressesByLookupTableAddress,
    maxComputeUnitPrice: bigint,
    pollMs = POLL_MS,
): Promise<IntentCheck> {
    let wire: string | null = null;
    for (let i = 0; i < LANDED_READ_TRIES && wire === null; i++) {
        const tx = await client.rpc
            .getTransaction(signature, {
                encoding: 'base64',
                maxSupportedTransactionVersion: 0,
                commitment: 'confirmed',
            })
            .send();
        const data = (tx as { transaction?: readonly [string, string] } | null)?.transaction;
        if (data) wire = data[0];
        else await sleep(pollMs);
    }
    if (wire === null) {
        throw new LandedIntegrityError(
            signature,
            'The transaction your wallet sent confirmed, but it could not be read back to compare with this review. Check it in the explorer.',
        );
    }
    const landed = getTransactionDecoder().decode(getBase64Encoder().encode(wire));
    try {
        return assertSameIntent(expected, landed.messageBytes, tables, {
            maxComputeUnitPrice,
            allowLifetimeChange: true,
        });
    } catch (e) {
        throw new LandedIntegrityError(
            signature,
            `The transaction your wallet sent differs from this review (${(e as Error).message}). It has landed: check your wallet and the explorer.`,
        );
    }
}
