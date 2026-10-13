/**
 * Error mapping (FRONTEND.md §5.9): a simulation or send failure → a `TxError` with a human
 * message. Sources: the RPC's `TransactionError` JSON (`{ InstructionError: [i, { Custom: n }] }`
 * and the transaction-level variants), the program logs (which program failed), Kit's
 * `SolanaError`s, and Wallet Standard rejections.
 */
import { walletCopy } from '../config/copyWallet.ts';
import { programError } from '../config/errors.ts';
import { SHORTWIN_PROGRAM, TOKEN_2022_PROGRAM, TOKEN_PROGRAM } from '../config/programs.ts';
import { localNetwork } from '../wallet/localNetwork.ts';
import { TOKEN_ERROR_CPI_GUARD_TRANSFER_BLOCKED, TOKEN_ERROR_NO_MEMO } from './tokenAccount.ts';
import type { TxError } from './types.ts';

/** A plan cannot be built (or rebuilt) from the chain as it is: the inputs no longer fit. */
export class PlanError extends Error {
    constructor(message: string) {
        super(message);
        this.name = 'PlanError';
    }
}

const TOKEN_ERRORS: Readonly<Record<number, string>> = {
    1: 'Not enough tokens in the source account.',
    4: 'The token account belongs to another owner.',
    17: 'The token account is frozen.',
    [TOKEN_ERROR_NO_MEMO]: 'The destination token account requires a memo on incoming transfers.',
    [TOKEN_ERROR_CPI_GUARD_TRANSFER_BLOCKED]:
        'CPI Guard is on for this token account, so short.win cannot move tokens from it. Turn it off for this deposit or use another account.',
};

/**
 * Anchor framework errors (below 6000) short.win raises in normal use, when short.win itself
 * failed. `AccountNotInitialized` (3012): an account the instruction names no longer exists.
 * Since KI-67 the claim that empties a lender position closes it (as a full withdrawal does), so a
 * plan built before such a close fails here: the claimant's own position closed by a claim from
 * another tab or the dashboard, or a lender the claim syncs first closed meanwhile (FE-30). A new
 * plan re-reads the chain (`plan.rebuild`): it leaves a closed sync target out, and a claim of a
 * closed position is refused with a plain message. `AccountOwnedByWrongProgram` (3007) is the same
 * close seen after someone sent lamports to the closed position's address (a system account there,
 * not a missing one; FE-30): mapped alike, but only when the account the error names is `lender` —
 * the instruction's `LenderPosition`, the claimant's own in `claim_sol` or a sync target's in
 * `sync_lender` — since another account owned by the wrong program is not fixed by a new plan.
 */
const ANCHOR_ERRORS: Readonly<
    Record<number, Pick<TxError, 'name' | 'message' | 'action'> & { readonly account?: string }>
> = {
    3012: {
        name: 'AccountNotInitialized',
        action: 'retry',
        message:
            'An account in this transaction no longer exists (for example a lender position closed by a claim or withdrawal). A new review re-reads the chain.',
    },
    3007: {
        name: 'AccountOwnedByWrongProgram',
        action: 'retry',
        account: 'lender',
        message:
            'A lender position in this transaction no longer exists (closed by a claim or withdrawal; its address has since received SOL). A new review re-reads the chain.',
    },
};

/** The account an Anchor account error names ("AnchorError caused by account: <name>."), if any. */
export function anchorErrorAccount(logs: readonly string[]): string | null {
    for (const line of logs) {
        const m = /AnchorError caused by account: (\w+)\./.exec(line);
        if (m?.[1]) return m[1];
    }
    return null;
}

const TX_LEVEL: Readonly<Record<string, Pick<TxError, 'kind' | 'message' | 'action'>>> = {
    InsufficientFundsForFee: { kind: 'funds', action: 'adjust', message: 'Not enough SOL to pay the transaction fee.' },
    InsufficientFundsForRent: {
        kind: 'funds',
        action: 'adjust',
        message: 'Not enough SOL to keep a new account rent-exempt.',
    },
    AccountNotFound: { kind: 'funds', action: 'adjust', message: 'Your wallet has no SOL on this cluster.' },
    BlockhashNotFound: {
        kind: 'expired',
        action: 'retry',
        message: 'The transaction expired before it landed. Try again.',
    },
    AlreadyProcessed: { kind: 'rpc', action: 'retry', message: 'This transaction was already processed.' },
    AccountInUse: { kind: 'rpc', action: 'retry', message: 'An account is busy. Try again.' },
    WouldExceedMaxBlockCostLimit: { kind: 'rpc', action: 'retry', message: 'The block is full. Try again.' },
    WouldExceedAccountDataBlockLimit: { kind: 'rpc', action: 'retry', message: 'The block is full. Try again.' },
    WouldExceedMaxAccountCostLimit: {
        kind: 'rpc',
        action: 'retry',
        message: 'A market account is at its per-block limit. Try again.',
    },
    ProgramAccountNotFound: {
        kind: 'rpc',
        action: 'report',
        message: 'A program in the transaction is not deployed here.',
    },
};

/**
 * The program that raised the error: the FIRST "Program <id> failed" line — the innermost failing
 * program; every caller up the CPI stack logs its own "failed" line after it.
 */
export function failingProgram(logs: readonly string[]): string | null {
    for (const line of logs) {
        const m = /^Program (\w{32,44}) failed/.exec(line);
        if (m?.[1]) return m[1];
    }
    return null;
}

/** Maps an RPC `TransactionError` (simulation `err`, or a confirmed transaction's `err`). */
export function mapTransactionError(err: unknown, logs: readonly string[] = []): TxError {
    if (typeof err === 'string') {
        const t = TX_LEVEL[err];
        if (t) return { ...t, logs };
        return { kind: 'unknown', action: 'report', message: `Transaction failed: ${err}.`, logs };
    }
    if (err && typeof err === 'object') {
        const o = err as Record<string, unknown>;
        const ie = o.InstructionError;
        if (Array.isArray(ie) && ie.length === 2) {
            const [index, detail] = ie as [number, unknown];
            const failed = failingProgram(logs);
            if (detail && typeof detail === 'object' && 'Custom' in detail) {
                const code = Number((detail as { Custom: number | bigint }).Custom);
                const token = failed === TOKEN_PROGRAM || failed === TOKEN_2022_PROGRAM;
                if (!token && code >= 6000) {
                    const info = programError(code);
                    if (info && (failed === null || failed === SHORTWIN_PROGRAM)) {
                        return {
                            kind: 'program',
                            code,
                            name: info.name,
                            message: info.message,
                            action: info.action,
                            instructionIndex: index,
                            logs,
                        };
                    }
                }
                const anchor = ANCHOR_ERRORS[code];
                if (
                    anchor &&
                    failed === SHORTWIN_PROGRAM &&
                    (anchor.account === undefined || anchorErrorAccount(logs) === anchor.account)
                ) {
                    const { account: _, ...mapped } = anchor;
                    return { kind: 'program', code, ...mapped, instructionIndex: index, logs };
                }
                const tokenMsg = TOKEN_ERRORS[code];
                if (token && tokenMsg) {
                    return { kind: 'token', code, message: tokenMsg, action: 'adjust', instructionIndex: index, logs };
                }
                return {
                    kind: 'unknown',
                    code,
                    message: `Instruction ${index} failed with error ${code}${failed ? ` in ${failed}` : ''}.`,
                    action: 'report',
                    instructionIndex: index,
                    logs,
                };
            }
            const name = typeof detail === 'string' ? detail : Object.keys((detail as object) ?? {})[0];
            if (name === 'InsufficientFunds') {
                return {
                    kind: 'funds',
                    message: 'Not enough SOL for this transaction.',
                    action: 'adjust',
                    instructionIndex: index,
                    logs,
                };
            }
            if (name === 'ComputationalBudgetExceeded' || name === 'ProgramFailedToComplete') {
                return {
                    kind: 'rpc',
                    message: 'The transaction ran out of compute. Try again.',
                    action: 'retry',
                    instructionIndex: index,
                    logs,
                };
            }
            return {
                kind: 'unknown',
                message: `Instruction ${index} failed: ${name ?? 'unknown error'}.`,
                action: 'report',
                instructionIndex: index,
                logs,
            };
        }
        const key = Object.keys(o)[0];
        if (key && TX_LEVEL[key]) return { ...(TX_LEVEL[key] as Pick<TxError, 'kind' | 'message' | 'action'>), logs };
    }
    return { kind: 'unknown', action: 'report', message: 'The transaction failed.', logs };
}

/** A wallet's own rejection: EIP-1193 / Wallet Standard 4001, the adapters' names, an abort. */
function isRejection(e: unknown): boolean {
    const err = e as { name?: string; message?: string; code?: unknown } | null;
    if (!err) return false;
    if (err.code === 4001 || err.name === 'WalletSignTransactionError' || err.name === 'WalletRejectedError')
        return true;
    if (err.name === 'AbortError') return true;
    return /reject|denied|declined|cancel/i.test(String(err.message ?? ''));
}

/** An error of the Mobile Wallet Adapter's protocol (`@solana-mobile/mobile-wallet-adapter-protocol`). */
interface MwaError {
    readonly name: 'SolanaMobileWalletAdapterError' | 'SolanaMobileWalletAdapterProtocolError';
    readonly code: unknown;
    readonly data?: unknown;
    readonly message?: string;
}

/**
 * The first Mobile Wallet Adapter error on a cause chain (at most 6 levels). The library wraps its
 * errors as `new Error(message, { cause })` and `send.ts` wraps that again; the package is lazy, so
 * the match is on `name` and `code`, never `instanceof`.
 */
function mwaCause(e: unknown): MwaError | null {
    let x: unknown = e;
    for (let depth = 0; depth < 6 && x && typeof x === 'object'; depth++) {
        const name = (x as { name?: unknown }).name;
        if (name === 'SolanaMobileWalletAdapterError' || name === 'SolanaMobileWalletAdapterProtocolError') {
            return x as MwaError;
        }
        x = (x as { cause?: unknown }).cause;
    }
    return null;
}

/** The transport failed: the session closed, timed out, or broke (the wallet may have acted). */
const MWA_TRANSPORT = new Set(['ERROR_SESSION_CLOSED', 'ERROR_SESSION_TIMEOUT', 'ERROR_ILLEGAL_TRANSPORT_STATE']);

/**
 * The Mobile Wallet Adapter's errors at Sign (FRONTEND.md §5.9; mobile.md §1.8), found on the
 * wallet call's cause chain. `null` when none is there (other wallets map as before).
 */
function mapMwa(wallet: { readonly cause?: unknown; readonly sending?: boolean }): TxError | null {
    const m = mwaCause(wallet.cause);
    if (!m) return null;
    const generic = (): TxError => {
        const inner = String((wallet.cause as { message?: string } | null)?.message ?? m.message ?? m.code);
        return { kind: 'wallet', action: 'retry', message: `Your wallet could not sign: ${inner.slice(0, 240)}` };
    };
    if (m.name === 'SolanaMobileWalletAdapterProtocolError') {
        // −3 (ERROR_NOT_SIGNED): declined in the wallet. −1 at Sign comes only from the
        // reauthorization, which never reaches here (the library hangs; R-S2 ends the sign), and the
        // others (−2, −4, −5, −100) are the wallet's own failures.
        return m.code === -3
            ? { kind: 'wallet-rejected', action: 'retry', message: 'You rejected the request in your wallet.' }
            : generic();
    }
    const code = String(m.code);
    // The sending path (unused for MWA by the signer policy): the wallet may already have sent it.
    if (wallet.sending && MWA_TRANSPORT.has(code)) {
        return {
            kind: 'unknown-outcome',
            action: 'report',
            message:
                'Your wallet sent this transaction itself and it has not been seen on chain. It may still land. Check the explorer before trying again: a new attempt would be a second transaction.',
        };
    }
    switch (code) {
        case 'ERROR_WALLET_NOT_FOUND':
            // The launch did not leave the page: no wallet app, or Android blocked it. Nothing reached
            // the wallet (no socket exists yet); the review stays with the retry notice.
            return { kind: 'wallet-launch', action: 'retry', message: walletCopy('wallet.mwa.retry') };
        case 'ERROR_LOOPBACK_ACCESS_BLOCKED':
            return { kind: 'local-network', action: 'retry', message: walletCopy('wallet.mwa.localNetwork') };
        case 'ERROR_SESSION_TIMEOUT':
            // No socket within 30 s: Chrome's permission may have blocked it.
            return localNetwork.get() !== 'ready'
                ? { kind: 'local-network', action: 'retry', message: walletCopy('wallet.mwa.localNetwork') }
                : generic();
        case 'ERROR_ASSOCIATION_CANCELLED':
            // With an event: the user closed one of the library's modals (Escape in the LNA race).
            // Without: the library's association timer, which is not a safe launch retry.
            return (m.data as { event?: unknown } | undefined)?.event
                ? { kind: 'wallet-rejected', action: 'retry', message: 'You rejected the request in your wallet.' }
                : generic();
        default:
            return generic();
    }
}

/** Kit's RPC transport error (`SOLANA_ERROR__RPC__TRANSPORT_HTTP_ERROR`) carries the HTTP status. */
function httpStatus(e: unknown): number | null {
    const ctx = (e as { context?: Record<string, unknown> } | null)?.context;
    const status = ctx?.statusCode ?? ctx?.status;
    return typeof status === 'number' ? status : null;
}

/**
 * Maps anything thrown while signing / sending (wallet rejections, RPC transport, Kit errors).
 * Only an error thrown by the wallet call itself (`send.ts` wraps it in `WalletCallError`, or a
 * wallet-specific code / name) can read as "you rejected the request"; an RPC 403 "access denied",
 * a rate limit's "request denied" or a load balancer's "cancelled" is an RPC failure (M9-UX-7).
 */
export function mapThrown(e: unknown): TxError {
    const err = e as {
        name?: string;
        message?: string;
        code?: unknown;
        context?: Record<string, unknown>;
        cause?: unknown;
    };
    const msg = String(err?.message ?? e);
    if (err?.name === 'LandedIntegrityError') return { kind: 'integrity', action: 'report', message: msg };
    if (err?.name === 'IntegrityError') return { kind: 'integrity', action: 'report', message: msg };
    if (err?.name === 'OutcomeUnknownError') {
        return {
            kind: 'unknown-outcome',
            action: 'report',
            message: `${msg} Check the explorer before trying again: a new attempt would be a second transaction.`,
        };
    }
    if (err?.name === 'PlanError' || err?.name === 'LendRefusal' || err?.name === 'VenueError') {
        return { kind: 'plan', action: 'adjust', message: msg };
    }
    if (err?.name === 'WalletCallError') {
        // The Mobile Wallet Adapter first: its messages ("…permission denied", "…cancelled by user")
        // must not read as a rejection by the generic test below.
        const mwa = mapMwa(err as { cause?: unknown; sending?: boolean });
        if (mwa) return mwa;
        const cause = err.cause;
        if (isRejection(cause)) {
            return { kind: 'wallet-rejected', action: 'retry', message: 'You rejected the request in your wallet.' };
        }
        const inner = String((cause as { message?: string } | null)?.message ?? cause);
        return { kind: 'wallet', action: 'retry', message: `Your wallet could not sign: ${inner.slice(0, 240)}` };
    }
    if (err?.code === 4001 || err?.name === 'WalletSignTransactionError' || err?.name === 'WalletRejectedError') {
        return { kind: 'wallet-rejected', action: 'retry', message: 'You rejected the request in your wallet.' };
    }
    if (err?.name === 'AbortError') return { kind: 'rpc', action: 'retry', message: 'The request was cancelled.' };
    // Kit: a preflight failure carries the TransactionError and logs in its context / cause.
    const ctx = err?.context;
    if (ctx && typeof ctx === 'object') {
        const cause = (e as { cause?: { context?: Record<string, unknown> } }).cause?.context;
        const logs = (ctx.logs ?? cause?.logs) as string[] | undefined;
        const txErr = ctx.transactionError ?? cause?.transactionError;
        if (txErr !== undefined) return mapTransactionError(txErr, logs ?? []);
        if (typeof ctx.code === 'number' && 'index' in ctx) {
            return mapTransactionError({ InstructionError: [ctx.index, { Custom: ctx.code }] }, logs ?? []);
        }
    }
    const status = httpStatus(e);
    if (status !== null) {
        return {
            kind: 'rpc',
            action: 'retry',
            message: `The RPC node refused the request (HTTP ${status}). Try again.`,
        };
    }
    if (/block height exceeded|blockhash/i.test(msg)) {
        return { kind: 'expired', action: 'retry', message: 'The transaction expired before it landed. Try again.' };
    }
    if (/fetch|network|Failed to fetch|ECONN/i.test(msg)) {
        return { kind: 'rpc', action: 'retry', message: 'The RPC node is unreachable. Try again.' };
    }
    if (/reject|denied|declined|cancel|forbidden|rate.?limit|too many requests/i.test(msg)) {
        return {
            kind: 'rpc',
            action: 'retry',
            message: `The RPC node refused or cancelled the request: ${msg.slice(0, 200)}`,
        };
    }
    return { kind: 'unknown', action: 'report', message: msg.slice(0, 300) };
}
