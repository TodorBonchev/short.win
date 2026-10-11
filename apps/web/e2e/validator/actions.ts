/**
 * Node-side actions the specs take on the chain between UI steps (FRONTEND.md §10.2–§10.3): move a
 * mock pool's price, flip CPI Guard or required memos on a wallet's token account (the race tests),
 * set a coverage pause, donate to a market's trader and sweep it (vesting income), run a market's
 * lender prologue (rent credited up to now), a lender's and a keeper's part of a full wipe (FE-18),
 * and readers for positions, lenders, markets, landed transactions and their events. Every key is a
 * generated E2E key.
 */
import {
    EPOCH_ROLLED_EVENT_DISCRIMINATOR,
    type EpochRolledEvent,
    findShareEpochPda,
    getClaimSolInstructionAsync,
    getCloseShortInstructionAsync,
    getCrankPriceInstructionAsync,
    getDepositInstructionAsync,
    getEpochRolledEventDecoder,
    getLenderPositionDecoder,
    getMarketDecoder,
    getMarketVaultDecoder,
    getSetCoveragePauseInstructionAsync,
    getShareEpochAccountMetas,
    getShortPositionDecoder,
    getSolSettleEventDecoder,
    getSolSettleInstructionAsync,
    getSweepTraderInstructionAsync,
    getSyncLenderInstructionAsync,
    getWithdrawInstructionAsync,
    type LenderPosition,
    type Market,
    type MarketVault,
    podU128ToBigInt,
    SHORTWIN_PROGRAM_ADDRESS,
    type ShortPosition,
    SOL_SETTLE_EVENT_DISCRIMINATOR,
    type SolSettleEvent,
} from '@shortwin/client';
import {
    type AccountMeta,
    AccountRole,
    type Address,
    appendTransactionMessageInstructions,
    createTransactionMessage,
    getBase64EncodedWireTransaction,
    type Instruction,
    type KeyPairSigner,
    pipe,
    type ReadonlyUint8Array,
    type Signature,
    setTransactionMessageFeePayerSigner,
    setTransactionMessageLifetimeUsingBlockhash,
    signTransactionMessageWithSigners,
} from '@solana/kit';
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
import { ata, eventAuthority, getAccount, PROGRAMS, pda, rpc, send, sleep, WSOL_MINT } from './chain.ts';
import { signerFor, type WalletName } from './keys.ts';
import { derivePool, reserves, swapExactInIx } from './mock-cpamm.ts';
import { mockTradeAccounts, openShortFromNode } from './seed.ts';
import type { MarketInfo, PositionInfo } from './state.ts';

/** `VS`, `VA` (ECON §9.2; `shortwin_math::pool`). */
const VIRTUAL_SHARES = 1_000n;
const VIRTUAL_ASSETS = 1n;

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
    await lendFromNode(owner, m, source, amount);
}

/** `deposit(amount, min_shares = 0)` of `owner` from `source`, from Node (no epoch records). */
export async function lendFromNode(
    owner: WalletName,
    m: MarketInfo,
    source: Address,
    amount: bigint,
): Promise<Signature> {
    const o = await signerFor(owner);
    return send(o, [
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
 * Rent paid in: the seeding payer (not a spec wallet) opens a small short on mock market `m`, holds
 * it `holdMs` and closes it, paying at least the minimum rent into the pool — lenders then have SOL
 * to claim (rent is credited continuously but becomes claimable once a borrower pays it in). Only the
 * rent credited while the position was open is claimable at once: the minimum rent's excess above it
 * vests over `lump_vest_s` like a lump (M10-8C-1), hence the hold.
 */
export async function payRentIn(m: MarketInfo, qRaw: bigint, holdMs = 30_000): Promise<void> {
    const payer = await signerFor('payer');
    const nonce = BigInt(Date.now());
    const p = await openShortFromNode(payer, m, qRaw, nonce);
    await new Promise((resolve) => setTimeout(resolve, holdMs));
    await closeShortFromNode(payer, m, p);
}

/** `close_short` of position `p` on mock market `m` by its borrower, from Node, the buyback unbounded. */
export async function closeShortFromNode(borrower: KeyPairSigner, m: MarketInfo, p: PositionInfo): Promise<Signature> {
    const ix = await getCloseShortInstructionAsync({
        borrower,
        mint: m.mint,
        position: p.position,
        trader: m.trader,
        tokenProgram: m.tokenProgram,
        eventAuthority: await eventAuthority(),
        program: SHORTWIN_PROGRAM_ADDRESS,
        maxBuybackCost: 0xffff_ffff_ffff_ffffn,
        extras: { set: 0, bumps: new Uint8Array(4) },
    });
    return send(borrower, [{ ...ix, accounts: [...ix.accounts, ...mockTradeAccounts(m)] }], {
        computeUnits: 1_000_000,
    });
}

/**
 * Runs mock market `m`'s lender prologue from the seeding payer (a `sweep_trader` with nothing to
 * sweep): the rent of its open positions is credited up to now (`Market.receivable`, the lender
 * rent not yet paid in, grows with it) — what the deposit review's unpaid-rent rows read. Not
 * `crank_price`: a crank records a price observation that a later open's prologue integrates, which
 * moves that open's margin off the preview the short panel computed before it (short-open.spec).
 */
export async function accrue(m: MarketInfo): Promise<Signature> {
    const payer = await signerFor('payer');
    const ix = await getSweepTraderInstructionAsync({
        market: m.market,
        trader: m.trader,
        traderWsol: m.traderWsol,
        traderWsolAta: await ata(m.trader, WSOL_MINT, PROGRAMS.token),
        eventAuthority: await eventAuthority(),
        program: SHORTWIN_PROGRAM_ADDRESS,
    });
    return send(payer, [ix]);
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
    /** The instruction's accounts in order (resolved through the loaded addresses). */
    readonly accounts: readonly Address[];
}

export interface LandedTransaction {
    readonly fee: bigint;
    readonly err: unknown;
    readonly logs: readonly string[];
    /** Top-level instructions in order (program ids resolved through the loaded addresses). */
    readonly instructions: readonly LandedInstruction[];
    /** The transaction's signers (the message's first `numRequiredSignatures` account keys). */
    readonly signers: readonly Address[];
    /** The accounts the message makes writable (its header's static keys, then the loaded writable ones). */
    readonly writable: readonly Address[];
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
        accounts: ix.accounts.map((i) => keys[i] as Address),
    }));
    let computeUnitPrice: bigint | null = null;
    let computeUnitLimit: number | null = null;
    for (const ix of instructions) {
        if (ix.program !== PROGRAMS.computeBudget) continue;
        const v = new DataView(ix.data.buffer, ix.data.byteOffset);
        if (ix.data[0] === 3 && ix.data.length === 9) computeUnitPrice = v.getBigUint64(1, true);
        if (ix.data[0] === 2 && ix.data.length === 5) computeUnitLimit = v.getUint32(1, true);
    }
    const { header, accountKeys } = t.transaction.message;
    const signed = header.numRequiredSignatures;
    const writable = [
        ...accountKeys.slice(0, signed - header.numReadonlySignedAccounts),
        ...accountKeys.slice(signed, accountKeys.length - header.numReadonlyUnsignedAccounts),
        ...(t.meta?.loadedAddresses?.writable ?? []),
    ] as Address[];
    return {
        fee: BigInt(t.meta?.fee ?? 0),
        err: t.meta?.err ?? null,
        logs: t.meta?.logMessages ?? [],
        instructions,
        signers: accountKeys.slice(0, signed),
        writable,
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

// ---- FE-18: the full wipe and the claim-time pot (lend-post-wipe, lend-claim-sync) ----------

/** Anchor's `EVENT_IX_TAG` (`0x1d9acb512ea545e4` LE): the prefix of an `emit_cpi!` self-CPI's data. */
const EVENT_IX_TAG = Uint8Array.from([0xe4, 0x45, 0xa5, 0x2e, 0x51, 0xcb, 0x9a, 0x1d]);

/**
 * The events a confirmed transaction emitted (`emit_cpi!`: short.win's inner self-CPIs whose data
 * is `EVENT_IX_TAG ‖ event discriminator ‖ fields`), as the bytes after the tag, in order.
 */
export async function landedEvents(signature: Signature | string): Promise<Uint8Array[]> {
    const t = await rpc()
        .getTransaction(signature as Signature, {
            commitment: 'confirmed',
            maxSupportedTransactionVersion: 0,
            encoding: 'json',
        })
        .send();
    if (!t) throw new Error(`transaction ${signature} not found`);
    const keys = [
        ...t.transaction.message.accountKeys,
        ...(t.meta?.loadedAddresses?.writable ?? []),
        ...(t.meta?.loadedAddresses?.readonly ?? []),
    ];
    const out: Uint8Array[] = [];
    for (const group of t.meta?.innerInstructions ?? []) {
        for (const ix of group.instructions) {
            if (keys[ix.programIdIndex] !== PROGRAMS.shortwin) continue;
            const data = bs58ToBytes(String(ix.data));
            if (data.length > 8 && EVENT_IX_TAG.every((b, i) => data[i] === b)) out.push(data.subarray(8));
        }
    }
    return out;
}

/** The events of one type (`discriminator`, decoded with `decoder`) in a confirmed transaction. */
export async function eventsOf<E>(
    signature: Signature | string,
    discriminator: ReadonlyUint8Array,
    decoder: { decode(bytes: Uint8Array): E },
): Promise<E[]> {
    return (await landedEvents(signature))
        .filter((d) => discriminator.every((b, i) => d[i] === b))
        .map((d) => decoder.decode(d));
}

/** The custom program error number in a failed `send` (Anchor's `Error Number: N` log), or `null`. */
export function programErrorNumber(e: unknown): number | null {
    const m = /Error Number: (\d+)/.exec(e instanceof Error ? e.message : String(e));
    return m ? Number(m[1]) : null;
}

/** A market's `Market` account, decoded. */
export async function marketAccount(m: MarketInfo): Promise<Market> {
    const a = await getAccount(m.market);
    if (!a) throw new Error(`no market ${m.market}`);
    return getMarketDecoder().decode(a.data);
}

/** A market's `MarketVault` account, decoded (`null` if it does not exist). */
export async function marketVaultAccount(m: MarketInfo): Promise<MarketVault | null> {
    const a = await getAccount(m.marketVault);
    return a ? getMarketVaultDecoder().decode(a.data) : null;
}

/** The mock venue's `ReadSet` `[pool, base vault, quote vault]` (read-only; `crank_price`, `sol_settle`). */
function mockReadSet(m: MarketInfo): AccountMeta[] {
    if (!m.pool) throw new Error(`${m.symbol} is not a mock market`);
    return [m.pool.pool, m.pool.baseVault, m.pool.quoteVault].map((address) => ({
        address,
        role: AccountRole.READONLY,
    }));
}

/**
 * The lender withdraws every free token of `m` (`T = 0` after it; settle_it.rs's
 * `withdraw_down_to(0)`): the most shares whose burn pays at most `T`, `⌊s·(T + ΣB_Q + VA)/(S +
 * VS)⌋ ≤ T`, with `min_tokens = T` so the burn pays exactly `T`. Returns the tokens withdrawn.
 */
export async function withdrawAllFree(owner: WalletName, m: MarketInfo, destination: Address): Promise<bigint> {
    return withdrawFreeDownTo(owner, m, destination, 0n);
}

/**
 * The lender withdraws the free tokens of `m` down to `keep` raw tokens (`T = keep` after it; its
 * allowance must cover them: the only unlocked lender's does), as `withdrawAllFree`. Returns the
 * tokens withdrawn.
 */
export async function withdrawFreeDownTo(
    owner: WalletName,
    m: MarketInfo,
    destination: Address,
    keep: bigint,
): Promise<bigint> {
    const o = await signerFor(owner);
    const mk = await marketAccount(m);
    const lp = await lenderPosition(m.market, o.address);
    if (!lp) throw new Error(`${owner} does not lend in ${m.symbol}`);
    if (mk.tokensFree <= keep) throw new Error(`${m.symbol}: only ${mk.tokensFree} free tokens, keeping ${keep}`);
    const want = mk.tokensFree - keep;
    const claims = mk.tokensFree + mk.borrowedQ + VIRTUAL_ASSETS;
    const sharesVs = podU128ToBigInt(mk.totalShares) + VIRTUAL_SHARES;
    let shares = ((want + 1n) * sharesVs - 1n) / claims;
    if (shares > lp.shares) shares = lp.shares;
    await send(o, [
        await getWithdrawInstructionAsync({
            owner: o,
            mint: m.mint,
            ownerTokenAccount: destination,
            tokenProgram: m.tokenProgram,
            memoProgram: m.tokenProgram === PROGRAMS.token2022 ? PROGRAMS.memo : PROGRAMS.shortwin,
            eventAuthority: await eventAuthority(),
            program: SHORTWIN_PROGRAM_ADDRESS,
            shares,
            minTokens: want,
            minSol: 0n,
        }),
    ]);
    const left = (await marketAccount(m)).tokensFree;
    if (left !== keep) throw new Error(`${m.symbol}: ${left} free tokens left after the withdrawal, not ${keep}`);
    return want;
}

/**
 * `withdraw(shares)` of `owner` for about `tokens` raw tokens (the shares whose burn pays at most
 * that; `min_tokens = 0`, `min_sol = 0`), from Node: a lender leaving part of its position.
 */
export async function withdrawTokensFromNode(
    owner: WalletName,
    m: MarketInfo,
    destination: Address,
    tokens: bigint,
): Promise<Signature> {
    const o = await signerFor(owner);
    const mk = await marketAccount(m);
    const claims = mk.tokensFree + mk.borrowedQ + VIRTUAL_ASSETS;
    const shares = (tokens * (podU128ToBigInt(mk.totalShares) + VIRTUAL_SHARES)) / claims;
    return send(o, [
        await getWithdrawInstructionAsync({
            owner: o,
            mint: m.mint,
            ownerTokenAccount: destination,
            tokenProgram: m.tokenProgram,
            memoProgram: m.tokenProgram === PROGRAMS.token2022 ? PROGRAMS.memo : PROGRAMS.shortwin,
            eventAuthority: await eventAuthority(),
            program: SHORTWIN_PROGRAM_ADDRESS,
            shares,
            minTokens: 0n,
            minSol: 0n,
        }),
    ]);
}

/**
 * The in-house keeper's seconds (ECON §5.8 duties 1 and 4; settle_support.rs `Sc::second`): one
 * `[crank_price, sol_settle]` by the seeding payer about every second until the position is
 * settled (its account closed) or `timeoutMs` passes. Returns the settling transaction.
 */
export async function settleUntilClosed(
    m: MarketInfo,
    p: PositionInfo,
    timeoutMs = 300_000,
): Promise<{ signature: Signature; rounds: number }> {
    const keeper = await signerFor('payer');
    const ea = await eventAuthority();
    const deadline = Date.now() + timeoutMs;
    for (let rounds = 1; ; rounds++) {
        const { shareEpoch } = await marketAccount(m);
        const crank = await getCrankPriceInstructionAsync({
            mint: m.mint,
            eventAuthority: ea,
            program: SHORTWIN_PROGRAM_ADDRESS,
        });
        const settle = await getSolSettleInstructionAsync({
            keeper,
            position: p.position,
            borrower: p.borrower,
            mint: m.mint,
            shareEpochRec: (await findShareEpochPda({ market: m.market, epoch: shareEpoch }))[0],
            eventAuthority: ea,
            program: SHORTWIN_PROGRAM_ADDRESS,
        });
        const signature = await send(
            keeper,
            [
                { ...crank, accounts: [...crank.accounts, ...mockReadSet(m)] },
                { ...settle, accounts: [...settle.accounts, ...mockReadSet(m)] },
            ],
            { computeUnits: 600_000 },
        );
        if (!(await getAccount(p.position))) return { signature, rounds };
        if (Date.now() > deadline) throw new Error(`${p.position} not settled after ${rounds} keeper seconds`);
        await sleep(1_000);
    }
}

/** What `fullWipe` did. */
export interface WipeOutcome extends GapSettlement {
    /** Tokens the only lender withdrew (every free token). */
    readonly withdrawn: bigint;
}

/** A spot move far beyond any tier's S1 boundary (`cr_open` ≤ 4×: the buyback alone exceeds `V`). */
export const WIPE_GAP_BPS = 90_000;

/** What `gapSettle` did. */
export interface GapSettlement {
    /** The settling `sol_settle`'s events and signature. */
    readonly settle: SolSettleEvent;
    readonly rolled: EpochRolledEvent;
    readonly signature: Signature;
    readonly rounds: number;
}

/**
 * A gap of `bps` on mock market `m`'s pool, then keeper seconds until position `p` — the last
 * borrowed tokens — settles S1, which rolls the share epoch (a full wipe with no free tokens left, a
 * rebase with fewer than a thousandth of the claims left).
 */
export async function gapSettle(m: MarketInfo, p: PositionInfo, bps = WIPE_GAP_BPS): Promise<GapSettlement> {
    await movePrice(m, bps);
    const { signature, rounds } = await settleUntilClosed(m, p);
    const [settle] = await eventsOf(signature, SOL_SETTLE_EVENT_DISCRIMINATOR, getSolSettleEventDecoder());
    const [rolled] = await eventsOf(signature, EPOCH_ROLLED_EVENT_DISCRIMINATOR, getEpochRolledEventDecoder());
    if (!settle || !rolled) throw new Error(`the settlement ${signature} did not roll the epoch`);
    return { settle, rolled, signature, rounds };
}

/**
 * An organic full wipe of mock market `m` (settle_it.rs `a_full_wipe_rolls_the_epoch_into_a_live_
 * record`, on the validator): its only lender `lender` withdraws every free token into
 * `destination`, the pool gaps up by `WIPE_GAP_BPS`, and keeper seconds run until position `p`
 * — the last borrowed tokens — settles S1, which rolls the epoch with `num = 0`.
 */
export async function fullWipe(
    m: MarketInfo,
    lender: WalletName,
    destination: Address,
    p: PositionInfo,
): Promise<WipeOutcome> {
    const withdrawn = await withdrawAllFree(lender, m, destination);
    return { withdrawn, ...(await gapSettle(m, p)) };
}

/** `crank_price` of mock market `m` by the seeding payer (an EMA observation now). */
export async function crankPrice(m: MarketInfo): Promise<Signature> {
    const crank = await getCrankPriceInstructionAsync({
        mint: m.mint,
        eventAuthority: await eventAuthority(),
        program: SHORTWIN_PROGRAM_ADDRESS,
    });
    return send(await signerFor('payer'), [{ ...crank, accounts: [...crank.accounts, ...mockReadSet(m)] }]);
}

/**
 * `open_short` once the market admits it (after a gap the EMA must come within
 * `open_ema_dev_bps` of the spot): a crank, then the open, every 2 s until it lands or
 * `timeoutMs` passes.
 */
export async function openShortWhenAllowed(
    borrower: KeyPairSigner,
    m: MarketInfo,
    q: bigint,
    nonce: bigint,
    timeoutMs = 240_000,
): Promise<PositionInfo> {
    const deadline = Date.now() + timeoutMs;
    for (;;) {
        await crankPrice(m);
        try {
            return await openShortFromNode(borrower, m, q, nonce);
        } catch (e) {
            if (Date.now() > deadline) throw e;
        }
        await sleep(2_000);
    }
}

/**
 * Simulates `ix` (signed by `feePayer`, not sent) on the validator and returns the raw data of
 * account `address` after it.
 */
async function simulateAccount(feePayer: KeyPairSigner, ix: Instruction, address: Address): Promise<Uint8Array> {
    const { value: blockhash } = await rpc().getLatestBlockhash({ commitment: 'confirmed' }).send();
    const message = pipe(
        createTransactionMessage({ version: 0 }),
        (x) => setTransactionMessageFeePayerSigner(feePayer, x),
        (x) => setTransactionMessageLifetimeUsingBlockhash(blockhash, x),
        (x) => appendTransactionMessageInstructions([ix], x),
    );
    const wire = getBase64EncodedWireTransaction(await signTransactionMessageWithSigners(message));
    const { value } = await rpc()
        .simulateTransaction(wire, {
            encoding: 'base64',
            commitment: 'confirmed',
            accounts: { addresses: [address], encoding: 'base64' },
        })
        .send();
    if (value.err) throw new Error(`simulation failed: ${JSON.stringify(value.err)}\n${(value.logs ?? []).join('\n')}`);
    const acct = value.accounts?.[0];
    if (!acct) throw new Error(`the simulation returned no account ${address}`);
    return new Uint8Array(Buffer.from(acct.data[0], 'base64'));
}

/**
 * What `claim_sol` alone would pay `owner` in `m` now: the change of its `claimed` in a validator
 * simulation (signed, not sent), with the epoch records it is behind on.
 */
export async function simulateClaimSol(owner: WalletName, m: MarketInfo): Promise<bigint> {
    const o = await signerFor(owner);
    const lender = await lenderPositionAddress(m.market, o.address);
    const lp = await lenderPosition(m.market, o.address);
    if (!lp) throw new Error(`${owner} does not lend in ${m.symbol}`);
    const { shareEpoch } = await marketAccount(m);
    const records = await getShareEpochAccountMetas({ market: m.market, fromEpoch: lp.epoch, shareEpoch });
    const ix = await getClaimSolInstructionAsync({
        owner: o,
        market: m.market,
        eventAuthority: await eventAuthority(),
        program: SHORTWIN_PROGRAM_ADDRESS,
    });
    const data = await simulateAccount(o, { ...ix, accounts: [...ix.accounts, ...records] }, lender);
    return getLenderPositionDecoder().decode(data).claimed - lp.claimed;
}

/**
 * `claim_sol` of `owner` in `m`, landed from Node with the epoch records it is behind on — the claim
 * alone (no other lender synced first): it pays what the pot as it stands allows.
 */
export async function claimSolFromNode(owner: WalletName, m: MarketInfo): Promise<Signature> {
    const o = await signerFor(owner);
    const lp = await lenderPosition(m.market, o.address);
    if (!lp) throw new Error(`${owner} does not lend in ${m.symbol}`);
    const { shareEpoch } = await marketAccount(m);
    const records = await getShareEpochAccountMetas({ market: m.market, fromEpoch: lp.epoch, shareEpoch });
    const ix = await getClaimSolInstructionAsync({
        owner: o,
        market: m.market,
        eventAuthority: await eventAuthority(),
        program: SHORTWIN_PROGRAM_ADDRESS,
    });
    return send(o, [{ ...ix, accounts: [...ix.accounts, ...records] }]);
}

/**
 * The market vault after a plain touch of `owner`'s lender position in `m` (`sync_lender(0)`,
 * signed by the payer, simulated, not sent): its `unmatched` less the current one is the
 * surplus that touch moves to the pot (R3A-2).
 */
export async function simulateSyncLender(owner: WalletName, m: MarketInfo): Promise<MarketVault> {
    const o = await signerFor(owner);
    const ix = await getSyncLenderInstructionAsync({
        market: m.market,
        lender: await lenderPositionAddress(m.market, o.address),
        owner: o.address,
        eventAuthority: await eventAuthority(),
        program: SHORTWIN_PROGRAM_ADDRESS,
        maxEpochs: 0,
    });
    return getMarketVaultDecoder().decode(await simulateAccount(await signerFor('payer'), ix, m.marketVault));
}

/**
 * The market vault and the lender after a third party's `sync_lender(n)` of `owner`'s position in
 * `m` — unsigned by the owner (the payer signs): the pot-only touch, KI-66 full — with the `n`
 * records of `epochs`, read-only (simulated, not sent): the vault's `unmatched` less the current
 * one is what that sync moves to the pot.
 */
export async function simulatePotSync(
    owner: WalletName,
    m: MarketInfo,
    epochs: readonly number[],
): Promise<{ vault: MarketVault; lender: LenderPosition }> {
    const o = await signerFor(owner);
    const lender = await lenderPositionAddress(m.market, o.address);
    const ix = await getSyncLenderInstructionAsync({
        market: m.market,
        lender,
        owner: o.address,
        eventAuthority: await eventAuthority(),
        program: SHORTWIN_PROGRAM_ADDRESS,
        maxEpochs: epochs.length,
    });
    const records: AccountMeta[] = await Promise.all(
        epochs.map(async (epoch) => ({
            address: (await findShareEpochPda({ market: m.market, epoch }))[0],
            role: AccountRole.READONLY,
        })),
    );
    const payer = await signerFor('payer');
    const full = { ...ix, accounts: [...ix.accounts, ...records] };
    const [vault, after] = await Promise.all([
        simulateAccount(payer, full, m.marketVault),
        simulateAccount(payer, full, lender),
    ]);
    return { vault: getMarketVaultDecoder().decode(vault), lender: getLenderPositionDecoder().decode(after) };
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
