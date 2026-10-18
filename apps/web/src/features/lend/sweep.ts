/**
 * The sweep bundle a depositor reviews (FRONTEND.md §3.6; ARCH §3.3, §3.9; R4A-7): after a
 * full wipe, last lender's exit or dust flush clears the sweep bits, `deposit` refuses `WipeSweepPending`
 * until `sweep_trader` has run — and, on the pump venues and on every market with the trader's
 * pump UVA (a curve market that left pump, AIR-05; `rewardsRequired`), until the trader's venue
 * cashback is claimed (`wipeSweepPending`). Every step is permissionless. Surviving shares require
 * a standalone cleanup; an empty pool can bundle cleanup with deposit. `sweep_trader` names the
 * wipe record only while one is live.
 *
 * A TypeScript port of the keeper's `services/keeper/src/sweep.rs` (`post_wipe_ixs`, `claim_ixs`,
 * `sweep_trader_ix`, the pump / PumpSwap `claim_cashback` and PumpSwap's
 * `init_user_volume_accumulator`), with the same accounts, roles and order:
 *
 * | trader side | instructions |
 * |---|---|
 * | nothing claimable | `sweep_trader` |
 * | pump UVA above rent (any venue: pump's claim is venue-independent) | pump `claim_cashback(user = trader)`, `sweep_trader` |
 * | pAMM UVA's WSOL ATA non-empty (PumpSwap) | [pAMM `init_user_volume_accumulator` if the UVA is missing], `create_idempotent(ATA(trader, WSOL))`, pAMM `claim_cashback`, `sweep_trader` |
 *
 * The venue instructions are hand-encoded as the keeper's are (pump and PumpSwap have no Codama
 * client here); these instructions are in `tx/allowlist.ts` for exactly this bundle.
 */
import { getSweepTraderInstructionAsync, type Market } from '@shortwin/client';
import {
    type AccountMeta,
    AccountRole,
    type Address,
    getAddressEncoder,
    getBase64Encoder,
    getProgramDerivedAddress,
    type Instruction,
    type TransactionSigner,
} from '@solana/kit';
import { getCreateAssociatedTokenIdempotentInstruction } from '@solana-program/token';
import { findAssociatedTokenPda } from '@solana-program/token-2022';
import {
    PUMP_AMM_PROGRAM,
    PUMP_PROGRAM,
    SHORTWIN_PROGRAM,
    SYSTEM_PROGRAM,
    TOKEN_PROGRAM,
    WSOL_MINT,
} from '../../config/programs.ts';
import type { AppRpc } from '../../rpc/client.ts';
import { eventAuthority } from '../../rpc/pda.ts';
import { tokenAmount } from '../../tx/tokenAccount.ts';
import {
    IX_CLAIM_CASHBACK,
    IX_INIT_USER_VOLUME_ACCUMULATOR,
    PUMP_AMM_EVENT_AUTHORITY,
    PUMP_EVENT_AUTHORITY,
} from '../../venues/constants.ts';
import { rewardsRequired, wipeSweepPending } from './previews.ts';

export { IX_CLAIM_CASHBACK, IX_INIT_USER_VOLUME_ACCUMULATOR, PUMP_AMM_EVENT_AUTHORITY, PUMP_EVENT_AUTHORITY };

/** `Market.pool_cache` slots while the venue is PumpSwap (`state::market::pump_swap_cache`). */
const PAMM_UVA = 4;
const PAMM_UVA_QUOTE_ATA = 5;
const DEFAULT_ADDRESS = '11111111111111111111111111111111';
const VENUE_PUMP_SWAP = 3;
const PUMP_KINDS = new Set([1, 2, 3]);

const ro = (a: Address): AccountMeta => ({ address: a, role: AccountRole.READONLY });
const rw = (a: Address): AccountMeta => ({ address: a, role: AccountRole.WRITABLE });

/** `PDA(program, ["user_volume_accumulator", user])` (pump and PumpSwap use the same seed). */
export async function userVolumeAccumulator(program: Address, user: Address): Promise<Address> {
    const [pda] = await getProgramDerivedAddress({
        programAddress: program,
        seeds: ['user_volume_accumulator', getAddressEncoder().encode(user)],
    });
    return pda;
}

/** `ATA(owner, WSOL)` under SPL Token. */
export async function wsolAta(owner: Address): Promise<Address> {
    const [ata] = await findAssociatedTokenPda({ owner, mint: WSOL_MINT, tokenProgram: TOKEN_PROGRAM });
    return ata;
}

const isDefault = (a: Address) => a === DEFAULT_ADDRESS;

/** The trader's `ATA(trader, WSOL)`: cached at listing, else derived (`pump_curve::trader_wsol_ata`). */
export async function traderWsolAta(market: Pick<Market, 'trader' | 'traderWsolAta'>): Promise<Address> {
    return isDefault(market.traderWsolAta) ? wsolAta(market.trader) : market.traderWsolAta;
}

/** The trader's PumpSwap UVA: the PumpSwap cache, else derived (`keeper venue::uva_of`). */
export async function pammUva(market: Pick<Market, 'trader' | 'poolCache'>): Promise<Address> {
    const c = market.poolCache[PAMM_UVA];
    return c && !isDefault(c) ? c : userVolumeAccumulator(PUMP_AMM_PROGRAM, market.trader);
}

/** `ATA(pAMM UVA(trader), WSOL)`, where PumpSwap accrues the trader's cashback (`sweep::pamm_cashback_ata`). */
export async function pammCashbackAta(market: Pick<Market, 'trader' | 'poolCache'>): Promise<Address> {
    const c = market.poolCache[PAMM_UVA_QUOTE_ATA];
    return c && !isDefault(c) ? c : wsolAta(await pammUva(market));
}

/** What the trader side of a market holds (`keeper sweep::SweepFacts`, the cashback part). */
export interface SweepFacts {
    /** The trader's pump UVA's lamports above its rent-exempt minimum. */
    readonly pumpClaimable: bigint;
    /** WSOL in `ATA(pAMM UVA(trader), WSOL)` (PumpSwap markets only). */
    readonly pammClaimable: bigint;
    /** The trader's PumpSwap UVA exists (pAMM's `claim_cashback` fails 3012 without it). */
    readonly pammUvaExists: boolean;
}

export const NO_CASHBACK: SweepFacts = { pumpClaimable: 0n, pammClaimable: 0n, pammUvaExists: true };

/**
 * Reads the cashback facts of a market that waits for the venue-reward sweep (`rewardsRequired`: a
 * pump venue, or since AIR-05 a market that kept the trader's pump UVA after leaving pump; nothing
 * to read elsewhere). Off the pump venues only that UVA is read (the keeper's `SweepFacts::of`
 * reads its claimable cashback on any venue; PumpSwap's only on PumpSwap).
 */
export async function readSweepFacts(rpc: AppRpc, market: Market): Promise<SweepFacts> {
    if (!rewardsRequired(market)) return NO_CASHBACK;
    const pump = PUMP_KINDS.has(market.venue);
    const pamm = market.venue === VENUE_PUMP_SWAP;
    const keys = pump ? [market.pumpUva, await pammUva(market), await pammCashbackAta(market)] : [market.pumpUva];
    const { value } = await rpc.getMultipleAccounts(keys, { encoding: 'base64', commitment: 'confirmed' }).send();
    const [pumpUva, uva, ata] = value;
    let pumpClaimable = 0n;
    if (pumpUva && pumpUva.owner === PUMP_PROGRAM) {
        const len = getBase64Encoder().encode(pumpUva.data[0]).length;
        const floor = await rpc.getMinimumBalanceForRentExemption(BigInt(len)).send();
        const lamports = BigInt(pumpUva.lamports);
        pumpClaimable = lamports > floor ? lamports - floor : 0n;
    }
    let pammClaimable = 0n;
    if (pamm && ata && ata.owner === TOKEN_PROGRAM) {
        pammClaimable = tokenAmount(new Uint8Array(getBase64Encoder().encode(ata.data[0]))) ?? 0n;
    }
    // Off the pump venues no PumpSwap account is read, and no PumpSwap claim is built.
    const pammUvaExists =
        !pump || (!!uva && uva.owner === PUMP_AMM_PROGRAM && getBase64Encoder().encode(uva.data[0]).length > 0);
    return { pumpClaimable, pammClaimable, pammUvaExists };
}

/** pump `claim_cashback(user)`: `[user W, UVA W, System, event authority, pump]`; `user` does not sign. */
export async function pumpClaimCashbackIx(user: Address): Promise<Instruction> {
    return {
        programAddress: PUMP_PROGRAM,
        accounts: [
            rw(user),
            rw(await userVolumeAccumulator(PUMP_PROGRAM, user)),
            ro(SYSTEM_PROGRAM),
            ro(PUMP_EVENT_AUTHORITY),
            ro(PUMP_PROGRAM),
        ],
        data: IX_CLAIM_CASHBACK,
    };
}

/** PumpSwap `init_user_volume_accumulator(payer, user)`: `[payer S W, user, UVA W, System, event authority, pAMM]`. */
export async function pammInitUvaIx(payer: TransactionSigner, user: Address): Promise<Instruction> {
    return {
        programAddress: PUMP_AMM_PROGRAM,
        accounts: [
            { address: payer.address, role: AccountRole.WRITABLE_SIGNER, signer: payer } as AccountMeta,
            ro(user),
            rw(await userVolumeAccumulator(PUMP_AMM_PROGRAM, user)),
            ro(SYSTEM_PROGRAM),
            ro(PUMP_AMM_EVENT_AUTHORITY),
            ro(PUMP_AMM_PROGRAM),
        ],
        data: IX_INIT_USER_VOLUME_ACCUMULATOR,
    };
}

/**
 * PumpSwap `claim_cashback(user, user_wsol)`: `[user W, UVA W, WSOL, SPL Token, ATA(UVA, WSOL) W,
 * user_wsol W, System, event authority, pAMM]`; `user` does not sign.
 */
export async function pammClaimCashbackIx(user: Address, uva: Address, userWsol: Address): Promise<Instruction> {
    return {
        programAddress: PUMP_AMM_PROGRAM,
        accounts: [
            rw(user),
            rw(uva),
            ro(WSOL_MINT),
            ro(TOKEN_PROGRAM),
            rw(await wsolAta(uva)),
            rw(userWsol),
            ro(SYSTEM_PROGRAM),
            ro(PUMP_AMM_EVENT_AUTHORITY),
            ro(PUMP_AMM_PROGRAM),
        ],
        data: IX_CLAIM_CASHBACK,
    };
}

/**
 * `sweep_trader()` of the market (`sweep::sweep_trader_ix`): the fixed accounts, then on a pump
 * venue its token leg `[trader_base_ata W, pool_vault W, mint, token program, pump UVA]` (+ on
 * PumpSwap `ATA(pAMM UVA(trader), WSOL)`) — off the pump venues the trader's pump UVA alone (read
 * only) when the market has one (AIR-05: a curve market that left pump; the program's
 * `TOKEN_LEG_LEN_UVA`, key-checked against `Market.pump_uva`) —, then the live wipe record (W)
 * while one is live.
 */
export async function sweepTraderIx(
    marketAddress: Address,
    market: Market,
    liveRecord: Address | null,
): Promise<Instruction> {
    const ix = await getSweepTraderInstructionAsync({
        market: marketAddress,
        marketVault: market.marketVault,
        trader: market.trader,
        traderWsol: market.traderWsol,
        traderWsolAta: await traderWsolAta(market),
        eventAuthority: await eventAuthority(),
        program: SHORTWIN_PROGRAM,
    });
    const extra: AccountMeta[] = [];
    if (PUMP_KINDS.has(market.venue)) {
        extra.push(
            rw(market.traderBaseAta),
            rw(market.poolVault),
            ro(market.mint),
            ro(market.tokenProgram),
            ro(market.pumpUva),
        );
        if (market.venue === VENUE_PUMP_SWAP) extra.push(ro(await pammCashbackAta(market)));
    } else if (!isDefault(market.pumpUva)) {
        // AIR-05: a market that left pump keeps its trader's pump UVA (curve-era cashback).
        extra.push(ro(market.pumpUva));
    }
    if (liveRecord) extra.push(rw(liveRecord));
    return { ...ix, accounts: [...ix.accounts, ...extra] };
}

/** The claims to prepend to `sweep_trader` (`sweep::claim_ixs`). */
export async function cashbackClaimIxs(
    payer: TransactionSigner,
    market: Market,
    facts: SweepFacts,
): Promise<Instruction[]> {
    const out: Instruction[] = [];
    if (facts.pumpClaimable > 0n) out.push(await pumpClaimCashbackIx(market.trader));
    if (facts.pammClaimable > 0n) {
        const uva = await pammUva(market);
        if (!facts.pammUvaExists) out.push(await pammInitUvaIx(payer, market.trader));
        out.push(
            getCreateAssociatedTokenIdempotentInstruction({
                payer,
                ata: await wsolAta(market.trader),
                owner: market.trader,
                mint: WSOL_MINT,
                tokenProgram: TOKEN_PROGRAM,
            }),
        );
        out.push(await pammClaimCashbackIx(market.trader, uva, await traderWsolAta(market)));
    }
    return out;
}

/** The whole post-wipe bundle (`sweep::post_wipe_ixs`), or nothing when no sweep is pending. */
export async function postWipeSweepIxs(
    payer: TransactionSigner,
    marketAddress: Address,
    market: Market,
    facts: SweepFacts,
    liveRecord: Address | null,
): Promise<Instruction[]> {
    if (!wipeSweepPending(market)) return [];
    return [...(await cashbackClaimIxs(payer, market, facts)), await sweepTraderIx(marketAddress, market, liveRecord)];
}
