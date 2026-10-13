/**
 * The E2E's fake Mobile Wallet Adapter wallet (`fake-mwa.ts`) against the real library, in-process
 * (Vitest, jsdom): the page side is the installed `@solana-mobile/wallet-standard-mobile` 0.7.0 and
 * its protocol 3.0.0, unchanged; `mwa-hooks.js` stands in for the app switch as it does in the
 * browser; the association socket is a small in-process stand-in for Playwright's routed socket
 * (`PageSocket`: a `WebSocket` for the protocol, an `MwaTransport` for the wallet). So the wallet's
 * handshake, encryption, sequence numbers and every behaviour the E2E relies on are checked against
 * the library itself, without a browser: connect, sign (reauthorization first), decline, tamper,
 * a closed session, silence, a declined / closed / switched reauthorization, an unanswered connect
 * and its reset, the sending-only and per-account-features variants, and a launch nobody answers.
 */
import { webcrypto } from 'node:crypto';
import {
    address,
    appendTransactionMessageInstructions,
    type Blockhash,
    compileTransaction,
    createNoopSigner,
    createTransactionMessage,
    getAddressDecoder,
    getBase58Decoder,
    getCompiledTransactionMessageDecoder,
    getTransactionDecoder,
    getTransactionEncoder,
    pipe,
    setTransactionMessageComputeUnitLimit,
    setTransactionMessageFeePayerSigner,
    setTransactionMessageLifetimeUsingBlockhash,
    type Transaction,
    verifySignature,
} from '@solana/kit';
import {
    createDefaultAuthorizationCache,
    createDefaultChainSelector,
    LocalSolanaMobileWalletAdapterWallet,
} from '@solana-mobile/wallet-standard-mobile';
import { getTransferSolInstruction } from '@solana-program/system';
import { afterAll, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { FakeMwa, MWA_SOCKET_URL, type MwaBehaviour, type MwaTransport } from './fake-mwa.ts';

/** The account the fake wallet signs as: a fresh throwaway keypair (never a real key). */
async function secretKey(): Promise<Uint8Array> {
    const pair = (await webcrypto.subtle.generateKey('Ed25519', true, ['sign', 'verify'])) as webcrypto.CryptoKeyPair;
    const jwk = await webcrypto.subtle.exportKey('jwk', pair.privateKey);
    if (!jwk.d || !jwk.x) throw new Error('ed25519 JWK without d / x');
    const out = new Uint8Array(64);
    out.set(Buffer.from(jwk.d, 'base64url'), 0);
    out.set(Buffer.from(jwk.x, 'base64url'), 32);
    return out;
}

let fake: FakeMwa;

/** The page side of a routed association socket (as Playwright's mock: open once routed, Blob data). */
class PageSocket extends EventTarget {
    static readonly CONNECTING = 0;
    static readonly OPEN = 1;
    static readonly CLOSING = 2;
    static readonly CLOSED = 3;
    readyState = 0;
    protocol = '';
    binaryType = 'blob';
    readonly url: string;
    private toWallet: ((m: Buffer) => unknown) | null = null;
    private closedByPage: (() => unknown) | null = null;

    constructor(url: string, protocols?: string | string[]) {
        super();
        this.url = url;
        if (!MWA_SOCKET_URL.test(url)) throw new Error(`unexpected socket ${url}`);
        const transport: MwaTransport = {
            url: () => url,
            send: (m) => {
                setTimeout(() => this.deliver(typeof m === 'string' ? Buffer.from(m) : m), 0);
            },
            close: (o) => {
                setTimeout(() => this.closeFromWallet(o?.code ?? 1000, o?.reason ?? ''), 0);
            },
            onMessage: (h) => {
                this.toWallet = h;
            },
            onClose: (h) => {
                this.closedByPage = () => h(undefined, undefined);
            },
        };
        setTimeout(() => {
            fake.session(transport);
            this.protocol = (Array.isArray(protocols) ? protocols[0] : protocols) ?? '';
            this.readyState = PageSocket.OPEN;
            this.dispatchEvent(new Event('open'));
        }, 0);
    }

    send(data: ArrayBuffer | ArrayBufferView): void {
        const bytes =
            data instanceof ArrayBuffer
                ? Buffer.from(new Uint8Array(data))
                : Buffer.from(new Uint8Array(data.buffer, data.byteOffset, data.byteLength));
        setTimeout(() => this.toWallet?.(bytes), 0);
    }

    close(): void {
        if (this.readyState >= PageSocket.CLOSING) return;
        this.readyState = PageSocket.CLOSED;
        this.closedByPage?.();
    }

    private deliver(m: Buffer): void {
        if (this.readyState !== PageSocket.OPEN) return;
        const ab = m.buffer.slice(m.byteOffset, m.byteOffset + m.byteLength);
        this.dispatchEvent(new MessageEvent('message', { data: { arrayBuffer: async () => ab } }));
    }

    private closeFromWallet(code: number, reason: string): void {
        if (this.readyState === PageSocket.CLOSED) return;
        this.readyState = PageSocket.CLOSED;
        this.dispatchEvent(new CloseEvent('close', { code, reason, wasClean: true }));
    }
}

const CHAIN = 'solana:localnet';
let key: Uint8Array;
let accountAddress: string;
let notFound = 0;

function newWallet(): LocalSolanaMobileWalletAdapterWallet {
    return new LocalSolanaMobileWalletAdapterWallet({
        appIdentity: { name: 'ShortWin', uri: 'http://localhost', icon: 'icon-192.png' },
        chains: [CHAIN],
        authorizationCache: createDefaultAuthorizationCache(),
        chainSelector: createDefaultChainSelector(),
        onWalletNotFound: async () => {
            notFound++;
        },
    });
}

function makeFake(opts: { signFeature?: boolean; accountFeatures?: readonly string[] } = {}): FakeMwa {
    return new FakeMwa({
        secretKey: key,
        launches: async () => window.__mwaE2E?.launches ?? [],
        ...opts,
    });
}

type Features = Record<string, Record<string, (...a: unknown[]) => unknown>>;
const feature = (w: LocalSolanaMobileWalletAdapterWallet, name: string) =>
    (w.features as unknown as Features)[name] ?? {};
const connect = (w: LocalSolanaMobileWalletAdapterWallet) =>
    feature(w, 'standard:connect').connect?.() as Promise<{ accounts: readonly { address: string }[] }>;

/** A transfer from the wallet's account (the fee payer) after a compute-unit limit: wire bytes. */
function unsignedTransfer(from: string): Uint8Array {
    const payer = createNoopSigner(address(from));
    const message = pipe(
        createTransactionMessage({ version: 0 }),
        (m) => setTransactionMessageFeePayerSigner(payer, m),
        (m) =>
            setTransactionMessageLifetimeUsingBlockhash(
                { blockhash: '11111111111111111111111111111111' as Blockhash, lastValidBlockHeight: 100n },
                m,
            ),
        (m) => setTransactionMessageComputeUnitLimit(50_000, m),
        (m) =>
            appendTransactionMessageInstructions(
                [
                    getTransferSolInstruction({
                        source: payer,
                        destination: address('11111111111111111111111111111112'),
                        amount: 1_234n,
                    }),
                ],
                m,
            ),
    );
    return new Uint8Array(getTransactionEncoder().encode(compileTransaction(message)));
}

async function sign(w: LocalSolanaMobileWalletAdapterWallet, wire: Uint8Array): Promise<Uint8Array> {
    const signTransaction = feature(w, 'solana:signTransaction').signTransaction;
    if (!signTransaction) throw new Error('no solana:signTransaction');
    const [out] = (await signTransaction({ transaction: wire, account: w.accounts[0] })) as {
        signedTransaction: Uint8Array;
    }[];
    if (!out) throw new Error('no signed transaction');
    return out.signedTransaction;
}

/** A legacy or v0 message's instructions (the compiled message codec also knows v1). */
function instructionsOf(messageBytes: Transaction['messageBytes']) {
    const m = getCompiledTransactionMessageDecoder().decode(messageBytes);
    if (!('instructions' in m)) throw new Error('not a legacy or v0 message');
    return m.instructions;
}

/** The library's error, unwrapped (`new Error(message, { cause })`, possibly twice). */
function rootCause(e: unknown): { name?: string; code?: unknown } {
    let c = e as { name?: string; code?: unknown; cause?: unknown };
    for (let i = 0; i < 6 && c?.cause; i++) c = c.cause as typeof c;
    return c;
}

/** Settles within `ms`, or `'pending'`. */
async function within<T>(p: Promise<T>, ms: number): Promise<T | 'pending'> {
    return Promise.race([p, new Promise<'pending'>((r) => setTimeout(() => r('pending'), ms))]);
}

let restoreSecure: () => void = () => {};
const assign = vi.fn();
beforeAll(async () => {
    if (!globalThis.crypto?.subtle) vi.stubGlobal('crypto', webcrypto);
    vi.stubGlobal('WebSocket', PageSocket);
    const was = Object.getOwnPropertyDescriptor(window, 'isSecureContext');
    Object.defineProperty(window, 'isSecureContext', { configurable: true, value: true });
    restoreSecure = () => {
        if (was) Object.defineProperty(window, 'isSecureContext', was);
        else Reflect.deleteProperty(window, 'isSecureContext');
    };
    // The library logs its spinner's life ("Injecting modal", "Modal open").
    vi.spyOn(console, 'log').mockImplementation(() => {});
    vi.spyOn(console, 'debug').mockImplementation(() => {});
    // The intent (`location.assign('solana-wallet:…')`): recorded, as headless Chromium leaves the
    // page where it is (jsdom would report a navigation it does not implement).
    vi.stubGlobal('location', {
        href: 'http://localhost:3000/',
        origin: 'http://localhost:3000',
        protocol: 'http:',
        host: 'localhost:3000',
        hostname: 'localhost',
        port: '3000',
        pathname: '/',
        search: '',
        hash: '',
        assign,
    });
    // The init script the E2E installs, evaluated as the page evaluates it (a side-effect import).
    const hooks = './mwa-hooks.js';
    await import(/* @vite-ignore */ hooks);
    key = await secretKey();
    accountAddress = getAddressDecoder().decode(key.subarray(32, 64));
});
afterAll(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
    restoreSecure();
});
beforeEach(() => {
    localStorage.clear();
    if (window.__mwaE2E) window.__mwaE2E.launch = 'ok';
    fake = makeFake();
});

describe('the fake MWA wallet answers the real library (wallet-standard-mobile 0.7.0, protocol 3.0.0)', () => {
    it('connect: one association, the handshake, get_capabilities and authorize; the account and features', async () => {
        const w = newWallet();
        const launchesBefore = window.__mwaE2E?.launches.length ?? 0;
        const { accounts } = await connect(w);
        expect(accounts.map((a) => a.address)).toEqual([accountAddress]);
        expect(window.__mwaE2E?.launches.length).toBe(launchesBefore + 1);
        const launch = window.__mwaE2E?.launches.at(-1);
        expect(launch?.port).toBeGreaterThanOrEqual(49152);
        expect(launch?.association).toMatch(/^[A-Za-z0-9_\-.]+$/);
        // The intent itself, as Android would receive it.
        const intent = new URL(String(assign.mock.calls.at(-1)?.[0]));
        expect(`${intent.protocol}${intent.pathname}`).toBe('solana-wallet:/v1/associate/local');
        expect(intent.searchParams.get('port')).toBe(String(launch?.port));
        expect(intent.searchParams.get('association')).toBe(launch?.association);
        expect(intent.searchParams.get('v')).toBe('v1');
        expect(fake.errors).toEqual([]);
        expect([...fake.methods()].sort()).toEqual(['authorize', 'get_capabilities']);
        expect(fake.calls.find((c) => c.method === 'authorize')?.params).toMatchObject({
            chain: CHAIN,
            identity: { name: 'ShortWin', uri: 'http://localhost', icon: 'icon-192.png' },
        });
        expect(fake.sessions).toHaveLength(1);
        expect(fake.sessions[0]).toMatchObject({ port: launch?.port, established: true, closedByWallet: false });
        await vi.waitFor(() => expect(fake.sessions[0]?.closedByPage).toBe(true));
        expect(Object.keys(w.features)).toEqual(expect.arrayContaining(['solana:signTransaction']));
    });

    it('sign: a new association, the reauthorization, then sign_transactions; the signature verifies', async () => {
        const w = newWallet();
        await connect(w);
        const wire = unsignedTransfer(accountAddress);
        const signed = getTransactionDecoder().decode(await sign(w, wire));
        expect(fake.errors).toEqual([]);
        expect(fake.methods().slice(-2)).toEqual(['authorize', 'sign_transactions']);
        expect(fake.calls.at(-2)?.params.auth_token).toBe('e2e');
        expect(fake.sessions).toHaveLength(2);
        const original = getTransactionDecoder().decode(wire);
        expect(Buffer.from(signed.messageBytes)).toEqual(Buffer.from(original.messageBytes));
        const sig = signed.signatures[address(accountAddress)];
        if (!sig) throw new Error('not signed by the account');
        const pub = await webcrypto.subtle.importKey('raw', key.subarray(32, 64), 'Ed25519', true, ['verify']);
        expect(await verifySignature(pub as CryptoKey, sig, signed.messageBytes)).toBe(true);
        expect(fake.signed).toEqual([{ signature: getBase58Decoder().decode(sig), tampered: false }]);
    });

    it('tamper: the last non-compute-budget instruction changes (first data byte XOR 1), and is signed', async () => {
        const w = newWallet();
        await connect(w);
        fake.next('tamper');
        const wire = unsignedTransfer(accountAddress);
        const signed = getTransactionDecoder().decode(await sign(w, wire));
        const before = instructionsOf(getTransactionDecoder().decode(wire).messageBytes);
        const after = instructionsOf(signed.messageBytes);
        const last = (ixs: typeof before) => Array.from(ixs.at(-1)?.data ?? []);
        expect(last(after)[0]).toBe((last(before)[0] ?? 0) ^ 1);
        expect(last(after).slice(1)).toEqual(last(before).slice(1));
        expect(Array.from(after[0]?.data ?? [])).toEqual(Array.from(before[0]?.data ?? []));
        expect(fake.signed.map((s) => s.tampered)).toEqual([true]);
    });

    it.each([
        ['decline', 'SolanaMobileWalletAdapterProtocolError', -3],
        ['close', 'SolanaMobileWalletAdapterError', 'ERROR_SESSION_CLOSED'],
    ] as const)('%s on sign_transactions: the library rejects with %s %s', async (b, name, code) => {
        const w = newWallet();
        await connect(w);
        fake.next(b as MwaBehaviour);
        const err = await sign(w, unsignedTransfer(accountAddress)).then(
            () => null,
            (e: unknown) => e,
        );
        expect(rootCause(err)).toMatchObject({ name, code });
        expect(fake.calls.at(-1)?.method).toBe('sign_transactions');
        expect(w.connected).toBe(true);
        if (b === 'close') expect(fake.sessions.at(-1)?.closedByWallet).toBe(true);
    });

    it('silent on sign_transactions: never answered, the session stays open', async () => {
        const w = newWallet();
        await connect(w);
        fake.next('silent');
        expect(await within(sign(w, unsignedTransfer(accountAddress)), 1_000)).toBe('pending');
        expect(fake.calls.at(-1)).toMatchObject({ method: 'sign_transactions', outcome: 'pending' });
        expect(fake.sessions.at(-1)).toMatchObject({ closedByPage: false, closedByWallet: false });
    });

    it.each(['reauthDecline', 'reauthClose'] as const)(
        '%s: the library drops the connection (change, no accounts) and the sign never settles',
        async (b) => {
            const w = newWallet();
            await connect(w);
            const changes: { accounts?: readonly unknown[] }[] = [];
            feature(w, 'standard:events').on?.('change', (e: unknown) => changes.push(e as { accounts?: [] }));
            fake.next(b);
            const tapped = Date.now();
            expect(await within(sign(w, unsignedTransfer(accountAddress)), 1_000)).toBe('pending');
            expect(changes.some((c) => c.accounts?.length === 0)).toBe(true);
            expect(w.connected).toBe(false);
            expect(fake.methods()).not.toContain('sign_transactions');
            // The reauthorization's answer is timed (mobile-mwa M13 / M14 measure the page from it).
            const reauth = fake.calls.at(-1);
            expect(reauth).toMatchObject({
                method: 'authorize',
                params: expect.objectContaining({ auth_token: 'e2e' }),
                outcome: b === 'reauthDecline' ? 'error' : 'closed',
            });
            expect(reauth?.answeredAt).toBeGreaterThanOrEqual(tapped);
            expect(fake.calls.every((c) => c.outcome === 'pending' || typeof c.answeredAt === 'number')).toBe(true);
        },
    );

    it('reauthSwitch: another account at the reauthorization, then sign_transactions declined with -2', async () => {
        const w = newWallet();
        await connect(w);
        const changes: { accounts?: readonly { address: string }[] }[] = [];
        feature(w, 'standard:events').on?.('change', (e: unknown) => changes.push(e as (typeof changes)[number]));
        fake.next('reauthSwitch');
        const err = await sign(w, unsignedTransfer(accountAddress)).then(
            () => null,
            (e: unknown) => e,
        );
        expect(rootCause(err)).toMatchObject({ name: 'SolanaMobileWalletAdapterProtocolError', code: -2 });
        const switched = changes.find((c) => c.accounts?.length === 1);
        expect(switched?.accounts?.[0]?.address).not.toBe(accountAddress);
        expect(w.accounts[0]?.address).not.toBe(accountAddress);
    });

    it('silentConnect: the connect never settles; standard:disconnect resets the lock, a new connect associates', async () => {
        const w = newWallet();
        fake.next('silentConnect');
        expect(await within(connect(w), 1_000)).toBe('pending');
        const launches = window.__mwaE2E?.launches.length ?? 0;
        // The library's #connecting lock: another connect returns no accounts without launching.
        expect((await connect(w)).accounts).toEqual([]);
        expect(window.__mwaE2E?.launches.length).toBe(launches);
        await feature(w, 'standard:disconnect').disconnect?.();
        const { accounts } = await connect(w);
        expect(accounts.map((a) => a.address)).toEqual([accountAddress]);
        expect(window.__mwaE2E?.launches.length).toBe(launches + 1);
    });

    it('signFeature false: no solana:signTransactions, so the wallet offers signAndSendTransaction only', async () => {
        fake = makeFake({ signFeature: false });
        const w = newWallet();
        await connect(w);
        expect(Object.keys(w.features)).not.toContain('solana:signTransaction');
        expect(Object.keys(w.features)).toContain('solana:signAndSendTransaction');
        expect(w.accounts[0]?.features).toContain('solana:signTransaction'); // the library's default
    });

    it('accountFeatures: per-account features in MWA ids reach the account as given', async () => {
        fake = makeFake({ accountFeatures: ['solana:signTransactions', 'solana:signAndSendTransaction'] });
        const w = newWallet();
        await connect(w);
        expect(w.accounts[0]?.features).toEqual(['solana:signTransactions', 'solana:signAndSendTransaction']);
        expect(Object.keys(w.features)).toContain('solana:signTransaction');
    });

    it("launch 'not-found': no blur follows the intent, so the library reports ERROR_WALLET_NOT_FOUND", async () => {
        const w = newWallet();
        if (!window.__mwaE2E) throw new Error('hooks not installed');
        window.__mwaE2E.launch = 'not-found';
        const before = notFound;
        const err = await connect(w).then(
            () => null,
            (e: unknown) => e,
        );
        expect(rootCause(err)).toMatchObject({ code: 'ERROR_WALLET_NOT_FOUND' });
        expect(notFound).toBe(before + 1);
        expect(fake.sessions).toHaveLength(0);
    }, 10_000);

    it('returnToPage dispatches focus on window', () => {
        const seen = vi.fn();
        window.addEventListener('focus', seen);
        window.__mwaE2E?.returnToPage();
        window.removeEventListener('focus', seen);
        expect(seen).toHaveBeenCalledTimes(1);
    });
});
