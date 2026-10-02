/**
 * `create_market` (ARCH §3.2; FRONTEND.md §3.5, §5.10): the listing choice, the cost of the new
 * accounts, and the `TxPlan` the pipeline simulates and signs.
 *
 * The instruction comes from the Codama builder (no hand-encoded data); the remaining accounts and
 * any instruction to prepend (pump `extend_account` for a curve under 115 bytes) from the venue
 * layer (`detectVenue` / `raydiumCandidates`, SHORT): `[bonding_curve, canonical_pool]` then the
 * pump init set or the Raydium candidate's ReadSet. `max_creation_fee` is the fee read from
 * `GlobalConfig`, so a fee raised between the review and execution fails `CreationFeeAboveMax`.
 */
import {
    findMarketPda,
    findTraderPda,
    getCreateMarketInstructionAsync,
    getMarketSize,
    getMarketVaultSize,
} from '@shortwin/client';
import type { Address, Instruction, TransactionSigner } from '@solana/kit';
import { findAssociatedTokenPda } from '@solana-program/token';
import type { QueryKey } from '@tanstack/react-query';
import { qk } from '../../../api/keys.ts';
import type { VenueName } from '../../../api/types.ts';
import { SHORTWIN_PROGRAM } from '../../../config/programs.ts';
import { formatSol } from '../../../lib/format.ts';
import { eventAuthority } from '../../../rpc/pda.ts';
import type { TxPlan } from '../../../tx/types.ts';
import { type PoolCandidate, VENUE_HINT, type VenueDetection } from '../../../venues/types.ts';

/** `POOL_VAULT_LEN` (create_market.rs): a plain token account. */
export const POOL_VAULT_LEN = 165;
/** The trader's pump / PumpSwap user-volume accumulator (`USER_VOLUME_ACCUMULATOR_LEN`). */
export const UVA_LEN = 137;
/** An ATA: SPL Token 165 B; Token-2022 adds the account type and `ImmutableOwner` (170 B). */
export const ATA_LEN = { token: 165, 'token-2022': 170 } as const;

/** What the user lists the mint as. */
export type ListingChoice =
    | { readonly kind: 'auto'; readonly detection: VenueDetection }
    | { readonly kind: 'raydium'; readonly candidate: PoolCandidate };

/** The venue the market will have after the listing. */
export function listingVenue(choice: ListingChoice): VenueName {
    return choice.kind === 'auto' ? choice.detection.kind : choice.candidate.kind;
}

export function venueHint(choice: ListingChoice): number {
    return choice.kind === 'auto' ? VENUE_HINT.auto : VENUE_HINT[choice.candidate.kind];
}

export interface CostItem {
    readonly key: 'market' | 'vault' | 'pool-vault' | 'trader-ata' | 'trader-uva';
    readonly label: string;
    readonly size: number;
}

/**
 * The accounts `create_market` creates and the creator pays rent for: `Market`, `MarketVault` and
 * the pool vault always; on the pump venues also the trader's base ATA and its user-volume
 * accumulator (each skipped by the program if it already exists, so these two are an upper bound).
 */
export function creationAccounts(venue: VenueName, program: 'token' | 'token-2022'): CostItem[] {
    const items: CostItem[] = [
        { key: 'market', label: 'Market account', size: getMarketSize() },
        { key: 'vault', label: 'Market vault', size: getMarketVaultSize() },
        { key: 'pool-vault', label: 'Pool vault (token account)', size: POOL_VAULT_LEN },
    ];
    if (venue === 'pump_curve' || venue === 'pump_migrating' || venue === 'pump_swap') {
        items.push(
            { key: 'trader-ata', label: 'Trading token account', size: ATA_LEN[program] },
            { key: 'trader-uva', label: 'Venue volume account', size: UVA_LEN },
        );
    }
    return items;
}

export interface CreationCost {
    readonly fee: bigint;
    readonly rent: bigint;
    readonly total: bigint;
}

export function creationCost(fee: bigint, rents: readonly bigint[]): CreationCost {
    const rent = rents.reduce((a, b) => a + b, 0n);
    return { fee, rent, total: fee + rent };
}

export interface CreatePlanArgs {
    readonly signer: TransactionSigner;
    readonly mint: Address;
    /** The mint's owner (SPL Token or Token-2022; `create_market` requires `token_program == mint.owner`). */
    readonly tokenProgram: Address;
    readonly choice: ListingChoice;
    /** `GlobalConfig.params.market_creation_fee` as read: becomes `max_creation_fee`. */
    readonly creationFee: bigint;
    readonly cost: CreationCost;
    /** A short label for the title (the symbol, else the short mint). */
    readonly label: string;
}

/** Every indexer list of markets (the WebSocket patches rows but cannot add a new one). */
const MARKET_LISTS: QueryKey = qk.markets({}).slice(0, 2);

export async function buildCreateMarketPlan(a: CreatePlanArgs): Promise<TxPlan> {
    const [market] = await findMarketPda({ mint: a.mint });
    const [trader] = await findTraderPda({ market });
    const [traderBaseAta] = await findAssociatedTokenPda({ owner: trader, mint: a.mint, tokenProgram: a.tokenProgram });
    const ix = await getCreateMarketInstructionAsync({
        creator: a.signer,
        mint: a.mint,
        traderBaseAta,
        tokenProgram: a.tokenProgram,
        eventAuthority: await eventAuthority(),
        program: SHORTWIN_PROGRAM,
        venueHint: venueHint(a.choice),
        maxCreationFee: a.creationFee,
    });
    const remaining =
        a.choice.kind === 'auto' ? a.choice.detection.remainingAccounts : a.choice.candidate.remainingAccounts;
    const pre: readonly Instruction[] = a.choice.kind === 'auto' ? a.choice.detection.preInstructions : [];
    const createIx: Instruction = { ...ix, accounts: [...ix.accounts, ...remaining] };
    const fee = `${formatSol(a.cost.fee, 6)} SOL`;
    const rent = `${formatSol(a.cost.rent, 6)} SOL`;
    return {
        kind: 'create_market',
        title: `Create market · ${a.label}`,
        instructions: [...pre, createIx],
        watch: [
            { kind: 'lamports', address: a.signer.address, label: 'Your SOL' },
            { kind: 'lamports', address: market, label: 'Market account (rent)' },
        ],
        disclosures: ['market.create'],
        disclosureVars: { 'market.create': { fee, rent } },
        // Also what a new review re-reads after a refusal (the venue detection under the market key,
        // the creation fee in `GlobalConfig`; M9-UX-1).
        invalidate: [
            qk.marketAccount(a.mint),
            qk.market(a.mint),
            [...qk.account(a.mint), 'raw'],
            qk.config(),
            MARKET_LISTS,
        ],
    };
}
