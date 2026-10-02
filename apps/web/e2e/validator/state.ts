/**
 * The seeded scenario (`.e2e/state.json`, written by `seed.ts`, read by the fixtures and the
 * indexer mock): every address a spec needs and the paths of the generated keys. FRONTEND.md §10.2.
 */
import { existsSync, readFileSync } from 'node:fs';
import type { Address } from '@solana/kit';
import { PATHS } from '../env.ts';
import type { WalletName } from './keys.ts';

export interface MockPoolInfo {
    readonly pool: Address;
    readonly baseVault: Address;
    readonly quoteVault: Address;
    readonly authority: Address;
    readonly feePpm: number;
}

export interface MarketInfo {
    /** What the indexer mock reports as the token's symbol and name (the chain has none for SPL mints). */
    readonly symbol: string;
    readonly name: string;
    readonly mint: Address;
    readonly tokenProgram: Address;
    readonly decimals: number;
    readonly market: Address;
    readonly marketVault: Address;
    readonly poolVault: Address;
    readonly trader: Address;
    readonly traderWsol: Address;
    readonly venue: 'mock' | 'none';
    readonly pool: MockPoolInfo | null;
    readonly creator: Address;
}

export interface PositionInfo {
    readonly position: Address;
    readonly market: Address;
    readonly mint: Address;
    readonly borrower: Address;
    readonly nonce: string;
    readonly q: string;
}

export interface WalletInfo {
    readonly address: Address;
    /** `.e2e/keys/<name>.json` (64-byte array; generated per run). */
    readonly keyPath: string;
}

export interface E2EState {
    readonly version: 1;
    readonly createdAt: string;
    readonly program: 'test-venue' | 'release';
    readonly venues: boolean;
    readonly protocolAlt: Address | null;
    readonly admin: {
        readonly authority: Address;
        readonly governance: Address;
        readonly pause: Address;
        readonly coverage: Address;
        readonly feeRecipient: Address;
        readonly payer: Address;
        readonly mintAuthority: Address;
        readonly poolAdmin: Address;
        readonly seedLender: Address;
    };
    readonly wallets: Readonly<Record<WalletName, WalletInfo>>;
    /**
     * `main` (mock venue: trading), `move` (mock: the price-move specs), `paused` (mock, coverage
     * paused), `t22` (list-only Token-2022: CPI Guard and memo). On a release-build run the three
     * mock markets are list-only (`venue: 'none'`) and nothing is shorted.
     */
    readonly markets: {
        readonly main: MarketInfo;
        readonly move: MarketInfo;
        readonly paused: MarketInfo;
        readonly t22: MarketInfo;
    };
    /** Mints without a market for create-market (§3.5): safe, freeze authority, a disallowed extension. */
    readonly createMints: {
        /** Passes the filter (verdict only). */
        readonly ok: Address;
        /** Passes the filter; listed list-only through the pipeline by create-market.spec. */
        readonly listable: Address;
        readonly freeze: Address;
        readonly badExtension: Address;
    };
    /** Seeded open positions (opened from Node before the run); `null` on a release-build run. */
    readonly positions: {
        readonly margin: PositionInfo;
        readonly closer: PositionInfo;
        readonly holder: PositionInfo;
    } | null;
    /** Token accounts of the wallets (FRONTEND.md §10.3). */
    readonly tokenAccounts: {
        /** Main-mint ATAs with tokens, per wallet that deposits into `main`. */
        readonly mainAta: Readonly<Partial<Record<WalletName, Address>>>;
        readonly guard: { readonly ata: Address; readonly guarded: Address; readonly race: Address };
        readonly memo: { readonly ata: Address; readonly source: Address };
        readonly memoRace: { readonly ata: Address; readonly source: Address };
        readonly errors: { readonly t22Ata: Address; readonly race: Address };
    };
    /** Dumped mainnet mints present on the validator (SHORTWIN_E2E_VENUES=1), for venue detection. */
    readonly venueMints: {
        readonly pumpCurve: Address;
        readonly pumpSwap: Address;
        readonly raydiumV4: Address;
        readonly raydiumCpmm: Address;
    } | null;
}

export function loadState(): E2EState {
    if (!existsSync(PATHS.state)) {
        throw new Error(`no ${PATHS.state}: the E2E global setup (or \`node e2e/validator/start.ts\`) seeds it`);
    }
    const s = JSON.parse(readFileSync(PATHS.state, 'utf8')) as E2EState;
    if (s.version !== 1) throw new Error(`unsupported state.json version ${String(s.version)}`);
    return s;
}
