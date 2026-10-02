/**
 * "ShortWin E2E" — a TEST-ONLY Wallet Standard wallet injected by Playwright (`page.addInitScript`,
 * FRONTEND.md §10.2). Plain browser JS, no imports: it registers through the Wallet Standard
 * `wallet-standard:register-wallet` / `wallet-standard:app-ready` events, on chain
 * `solana:localnet`, with `standard:connect`, `standard:disconnect`, `standard:events`,
 * `solana:signTransaction` (versions `legacy` and `0`) and `solana:signMessage`, and signs with
 * WebCrypto Ed25519 from a secret key generated per run (never a real key).
 *
 * Usage: the init script is this file followed by
 *   `window.__shortwinInstallE2EWallet({ secretKey: number[64], chains: ['solana:localnet'] })`.
 *
 * Test controls on `window.__shortwinE2E`:
 * - `rejectNext(n = 1)`         the next n sign requests are rejected (code 4001, "User rejected the request.")
 * - `tamperNext(mode = 'data')` the next signature is made over a CHANGED message — the app must
 *                               refuse to send it (FRONTEND.md §5.8). Modes: 'data' (flip the last data
 *                               byte of the last non-compute-budget instruction), 'drop' (drop the last
 *                               instruction), or a function (Uint8Array message) => Uint8Array.
 * - `raiseFeeNext(price)`       the next signature raises the SetComputeUnitPrice instruction to
 *                               `price` µlamports/CU — a compute-budget-only change the app accepts
 *                               (FE-2) when within its ceiling.
 * - `signed()`                  the transactions signed so far on this page: { wire (base64),
 *                               signature (base58), tampered, raisedFeeTo }
 * - `address`, `connected()`, `reset()`
 *
 * Like a real wallet extension it remembers an approved site (localStorage of the app origin), so a
 * reload reconnects silently; a fresh browser context starts disconnected.
 */
(() => {
    const NAME = 'ShortWin E2E';
    const ICON =
        'data:image/svg+xml;base64,' +
        btoa(
            '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32"><rect width="32" height="32" rx="6" fill="#0b0f14"/><path d="M8 10h16l-8 13z" fill="#ff4d5e"/></svg>',
        );
    const AUTH_KEY = 'shortwin-e2e-wallet:authorized';
    const COMPUTE_BUDGET = 'ComputeBudget111111111111111111111111111111';
    const ALPHABET = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
    const PKCS8_ED25519_PREFIX = new Uint8Array([
        0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20,
    ]);

    function b58encode(bytes) {
        const digits = [0];
        for (const b of bytes) {
            let carry = b;
            for (let i = 0; i < digits.length; i++) {
                carry += digits[i] << 8;
                digits[i] = carry % 58;
                carry = (carry / 58) | 0;
            }
            while (carry > 0) {
                digits.push(carry % 58);
                carry = (carry / 58) | 0;
            }
        }
        let out = '';
        for (const b of bytes) {
            if (b !== 0) break;
            out += '1';
        }
        for (let i = digits.length - 1; i >= 0; i--) out += ALPHABET[digits[i]];
        return out;
    }

    function b64encode(bytes) {
        let s = '';
        for (const b of bytes) s += String.fromCharCode(b);
        return btoa(s);
    }

    /** Solana's compact-u16. Returns [value, nextOffset]. */
    function readShortVec(buf, off) {
        let value = 0;
        for (let shift = 0, i = 0; i < 3; i++, shift += 7) {
            const b = buf[off++];
            value |= (b & 0x7f) << shift;
            if ((b & 0x80) === 0) break;
        }
        return [value, off];
    }

    function writeShortVec(n) {
        const out = [];
        let v = n;
        for (;;) {
            let b = v & 0x7f;
            v >>= 7;
            if (v === 0) {
                out.push(b);
                return out;
            }
            b |= 0x80;
            out.push(b);
        }
    }

    /** A legacy or v0 message: header, static keys, blockhash, instructions (with byte offsets). */
    function parseMessage(m) {
        let o = 0;
        const versioned = (m[0] & 0x80) !== 0;
        if (versioned) o = 1;
        const header = { numRequiredSignatures: m[o], numReadonlySigned: m[o + 1], numReadonlyUnsigned: m[o + 2] };
        o += 3;
        let nKeys;
        [nKeys, o] = readShortVec(m, o);
        const keys = [];
        for (let i = 0; i < nKeys; i++) {
            keys.push(m.subarray(o, o + 32));
            o += 32;
        }
        o += 32; // recent blockhash
        const ixStart = o;
        let nIx;
        [nIx, o] = readShortVec(m, o);
        const instructions = [];
        for (let i = 0; i < nIx; i++) {
            const start = o;
            const programIdIndex = m[o++];
            let nAcc;
            [nAcc, o] = readShortVec(m, o);
            o += nAcc;
            let dataLen;
            [dataLen, o] = readShortVec(m, o);
            instructions.push({ start, end: o + dataLen, programIdIndex, dataOffset: o, dataLength: dataLen });
            o += dataLen;
        }
        return { versioned, header, keys, ixStart, ixEnd: o, instructions };
    }

    function parseWire(wire) {
        const [nSigs, sigOff] = readShortVec(wire, 0);
        const msgOff = sigOff + 64 * nSigs;
        return { nSigs, sigOff, message: wire.slice(msgOff) };
    }

    function programOf(msg, ix) {
        const k = msg.keys[ix.programIdIndex];
        return k ? b58encode(k) : null;
    }

    function tamper(message, mode) {
        if (typeof mode === 'function') return new Uint8Array(mode(new Uint8Array(message)));
        const msg = parseMessage(message);
        const candidates = msg.instructions.filter((ix) => programOf(msg, ix) !== COMPUTE_BUDGET);
        const target = candidates[candidates.length - 1];
        if (!target) throw new Error('tamperNext: no instruction to change');
        if (mode === 'drop') {
            const out = [];
            out.push(...message.subarray(0, msg.ixStart));
            out.push(...writeShortVec(msg.instructions.length - 1));
            for (const ix of msg.instructions) if (ix !== target) out.push(...message.subarray(ix.start, ix.end));
            out.push(...message.subarray(msg.ixEnd));
            return new Uint8Array(out);
        }
        if (target.dataLength === 0) throw new Error('tamperNext: the last instruction has no data');
        const out = new Uint8Array(message);
        out[target.dataOffset + target.dataLength - 1] ^= 0x01;
        return out;
    }

    function raiseFee(message, price) {
        const msg = parseMessage(message);
        const ix = msg.instructions.find((i) => programOf(msg, i) === COMPUTE_BUDGET && message[i.dataOffset] === 3);
        if (ix?.dataLength !== 9) throw new Error('raiseFeeNext: the message has no SetComputeUnitPrice instruction');
        const out = new Uint8Array(message);
        new DataView(out.buffer).setBigUint64(ix.dataOffset + 1, BigInt(price), true);
        return out;
    }

    function rejection() {
        const e = new Error('User rejected the request.');
        e.name = 'WalletSignTransactionError';
        e.code = 4001;
        return e;
    }

    class RegisterWalletEvent extends Event {
        #detail;
        get detail() {
            return this.#detail;
        }
        get type() {
            return 'wallet-standard:register-wallet';
        }
        constructor(callback) {
            super('wallet-standard:register-wallet', { bubbles: false, cancelable: false, composed: false });
            this.#detail = callback;
        }
        preventDefault() {
            throw new Error('preventDefault cannot be called');
        }
        stopImmediatePropagation() {
            throw new Error('stopImmediatePropagation cannot be called');
        }
        stopPropagation() {
            throw new Error('stopPropagation cannot be called');
        }
    }

    window.__shortwinInstallE2EWallet = (cfg) => {
        if (window.__shortwinE2E) return;
        const secret = Uint8Array.from(cfg.secretKey);
        if (secret.length !== 64) throw new Error('ShortWin E2E wallet: secretKey must be 64 bytes');
        const publicKey = secret.slice(32, 64);
        const address = b58encode(publicKey);
        const chains = Object.freeze([...(cfg.chains ?? ['solana:localnet'])]);
        const pkcs8 = new Uint8Array(48);
        pkcs8.set(PKCS8_ED25519_PREFIX, 0);
        pkcs8.set(secret.subarray(0, 32), 16);
        const keyPromise = crypto.subtle.importKey('pkcs8', pkcs8, { name: 'Ed25519' }, false, ['sign']);
        const sign = async (bytes) =>
            new Uint8Array(await crypto.subtle.sign({ name: 'Ed25519' }, await keyPromise, bytes));

        const account = Object.freeze({
            address,
            publicKey,
            chains,
            features: Object.freeze(['solana:signTransaction', 'solana:signMessage']),
            label: cfg.label ?? 'E2E',
        });

        const authorized = () => {
            try {
                return localStorage.getItem(AUTH_KEY) === address;
            } catch {
                return false;
            }
        };
        const st = {
            connected: false,
            rejectNext: 0,
            tamperNext: null,
            raiseFeeNext: null,
            signed: [],
            listeners: new Set(),
        };
        const emit = () => {
            for (const l of [...st.listeners]) {
                try {
                    l({ accounts: st.connected ? [account] : [] });
                } catch (e) {
                    console.error('ShortWin E2E wallet listener', e);
                }
            }
        };

        async function signOne(input) {
            if (!st.connected) throw new Error('ShortWin E2E wallet: not connected');
            if (input.account && input.account.address !== address)
                throw new Error('ShortWin E2E wallet: unknown account');
            if (st.rejectNext > 0) {
                st.rejectNext--;
                throw rejection();
            }
            const wire = new Uint8Array(input.transaction);
            const { nSigs, sigOff, message: original } = parseWire(wire);
            let message = original;
            let raisedFeeTo = null;
            let tampered = false;
            if (st.raiseFeeNext !== null) {
                raisedFeeTo = st.raiseFeeNext;
                st.raiseFeeNext = null;
                message = raiseFee(message, raisedFeeTo);
            }
            if (st.tamperNext !== null) {
                const mode = st.tamperNext;
                st.tamperNext = null;
                message = tamper(message, mode);
                tampered = true;
            }
            const msg = parseMessage(message);
            let slot = -1;
            for (let i = 0; i < msg.header.numRequiredSignatures; i++) {
                if (b58encode(msg.keys[i]) === address) slot = i;
            }
            if (slot < 0) throw new Error('ShortWin E2E wallet: this transaction does not need our signature');
            const sig = await sign(message);
            const sigs = new Uint8Array(64 * nSigs);
            sigs.set(wire.subarray(sigOff, sigOff + 64 * nSigs));
            sigs.set(sig, 64 * slot);
            const prefix = writeShortVec(nSigs);
            const out = new Uint8Array(prefix.length + sigs.length + message.length);
            out.set(prefix, 0);
            out.set(sigs, prefix.length);
            out.set(message, prefix.length + sigs.length);
            st.signed.push({
                wire: b64encode(out),
                signature: b58encode(slot === 0 ? sig : out.subarray(prefix.length, prefix.length + 64)),
                tampered,
                raisedFeeTo: raisedFeeTo === null ? null : String(raisedFeeTo),
            });
            return { signedTransaction: out };
        }

        const features = Object.freeze({
            'standard:connect': {
                version: '1.0.0',
                connect: async (input) => {
                    if (input?.silent && !authorized()) return { accounts: [] };
                    st.connected = true;
                    try {
                        localStorage.setItem(AUTH_KEY, address);
                    } catch {
                        // storage unavailable: no silent reconnect
                    }
                    emit();
                    return { accounts: [account] };
                },
            },
            'standard:disconnect': {
                version: '1.0.0',
                disconnect: async () => {
                    st.connected = false;
                    try {
                        localStorage.removeItem(AUTH_KEY);
                    } catch {
                        // ignore
                    }
                    emit();
                },
            },
            'standard:events': {
                version: '1.0.0',
                on: (event, listener) => {
                    if (event !== 'change') return () => {};
                    st.listeners.add(listener);
                    return () => st.listeners.delete(listener);
                },
            },
            'solana:signTransaction': {
                version: '1.0.0',
                supportedTransactionVersions: Object.freeze(['legacy', 0]),
                signTransaction: async (...inputs) => {
                    const out = [];
                    for (const input of inputs) out.push(await signOne(input));
                    return out;
                },
            },
            'solana:signMessage': {
                version: '1.0.0',
                signMessage: async (...inputs) => {
                    const out = [];
                    for (const input of inputs) {
                        if (!st.connected) throw new Error('ShortWin E2E wallet: not connected');
                        if (st.rejectNext > 0) {
                            st.rejectNext--;
                            throw rejection();
                        }
                        const message = new Uint8Array(input.message);
                        out.push({ signedMessage: message, signature: await sign(message), signatureType: 'ed25519' });
                    }
                    return out;
                },
            },
        });

        const wallet = Object.freeze({
            get version() {
                return '1.0.0';
            },
            get name() {
                return NAME;
            },
            get icon() {
                return ICON;
            },
            get chains() {
                return chains;
            },
            get features() {
                return features;
            },
            get accounts() {
                return st.connected ? [account] : [];
            },
        });

        window.__shortwinE2E = Object.freeze({
            address,
            name: NAME,
            connected: () => st.connected,
            rejectNext: (n = 1) => {
                st.rejectNext = n;
            },
            tamperNext: (mode = 'data') => {
                st.tamperNext = mode;
            },
            raiseFeeNext: (microLamportsPerCu) => {
                st.raiseFeeNext = BigInt(microLamportsPerCu);
            },
            signed: () => st.signed.map((s) => ({ ...s })),
            reset: () => {
                st.rejectNext = 0;
                st.tamperNext = null;
                st.raiseFeeNext = null;
                st.signed.length = 0;
            },
            /** Signs a base64 wire transaction as the app would ask (harness self-tests). */
            signWireForTest: async (base64) => {
                const bytes = Uint8Array.from(atob(base64), (c) => c.charCodeAt(0));
                const [r] = await features['solana:signTransaction'].signTransaction({ account, transaction: bytes });
                return b64encode(r.signedTransaction);
            },
        });

        const callback = ({ register }) => register(wallet);
        try {
            window.dispatchEvent(new RegisterWalletEvent(callback));
        } catch (e) {
            console.error('wallet-standard:register-wallet event could not be dispatched', e);
        }
        try {
            window.addEventListener('wallet-standard:app-ready', ({ detail: api }) => callback(api));
        } catch (e) {
            console.error('wallet-standard:app-ready event listener could not be added', e);
        }
    };
})();
