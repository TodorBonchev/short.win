/**
 * SHORT track — the borrower's transaction plans (FRONTEND.md §5.1, §5.3, §5.10): `open_short`,
 * `close_short` and `add_margin` from the Codama builders of `@shortwin/client`, with the venue's
 * remaining accounts (`ReadSet ++ TradeExtras ++ Extras`) of the venue layer appended in the
 * program's order, the accounts the review shows before / after (§5.3 "watched accounts per
 * flow") and the disclosures that gate the Sign button (§4). Every signer input is the connected
 * wallet's signer.
 */
import {
    findPositionPda,
    getAddMarginInstruction,
    getCloseShortInstructionAsync,
    getMarketDecoder,
    getOpenShortInstructionAsync,
    getShortPositionDecoder,
    type Params,
    type ShortPosition,
    type TierParams,
} from '@shortwin/client';
import type { AccountMeta, Address, Instruction, TransactionSigner } from '@solana/kit';
import { qk } from '../../api/keys.ts';
import type { DisclosureId } from '../../config/disclosures.ts';
import { keeperRewardText, percent, proseDuration } from '../../config/disclosureVars.ts';
import { SHORTWIN_PROGRAM } from '../../config/programs.ts';
import { cleanSymbol, tokenLabel } from '../../lib/displayText.ts';
import { formatBps, formatSol, formatTokens, shortAddress } from '../../lib/format.ts';
import { failingProgram } from '../../tx/errors.ts';
import type { TxError, TxPlan, WatchedAccount } from '../../tx/types.ts';
import { EXTRAS_IX, PAMM_POOL_LEN_2026_10, PUMP_CURVE_LEN_2026_10 } from '../../venues/constants.ts';
import {
    CURVE_SLOT,
    type ExtrasChoice,
    extrasCandidates,
    NO_EXTRAS,
    PAMM_SLOT,
    pammCreatorVaultPrep,
    type RecipientIndex,
    randomRecipientIndex,
    readMetas,
    tradeExtras,
} from '../../venues/lists.ts';
import { eventAuthority } from '../../venues/pda.ts';
import type { ChainSnapshot } from './snapshot.ts';

/** A random u64 `nonce` for a new position (`["position", market, borrower, nonce]`). */
export function randomNonce(): bigint {
    const b = new Uint8Array(8);
    crypto.getRandomValues(b);
    let v = 0n;
    for (let i = 7; i >= 0; i--) v = (v << 8n) | BigInt(b[i] ?? 0);
    return v;
}

export { proseDuration };

/**
 * The `borrower.liquidation` / `borrower.residuals` placeholders from the live tier terms and
 * `GlobalConfig` parameters (§4.2; M9-UX-3): what an open would lock.
 */
export function borrowerDisclosureVars(
    tp: TierParams,
    p: Pick<Params, 'expiryFeeBps' | 's3WaitS' | 'settleKeeperFixed' | 'settleKeeperBps' | 'settleKeeperCap'>,
): Record<string, string> {
    return {
        liqCr: formatBps(tp.crLiqBps, 2),
        penalty: formatBps(tp.penaltyBps, 2),
        maxDuration: proseDuration(tp.maxDurationS),
        maxSlippage: formatBps(tp.maxLiqSlipBps, 2),
        minRent: proseDuration(tp.minRentS),
        expiryFee: percent(p.expiryFeeBps),
        s3Wait: proseDuration(p.s3WaitS),
        keeperReward: keeperRewardText(p.settleKeeperFixed, p.settleKeeperBps, p.settleKeeperCap),
    };
}

/**
 * The same placeholders from the terms locked into an open position (A26): its own expiry fee, S3
 * wait and keeper reward, and its own duration (`deadline − t_open`), not today's tier table.
 */
export function positionDisclosureVars(p: ShortPosition, tp: TierParams | undefined): Record<string, string> {
    return {
        liqCr: formatBps(p.crLiqBps, 2),
        penalty: formatBps(p.penaltyBps, 2),
        maxDuration: proseDuration(Number(p.deadline - p.tOpen)),
        maxSlippage: formatBps(p.maxLiqSlipBps, 2),
        minRent: proseDuration(tp?.minRentS ?? 3_600),
        expiryFee: percent(p.expiryFeeBps),
        s3Wait: proseDuration(p.s3WaitS),
        keeperReward: keeperRewardText(p.settleKeeperFixed, p.settleKeeperBps, p.settleKeeperCap),
    };
}

/** The venue accounts of a trade (`buy` / `sell`) on the snapshot, with an extras choice. */
export async function venueAccounts(
    s: ChainSnapshot,
    extras: ExtrasChoice,
    pick: RecipientIndex = randomRecipientIndex,
): Promise<AccountMeta[]> {
    return [
        ...readMetas(s.kind, s.readKeys, true),
        ...(await tradeExtras(s.kind, s.keys, s.read, pick)),
        ...extras.metas,
    ];
}

/** The extras sets to try for a trade on the snapshot's venue (current, none, previous, stable). */
export function extrasFor(s: ChainSnapshot, side: 'sell' | 'buy'): Promise<ExtrasChoice[]> {
    return extrasCandidates(s.kind, side === 'sell' ? EXTRAS_IX.sell : EXTRAS_IX.buy, s.keys, s.extras);
}

const withRemaining = (ix: Instruction, remaining: readonly AccountMeta[]): Instruction => ({
    ...ix,
    accounts: [...(ix.accounts ?? []), ...remaining],
});

/** The unit of a token amount: its (clean) symbol, else the short mint. */
function unitOf(symbol: string, mint: Address): string {
    return cleanSymbol(symbol) ?? shortAddress(mint);
}

function positionWatch(
    address: Address,
    decimals: number,
    symbol: string,
    mint: Address,
    fields: readonly (keyof ShortPosition)[],
): WatchedAccount {
    // Labels name the token by symbol AND mint (M9-SEC-5: a symbol alone can impersonate a ticker).
    const all = {
        q: { label: `Borrowed (${tokenLabel(symbol, mint)})`, unit: unitOf(symbol, mint), scale: decimals },
        s: { label: 'Sale proceeds', unit: 'SOL', scale: 9 },
        m: { label: 'Margin', unit: 'SOL', scale: 9 },
        v: { label: 'Vault', unit: 'SOL', scale: 9 },
        // A date in the review (UTC), not a count of seconds (D4).
        deadline: { label: 'Deadline', unit: '', scale: 0, kind: 'timestamp' },
    } as const;
    const decoder = getShortPositionDecoder();
    return {
        kind: 'program',
        address,
        label: 'Position',
        decode: (data) => {
            const p = decoder.decode(data);
            return { q: p.q, s: p.s, m: p.m, v: p.v, deadline: p.deadline };
        },
        fields: Object.fromEntries(fields.map((f) => [f, all[f as keyof typeof all]])),
    };
}

function marketWatch(address: Address, decimals: number, symbol: string, mint: Address): WatchedAccount {
    const decoder = getMarketDecoder();
    return {
        kind: 'program',
        address,
        label: 'Market',
        decode: (data) => ({ tokensFree: decoder.decode(data).tokensFree }),
        fields: {
            tokensFree: {
                label: `Free inventory (${tokenLabel(symbol, mint)})`,
                unit: unitOf(symbol, mint),
                scale: decimals,
            },
        },
    };
}

function invalidations(s: ChainSnapshot, owner: Address, position: Address) {
    return [
        qk.marketAccount(s.mint),
        qk.positionAccount(position),
        qk.position(position),
        qk.market(s.mint),
        ['indexer', 'borrower', owner],
        qk.exposure(s.mint, owner),
    ] as const;
}

export interface OpenPlanInput {
    readonly snapshot: ChainSnapshot;
    readonly signer: TransactionSigner;
    readonly symbol: string;
    readonly q: bigint;
    readonly minProceeds: bigint;
    readonly maxMargin: bigint;
    readonly maxOpenFee: bigint;
    readonly tier: TierParams;
    readonly extras?: ExtrasChoice;
    readonly nonce?: bigint;
    readonly pick?: RecipientIndex;
}

/** `open_short(nonce, q, min_proceeds, max_margin, max_open_fee, extras)` (§3.7, §5.10). */
export async function planOpenShort(
    inp: OpenPlanInput,
): Promise<TxPlan & { readonly position: Address; readonly nonce: bigint }> {
    const s = inp.snapshot;
    const nonce = inp.nonce ?? randomNonce();
    const extras = inp.extras ?? NO_EXTRAS;
    const [position] = await findPositionPda({ market: s.marketAddress, borrower: inp.signer.address, nonce });
    const ix = await getOpenShortInstructionAsync({
        borrower: inp.signer,
        mint: s.mint,
        trader: s.market.trader,
        tokenProgram: s.tokenProgram,
        eventAuthority: await eventAuthority(),
        program: SHORTWIN_PROGRAM,
        nonce,
        q: inp.q,
        minProceeds: inp.minProceeds,
        maxMargin: inp.maxMargin,
        maxOpenFee: inp.maxOpenFee,
        extras: { set: extras.sel.set, bumps: extras.sel.bumps },
    });
    const remaining = await venueAccounts(s, extras, inp.pick);
    const prep = await pammCreatorVaultPrep(inp.signer, s.keys, s.kind, s.read);
    const disclosures: DisclosureId[] = ['borrower.liquidation'];
    return {
        kind: 'open_short',
        title: `Open short · ${formatTokens(inp.q, s.decimals)} ${tokenLabel(inp.symbol, s.mint)}`,
        instructions: [...(prep ? [prep] : []), withRemaining(ix, remaining)],
        watch: [
            { kind: 'lamports', address: inp.signer.address, label: 'Your SOL' },
            positionWatch(position, s.decimals, inp.symbol, s.mint, ['q', 's', 'm', 'v', 'deadline']),
            marketWatch(s.marketAddress, s.decimals, inp.symbol, s.mint),
        ],
        disclosures,
        disclosureVars: { 'borrower.liquidation': borrowerDisclosureVars(inp.tier, s.config.params) },
        invalidate: invalidations(s, inp.signer.address, position),
        ...growthRefinement(s),
        // The review's confirmed banner leads to the new position (D4).
        next: { href: `/positions/${position}`, text: 'View position' },
        position,
        nonce,
    };
}

/** `ProceedsBelowMin` (6063): the open's proceeds fell below its `min_proceeds`. */
const PROCEEDS_BELOW_MIN = 6063;

/**
 * The refusal of an open on a pump account its first trade grows (PUMP-2026-10-2; disclosed, the
 * owner's decision of 2026-10-03).
 */
export const VENUE_GROWTH_MESSAGES = {
    pool: 'This pool needs a first trade to grow its account: PumpSwap charges that trade the account’s rent, which this open’s slippage tolerance cannot cover at this size. Try again after another trade on the pool, or with a larger size.',
    curve: 'This bonding curve needs a first trade to grow its account: pump charges that trade the account’s rent, which this open’s slippage tolerance cannot cover at this size. Try again after another trade on the curve, or with a larger size.',
} as const;

/**
 * The pump account an open's sale would grow (PUMP-2026-10-2): since pump's 2026-10-02 redeploys
 * the first trade on a PumpSwap pool shorter than 287 bytes, or a curve shorter than 141, grows it
 * and charges that trade the rent (81,280–386,080 lamports measured), which the program nets out of
 * the open's proceeds `S`; when the open's tolerance (`exec_tol_bps`, 10 bps) cannot cover it, the
 * open fails `ProceedsBelowMin` though no price moved. From the snapshot's ReadSet (the pool P0, the
 * curve C0); `null` for any other venue or an account at its size.
 */
export function venueGrowth(s: Pick<ChainSnapshot, 'kind' | 'read'>): keyof typeof VENUE_GROWTH_MESSAGES | null {
    if (s.kind === 'pump_swap') {
        const pool = s.read[PAMM_SLOT.pool];
        return pool && pool.data.length < PAMM_POOL_LEN_2026_10 ? 'pool' : null;
    }
    if (s.kind === 'pump_curve') {
        const curve = s.read[CURVE_SLOT.curve];
        return curve && curve.data.length < PUMP_CURVE_LEN_2026_10 ? 'curve' : null;
    }
    return null;
}

/**
 * An open plan's `refineError` on such an account: short.win's `ProceedsBelowMin` gets the specific
 * message (`VENUE_GROWTH_MESSAGES`); everything else, and any other venue, keeps the mapped error.
 * Only when the error's logs name short.win as the failing program: the mapper also reads a 6063
 * whose logs name no failing program (truncated logs) as short.win's, and PumpSwap's own 6063 has
 * the same number, so an unattributed one keeps the generic message. The account length is the
 * snapshot's: a trade that grows the account between the plan and the simulation leaves the
 * specific message on a refusal the price move caused (its advice, retry, still holds; §5.9).
 */
export function growthRefinement(s: Pick<ChainSnapshot, 'kind' | 'read'>): Pick<TxPlan, 'refineError'> {
    const growth = venueGrowth(s);
    if (growth === null) return {};
    return {
        refineError: (e: TxError): TxError =>
            e.kind === 'program' && e.code === PROCEEDS_BELOW_MIN && failingProgram(e.logs ?? []) === SHORTWIN_PROGRAM
                ? { ...e, message: VENUE_GROWTH_MESSAGES[growth] }
                : e,
    };
}

export interface ClosePlanInput {
    readonly snapshot: ChainSnapshot;
    readonly signer: TransactionSigner;
    readonly symbol: string;
    readonly position: Address;
    readonly q: bigint;
    readonly maxBuybackCost: bigint;
    readonly extras?: ExtrasChoice;
    readonly pick?: RecipientIndex;
}

/** `close_short(max_buyback_cost, extras)` (§3.8, §5.10): the position closes to the borrower. */
export async function planCloseShort(inp: ClosePlanInput): Promise<TxPlan> {
    const s = inp.snapshot;
    const extras = inp.extras ?? NO_EXTRAS;
    const ix = await getCloseShortInstructionAsync({
        borrower: inp.signer,
        position: inp.position,
        mint: s.mint,
        trader: s.market.trader,
        tokenProgram: s.tokenProgram,
        eventAuthority: await eventAuthority(),
        program: SHORTWIN_PROGRAM,
        maxBuybackCost: inp.maxBuybackCost,
        extras: { set: extras.sel.set, bumps: extras.sel.bumps },
    });
    const remaining = await venueAccounts(s, extras, inp.pick);
    const prep = await pammCreatorVaultPrep(inp.signer, s.keys, s.kind, s.read);
    return {
        kind: 'close_short',
        title: `Close short · ${formatTokens(inp.q, s.decimals)} ${tokenLabel(inp.symbol, s.mint)}`,
        instructions: [...(prep ? [prep] : []), withRemaining(ix, remaining)],
        watch: [
            { kind: 'lamports', address: inp.signer.address, label: 'Your SOL' },
            positionWatch(inp.position, s.decimals, inp.symbol, s.mint, ['q', 'v']),
            marketWatch(s.marketAddress, s.decimals, inp.symbol, s.mint),
        ],
        disclosures: [],
        invalidate: invalidations(s, inp.signer.address, inp.position),
    };
}

/** `add_margin(amount)` (§3.8, §5.10): `v` and `m` rise by `amount`, nothing else changes. */
export async function planAddMargin(inp: {
    readonly signer: TransactionSigner;
    readonly market: Address;
    readonly mint: Address;
    readonly position: Address;
    readonly amount: bigint;
    readonly decimals: number;
    readonly symbol: string;
}): Promise<TxPlan> {
    const ix = getAddMarginInstruction({
        borrower: inp.signer,
        market: inp.market,
        position: inp.position,
        eventAuthority: await eventAuthority(),
        program: SHORTWIN_PROGRAM,
        amount: inp.amount,
    });
    return {
        kind: 'add_margin',
        title: `Add margin · ${formatSol(inp.amount)} SOL · ${tokenLabel(inp.symbol, inp.mint)} short`,
        instructions: [ix],
        watch: [
            { kind: 'lamports', address: inp.signer.address, label: 'Your SOL' },
            positionWatch(inp.position, inp.decimals, inp.symbol, inp.mint, ['m', 'v']),
        ],
        disclosures: [],
        invalidate: [
            qk.positionAccount(inp.position),
            qk.position(inp.position),
            qk.marketAccount(inp.mint),
            ['indexer', 'borrower', inp.signer.address],
        ],
    };
}

/**
 * Program errors after which a trade is retried with the next extras set (ARCH §7.1 duty 10): the
 * refusals of the selected set itself — `DeniedAccount` (raised only by the extras parser; since M10
 * batch 3 also for a set naming the borrower or the adopted pool's own accounts), `BadRemainingAccounts`
 * and `BadExtras`. A hostile `current` set then costs a retry, never the trade (M2-SEC-2).
 */
export const EXTRAS_RETRY_CODES: ReadonlySet<number> = new Set([6003, 6004, 6005]);
