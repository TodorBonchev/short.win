/**
 * Liquidate one seeded mock-market position on the local E2E validator.
 *
 * From apps/web, after `node e2e/validator/start.ts --env`:
 *   node e2e/demo/liquidate.ts main
 *
 * `main` uses the seeded margin position. `pot`, `chain` and `wipe` are also available, but
 * those markets are reserved for their own lender scenarios. This changes the local chain;
 * start a fresh validator before running specs that expect the original position.
 */
import { fileURLToPath } from 'node:url';
import {
    getLiquidateEventDecoder,
    getLiquidateInstructionAsync,
    LIQUIDATE_EVENT_DISCRIMINATOR,
    type LiquidateEvent,
    SHORTWIN_ERROR__EXIT_BOUND_NOT_MET,
    SHORTWIN_ERROR__NOT_LIQUIDATABLE,
    SHORTWIN_ERROR__REFERENCE_STALE,
    SHORTWIN_ERROR__USE_SOL_SETTLE,
    SHORTWIN_PROGRAM_ADDRESS,
} from '@shortwin/client';
import {
    appendTransactionMessageInstructions,
    createTransactionMessage,
    getBase64EncodedWireTransaction,
    type Instruction,
    type KeyPairSigner,
    pipe,
    type Signature,
    setTransactionMessageComputeUnitLimit,
    setTransactionMessageFeePayerSigner,
    setTransactionMessageLifetimeUsingBlockhash,
    signTransactionMessageWithSigners,
} from '@solana/kit';
import { crankPrice, eventsOf, movePrice, shortPosition } from '../validator/actions.ts';
import { clock, eventAuthority, rpc, send, waitForClock } from '../validator/chain.ts';
import { signerFor } from '../validator/keys.ts';
import { derivePool, reserves } from '../validator/mock-cpamm.ts';
import { mockTradeAccounts } from '../validator/seed.ts';
import { loadState, type MarketInfo, type PositionInfo } from '../validator/state.ts';

const COMPUTE_UNITS = 1_000_000;
const PRICE_STEP_BPS = 400;
const MAX_STEPS = 60;
const MAX_DURATION_MS = 180_000;
const PPM = 1_000_000n;
const MOCK_PROTOCOL_FEE_PPM = 120_000n;

function ceilDiv(n: bigint, d: bigint): bigint {
    return (n + d - 1n) / d;
}

/** The mock venue's exact-out SOL cost, with its input fee (INTERFACE.md, Swap arithmetic). */
function buybackCost(q: bigint, x: bigint, y: bigint, feePpm: number): bigint {
    if (q <= 0n || q >= x || y <= 0n || feePpm < 0 || feePpm >= Number(PPM)) {
        throw new Error('the mock pool cannot quote this buyback');
    }
    const swapped = ceilDiv(y * q, x - q);
    return ceilDiv(swapped * PPM, PPM - BigInt(feePpm));
}

/** The reserves after the next `movePrice(m, bps)` buy, using the mock's exact-in math. */
function reservesAfterBuy(x: bigint, y: bigint, feePpm: number, bps: number): { x: bigint; y: bigint } {
    const ratio = Math.sqrt(1 + bps / 10_000);
    const dy = (y * BigInt(Math.round((ratio - 1) * 1e9))) / 1_000_000_000n;
    const tradeFee = ceilDiv(dy * BigInt(feePpm), PPM);
    const net = dy - tradeFee;
    if (dy <= 0n || net <= 0n) throw new Error('the next mock-pool buy is too small');
    const tokensOut = (x * net) / (y + net);
    const protocolFee = (tradeFee * MOCK_PROTOCOL_FEE_PPM) / PPM;
    return { x: x - tokensOut, y: y + dy - protocolFee };
}

/** A conservative ceiling below the program's S1 boundary `B + ert <= V - K_s`. */
function safeBuybackLimit(pos: NonNullable<Awaited<ReturnType<typeof shortPosition>>>): bigint {
    const keeper = pos.settleKeeperFixed + (pos.v * BigInt(pos.settleKeeperBps)) / 10_000n;
    const keeperBound = keeper < pos.settleKeeperCap ? keeper : pos.settleKeeperCap;
    return pos.v - keeperBound - pos.ert - pos.v / 10n;
}

/** Simulate the actual liquidation instruction without changing the validator. */
async function simulateLiquidation(
    keeper: KeyPairSigner,
    instruction: Instruction,
): Promise<{ ok: true } | { ok: false; code: number | null; detail: string }> {
    const { value: blockhash } = await rpc().getLatestBlockhash({ commitment: 'confirmed' }).send();
    const message = pipe(
        createTransactionMessage({ version: 0 }),
        (m) => setTransactionMessageFeePayerSigner(keeper, m),
        (m) => setTransactionMessageLifetimeUsingBlockhash(blockhash, m),
        (m) => setTransactionMessageComputeUnitLimit(COMPUTE_UNITS, m),
        (m) => appendTransactionMessageInstructions([instruction], m),
    );
    const wire = getBase64EncodedWireTransaction(await signTransactionMessageWithSigners(message));
    const { value } = await rpc()
        .simulateTransaction(wire, {
            encoding: 'base64',
            sigVerify: false,
            replaceRecentBlockhash: true,
            commitment: 'confirmed',
        })
        .send();
    if (!value.err) return { ok: true };
    const error = JSON.stringify(value.err, (_key, v: unknown) => (typeof v === 'bigint' ? v.toString() : v));
    const logs = (value.logs ?? []).join('\n');
    const number = /Error Number: (\d+)/.exec(logs)?.[1] ?? /"Custom":\s*"?(\d+)/.exec(error)?.[1];
    return { ok: false, code: number ? Number(number) : null, detail: `${error}\n${logs}` };
}

export interface LiquidationOutcome {
    readonly signature: Signature;
    readonly event: LiquidateEvent;
    /** Number of 4% spot-price moves before the liquidation landed. */
    readonly priceMoves: number;
}

/**
 * Raise a seeded mock pool's price by 4% per chain second until `liquidate` simulates
 * successfully, then send it. The gradual path is the keeper scenario in
 * `services/keeper-litesvm/tests/scenarios/liquidate.rs`. Each refusal records a fresh price
 * bucket for the next attempt. The exact buyback is checked against the vault before every
 * attempt; an S1 result or an unexpected program error stops the driver.
 */
export async function liquidateSeededPosition(m: MarketInfo, p: PositionInfo): Promise<LiquidationOutcome> {
    if (!m.pool || m.venue !== 'mock' || p.market !== m.market || p.mint !== m.mint) {
        throw new Error('liquidation needs a matching seeded mock market and position');
    }
    const keeper = await signerFor('payer');
    const ix = await getLiquidateInstructionAsync({
        keeper,
        borrower: p.borrower,
        position: p.position,
        mint: m.mint,
        trader: m.trader,
        tokenProgram: m.tokenProgram,
        eventAuthority: await eventAuthority(),
        program: SHORTWIN_PROGRAM_ADDRESS,
        extras: { set: 0, bumps: new Uint8Array(4) },
    });
    const instruction = { ...ix, accounts: [...ix.accounts, ...mockTradeAccounts(m)] };
    const pool = await derivePool(m.mint, m.tokenProgram);
    const deadline = Date.now() + MAX_DURATION_MS;

    // A long-idle seed needs a completed bucket before the first exit simulation.
    await crankPrice(m);
    await waitForClock((await clock()).unixTimestamp + 1n);
    await crankPrice(m);

    for (let priceMoves = 0; priceMoves <= MAX_STEPS; priceMoves++) {
        const pos = await shortPosition(p.position);
        if (!pos) throw new Error(`position ${p.position} is already closed`);
        const { x, y, feePpm } = await reserves(pool);
        const cost = buybackCost(pos.q, x, y, feePpm);
        const limit = safeBuybackLimit(pos);
        if (cost > limit) {
            throw new Error(`stopped before S1 range: buyback ${cost} lamports exceeds safe limit ${limit}`);
        }

        const simulation = await simulateLiquidation(keeper, instruction);
        if (simulation.ok) {
            const signature = await send(keeper, [instruction], { computeUnits: COMPUTE_UNITS });
            const [event] = await eventsOf(signature, LIQUIDATE_EVENT_DISCRIMINATOR, getLiquidateEventDecoder());
            if (!event || event.position !== p.position) throw new Error(`no matching Liquidate event in ${signature}`);
            return { signature, event, priceMoves };
        }
        if (simulation.code === SHORTWIN_ERROR__USE_SOL_SETTLE) {
            throw new Error(`stopped because the program requires SOL settlement: ${simulation.detail}`);
        }
        if (
            simulation.code !== SHORTWIN_ERROR__NOT_LIQUIDATABLE &&
            simulation.code !== SHORTWIN_ERROR__REFERENCE_STALE &&
            simulation.code !== SHORTWIN_ERROR__EXIT_BOUND_NOT_MET
        ) {
            throw new Error(`liquidation simulation failed: ${simulation.detail}`);
        }
        if (priceMoves === MAX_STEPS || Date.now() >= deadline) {
            throw new Error(`position ${p.position} did not become safely liquidatable`);
        }
        const next = reservesAfterBuy(x, y, feePpm, PRICE_STEP_BPS);
        const nextCost = buybackCost(pos.q, next.x, next.y, feePpm);
        if (nextCost > limit) {
            throw new Error(`stopped before the next price move: projected buyback ${nextCost} exceeds ${limit}`);
        }
        await crankPrice(m);
        await waitForClock((await clock()).unixTimestamp + 1n);
        await movePrice(m, PRICE_STEP_BPS);
    }
    throw new Error('unreachable');
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
    const state = loadState();
    const choice = process.argv[2] ?? 'main';
    const positionName = { main: 'margin', pot: 'pot', chain: 'chain', wipe: 'wipe' } as const;
    if (!(choice in positionName) || state.program !== 'test-venue' || !state.positions) {
        throw new Error('usage: node e2e/demo/liquidate.ts [main|pot|chain|wipe] (test-venue seed required)');
    }
    const name = choice as keyof typeof positionName;
    const outcome = await liquidateSeededPosition(state.markets[name], state.positions[positionName[name]]);
    console.log(
        `[demo] liquidated ${outcome.event.position} in ${outcome.priceMoves} price moves: ${outcome.signature}`,
    );
}
