/**
 * A TEST-ONLY Mobile Wallet Adapter wallet, at the protocol level, in Node (FRONTEND.md §10.2;
 * mobile.md §5.2). The page's library (`@solana-mobile/wallet-standard-mobile` 0.7.0 over
 * `@solana-mobile/mobile-wallet-adapter-protocol` 3.0.0) runs unchanged; its association socket
 * `ws://localhost:<port>/solana-wallet` is routed here (`page.routeWebSocket`), and this wallet
 * answers it as a wallet app would, with `node:crypto` only:
 *
 * 1. HELLO_REQ: Qd (65 bytes) ‖ ECDSA P-256 (IEEE-P1363) over Qd. The association public key comes
 *    from the intent the page launched (`e2e/wallet/mwa-hooks.js` records it, with its port); the
 *    signature is verified (it tests the page's side) and the port must be the socket's.
 * 2. The session key: ECDH P-256 with a fresh key, then HKDF-SHA-256 (salt: the association public
 *    key, raw; no info) to an AES-128-GCM key.
 * 3. HELLO_RSP: Qw (65 bytes) ‖ the session properties `{"v":"v1"}` encrypted with sequence number 1.
 *    Every encrypted message is a 4-byte big-endian sequence number, a 12-byte IV, the ciphertext and
 *    its tag, the sequence number being the AAD; this wallet's sequence numbers run 1, 2, 3, …
 * 4. JSON-RPC: `get_capabilities`, `authorize` (fresh, or with `auth_token`: the reauthorization
 *    that precedes every sign), `sign_transactions` (Kit's `partiallySignTransaction` with the
 *    configured key), `sign_and_send_transactions` (signs and sends; never called under the app's
 *    signer policy, which the specs assert through `calls`), `deauthorize`.
 *
 * One-shot behaviours (`next(…)`), each on the call it names:
 * - on `sign_transactions`: `decline` (error −3, ERROR_NOT_SIGNED), `tamper` (the first data byte of
 *   the last non-compute-budget instruction XOR 1, re-encoded, then signed), `close` (the socket
 *   closed, code 1011, no reply), `silent` (never answered; the socket stays open);
 * - on `authorize` with `auth_token`: `reauthDecline` (error −1, ERROR_AUTHORIZATION_FAILED),
 *   `reauthClose` (closed, 1011), `reauthSwitch` (answered with another account, a fresh key with no
 *   funds; the next `sign_transactions` is then declined with −2);
 * - on `authorize` without `auth_token`: `silentConnect` (never answered).
 * Variants (constructor): `signFeature: false` (no `solana:signTransactions` in the capabilities),
 * `accountFeatures` (per-account `features` on every account `authorize` returns).
 *
 * The transport is anything shaped like Playwright's `WebSocketRoute` (`MwaTransport`), so the same
 * wallet also answers the real library in a unit test (`fake-mwa.test.ts`).
 */
import {
    createCipheriv,
    createDecipheriv,
    createECDH,
    createPublicKey,
    generateKeyPairSync,
    hkdfSync,
    randomBytes,
    verify as verifySignature,
} from 'node:crypto';
import {
    createKeyPairSignerFromBytes,
    getBase58Decoder,
    getBase64EncodedWireTransaction,
    getCompiledTransactionMessageDecoder,
    getCompiledTransactionMessageEncoder,
    getTransactionDecoder,
    getTransactionEncoder,
    type KeyPairSigner,
    partiallySignTransaction,
    type Transaction,
} from '@solana/kit';

/** The association socket the protocol opens (mwap-300 `startScenario`). */
export const MWA_SOCKET_URL = /^ws:\/\/localhost:\d+\/solana-wallet$/;

const COMPUTE_BUDGET = 'ComputeBudget111111111111111111111111111111';
const SESSION_PROPERTIES = JSON.stringify({ v: 'v1' });
/** The token this wallet issues; a reauthorization must present it. */
export const FAKE_MWA_AUTH_TOKEN = 'e2e';
/** Between two messages to the page: the page reads each one asynchronously (`Blob.arrayBuffer()`). */
const SEND_GAP_MS = 25;

/** What the page recorded when the protocol built the intent (`mwa-hooks.js`). */
export interface MwaLaunch {
    readonly association: string;
    readonly port: number | null;
    /** `navigator.userActivation.isActive` at that moment (`null` without the API). */
    readonly active: boolean | null;
    /** `performance.now()` in the page. */
    readonly at: number;
}

/** `window.__mwaE2E`, the page's side (`mwa-hooks.js`). */
export interface MwaHooks {
    readonly launches: MwaLaunch[];
    /** `'not-found'`: no wallet app comes to the front (the protocol's ERROR_WALLET_NOT_FOUND). */
    launch: 'ok' | 'not-found';
    /** The browser comes back to the front (`focus` on `window`). */
    returnToPage(): void;
}

declare global {
    interface Window {
        __mwaE2E?: MwaHooks;
    }
}

/** The part of Playwright's `WebSocketRoute` a session uses. */
export interface MwaTransport {
    url(): string;
    send(message: string | Buffer): void;
    close(options?: { code?: number; reason?: string }): unknown;
    onMessage(handler: (message: string | Buffer) => unknown): void;
    onClose(handler: (code: number | undefined, reason: string | undefined) => unknown): void;
}

export type MwaBehaviour =
    | 'decline'
    | 'tamper'
    | 'close'
    | 'silent'
    | 'reauthDecline'
    | 'reauthClose'
    | 'reauthSwitch'
    | 'silentConnect';

const ON_SIGN: readonly MwaBehaviour[] = ['decline', 'tamper', 'close', 'silent'];
const ON_REAUTH: readonly MwaBehaviour[] = ['reauthDecline', 'reauthClose', 'reauthSwitch'];
const ON_CONNECT: readonly MwaBehaviour[] = ['silentConnect'];

export interface FakeMwaOptions {
    /** The 64-byte keypair (`seed ‖ public key`) the wallet's account signs with. */
    readonly secretKey: Uint8Array;
    /** The associations the page recorded, latest last (`window.__mwaE2E.launches`). */
    readonly launches: () => Promise<readonly MwaLaunch[]>;
    /** Sends a signed wire transaction (base64) and returns its base58 signature (`sign_and_send`). */
    readonly sendTransaction?: (wireBase64: string) => Promise<string>;
    /** `false`: the capabilities leave out `solana:signTransactions` (a sending-only wallet). */
    readonly signFeature?: boolean;
    /** Per-account `features` on every account `authorize` returns. */
    readonly accountFeatures?: readonly string[];
    /** The account's label. */
    readonly label?: string;
}

/** One JSON-RPC request the wallet received. */
export interface MwaCall {
    readonly session: number;
    readonly id: number;
    readonly method: string;
    readonly params: Record<string, unknown>;
    /** Whether the wallet answered it (a reply or an error), or closed the socket instead. */
    outcome: 'pending' | 'result' | 'error' | 'closed';
    /**
     * `Date.now()` as the wallet answered it (queued its reply or error, or closed the socket);
     * `null` while pending. The page's `performance.timeOrigin + performance.now()` is on the same
     * clock, so a spec can time the page's reaction to the answer.
     */
    answeredAt: number | null;
}

/** One association socket. */
export interface MwaSession {
    readonly id: number;
    readonly port: number;
    established: boolean;
    closedByPage: boolean;
    closedByWallet: boolean;
}

interface JsonRpcRequest {
    readonly id: number;
    readonly jsonrpc: string;
    readonly method: string;
    readonly params?: Record<string, unknown>;
}

const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

/** The protocol's URL-safe base64 (`/`→`_`, `+`→`-`, `=`→`.`) back to bytes. */
export function decodeAssociation(value: string): Buffer {
    const b64 = value.replace(/[_\-.]/g, (c) => ({ _: '/', '-': '+', '.': '=' })[c] ?? c);
    return Buffer.from(b64, 'base64');
}

/** A raw uncompressed P-256 public key (65 bytes) as a Node key. */
function p256PublicKey(raw: Buffer) {
    if (raw.length !== 65 || raw[0] !== 4) throw new Error(`not an uncompressed P-256 key (${raw.length} bytes)`);
    return createPublicKey({
        key: {
            kty: 'EC',
            crv: 'P-256',
            x: raw.subarray(1, 33).toString('base64url'),
            y: raw.subarray(33, 65).toString('base64url'),
        },
        format: 'jwk',
    });
}

function sequenceBytes(n: number): Buffer {
    const b = Buffer.alloc(4);
    b.writeUInt32BE(n, 0);
    return b;
}

/** seq ‖ IV ‖ AES-128-GCM(plaintext) ‖ tag, the AAD being seq (mwap-300 `encryptMessage`). */
export function encryptMwa(key: Buffer, sequence: number, plaintext: string): Buffer {
    const seq = sequenceBytes(sequence);
    const iv = randomBytes(12);
    const cipher = createCipheriv('aes-128-gcm', key, iv);
    cipher.setAAD(seq);
    const body = Buffer.concat([cipher.update(plaintext, 'utf8'), cipher.final()]);
    return Buffer.concat([seq, iv, body, cipher.getAuthTag()]);
}

/** The inverse of {@link encryptMwa}: the sequence number and the plaintext. */
export function decryptMwa(key: Buffer, message: Buffer): { sequence: number; plaintext: string } {
    if (message.length < 4 + 12 + 16) throw new Error(`encrypted message too short (${message.length} bytes)`);
    const seq = message.subarray(0, 4);
    const iv = message.subarray(4, 16);
    const rest = message.subarray(16);
    const decipher = createDecipheriv('aes-128-gcm', key, iv);
    decipher.setAAD(seq);
    decipher.setAuthTag(rest.subarray(rest.length - 16));
    const text = Buffer.concat([decipher.update(rest.subarray(0, rest.length - 16)), decipher.final()]);
    return { sequence: seq.readUInt32BE(0), plaintext: text.toString('utf8') };
}

/** A fresh Ed25519 public key (32 bytes) that holds nothing: `reauthSwitch`'s other account. */
function freshPublicKey(): Buffer {
    const x = generateKeyPairSync('ed25519').publicKey.export({ format: 'jwk' }).x;
    if (!x) throw new Error('ed25519 JWK without x');
    return Buffer.from(x, 'base64url');
}

class WalletError extends Error {
    readonly code: number;
    constructor(code: number, message: string) {
        super(message);
        this.code = code;
    }
}

export class FakeMwa {
    /** Every JSON-RPC request received, in order. */
    readonly calls: MwaCall[] = [];
    /** Every association socket, in order. */
    readonly sessions: MwaSession[] = [];
    /** Every transaction this wallet signed for the page: its signature (base58), and whether it was tampered. */
    readonly signed: { readonly signature: string; readonly tampered: boolean }[] = [];
    /** What went wrong inside the wallet (a bad handshake, an undecryptable message): must stay empty. */
    readonly errors: string[] = [];

    private readonly opts: FakeMwaOptions;
    private readonly queued: MwaBehaviour[] = [];
    private signer: Promise<KeyPairSigner> | null = null;
    /** After `reauthSwitch`: the next `sign_transactions` is declined (−2). */
    private switched = false;

    constructor(opts: FakeMwaOptions) {
        this.opts = opts;
    }

    /** Queues a one-shot behaviour for the next call it applies to. */
    next(b: MwaBehaviour): void {
        this.queued.push(b);
    }

    /** The methods received, in order. */
    methods(): string[] {
        return this.calls.map((c) => c.method);
    }

    /** The account's public key (32 bytes). */
    get publicKey(): Uint8Array {
        return this.opts.secretKey.subarray(32, 64);
    }

    private keyPair(): Promise<KeyPairSigner> {
        this.signer ??= createKeyPairSignerFromBytes(this.opts.secretKey);
        return this.signer;
    }

    /** The first queued behaviour among `kinds`, removed from the queue. */
    private take(kinds: readonly MwaBehaviour[]): MwaBehaviour | null {
        const i = this.queued.findIndex((b) => kinds.includes(b));
        if (i < 0) return null;
        const [b] = this.queued.splice(i, 1);
        return b ?? null;
    }

    /** Answers one association socket (the `page.routeWebSocket` handler). */
    session(t: MwaTransport): void {
        const session: MwaSession = {
            id: this.sessions.length + 1,
            port: Number(new URL(t.url()).port),
            established: false,
            closedByPage: false,
            closedByWallet: false,
        };
        this.sessions.push(session);
        let key: Buffer | null = null;
        let outbound = 0;
        let inbound = Promise.resolve();
        let outgoing = Promise.resolve();
        const open = () => !session.closedByPage && !session.closedByWallet;
        const send = (plaintext: string, prefix?: Buffer): Promise<void> => {
            outgoing = outgoing.then(async () => {
                if (!open() || !key) return;
                const body = encryptMwa(key, ++outbound, plaintext);
                t.send(prefix ? Buffer.concat([prefix, body]) : body);
                await sleep(SEND_GAP_MS);
            });
            return outgoing;
        };
        const close = async () => {
            await outgoing;
            if (!open()) return;
            session.closedByWallet = true;
            await t.close({ code: 1011, reason: 'the E2E wallet closed the session' });
        };
        t.onClose(() => {
            session.closedByPage = true;
        });
        t.onMessage((message) => {
            inbound = inbound
                .then(async () => {
                    if (!open()) return;
                    if (typeof message === 'string') throw new Error(`unexpected text frame: ${message.slice(0, 80)}`);
                    if (!key) {
                        const hello = await this.hello(session, message);
                        key = hello.key;
                        // HELLO_RSP: Qw ‖ the session properties (sequence number 1).
                        await send(SESSION_PROPERTIES, hello.qw);
                        session.established = true;
                        return;
                    }
                    const { sequence, plaintext } = decryptMwa(key, message);
                    const request = JSON.parse(plaintext) as JsonRpcRequest;
                    if (request.id !== sequence) throw new Error(`message ${sequence} carries id ${request.id}`);
                    const call: MwaCall = {
                        session: session.id,
                        id: request.id,
                        method: request.method,
                        params: request.params ?? {},
                        outcome: 'pending',
                        answeredAt: null,
                    };
                    this.calls.push(call);
                    // Each request is answered on its own: a silent one must not hold the next.
                    void this.answer(call, send, close).catch((e: unknown) => {
                        this.errors.push(`${request.method}: ${(e as Error).message}`);
                    });
                })
                .catch((e: unknown) => {
                    this.errors.push(`session ${session.id}: ${(e as Error).message}`);
                });
        });
    }

    /** Verifies HELLO_REQ against the launched association and derives the session key. */
    private async hello(session: MwaSession, message: Buffer): Promise<{ key: Buffer; qw: Buffer }> {
        if (message.length !== 65 + 64) throw new Error(`HELLO_REQ of ${message.length} bytes`);
        const launches = await this.opts.launches();
        const launch = [...launches].reverse().find((l) => l.port === session.port);
        if (!launch) throw new Error(`no association launched for port ${session.port}`);
        const associationKey = decodeAssociation(launch.association);
        const qd = message.subarray(0, 65);
        const signature = message.subarray(65);
        const ok = verifySignature(
            'sha256',
            qd,
            { key: p256PublicKey(associationKey), dsaEncoding: 'ieee-p1363' },
            signature,
        );
        if (!ok) throw new Error('HELLO_REQ: the signature does not verify with the association key');
        const ecdh = createECDH('prime256v1');
        const qw = ecdh.generateKeys();
        const secret = ecdh.computeSecret(qd);
        const key = Buffer.from(hkdfSync('sha256', secret, associationKey, Buffer.alloc(0), 16));
        return { key, qw };
    }

    private async answer(
        call: MwaCall,
        send: (plaintext: string) => Promise<void>,
        close: () => Promise<void>,
    ): Promise<void> {
        const reply = async (result: unknown) => {
            call.outcome = 'result';
            call.answeredAt = Date.now();
            await send(JSON.stringify({ jsonrpc: '2.0', id: call.id, result }));
        };
        const fail = async (code: number, message: string) => {
            call.outcome = 'error';
            call.answeredAt = Date.now();
            await send(JSON.stringify({ jsonrpc: '2.0', id: call.id, error: { code, message } }));
        };
        const shut = async () => {
            call.outcome = 'closed';
            call.answeredAt = Date.now();
            await close();
        };
        try {
            switch (call.method) {
                case 'get_capabilities':
                    return await reply({
                        max_transactions_per_request: 10,
                        max_messages_per_request: 10,
                        supported_transaction_versions: ['legacy', 0],
                        features: this.opts.signFeature === false ? [] : ['solana:signTransactions'],
                    });
                case 'authorize':
                    return await this.authorize(call, reply, fail, shut);
                case 'sign_transactions':
                    return await this.signTransactions(call, reply, fail, shut);
                case 'sign_and_send_transactions':
                    return await this.signAndSend(call, reply);
                case 'deauthorize':
                    return await reply({});
                default:
                    return await fail(-32601, `method not supported by the E2E wallet: ${call.method}`);
            }
        } catch (e) {
            if (e instanceof WalletError) return fail(e.code, e.message);
            throw e;
        }
    }

    private account(publicKey: Uint8Array) {
        return {
            address: Buffer.from(publicKey).toString('base64'),
            label: this.opts.label ?? 'E2E android',
            ...(this.opts.accountFeatures ? { features: [...this.opts.accountFeatures] } : {}),
        };
    }

    private async authorize(
        call: MwaCall,
        reply: (r: unknown) => Promise<void>,
        fail: (code: number, message: string) => Promise<void>,
        shut: () => Promise<void>,
    ): Promise<void> {
        const token = call.params.auth_token;
        if (typeof token === 'string') {
            const b = this.take(ON_REAUTH);
            if (b === 'reauthDecline') return fail(-1, 'authorization declined (E2E)');
            if (b === 'reauthClose') return shut();
            if (token !== FAKE_MWA_AUTH_TOKEN) return fail(-1, 'unknown auth_token');
            if (b === 'reauthSwitch') {
                this.switched = true;
                return reply({ auth_token: FAKE_MWA_AUTH_TOKEN, accounts: [this.account(freshPublicKey())] });
            }
            return reply({ auth_token: FAKE_MWA_AUTH_TOKEN, accounts: [this.account(this.publicKey)] });
        }
        if (this.take(ON_CONNECT) === 'silentConnect') return;
        return reply({ auth_token: FAKE_MWA_AUTH_TOKEN, accounts: [this.account(this.publicKey)] });
    }

    private payloads(call: MwaCall): Uint8Array[] {
        const p = call.params.payloads;
        if (!Array.isArray(p) || p.some((x) => typeof x !== 'string'))
            throw new WalletError(-2, 'payloads must be base64 strings');
        return (p as string[]).map((x) => new Uint8Array(Buffer.from(x, 'base64')));
    }

    /** Signs a wire transaction with the account's key (after `tamper`: a changed message). */
    private async sign(wire: Uint8Array, tamper: boolean): Promise<Transaction> {
        let tx = getTransactionDecoder().decode(wire) as Transaction;
        if (tamper) {
            const message = getCompiledTransactionMessageDecoder().decode(tx.messageBytes);
            if (!('instructions' in message)) throw new WalletError(-2, 'not a legacy or v0 message');
            const programOf = (i: number) => message.staticAccounts[i] as string | undefined;
            const idx = message.instructions.findLastIndex(
                (ix) => programOf(ix.programAddressIndex) !== COMPUTE_BUDGET && (ix.data?.length ?? 0) > 0,
            );
            const target = message.instructions[idx];
            if (!target?.data) throw new WalletError(-2, 'nothing to tamper with');
            const data = Uint8Array.from(target.data);
            data[0] = (data[0] ?? 0) ^ 1;
            const changed = {
                ...message,
                instructions: message.instructions.map((ix, i) => (i === idx ? { ...ix, data } : ix)),
            } as typeof message;
            const messageBytes = getCompiledTransactionMessageEncoder().encode(changed);
            tx = { ...tx, messageBytes: messageBytes as Transaction['messageBytes'] };
        }
        return partiallySignTransaction([(await this.keyPair()).keyPair], tx);
    }

    private async signTransactions(
        call: MwaCall,
        reply: (r: unknown) => Promise<void>,
        fail: (code: number, message: string) => Promise<void>,
        shut: () => Promise<void>,
    ): Promise<void> {
        if (this.switched) {
            this.switched = false;
            return fail(-2, 'the payloads are not for the authorized account (E2E)');
        }
        const b = this.take(ON_SIGN);
        if (b === 'decline') return fail(-3, 'the user declined (E2E)');
        if (b === 'close') return shut();
        if (b === 'silent') return;
        const signed: string[] = [];
        const me = (await this.keyPair()).address;
        for (const wire of this.payloads(call)) {
            const tx = await this.sign(wire, b === 'tamper');
            const sig = tx.signatures[me];
            if (sig) this.signed.push({ signature: getBase58Decoder().decode(sig), tampered: b === 'tamper' });
            signed.push(Buffer.from(getTransactionEncoder().encode(tx)).toString('base64'));
        }
        return reply({ signed_payloads: signed });
    }

    private async signAndSend(call: MwaCall, reply: (r: unknown) => Promise<void>): Promise<void> {
        const send = this.opts.sendTransaction;
        if (!send) throw new WalletError(-4, 'this E2E wallet cannot send');
        const signatures: string[] = [];
        for (const wire of this.payloads(call)) {
            const tx = await this.sign(wire, false);
            const base58 = await send(getBase64EncodedWireTransaction(tx));
            const raw = Object.values(tx.signatures)[0];
            if (!raw) throw new WalletError(-4, `no signature for ${base58}`);
            signatures.push(Buffer.from(raw).toString('base64'));
        }
        return reply({ signatures });
    }
}
