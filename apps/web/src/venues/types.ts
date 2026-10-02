/**
 * The venue layer's frozen interface (FRONTEND.md §5.1, §12): what MARKETS and SHORT code against
 * while SHORT builds the implementation (`venues/index.ts`). TypeScript ports of the keeper's
 * account lists (`services/keeper/src/venue.rs`) and the venue-state readers
 * (`crates/shortwin-venue-state`), checked against the fixture accounts and keeper vectors.
 */
import type { AccountMeta, Address, Instruction } from '@solana/kit';
import type { VenueName } from '../api/types.ts';

/** `VenueKind` codes (programs/shortwin/src/state/market.rs). */
export const VENUE_KIND = {
    none: 0,
    pump_curve: 1,
    pump_migrating: 2,
    pump_swap: 3,
    raydium_v4: 4,
    raydium_cpmm: 5,
    mock: 6,
} as const satisfies Partial<Record<VenueName, number>>;

/** `create_market`'s `venue_hint` (create_market.rs: `VENUE_HINT_AUTO` 0, Raydium kinds 4 / 5; Mock 6 test-only). */
export const VENUE_HINT = { auto: 0, raydium_v4: 4, raydium_cpmm: 5, mock: 6 } as const;

/** What `create_market` with `venue_hint = auto` would list a mint as (detection set D1). */
export interface VenueDetection {
    readonly kind: 'none' | 'pump_curve' | 'pump_migrating' | 'pump_swap';
    readonly bondingCurve: Address;
    readonly canonicalPool: Address;
    /** Why a pump coin cannot be listed, as the program error name (e.g. `CurveMayhemUnsupported`). */
    readonly refusal: string | null;
    /** `create_market`'s remaining accounts for this listing (detection set, then the init set). */
    readonly remainingAccounts: readonly AccountMeta[];
    /** Instructions to prepend (pump `extend_account` for a curve shorter than 115 B). */
    readonly preInstructions: readonly Instruction[];
}

/** A Raydium SOL pool of the mint, judged as `eligible()` would (ARCH §2.6). */
export interface PoolCandidate {
    readonly pool: Address;
    readonly kind: 'raydium_v4' | 'raydium_cpmm';
    /** Real SOL reserve, lamports. */
    readonly solReserve: bigint;
    /** Locked depth `D` (ARCH §2.6) and the locked share λ in bps. */
    readonly lockedDepth: bigint;
    readonly lambdaBps: number;
    /** One-way fee φ in ppm. */
    readonly feePpm: number;
    readonly eligible: boolean;
    /** Program error names it would fail with (`PoolIneligible`, `PoolTooThin`, `VenueUpgraded`). */
    readonly reasons: readonly string[];
    /** `create_market`'s remaining accounts with this candidate (detection set ++ its ReadSet). */
    readonly remainingAccounts: readonly AccountMeta[];
}

/** `ExtrasSel` as `open_short` / `close_short` data take it (ARCH §3.0, A41). */
export interface ExtrasSelection {
    readonly set: number;
    readonly bumps: Uint8Array;
}

/** One selectable extras set: the selection and the trailing metas (the `VenueExtras` account first). */
export interface ExtrasChoice {
    readonly sel: ExtrasSelection;
    readonly metas: readonly AccountMeta[];
    /** `current`, `none`, `previous` or `stable`. */
    readonly label: 'none' | 'current' | 'previous' | 'stable';
}

/** Everything a trading instruction needs from the venue at one slot. */
export interface TradeContext {
    readonly venue: VenueName;
    /** `ReadSet ++ TradeExtras ++ Extras` for `open_short` / `close_short` (ARCH §3.0). */
    readonly remainingAccounts: readonly AccountMeta[];
    /** The extras set `remainingAccounts` ends with (the instruction data's `ExtrasSel`). */
    readonly extras: ExtrasChoice;
    /**
     * Every usable set in fallback order (`current`, then `none`, `previous`, `stable`; §5.1): on
     * `BadExtras` / `BadRemainingAccounts` the plan is rebuilt with the next one and re-simulated.
     */
    readonly extrasCandidates: readonly ExtrasChoice[];
    /** Idempotent preparations (e.g. PumpSwap T11 `create_idempotent`, ARCH §5.4 case 4). */
    readonly preInstructions: readonly Instruction[];
    /** Reserves the quote used: token X, SOL Y (the curve's virtual reserves on a curve). */
    readonly reserves: { readonly x: bigint; readonly y: bigint };
    readonly feePpm: number;
    /** The RPC context slot of the reads. */
    readonly slot: bigint;
}

export class NotImplementedError extends Error {
    constructor(what: string) {
        super(`${what} is not implemented yet (SHORT track, FRONTEND.md §12)`);
        this.name = 'NotImplementedError';
    }
}
