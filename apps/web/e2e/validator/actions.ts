/**
 * Node-side actions the specs take on the chain between UI steps (FRONTEND.md §10.2–§10.3): move a
 * mock pool's price, flip CPI Guard or required memos on a wallet's token account (the race tests),
 * set a coverage pause, donate to a market's trader and sweep it (vesting income), and readers for
 * positions, lenders and landed transactions. Every key is a generated E2E key.
 */
import {
    getCloseShortInstructionAsync,
    getDepositInstructionAsync,
    getLenderPositionDecoder,
    getSetCoveragePauseInstructionAsync,
    getShortPositionDecoder,
    getSweepTraderInstructionAsync,
    type LenderPosition,
    SHORTWIN_PROGRAM_ADDRESS,
    type ShortPosition,
} from '@shortwin/client';
import type { Address, Signature } from '@solana/kit';
import { getTransferSolInstruction } from '@solana-program/system';
import {
    getCreateAssociatedTokenIdempotentInstructionAsync,
    getMintToInstruction,
    getSyncNativeInstruction,
} from '@solana-program/token';
import {
    getDisableCpiGuardInstruction,
    getDisableMemoTransfersInstruction,
    getEnableCpiGuardInstruction,
    getEnableMemoTransfersInstruction,
} from '@solana-program/token-2022';
import { ata, eventAuthority, getAccount, PROGRAMS, pda, rpc, send, WSOL_MINT } from './chain.ts';
import { signerFor, type WalletName } from './keys.ts';
import { derivePool, reserves, swapExactInIx } from './mock-cpamm.ts';
import { mockTradeAccounts, openShortFromNode } from './seed.ts';
import type { MarketInfo } from './state.ts';

/**
 * Moves a mock market's spot price by about `bps` (positive: up — someone buys tokens with SOL;
 * negative: down — someone sells tokens), with a constant-product swap from the pool admin.
 * Returns the new reserves. `crank_price` is not called: the EMA moves only on the next
 * short.win instruction that observes the market.
 */
export async function movePrice(m: MarketInfo, bps: number): Promise<{ x: bigint; y: bigint }> {
    if (!m.pool) throw new Error(`${m.symbol} is not a mock market`);
    const p = await derivePool(m.mint, m.tokenProgram);
    const { x, y } = await reserves(p);
    const admin = await signerFor('poolAdmin');
    const payer = await signerFor('payer');
    const baseAta = await ata(admin.address, m.mint, m.tokenProgram);
    const wsolAta = await ata(admin.address, WSOL_MINT, PROGRAMS.token);
    const prep = [
        await getCreateAssociatedTokenIdempotentInstructionAsync({
            payer,
            owner: admin.address,
            mint: m.mint,
            tokenProgram: m.tokenProgram,
        }),
        await getCreateAssociatedTokenIdempotentInstructionAsync({
            payer,
            owner: admin.address,
            mint: WSOL_MINT,
            tokenProgram: PROGRAMS.token,
        }),
    ];
    // P = Y/X; after a swap at constant k = X·Y the price is P·(Y'/Y)², so Y' = Y·√(1 + bps/10⁴).
    const ratio = Math.sqrt(1 + bps / 10_000);
    if (bps > 0) {
        const dy = (y * BigInt(Math.round((ratio - 1) * 1e9))) / 1_000_000_000n;
        await send(
            payer,
            [
                ...prep,
                getTransferSolInstruction({ source: payer, destination: wsolAta, amount: dy }),
                getSyncNativeInstruction({ account: wsolAta }),
                swapExactInIx(p, admin.address, wsolAta, baseAta, true, dy),
            ],
            { signers: [admin] },
        );
    } else if (bps < 0) {
        // X' = X/√(1 + bps/10⁴) for P' = P·(1 + bps/10⁴).
        const dx = (x * BigInt(Math.round((1 / ratio - 1) * 1e9))) / 1_000_000_000n;
        const mintAuthority = await signerFor('mintAuthority');
        await send(
            payer,
            [
                ...prep,
                getMintToInstruction({ mint: m.mint, token: baseAta, mintAuthority, amount: dx }),
                swapExactInIx(p, admin.address, baseAta, wsolAta, false, dx),
            ],
            { signers: [admin] },
        );
    }
    const r = await reserves(p);
    return { x: r.x, y: r.y };
}

/** Turns CPI Guard on or off on a Token-2022 account of `owner` (the account has the extension). */
export async function setCpiGuard(owner: WalletName, token: Address, enabled: boolean): Promise<Signature> {
    const o = await signerFor(owner);
    const ix = enabled
        ? getEnableCpiGuardInstruction({ token, owner: o })
        : getDisableCpiGuardInstruction({ token, owner: o });
    return send(await signerFor('payer'), [ix], { signers: [o] });
}

/** Requires (or stops requiring) memos on incoming transfers of a Token-2022 account of `owner`. */
export async function setRequireMemos(owner: WalletName, token: Address, required: boolean): Promise<Signature> {
    const o = await signerFor(owner);
    const ix = required
        ? getEnableMemoTransfersInstruction({ token, owner: o })
        : getDisableMemoTransfersInstruction({ token, owner: o });
    return send(await signerFor('payer'), [ix], { signers: [o] });
}

/** Reads a Token-2022 account's CPI Guard `lock_cpi` (extension 11) and MemoTransfer flag (extension 8). */
export async function tokenExtensions(
    token: Address,
): Promise<{ lockCpi: boolean | null; requireMemos: boolean | null }> {
    const a = await getAccount(token);
    if (!a) throw new Error(`no token account ${token}`);
    let lockCpi: boolean | null = null;
    let requireMemos: boolean | null = null;
    const d = a.data;
    if (d.length > 166 && d[165] === 2) {
        let o = 166;
        while (o + 4 <= d.length) {
            const type = (d[o] ?? 0) | ((d[o + 1] ?? 0) << 8);
            const len = (d[o + 2] ?? 0) | ((d[o + 3] ?? 0) << 8);
            if (type === 0) break;
            if (type === 11) lockCpi = d[o + 4] === 1;
            if (type === 8) requireMemos = d[o + 4] === 1;
            o += 4 + len;
        }
    }
    return { lockCpi, requireMemos };
}

/** `set_coverage_pause(paused)` by the generated coverage authority (O17). */
export async function setCoveragePause(m: MarketInfo, paused: boolean): Promise<Signature> {
    const coverage = await signerFor('coverage');
    return send(await signerFor('payer'), [
        await getSetCoveragePauseInstructionAsync({
            coverageAuthority: coverage,
            market: m.market,
            eventAuthority: await eventAuthority(),
            program: SHORTWIN_PROGRAM_ADDRESS,
            paused,
            reason: 1,
        }),
    ]);
}

/**
 * Donates `lamports` to the market's `trader` PDA and sweeps it to the lenders (`sweep_trader`,
 * SWEEP_TO_VEST): one-off income that vests over `lump_vest_s` (the vest-forfeit warning).
 */
export async function donateAndSweep(m: MarketInfo, lamports: bigint): Promise<Signature> {
    const payer = await signerFor('payer');
    const ix = await getSweepTraderInstructionAsync({
        market: m.market,
        trader: m.trader,
        traderWsol: m.traderWsol,
        traderWsolAta: await ata(m.trader, WSOL_MINT, PROGRAMS.token),
        eventAuthority: await eventAuthority(),
        program: SHORTWIN_PROGRAM_ADDRESS,
    });
    return send(payer, [getTransferSolInstruction({ source: payer, destination: m.trader, amount: lamports }), ix]);
}

/**
 * Makes sure `owner` has shares in `m` (a spec that withdraws everything can run again on a reused
 * validator): deposits `amount` raw tokens from `source` when the lender holds none.
 */
export async function ensureLent(owner: WalletName, m: MarketInfo, source: Address, amount: bigint): Promise<void> {
    const o = await signerFor(owner);
    if (((await lenderPosition(m.market, o.address))?.shares ?? 0n) > 0n) return;
    await send(o, [
        await getDepositInstructionAsync({
            owner: o,
            mint: m.mint,
            ownerTokenAccount: source,
            tokenProgram: m.tokenProgram,
            eventAuthority: await eventAuthority(),
            program: SHORTWIN_PROGRAM_ADDRESS,
            amount,
            minShares: 0n,
        }),
    ]);
}

/**
 * Rent paid in: the seeding payer (not a spec wallet) opens a small short on mock market `m` and
 * closes it at once, paying at least the minimum rent into the pool — lenders then have SOL to claim
 * (rent is credited continuously but becomes claimable once a borrower pays it in).
 */
export async function payRentIn(m: MarketInfo, qRaw: bigint): Promise<void> {
    const payer = await signerFor('payer');
    const nonce = BigInt(Date.now());
    const p = await openShortFromNode(payer, m, qRaw, nonce);
    const ix = await getCloseShortInstructionAsync({
        borrower: payer,
        mint: m.mint,
        position: p.position,
        trader: m.trader,
        tokenProgram: m.tokenProgram,
        eventAuthority: await eventAuthority(),
        program: SHORTWIN_PROGRAM_ADDRESS,
        maxBuybackCost: 0xffff_ffff_ffff_ffffn,
        extras: { set: 0, bumps: new Uint8Array(4) },
    });
    await send(payer, [{ ...ix, accounts: [...ix.accounts, ...mockTradeAccounts(m)] }], { computeUnits: 1_000_000 });
}

export async function shortPosition(position: Address): Promise<ShortPosition | null> {
    const a = await getAccount(position);
    return a ? getShortPositionDecoder().decode(a.data) : null;
}

export async function lenderPositionAddress(market: Address, owner: Address): Promise<Address> {
    return pda(PROGRAMS.shortwin, ['lender', market, owner]);
}

export async function lenderPosition(market: Address, owner: Address): Promise<LenderPosition | null> {
    const a = await getAccount(await lenderPositionAddress(market, owner));
    return a ? getLenderPositionDecoder().decode(a.data) : null;
}

export interface LandedInstruction {
    readonly program: Address;
    readonly data: Uint8Array;
}

export interface LandedTransaction {
    readonly fee: bigint;
    readonly err: unknown;
    readonly logs: readonly string[];
    /** Top-level instructions in order (program ids resolved through the loaded addresses). */
    readonly instructions: readonly LandedInstruction[];
    /** The SetComputeUnitPrice value, if any (µlamports per CU). */
    readonly computeUnitPrice: bigint | null;
    /** The SetComputeUnitLimit value, if any. */
    readonly computeUnitLimit: number | null;
}

/** A confirmed transaction (`getTransaction` at `confirmed`, v0 allowed), or `null` if unknown. */
export async function landedTransaction(signature: Signature | string): Promise<LandedTransaction | null> {
    const t = await rpc()
        .getTransaction(signature as Signature, {
            commitment: 'confirmed',
            maxSupportedTransactionVersion: 0,
            encoding: 'json',
        })
        .send();
    if (!t) return null;
    const keys = [
        ...t.transaction.message.accountKeys,
        ...(t.meta?.loadedAddresses?.writable ?? []),
        ...(t.meta?.loadedAddresses?.readonly ?? []),
    ];
    const instructions = t.transaction.message.instructions.map((ix) => ({
        program: keys[ix.programIdIndex] as Address,
        data: bs58ToBytes(String(ix.data)),
    }));
    let computeUnitPrice: bigint | null = null;
    let computeUnitLimit: number | null = null;
    for (const ix of instructions) {
        if (ix.program !== PROGRAMS.computeBudget) continue;
        const v = new DataView(ix.data.buffer, ix.data.byteOffset);
        if (ix.data[0] === 3 && ix.data.length === 9) computeUnitPrice = v.getBigUint64(1, true);
        if (ix.data[0] === 2 && ix.data.length === 5) computeUnitLimit = v.getUint32(1, true);
    }
    return {
        fee: BigInt(t.meta?.fee ?? 0),
        err: t.meta?.err ?? null,
        logs: t.meta?.logMessages ?? [],
        instructions,
        computeUnitPrice,
        computeUnitLimit,
    };
}

/** The landed top-level instructions other than compute-budget ones. */
export function intentOf(t: LandedTransaction): readonly LandedInstruction[] {
    return t.instructions.filter((ix) => ix.program !== PROGRAMS.computeBudget);
}

/** Whether a signature was ever seen by the validator (sent, landed or failed). */
export async function signatureKnown(signature: string): Promise<boolean> {
    const { value } = await rpc()
        .getSignatureStatuses([signature as Signature], { searchTransactionHistory: true })
        .send();
    return value[0] !== null;
}

const ALPHABET = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';

/** Base58 → bytes (instruction data in `json` encoding). */
function bs58ToBytes(s: string): Uint8Array {
    const bytes = [0];
    for (const ch of s) {
        let carry = ALPHABET.indexOf(ch);
        if (carry < 0) throw new Error('not base58');
        for (let i = 0; i < bytes.length; i++) {
            carry += (bytes[i] ?? 0) * 58;
            bytes[i] = carry & 0xff;
            carry >>= 8;
        }
        while (carry > 0) {
            bytes.push(carry & 0xff);
            carry >>= 8;
        }
    }
    for (const ch of s) {
        if (ch !== '1') break;
        bytes.push(0);
    }
    return new Uint8Array(bytes.reverse());
}
