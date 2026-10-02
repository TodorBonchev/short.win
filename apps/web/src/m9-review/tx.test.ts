/**
 * Regressions of the M9 review — the transaction pipeline (`src/tx`): sending-only wallets are
 * checked after landing and never end as "expired" (M9-SEC-1); a wallet-changed blockhash is
 * validated and bounded, confirmation has a hard timeout, a wallet-raised fee is reported
 * (M9-SEC-4); the top-level allowlist is per instruction (M9-SEC-7); only the wallet's own errors
 * read as a rejection (M9-UX-7).
 */
import { getClaimSolInstructionDataEncoder, getDepositInstructionDataEncoder } from '@shortwin/client';
import {
    AccountRole,
    address,
    type Blockhash,
    compileTransaction,
    createNoopSigner,
    getBase64Decoder,
    getTransactionEncoder,
    type Instruction,
    type Transaction,
} from '@solana/kit';
import { getSetAuthorityInstruction, getTransferCheckedInstruction } from '@solana-program/token';
import {
    getCreateAssociatedTokenIdempotentInstruction,
    getDisableCpiGuardInstruction,
    getEnableCpiGuardInstruction,
} from '@solana-program/token-2022';
import { describe, expect, it } from 'vitest';
import {
    COMPUTE_BUDGET_PROGRAM,
    PUMP_AMM_PROGRAM,
    PUMP_PROGRAM,
    SHORTWIN_PROGRAM,
    SYSTEM_PROGRAM,
    TOKEN_2022_PROGRAM,
    TOP_LEVEL_ALLOWLIST,
} from '../config/programs.ts';
import { ALLOWED_PROGRAMS } from '../tx/allowlist.ts';
import { describeInstruction } from '../tx/describe.ts';
import { mapThrown } from '../tx/errors.ts';
import { assertAllowedInstructions, assertSameIntent, IntegrityError } from '../tx/integrity.ts';
import { buildMessage } from '../tx/message.ts';
import {
    confirmationLifetime,
    MAX_BLOCKHASH_AGE,
    type SignedInfo,
    signSendConfirm,
    WalletCallError,
} from '../tx/send.ts';
import type { TxPlan } from '../tx/types.ts';
import { retryMode, type TxFlowState } from '../tx/useTxFlow.ts';
import { IX_CLAIM_CASHBACK, IX_INIT_USER_VOLUME_ACCUMULATOR, PUMP_IX_EXTEND_ACCOUNT } from '../venues/constants.ts';

const WALLET = address('9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin');
const ATTACKER = address('BPFLoaderUpgradeab1e11111111111111111111111');
const OTHER = address('So11111111111111111111111111111111111111112');
const BH_A = 'EkSnNWid2cvwEVnVx9aBqawnmiCNiDgp3gUdkDPTKN1N' as Blockhash;
const BH_B = '4uQeVj5tqViQh7yWWGStvkEG1Zmhx6uasJtWCJziofM' as Blockhash;
const CLAIM = Uint8Array.from(getClaimSolInstructionDataEncoder().encode({}));

function shortwinIx(): Instruction {
    return {
        programAddress: SHORTWIN_PROGRAM,
        accounts: [
            { address: WALLET, role: AccountRole.WRITABLE_SIGNER },
            { address: OTHER, role: AccountRole.WRITABLE },
        ],
        data: CLAIM,
    };
}

const drain: Instruction = {
    programAddress: SYSTEM_PROGRAM,
    accounts: [
        { address: WALLET, role: AccountRole.WRITABLE_SIGNER },
        { address: ATTACKER, role: AccountRole.WRITABLE },
    ],
    // SystemInstruction::Transfer (2) of u64::MAX lamports
    data: new Uint8Array([2, 0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255]),
};

const plan: TxPlan = {
    kind: 'claim_sol',
    title: 'Claim',
    instructions: [shortwinIx()],
    watch: [],
    disclosures: [],
    invalidate: [],
};

const payer = createNoopSigner(WALLET);
function messageBytes(blockhash: Blockhash, extra: Instruction[] = [], price = 0n) {
    return compileTransaction(
        buildMessage({
            instructions: [shortwinIx(), ...extra],
            feePayer: payer,
            lifetime: { blockhash, lastValidBlockHeight: 100n },
            computeUnitLimit: 200_000,
            computeUnitPrice: price,
            lookupTables: {},
            version: 0,
        }),
    ).messageBytes;
}

/** The wire transaction (base64) of `messageBytes` with a dummy signature, as `getTransaction` returns it. */
function wire(bytes: ReturnType<typeof messageBytes>): string {
    const tx = { messageBytes: bytes, signatures: { [WALLET]: new Uint8Array(64).fill(7) } } as unknown as Transaction;
    return getBase64Decoder().decode(getTransactionEncoder().encode(tx));
}

function fakeClient(o: { landed: 'ok' | 'never'; height: bigint; transaction?: string | null }) {
    const calls: string[] = [];
    const rpc = {
        getLatestBlockhash: () => ({
            send: async () => ({ value: { blockhash: BH_A, lastValidBlockHeight: 100n } }),
        }),
        getSignatureStatuses: () => ({
            send: async () => {
                calls.push('getSignatureStatuses');
                return { value: [o.landed === 'ok' ? { confirmationStatus: 'confirmed', err: null } : null] };
            },
        }),
        getBlockHeight: () => ({
            send: async () => {
                calls.push('getBlockHeight');
                return o.height;
            },
        }),
        getTransaction: () => ({
            send: async () => {
                calls.push('getTransaction');
                return o.transaction === null || o.transaction === undefined
                    ? null
                    : { transaction: [o.transaction, 'base64'] };
            },
        }),
    };
    return { client: { rpc, rpcSubscriptions: {} } as never, calls };
}

/** A wallet with only `solana:signAndSendTransaction` (Kit's `TransactionSendingSigner`). */
function sendingOnly(sent: Transaction[] = []) {
    return {
        address: WALLET,
        async signAndSendTransactions(txs: readonly Transaction[]) {
            sent.push(...txs);
            return txs.map(() => new Uint8Array(64).fill(7));
        },
    };
}

const args = (client: never, signer: unknown, onSigned?: (i: SignedInfo) => void) => ({
    client,
    plan,
    feePayer: signer as never,
    lookupTables: {},
    version: 0 as const,
    computeUnitLimit: 200_000,
    priorityFeeMicroLamports: 0n,
    maxComputeUnitPrice: 1_000_000n,
    confirmTimeoutMs: 5_000,
    pollMs: 1,
    ...(onSigned ? { onSigned } : {}),
});

describe('M9-SEC-1: a sending-only wallet is checked after landing, and never ends as "expired"', () => {
    it('reads the landed transaction back and accepts it when it is what was reviewed', async () => {
        const { client, calls } = fakeClient({ landed: 'ok', height: 50n, transaction: wire(messageBytes(BH_A)) });
        let info: SignedInfo | null = null;
        await signSendConfirm(
            args(client, sendingOnly(), (i) => {
                info = i;
            }),
        );
        expect(calls).toContain('getTransaction');
        expect((info as SignedInfo | null)?.sentByWallet).toBe(true);
    });

    it('reports an integrity incident when the wallet landed something else (an added drain)', async () => {
        const tampered = wire(messageBytes(BH_B, [drain]));
        const { client } = fakeClient({ landed: 'ok', height: 50n, transaction: tampered });
        const e = await signSendConfirm(args(client, sendingOnly())).catch((x: unknown) => x);
        expect((e as Error).name).toBe('LandedIntegrityError');
        expect((e as Error).message).toMatch(/differs from this review/);
        const mapped = mapThrown(e);
        expect(mapped.kind).toBe('integrity');
        // It has landed: the review offers no Retry (a new attempt would be a second transaction).
        const state = { stage: 'failed', plan, error: mapped, signature: 'sig' } as unknown as TxFlowState;
        expect(retryMode(state)).toBe('none');
    });

    it('a landed transaction that cannot be read back is reported, not called confirmed', async () => {
        const { client } = fakeClient({ landed: 'ok', height: 50n, transaction: null });
        const e = await signSendConfirm(args(client, sendingOnly())).catch((x: unknown) => x);
        expect((e as Error).name).toBe('LandedIntegrityError');
    });

    it('ends in "outcome unknown" — not expired / Retry — past the app’s own blockhash bound', async () => {
        // Height 101 > the app's lastValidBlockHeight (100) used to read as "expired"; the wallet chose
        // its own blockhash, so the app waits up to 100 + 151 and then says it does not know.
        const { client: early } = fakeClient({ landed: 'never', height: 101n });
        const timeout = signSendConfirm({ ...args(early, sendingOnly()), confirmTimeoutMs: 0 }).catch((x) => x);
        const e1 = await timeout;
        expect((e1 as Error).name).toBe('OutcomeUnknownError');
        const { client } = fakeClient({ landed: 'never', height: 100n + MAX_BLOCKHASH_AGE + 1n });
        const e = await signSendConfirm(args(client, sendingOnly())).catch((x: unknown) => x);
        expect((e as Error).name).toBe('OutcomeUnknownError');
        const mapped = mapThrown(e);
        expect(mapped.kind).toBe('unknown-outcome');
        expect(mapped.kind).not.toBe('expired');
        expect(mapped.message).toMatch(/explorer/);
        const state = { stage: 'failed', plan, error: mapped, signature: 'sig' } as unknown as TxFlowState;
        expect(retryMode(state)).toBe('none');
    });
});

describe('M9-SEC-4: a wallet-changed blockhash is validated and bounded; a raised fee is reported', () => {
    it('assertSameIntent refuses a changed lifetime unless the caller validates it', () => {
        expect(() => assertSameIntent(messageBytes(BH_A), messageBytes(BH_B), {})).toThrow(/blockhash/);
        const check = assertSameIntent(messageBytes(BH_A), messageBytes(BH_B), {}, { allowLifetimeChange: true });
        expect(check.lifetimeChanged).toBe(true);
        expect(check.lifetimeToken).toBe(BH_B);
    });

    it('bounds the confirmation of a changed blockhash by the current height + 151, after isBlockhashValid', async () => {
        const rpc = (valid: boolean) => ({
            isBlockhashValid: () => ({ send: async () => ({ value: valid }) }),
            getBlockHeight: () => ({ send: async () => 1_000n }),
        });
        const ours = { blockhash: BH_A, lastValidBlockHeight: 100n };
        expect(
            await confirmationLifetime(rpc(true) as never, { lifetimeChanged: false, lifetimeToken: BH_A }, ours),
        ).toBe(ours);
        const bounded = await confirmationLifetime(
            rpc(true) as never,
            { lifetimeChanged: true, lifetimeToken: BH_B },
            ours,
        );
        expect(bounded).toEqual({ blockhash: BH_B, lastValidBlockHeight: 1_000n + MAX_BLOCKHASH_AGE });
        expect(bounded.lastValidBlockHeight).not.toBe(0xffffffffffffffffn);
        await expect(
            confirmationLifetime(rpc(false) as never, { lifetimeChanged: true, lifetimeToken: BH_B }, ours),
        ).rejects.toThrow(IntegrityError);
    });

    it('a confirmation that never comes ends, dismissibly, as "outcome unknown"; a raised fee is shown', async () => {
        // A modifying wallet that raises the priority fee (FE-2) and keeps the blockhash.
        const signer = {
            address: WALLET,
            async modifyAndSignTransactions(txs: readonly Transaction[]) {
                return txs.map(() =>
                    Object.freeze({
                        messageBytes: messageBytes(BH_A, [], 5_000n),
                        signatures: { [WALLET]: new Uint8Array(64).fill(3) },
                        lifetimeConstraint: { blockhash: BH_A, lastValidBlockHeight: 100n },
                    }),
                );
            },
        };
        // RPC and subscriptions that accept the send and then never answer.
        const pending = () => ({ send: () => new Promise(() => {}) });
        const silent = {
            subscribe: async () => ({
                [Symbol.asyncIterator]: () => ({ next: () => new Promise(() => {}) }),
            }),
        };
        const rpc = new Proxy(
            {
                getLatestBlockhash: () => ({
                    send: async () => ({ value: { blockhash: BH_A, lastValidBlockHeight: 100n } }),
                }),
                sendTransaction: () => ({ send: async () => 'sig' }),
                getSignatureStatuses: () => ({ send: async () => ({ value: [null] }) }),
                getEpochInfo: () => ({ send: async () => ({ absoluteSlot: 1n, blockHeight: 1n }) }),
            } as Record<string, unknown>,
            { get: (t, k: string) => t[k] ?? pending },
        );
        const rpcSubscriptions = new Proxy({}, { get: () => () => silent });
        let info: SignedInfo | null = null;
        const e = await signSendConfirm({
            ...args({ rpc, rpcSubscriptions } as never, signer, (i) => {
                info = i;
            }),
            confirmTimeoutMs: 50,
        }).catch((x: unknown) => x);
        expect((e as Error).name).toBe('OutcomeUnknownError');
        expect(mapThrown(e).kind).toBe('unknown-outcome');
        // FE-2: the fee as the wallet signed it is reported (5,000 + ⌈200,000 · 5,000 / 10⁶⌉).
        expect((info as SignedInfo | null)?.finalFeeLamports).toBe(6_000n);
    });
});

describe('M9-SEC-7: the top-level allowlist is per (program, instruction)', () => {
    const owner = createNoopSigner(WALLET);
    it('refuses System, SPL Token, a pump buy and compute-budget instructions from a plan', () => {
        const setAuthority = getSetAuthorityInstruction({
            owned: OTHER,
            owner,
            authorityType: 2, // AccountOwner
            newAuthority: ATTACKER,
        });
        const transfer = getTransferCheckedInstruction({
            source: OTHER,
            mint: OTHER,
            destination: ATTACKER,
            authority: owner,
            amount: 1n,
            decimals: 6,
        });
        const pumpBuy: Instruction = { programAddress: PUMP_PROGRAM, accounts: [], data: new Uint8Array(24) };
        const budget: Instruction = { programAddress: COMPUTE_BUDGET_PROGRAM, data: new Uint8Array([3, 0, 0, 0, 0]) };
        const t22Transfer: Instruction = { programAddress: TOKEN_2022_PROGRAM, data: new Uint8Array([12, 1]) };
        for (const ix of [drain, setAuthority, transfer, pumpBuy, budget, t22Transfer]) {
            expect(() => assertAllowedInstructions([shortwinIx(), ix]), ix.programAddress).toThrow(IntegrityError);
            expect(describeInstruction(ix).allowed).toBe(false);
        }
    });

    it('lets exactly the instructions the plans build through', () => {
        const deposit: Instruction = {
            programAddress: SHORTWIN_PROGRAM,
            data: Uint8Array.from(getDepositInstructionDataEncoder().encode({ amount: 1n, minShares: 0n })),
        };
        const allowed: Instruction[] = [
            deposit,
            getCreateAssociatedTokenIdempotentInstruction({ payer: owner, ata: OTHER, owner: WALLET, mint: OTHER }),
            getDisableCpiGuardInstruction({ token: OTHER, owner }),
            getEnableCpiGuardInstruction({ token: OTHER, owner }),
            { programAddress: PUMP_PROGRAM, data: Uint8Array.from(PUMP_IX_EXTEND_ACCOUNT) },
            { programAddress: PUMP_PROGRAM, data: IX_CLAIM_CASHBACK },
            { programAddress: PUMP_AMM_PROGRAM, data: IX_INIT_USER_VOLUME_ACCUMULATOR },
            { programAddress: PUMP_AMM_PROGRAM, data: IX_CLAIM_CASHBACK },
        ];
        expect(() => assertAllowedInstructions(allowed)).not.toThrow();
        // `extend_account` is allowed on pump only; `init_user_volume_accumulator` on PumpSwap only.
        expect(() =>
            assertAllowedInstructions([
                { programAddress: PUMP_AMM_PROGRAM, data: Uint8Array.from(PUMP_IX_EXTEND_ACCOUNT) },
            ]),
        ).toThrow(IntegrityError);
        expect(new Set(TOP_LEVEL_ALLOWLIST)).toEqual(new Set(ALLOWED_PROGRAMS));
        expect(TOP_LEVEL_ALLOWLIST).not.toContain(SYSTEM_PROGRAM);
        expect(TOP_LEVEL_ALLOWLIST).not.toContain(COMPUTE_BUDGET_PROGRAM);
    });
});

describe('M9-UX-7: only the wallet’s own errors read as "You rejected the request"', () => {
    it.each([
        '403 Forbidden: access denied for this API key',
        'Request was cancelled by the RPC load balancer',
        'rate limit: request denied',
    ])('an RPC failure "%s" is kind rpc', (msg) => {
        const m = mapThrown(new Error(msg));
        expect(m.kind).toBe('rpc');
        expect(m.message).not.toMatch(/You rejected/);
    });

    it('an HTTP transport error is kind rpc with its status', () => {
        const e = Object.assign(new Error('HTTP error (403): Forbidden'), { context: { statusCode: 403 } });
        expect(mapThrown(e)).toMatchObject({ kind: 'rpc', action: 'retry' });
        expect(mapThrown(e).message).toMatch(/403/);
    });

    it('a rejection thrown by the wallet call is wallet-rejected; another wallet failure is kind wallet', () => {
        expect(mapThrown(new WalletCallError(Object.assign(new Error('nope'), { code: 4001 }))).kind).toBe(
            'wallet-rejected',
        );
        expect(mapThrown(new WalletCallError(new Error('User rejected the request.'))).kind).toBe('wallet-rejected');
        const broken = mapThrown(new WalletCallError(new Error('internal wallet error')));
        expect(broken.kind).toBe('wallet');
        expect(broken.message).toMatch(/Your wallet could not sign/);
    });

    it('send.ts wraps the wallet call, so the e2e wallet’s 4001 still reads as a rejection', async () => {
        const rejecting = {
            address: WALLET,
            async modifyAndSignTransactions() {
                throw Object.assign(new Error('User rejected the request.'), { code: 4001 });
            },
        };
        const { client } = fakeClient({ landed: 'ok', height: 1n });
        const e = await signSendConfirm(args(client, rejecting)).catch((x: unknown) => x);
        expect((e as Error).name).toBe('WalletCallError');
        expect(mapThrown(e).kind).toBe('wallet-rejected');
    });
});
