/**
 * pump v3 on the E2E validator (design pump-final §8; FRONTEND.md §10.2, §5.11): a curve market on
 * the dumped `near166` curve — pump's 2026-10-13 build, 166 bytes, near completion — listed with
 * `create_market(Auto)` and the pump init set and lent by `curveLender` (its tokens are planted at
 * genesis, `curveLenderAccount`: pump's mints have no authority), and a third party's `buy_v3` that
 * buys the curve down past a position's `q` (the crowd of the LiteSVM `synthetic_exit_it`).
 * TEST-ONLY.
 */
import {
    getCrankPriceInstructionAsync,
    getCreateMarketInstructionAsync,
    getDepositInstructionAsync,
    getMarketDecoder,
    SHORTWIN_PROGRAM_ADDRESS,
} from '@shortwin/client';
import {
    type AccountMeta,
    AccountRole,
    type Address,
    getAddressDecoder,
    type Instruction,
    type Signature,
} from '@solana/kit';
import { getCreateAssociatedTokenIdempotentInstructionAsync } from '@solana-program/token';
import { readMetas, readSet } from '../../src/venues/lists.ts';
import {
    ata,
    clock,
    detectionSet,
    eventAuthority,
    getAccount,
    marketKeys,
    PROGRAMS,
    pda,
    send,
    WSOL_MINT,
    waitForClock,
} from './chain.ts';
import { CURVE_LEND_RAW, fixtureAddress } from './genesis.ts';
import { signerFor } from './keys.ts';

const U64_MAX = 0xffff_ffff_ffff_ffffn;
const VENUE_HINT_AUTO = 0;
/** pump's `Global` and its curve `FeeConfig` (pump-fees), the accounts every v3 trade names. */
const PUMP_GLOBAL = '4wTV1YmiEkRvAtNtsSGPtUrqRYQMe5SKy2uB4Jjaxnjf' as Address;
const PUMP_FEE_CONFIG = '8Wf5TiAheLUqBrKXeYg2JtAFFMWtKdG2BSFgqUcPVwTt' as Address;
/** `sha256("global:buy_v3")[..8]` and `sha256("global:init_user_volume_accumulator")[..8]`. */
const IX_BUY_V3 = new Uint8Array([0x07, 0x05, 0x1d, 0xc4, 0xf5, 0x17, 0x65, 0x50]);
const IX_INIT_UVA = new Uint8Array([0x5e, 0x06, 0xca, 0x73, 0xff, 0x60, 0xe8, 0xb7]);
/** `Global.buyback_fee_recipients[8]` @741. */
const OFF_BUYBACK_RECIPIENTS = 741;

const ro = (address: Address): AccountMeta => ({ address, role: AccountRole.READONLY });
const w = (address: Address): AccountMeta => ({ address, role: AccountRole.WRITABLE });

export { CURVE_LEND_RAW };

/** The dumped `near166` coin: its mint (Token-2022), curve and curve base ATA. */
export function curveCoin(): { mint: Address; curve: Address; curveBaseAta: Address; tokenProgram: Address } {
    return {
        mint: fixtureAddress('pump_near166_mint'),
        curve: fixtureAddress('pump_near166_curve'),
        curveBaseAta: fixtureAddress('pump_near166_curve_base_ata'),
        tokenProgram: PROGRAMS.token2022,
    };
}

export interface CurveMarket {
    readonly mint: Address;
    readonly market: Address;
    /** The cluster time of the listing (`market_warmup_s` counts from it). */
    readonly createdAt: bigint;
}

/**
 * Lists `near166` (`create_market(Auto)`: the detection set, then pump's init set — pump, the
 * trader's pump UVA (created here), pump's event authority) and lends `CURVE_LEND_RAW` from
 * `curveLender`'s planted account.
 */
export async function listCurveMarket(): Promise<CurveMarket> {
    const lender = await signerFor('curveLender');
    const { mint, tokenProgram } = curveCoin();
    const k = await marketKeys(mint, tokenProgram);
    const det = await detectionSet(mint);
    const events = await eventAuthority();
    const create = await getCreateMarketInstructionAsync({
        creator: lender,
        mint,
        traderBaseAta: k.traderBaseAta,
        tokenProgram,
        eventAuthority: events,
        program: SHORTWIN_PROGRAM_ADDRESS,
        venueHint: VENUE_HINT_AUTO,
        maxCreationFee: U64_MAX,
    });
    const init = [
        ro(det.bondingCurve),
        ro(det.canonicalPool),
        ro(PROGRAMS.pump),
        w(await pda(PROGRAMS.pump, ['user_volume_accumulator', k.trader])),
        ro(await pda(PROGRAMS.pump, ['__event_authority'])),
    ];
    await send(lender, [{ ...create, accounts: [...create.accounts, ...init] }], { computeUnits: 600_000 });
    const deposit = await getDepositInstructionAsync({
        owner: lender,
        mint,
        ownerTokenAccount: await ata(lender.address, mint, tokenProgram),
        tokenProgram,
        eventAuthority: events,
        program: SHORTWIN_PROGRAM_ADDRESS,
        amount: CURVE_LEND_RAW,
        minShares: 0n,
    });
    await send(lender, [deposit], { computeUnits: 400_000 });
    return { mint, market: k.market, createdAt: (await clock()).unixTimestamp };
}

/** Complete a current Pump curve price bucket after the listing's 50-second warm-up. */
export async function freshCurveOpenPrice(m: CurveMarket): Promise<void> {
    const account = await getAccount(m.market);
    if (!account) throw new Error('the listed curve market is missing');
    const read = readMetas('pump_curve', await readSet('pump_curve', getMarketDecoder().decode(account.data)), false);
    const crank = await getCrankPriceInstructionAsync({
        mint: m.mint,
        eventAuthority: await eventAuthority(),
        program: SHORTWIN_PROGRAM_ADDRESS,
    });
    const ix = { ...crank, accounts: [...crank.accounts, ...read] };
    const payer = await signerFor('payer');
    await send(payer, [ix]);
    const recorded = await getAccount(m.market);
    if (!recorded) throw new Error('the listed curve market disappeared after its price crank');
    await waitForClock(getMarketDecoder().decode(recorded.data).bSec + 1n);
    await send(payer, [ix]);
}

/** The live curve's `(vT, vQ, rT, rQ, complete)` and its post-complete fields (@150, @158). */
export async function curveState(): Promise<{
    vt: bigint;
    vq: bigint;
    rt: bigint;
    rq: bigint;
    complete: boolean;
    pcb: bigint;
    pcq: bigint;
}> {
    const a = await getAccount(curveCoin().curve);
    if (!a) throw new Error('the near166 curve is missing');
    const v = new DataView(a.data.buffer, a.data.byteOffset, a.data.byteLength);
    const u64 = (o: number) => (o + 8 <= a.data.length ? v.getBigUint64(o, true) : 0n);
    return {
        vt: u64(8),
        vq: u64(16),
        rt: u64(24),
        rq: u64(32),
        complete: a.data[48] === 1,
        pcb: u64(150),
        pcq: u64(158),
    };
}

/** The market's `venue` byte (`VenueKind`: 1 PumpCurve, 2 PumpMigrating, 3 PumpSwap). */
export async function marketVenue(market: Address): Promise<number> {
    const a = await getAccount(market);
    if (!a) throw new Error('no market');
    return getMarketDecoder().decode(a.data).venue;
}

/**
 * A third party's `buy_v3(amount, max_sol_cost)` on the curve (pump's 17 accounts, 24 bytes): its
 * base ATA and pump UVA created first, the live `Global`'s first buyback recipient.
 */
export async function crowdBuy(amount: bigint, maxSolCost: bigint): Promise<Signature> {
    const crowd = await signerFor('curveCrowd');
    const { mint, curve, curveBaseAta, tokenProgram } = curveCoin();
    const global = await getAccount(PUMP_GLOBAL);
    if (!global) throw new Error('no pump Global');
    const buyback = getAddressDecoder().decode(
        global.data.subarray(OFF_BUYBACK_RECIPIENTS, OFF_BUYBACK_RECIPIENTS + 32),
    );
    const baseAta = await ata(crowd.address, mint, tokenProgram);
    const uva = await pda(PROGRAMS.pump, ['user_volume_accumulator', crowd.address]);
    const pumpEvents = await pda(PROGRAMS.pump, ['__event_authority']);
    const ixs: Instruction[] = [
        await getCreateAssociatedTokenIdempotentInstructionAsync({
            payer: crowd,
            owner: crowd.address,
            mint,
            tokenProgram,
        }),
    ];
    if (!(await getAccount(uva))) {
        ixs.push({
            programAddress: PROGRAMS.pump,
            accounts: [
                { address: crowd.address, role: AccountRole.WRITABLE_SIGNER, signer: crowd } as AccountMeta,
                ro(crowd.address),
                w(uva),
                ro(PROGRAMS.system),
                ro(pumpEvents),
                ro(PROGRAMS.pump),
            ],
            data: IX_INIT_UVA,
        });
    }
    const data = new Uint8Array(24);
    data.set(IX_BUY_V3, 0);
    const dv = new DataView(data.buffer);
    dv.setBigUint64(8, amount, true);
    dv.setBigUint64(16, maxSolCost, true);
    ixs.push({
        programAddress: PROGRAMS.pump,
        accounts: [
            ro(PUMP_GLOBAL),
            ro(mint),
            ro(WSOL_MINT),
            ro(tokenProgram),
            ro(PROGRAMS.token),
            w(curve),
            w(curveBaseAta),
            w(await ata(curve, WSOL_MINT, PROGRAMS.token)),
            { address: crowd.address, role: AccountRole.WRITABLE_SIGNER, signer: crowd } as AccountMeta,
            w(baseAta),
            w(await ata(crowd.address, WSOL_MINT, PROGRAMS.token)),
            w(uva),
            ro(PUMP_FEE_CONFIG),
            w(buyback),
            ro(PROGRAMS.system),
            ro(pumpEvents),
            ro(PROGRAMS.pump),
        ],
        data,
    });
    return send(crowd, ixs, { computeUnits: 400_000 });
}
