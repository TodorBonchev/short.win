/**
 * Pump detection (ARCH §2.5 `classify_pump`, D1; `programs/shortwin/src/venues/detect.rs`) and the
 * remaining accounts an `Auto` listing passes (`create_market`: the detection set, then on a pump
 * venue the init set of the trader's user volume accumulator). Classification is pure, on the
 * accounts as read; `detectVenue` in `index.ts` reads them.
 */
import { findMarketPda, findTraderPda } from '@shortwin/client';
import { type AccountMeta, AccountRole, type Address, type Instruction, type TransactionSigner } from '@solana/kit';
import { DEFAULT_KEY, hasDiscriminator } from './bytes.ts';
import {
    CURVE_MIN_LEN,
    PUMP_AMM_EVENT_AUTHORITY,
    PUMP_AMM_PROGRAM,
    PUMP_EVENT_AUTHORITY,
    PUMP_IX_EXTEND_ACCOUNT,
    PUMP_PROGRAM,
    SYSTEM_PROGRAM,
} from './constants.ts';
import { pammUserVolumeAccumulator, pumpUserVolumeAccumulator } from './pda.ts';
import { BONDING_CURVE_DISCRIMINATOR, type RawAccount, readCurve } from './readers.ts';
import { canonicalPoolState } from './state.ts';
import type { VenueDetection } from './types.ts';

export type PumpClass =
    | 'not_pump'
    | 'pump_curve'
    | 'pump_migrating'
    | 'pump_swap'
    | 'pump_pool_unreadable'
    | 'graduated_elsewhere';

/** `classify_pump`: the class, or the program error a listing fails with. */
export function classifyPump(
    bc: RawAccount | null,
    cp: RawAccount | null,
    mint: Address,
    authority: Address,
): { readonly class: PumpClass } | { readonly error: string } {
    if (!bc || bc.owner !== PUMP_PROGRAM || !hasDiscriminator(bc.data, BONDING_CURVE_DISCRIMINATOR)) {
        return { class: 'not_pump' };
    }
    const c = readCurve(bc.owner, bc.data);
    if (!c.ok) return { error: 'PumpVenuePrecedence' };
    if (c.value.isMayhemMode) return { error: 'CurveMayhemUnsupported' };
    if (c.value.quoteMint !== DEFAULT_KEY) return { error: 'CurveQuoteNotSol' };
    if (bc.data.length < CURVE_MIN_LEN) return { error: 'CurveNeedsExtend' };
    if (!c.value.complete) {
        // pump-final §6.3 (owner D1 (a)), on the live branch only: pump's v3 trades refuse a cashback
        // coin (6094), so a live cashback curve is refused until it graduates; a complete one lists as
        // before (PumpMigrating, PumpSwap). A SOL curve has depth 0 (defence in depth).
        if (c.value.isCashbackCoin) return { error: 'CurveCashbackCoin' };
        if (c.value.depth !== 0) return { error: 'CurveQuoteNotSol' };
        return { class: 'pump_curve' };
    }
    const pool = canonicalPoolState(cp, mint, authority).state;
    if (pool === 'valid') return { class: 'pump_swap' };
    if (pool === 'unreadable') return { class: 'pump_pool_unreadable' };
    return { class: c.value.vt > 0n ? 'pump_migrating' : 'graduated_elsewhere' };
}

/** `D1`: the pump venues take precedence over a Raydium listing. */
export function pumpVenue(c: PumpClass): boolean {
    return c === 'pump_curve' || c === 'pump_migrating' || c === 'pump_swap' || c === 'pump_pool_unreadable';
}

/** pump `extend_account` (R-PUMP §3.5): `[account W, user S W, system, event_authority, program]`, data the discriminator. */
export function pumpExtendAccountIx(curve: Address, user: TransactionSigner): Instruction {
    return {
        programAddress: PUMP_PROGRAM,
        accounts: [
            { address: curve, role: AccountRole.WRITABLE },
            { address: user.address, role: AccountRole.WRITABLE_SIGNER, signer: user } as AccountMeta,
            { address: SYSTEM_PROGRAM, role: AccountRole.READONLY },
            { address: PUMP_EVENT_AUTHORITY, role: AccountRole.READONLY },
            { address: PUMP_PROGRAM, role: AccountRole.READONLY },
        ],
        data: new Uint8Array(PUMP_IX_EXTEND_ACCOUNT),
    };
}

/**
 * What an `Auto` listing of `mint` would detect, from the detection set as read. `payer` (the
 * listing's creator) is needed only for a curve shorter than 115 bytes: with it the detection
 * carries pump's `extend_account` to prepend; without it the refusal is `CurveNeedsExtend`.
 */
export async function detectFrom(
    mint: Address,
    keys: { readonly bondingCurve: Address; readonly canonicalPool: Address; readonly authority: Address },
    bc: RawAccount | null,
    cp: RawAccount | null,
    payer?: TransactionSigner,
): Promise<VenueDetection> {
    const detection: AccountMeta[] = [
        { address: keys.bondingCurve, role: AccountRole.READONLY },
        { address: keys.canonicalPool, role: AccountRole.READONLY },
    ];
    const base = {
        bondingCurve: keys.bondingCurve,
        canonicalPool: keys.canonicalPool,
        preInstructions: [] as Instruction[],
    };
    let cls = classifyPump(bc, cp, mint, keys.authority);
    const preInstructions: Instruction[] = [];
    if ('error' in cls && cls.error === 'CurveNeedsExtend' && payer) {
        // With the curve extended first, the listing classifies it again.
        preInstructions.push(pumpExtendAccountIx(keys.bondingCurve, payer));
        const extended = bc ? { ...bc, data: padTo(bc.data, CURVE_MIN_LEN) } : null;
        cls = classifyPump(extended, cp, mint, keys.authority);
    }
    if ('error' in cls) {
        return { ...base, kind: 'none', refusal: cls.error, remainingAccounts: detection, preInstructions };
    }
    const [market] = await findMarketPda({ mint });
    const [trader] = await findTraderPda({ market });
    switch (cls.class) {
        case 'pump_curve':
        case 'pump_migrating':
            return {
                ...base,
                kind: cls.class,
                refusal: null,
                remainingAccounts: [
                    ...detection,
                    { address: PUMP_PROGRAM, role: AccountRole.READONLY },
                    { address: await pumpUserVolumeAccumulator(trader), role: AccountRole.WRITABLE },
                    { address: PUMP_EVENT_AUTHORITY, role: AccountRole.READONLY },
                ],
                preInstructions,
            };
        case 'pump_swap':
            return {
                ...base,
                kind: 'pump_swap',
                refusal: null,
                remainingAccounts: [
                    ...detection,
                    { address: PUMP_AMM_PROGRAM, role: AccountRole.READONLY },
                    { address: await pammUserVolumeAccumulator(trader), role: AccountRole.WRITABLE },
                    { address: PUMP_AMM_EVENT_AUTHORITY, role: AccountRole.READONLY },
                ],
                preInstructions,
            };
        default:
            // NotPump, GraduatedElsewhere and an unreadable canonical pool list as `None`.
            return { ...base, kind: 'none', refusal: null, remainingAccounts: detection, preInstructions };
    }
}

function padTo(d: Uint8Array, len: number): Uint8Array {
    if (d.length >= len) return d;
    const out = new Uint8Array(len);
    out.set(d);
    return out;
}
