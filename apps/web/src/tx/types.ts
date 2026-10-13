/**
 * The transaction pipeline's frozen interface (FRONTEND.md §5). A feature builds a `TxPlan`
 * (instructions from the Codama builders, the accounts whose changes the review shows, the
 * disclosures to acknowledge); `useTxFlow` simulates it, shows the `SimulationReport`, asks the
 * wallet to sign only after the user confirms, sends, confirms and invalidates the plan's queries.
 */
import type { Address, Instruction } from '@solana/kit';
import type { QueryKey } from '@tanstack/react-query';
import type { DisclosureId } from '../config/disclosures.ts';
import type { ErrorAction } from '../config/errors.ts';

export type TxKind =
    | 'create_market'
    | 'deposit'
    | 'withdraw'
    | 'claim_sol'
    | 'cover_deficit'
    | 'sync_lender'
    | 'open_short'
    | 'close_short'
    | 'add_margin'
    | 'sweep_trader';

/** How the review reads an account's before / after state. */
export type WatchedAccount =
    | {
          readonly kind: 'lamports';
          readonly address: Address;
          readonly label: string;
      }
    | {
          readonly kind: 'token';
          readonly address: Address;
          readonly label: string;
          readonly decimals: number;
          readonly symbol: string;
      }
    | {
          readonly kind: 'program';
          readonly address: Address;
          readonly label: string;
          /** Decodes the account into named integer fields (e.g. `q`, `v`, `shares`); `null` if absent. */
          readonly decode: (data: Uint8Array) => Readonly<Record<string, bigint>>;
          /** Per field: display unit and scale (`SOL` 9, a token's decimals, `bps` 2, `raw` 0). */
          readonly fields: Readonly<
              Record<
                  string,
                  {
                      readonly label: string;
                      readonly unit: string;
                      readonly scale: number;
                      /** `timestamp`: unix seconds, shown as a UTC date (no change column). */
                      readonly kind?: DeltaKind;
                  }
              >
          >;
      };

export interface TxPlan {
    readonly kind: TxKind;
    /** One line for the review header and the toast, e.g. "Open short · 1,000,000 BONK". */
    readonly title: string;
    /** Instructions without compute-budget ones (the pipeline adds them). */
    readonly instructions: readonly Instruction[];
    readonly watch: readonly WatchedAccount[];
    /** Disclosures shown in the review; those with an `ack` text gate the Sign button. */
    readonly disclosures: readonly DisclosureId[];
    readonly disclosureVars?: Partial<Record<DisclosureId, Readonly<Record<string, string>>>>;
    /** Address lookup tables to compress with (the protocol ALT is added by the pipeline). */
    readonly lookupTables?: readonly Address[];
    /** Queries to invalidate once the transaction is confirmed. */
    readonly invalidate: readonly QueryKey[];
    /**
     * Rebuilds the plan from the chain as it is now, with the same user inputs: a fresh read, a new
     * quote and new bounds (`min_*` / `max_*`), and the steps re-derived (memo flag, epoch records,
     * the sweep bundle, the venue accounts). The review's Retry uses it after a refusal that a
     * fresh plan can fix (FRONTEND.md §5.3); without it the review offers "Close and review again".
     */
    readonly rebuild?: () => Promise<TxPlan>;
    /**
     * Rewrites the message of a failed simulation with what the plan knows and the bare error code
     * does not (e.g. the venue's state at the build: an open's `ProceedsBelowMin` on a pump account
     * its first trade grows, PUMP-2026-10-2). Never changes the kind, the code or the action.
     */
    readonly refineError?: (error: TxError) => TxError;
    /**
     * Why a successful simulation must still not be signed (one line beside the disabled Sign; the
     * review's Refresh simulates again), or `null`: e.g. a claim whose simulation pays 0 lamports
     * reads "Nothing to claim yet" (the owner's decision of 2026-10-11, FRONTEND.md §3.6).
     */
    readonly holdSign?: (report: SimulationReport) => string | null;
    /**
     * Where the user goes next once it is confirmed (e.g. "View position"), shown in the review's
     * confirmed banner (`tx-confirmed-link`); without it the banner links the explorer.
     */
    readonly next?: { readonly href: string; readonly text: string };
}

/** `amount` (default): a scaled integer with a unit; `timestamp`: unix seconds, shown as a date. */
export type DeltaKind = 'amount' | 'timestamp';

export interface Delta {
    readonly key: string;
    readonly label: string;
    readonly before: bigint | null;
    readonly after: bigint | null;
    readonly unit: string;
    readonly scale: number;
    readonly kind?: DeltaKind;
}

export type TxErrorKind =
    | 'program'
    | 'token'
    | 'wallet-rejected'
    | 'wallet'
    | 'rpc'
    | 'funds'
    | 'size'
    | 'integrity'
    | 'expired'
    /** Sent, but not seen confirmed in time: it may still land (no blind retry). */
    | 'unknown-outcome'
    /** The plan could not be rebuilt from the chain as it is now (the inputs no longer fit). */
    | 'plan'
    /**
     * The Mobile Wallet Adapter could not launch the wallet app at Sign (the review stays open
     * with a retry notice; never a `failed` state).
     */
    | 'wallet-launch'
    /** Chrome's local-network permission is missing for the Mobile Wallet Adapter (a notice, as above). */
    | 'local-network'
    /**
     * The connected Mobile Wallet Adapter account changed or disconnected while it signed (R-S2):
     * "Connect a wallet that can sign.", retried by closing the review.
     */
    | 'wallet-changed'
    | 'unknown';

export interface TxError {
    readonly kind: TxErrorKind;
    readonly message: string;
    readonly action: ErrorAction;
    /** The custom error code (short.win 6000–6105, or a token program code). */
    readonly code?: number;
    readonly name?: string;
    /** Index of the failing instruction in the transaction. */
    readonly instructionIndex?: number;
    readonly logs?: readonly string[];
}

export interface SimulationReport {
    readonly ok: boolean;
    readonly unitsConsumed: bigint | null;
    readonly computeUnitLimit: number;
    readonly priorityFeeMicroLamports: bigint;
    /** Base fee + priority fee, lamports. */
    readonly feeLamports: bigint;
    readonly sizeBytes: number;
    readonly deltas: readonly Delta[];
    readonly logs: readonly string[];
    readonly error: TxError | null;
    readonly simulatedAt: number;
}

export type TxStage = 'idle' | 'simulating' | 'review' | 'signing' | 'confirming' | 'confirmed' | 'failed';
