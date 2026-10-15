/**
 * Generated E2E keypairs (FRONTEND.md §10.2): created per run with Node's Ed25519, stored as the
 * Solana CLI's 64-byte JSON arrays (`seed ‖ public key`) under `.e2e/keys/` (gitignored), and
 * loaded as Kit signers. Nothing here is a real key; the validator is reset on every run.
 */
import { generateKeyPairSync } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { type Address, createKeyPairSignerFromBytes, getBase58Decoder, type KeyPairSigner } from '@solana/kit';
import { PATHS } from '../env.ts';

/** A 64-byte Solana keypair (`seed ‖ public key`). */
export type SecretKey = Uint8Array;

function b64url(s: string): Uint8Array {
    return new Uint8Array(Buffer.from(s, 'base64url'));
}

/** A fresh Ed25519 keypair in the Solana CLI layout. */
export function generateSecretKey(): SecretKey {
    const { privateKey } = generateKeyPairSync('ed25519');
    const jwk = privateKey.export({ format: 'jwk' });
    if (!jwk.d || !jwk.x) throw new Error('ed25519 JWK without d / x');
    const out = new Uint8Array(64);
    out.set(b64url(jwk.d), 0);
    out.set(b64url(jwk.x), 32);
    return out;
}

/** The base58 public key of a 64-byte keypair. */
export function publicKeyOf(secret: SecretKey): Address {
    return getBase58Decoder().decode(secret.subarray(32, 64)) as Address;
}

export function keyPath(name: string): string {
    return join(PATHS.keys, `${name}.json`);
}

/** Loads `name`, generating and saving it first when `create` and it does not exist. */
export function loadSecretKey(name: string, create = false): SecretKey {
    const p = keyPath(name);
    if (!existsSync(p)) {
        if (!create) throw new Error(`missing E2E key ${p}; run the validator bootstrap first`);
        mkdirSync(PATHS.keys, { recursive: true });
        const k = generateSecretKey();
        writeFileSync(p, JSON.stringify(Array.from(k)), { mode: 0o600 });
        return k;
    }
    const raw = JSON.parse(readFileSync(p, 'utf8')) as number[];
    if (!Array.isArray(raw) || raw.length !== 64) throw new Error(`${p} is not a 64-byte keypair`);
    return new Uint8Array(raw);
}

const signers = new Map<string, Promise<KeyPairSigner>>();

/** The Kit signer of a stored key (cached per process). */
export function signerFor(name: string): Promise<KeyPairSigner> {
    let s = signers.get(name);
    if (!s) {
        s = createKeyPairSignerFromBytes(loadSecretKey(name));
        signers.set(name, s);
    }
    return s;
}

/**
 * Every key the bootstrap generates. Admin roles of `initialize_config`, the seeding payer and
 * authorities, and one wallet per independent spec group, so the specs do not depend on the order
 * they run in (each wallet's state is only changed by its own spec and the seed).
 */
export const KEY_NAMES = {
    admin: [
        'authority',
        'governance',
        'pause',
        'feeRecipient',
        'coverage',
        'payer',
        'mintAuthority',
        'poolAdmin',
        'seedLender',
    ],
    wallets: [
        'trader',
        'lender',
        'guard',
        'memo',
        'memoRace',
        'claimer',
        'creator',
        'closer',
        'margin',
        'holder',
        'errors',
        'keyboard',
        // FE-18: lend-post-wipe (the wiped market's only lender, its borrower, a new lender) and
        // lend-claim-sync (the claiming lender, the late lender, the borrower).
        'wiped',
        'wipeBorrower',
        'rejoin',
        'potClaimer',
        'potLate',
        'potBorrower',
        // KI66-R4: lend-claim-pot-chain (the claiming lender, the absent lender behind the rebase
        // record, the borrower whose S1 settlement rebases the shares and who borrows again after).
        'chainClaimer',
        'chainAbsent',
        'chainBorrower',
        // M10-RESP: the phone project's specs (mobile-*.spec.ts) sign only with it.
        'phone',
        // O-6: the Android project's Mobile Wallet Adapter specs (mobile-mwa.spec.ts): only the fake
        // MWA wallet (e2e/wallet/fake-mwa.ts) signs as it.
        'android',
        // pump v3 (design pump-final §8): short-close-curve.spec.ts — the borrower who opens and
        // closes on the near166 curve, its lender (tokens planted at genesis: `curveLenderAccount`)
        // and the third party that buys the curve down past the position's q.
        'curve',
        'curveLender',
        'curveCrowd',
    ],
} as const;

export type WalletName = (typeof KEY_NAMES.wallets)[number];
export type AdminKeyName = (typeof KEY_NAMES.admin)[number];
