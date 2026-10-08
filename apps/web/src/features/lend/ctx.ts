/**
 * What the lend panel's forms share: the chain state of the market (FE-D1), the WASM math, the
 * wallet's signer, the indexer's market detail (display only: symbol, loss history, creator
 * exposure) and the transaction flow; plus small display helpers.
 */
import type { Market, MarketVault } from '@shortwin/client';
import type { Address, TransactionSigner } from '@solana/kit';
import type { MarketDetail, TierName } from '../../api/types.ts';
import { BREAK_EVEN_DAYS } from '../../config/disclosures.ts';
import { parseBps } from '../../lib/amount.ts';
import { formatBps, formatSol } from '../../lib/format.ts';
import { formatUtc } from '../../lib/time.ts';
import type { MathModule } from '../../math/load.ts';
import type { AppRpc } from '../../rpc/client.ts';
import type { TxFlow } from '../../tx/useTxFlow.ts';
import { type LendChain, marketNow, recordLimitOf } from './chain.ts';
import type { LendContext } from './plans.ts';
import type { LendState, SubUnitConversion, UnpaidRentView } from './previews.ts';

export type LoadedChain = LendChain & { readonly market: Market; readonly vault: MarketVault };

export interface LendCtx {
    readonly mint: Address;
    readonly chain: LoadedChain;
    readonly math: MathModule;
    readonly rpc: AppRpc;
    /** Cluster time, unix seconds: the chain's clock at the read plus the wall time since (M9-UX-5). */
    readonly nowS: number;
    readonly symbol: string;
    readonly decimals: number;
    readonly signer: TransactionSigner | null;
    readonly detail: MarketDetail | undefined;
    readonly flow: TxFlow;
    /** The chain read behind the previews is stale (FRONTEND.md §6.6): actions wait for a fresh one. */
    readonly stale?: boolean;
}

/** The previews' inputs from the chain state. */
export function lendStateOf(chain: LoadedChain): LendState {
    return { market: chain.market, vault: chain.vault, lender: chain.lender, records: chain.records };
}

/** `now` for `deposit` / `withdraw` (`market_now`) and for the clock-reading lender instructions. */
export function nowFor(ctx: Pick<LendCtx, 'chain' | 'nowS'>, kind: 'market' | 'clock'): bigint {
    return kind === 'market' ? marketNow(ctx.chain.market, ctx.nowS) : BigInt(ctx.nowS);
}

/** `GlobalConfig.params.lock_grace_s` (ECON §15 default 1 day when the config is unreadable). */
export function lockGraceOf(chain: LendChain): number {
    return chain.config?.params.lockGraceS ?? 86_400;
}

/** The plan context of the connected wallet in a market read by `readLendChain`. */
export function planContextFor(
    c: LoadedChain,
    signer: TransactionSigner,
    display: { readonly symbol: string; readonly decimals: number },
): LendContext {
    if (!c.lenderAddress) throw new Error('no lender address without a wallet');
    return {
        signer,
        mint: c.mint,
        marketAddress: c.marketAddress,
        vaultAddress: c.vaultAddress,
        tokenProgram: c.market.tokenProgram,
        decimals: display.decimals,
        symbol: display.symbol,
        lenderAddress: c.lenderAddress,
        lenderEpoch: c.lender ? c.lender.epoch : null,
        shareEpoch: c.market.shareEpoch,
        recordLimit: recordLimitOf(c.lender),
    };
}

/** The plan context of the connected wallet in the panel's market. */
export function planContextOf(ctx: LendCtx, signer: TransactionSigner): LendContext {
    return planContextFor(ctx.chain, signer, ctx);
}

/** What the plan builders of `build.ts` need from the panel (Review and the review's Retry). */
export function lendEnvOf(ctx: LendCtx, signer: TransactionSigner) {
    return { rpc: ctx.rpc, math: ctx.math, mint: ctx.mint, signer, symbol: ctx.symbol };
}

export const TIER_NAMES: readonly TierName[] = ['A', 'B', 'C', 'D'];

/** The disclosure variables of `lender.lossHistory` and `market.creatorSelfDealing` (placeholders while the indexer has none). */
export function disclosureVarsOf(market: Market, detail: MarketDetail | undefined) {
    const tier = TIER_NAMES[market.tier] ?? 'D';
    const dash = '—';
    return {
        lossHistory: {
            tier,
            breakEvenDays: String(BREAK_EVEN_DAYS[tier]),
            settlements: detail ? String(detail.lender_loss.settlements) : dash,
            realisedLoss: detail ? formatSol(BigInt(detail.lender_loss.realised_lender_loss)) : dash,
        },
        creatorSelfDealing: {
            // The same wording as the market page's Overview (MARKETS) while the indexer is unavailable.
            creatorFee: detail
                ? `${formatSol(BigInt(detail.creator_exposure.creator_fee_income_lamports))} SOL`
                : 'n/a (indexer unavailable)',
        },
    };
}

/** The variables of `lender.unpaidRent` (exact lamports: an exposure can be a few lamports). */
export function unpaidRentVarsOf(v: UnpaidRentView): Readonly<Record<string, string>> {
    return { receivable: formatSol(v.receivable, 9), exposure: formatSol(v.exposure, 9), share: formatBps(v.shareBps) };
}

/** The variables of `lender.subUnitConversion`: when the pool's vest ends (UTC). */
export function subUnitVarsOf(v: SubUnitConversion): Readonly<Record<string, string>> {
    return { vestEnd: formatUtc(v.vestEnd) };
}

/** A raw amount as plain input text (no grouping): `1234.5`. */
export function toInputText(raw: bigint, decimals: number): string {
    if (decimals === 0) return raw.toString();
    const s = raw.toString().padStart(decimals + 1, '0');
    const int = s.slice(0, s.length - decimals);
    const frac = s.slice(s.length - decimals).replace(/0+$/, '');
    return frac ? `${int}.${frac}` : int;
}

/** FE-D6: tolerances are user-adjustable within [0.1 %, 50 %]; default 0.5 %. */
export const DEFAULT_TOLERANCE = '0.5';
export const MIN_TOLERANCE_BPS = 10;
export const MAX_TOLERANCE_BPS = 5_000;

/** The tolerance in bps, or `null` when outside [0.1 %, 50 %]. */
export function parseTolerance(text: string): number | null {
    const bps = parseBps(text, MAX_TOLERANCE_BPS);
    return bps !== null && bps >= MIN_TOLERANCE_BPS ? bps : null;
}
