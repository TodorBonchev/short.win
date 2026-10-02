/**
 * SHORT track — the trades a panel submits, built from one chain snapshot (FRONTEND.md §3.7, §3.8,
 * §5.10): the open and the close with their quote bounds (`min_proceeds`, `max_margin`,
 * `max_open_fee`; `max_buyback_cost`) and the venue's extras candidates. Submit builds from the
 * panel's snapshot; the review's Retry (`TxPlan.rebuild`, M9-UX-1) builds the same trade, with the
 * same size and slippage, from a fresh read — a new quote, new bounds, the venue accounts and
 * extras as they are now. A trade the fresh state refuses ends in a `PlanError` with the reason.
 */
import { fetchMaybeShortPosition, type ShortPosition } from '@shortwin/client';
import type { Address, TransactionSigner } from '@solana/kit';
import type { MathModule } from '../../math/load.ts';
import { readExposure } from '../../rpc/accounts.ts';
import type { AppRpc } from '../../rpc/client.ts';
import { PlanError } from '../../tx/errors.ts';
import type { ExtrasChoice } from '../../venues/lists.ts';
import { extrasFor, planCloseShort, planOpenShort } from './plans.ts';
import { assignTier, openBlocked, positionMetrics, previewCloseFor, previewOpen } from './previews.ts';
import { type ChainSnapshot, loadSnapshot } from './snapshot.ts';
import { errorByName } from './ui.tsx';
import type { PlanBuilder } from './useTradeFlow.ts';

/** A trade ready for `useTradeFlow`: its plan builder per extras set and the sets to try. */
export interface Trade {
    readonly build: PlanBuilder;
    readonly candidates: readonly ExtrasChoice[];
}

const because = (name: string) => new PlanError(errorByName(name)?.message ?? name);

async function freshSnapshot(rpc: AppRpc, math: MathModule, mint: Address): Promise<ChainSnapshot> {
    const { snapshot } = await loadSnapshot(rpc, math, mint);
    if (!snapshot) throw new PlanError('This market is no longer readable.');
    return snapshot;
}

// ---- open ---------------------------------------------------------------------------------

export interface OpenTradeInputs {
    readonly q: bigint;
    readonly slippageBps: number;
    readonly signer: TransactionSigner;
    readonly symbol: string;
    /** Told the new position's address once a plan is built. */
    readonly onPosition?: (position: Address) => void;
}

export async function openTrade(
    math: MathModule,
    s: ChainSnapshot,
    walletQ: bigint,
    inp: OpenTradeInputs,
): Promise<Trade> {
    const blocked = openBlocked(math, s);
    if (blocked) throw because(blocked.code);
    const tier = assignTier(math, s);
    const tp = tier === null ? undefined : s.config.tiers[tier];
    if (!tp) throw new PlanError('This market has no tier for new shorts now.');
    const p = previewOpen(math, s, { q: inp.q, slippageBps: inp.slippageBps, walletQ });
    if (!p) throw new PlanError('No quote for this size on the venue now.');
    if (p.refusal) throw because(p.refusal);
    const candidates = await extrasFor(s, 'sell');
    const build: PlanBuilder = async (extras) => {
        const plan = await planOpenShort({
            snapshot: s,
            signer: inp.signer,
            symbol: inp.symbol,
            q: p.q,
            minProceeds: p.minProceeds,
            maxMargin: p.maxMargin,
            maxOpenFee: p.maxOpenFee,
            tier: tp,
            extras,
        });
        inp.onPosition?.(plan.position);
        return plan;
    };
    return { build, candidates };
}

/** The same open on a fresh read of the market and of the wallet's exposure. */
export async function freshOpenTrade(rpc: AppRpc, math: MathModule, mint: Address, inp: OpenTradeInputs) {
    const s = await freshSnapshot(rpc, math, mint);
    const e = await readExposure(rpc, mint, inp.signer.address);
    return openTrade(math, s, e.exists ? e.data.qOpen : 0n, inp);
}

// ---- close --------------------------------------------------------------------------------

export interface CloseTradeInputs {
    readonly position: Address;
    readonly slippageBps: number;
    readonly signer: TransactionSigner;
    readonly symbol: string;
}

export async function closeTrade(
    math: MathModule,
    s: ChainSnapshot,
    pos: ShortPosition,
    inp: CloseTradeInputs,
): Promise<Trade> {
    if (pos.market !== s.marketAddress) throw new PlanError('This position belongs to another market.');
    const p = previewCloseFor(math, s, pos, inp.slippageBps);
    if (!p) {
        const mx = positionMetrics(math, s, pos);
        throw mx.straddle
            ? because('CurveStraddle')
            : mx.insufficientVault
              ? because('InsufficientVault')
              : new PlanError('No buyback quote now.');
    }
    const candidates = await extrasFor(s, 'buy');
    const build: PlanBuilder = (extras) =>
        planCloseShort({
            snapshot: s,
            signer: inp.signer,
            symbol: inp.symbol,
            position: inp.position,
            q: pos.q,
            maxBuybackCost: p.maxBuybackCost,
            extras,
        });
    return { build, candidates };
}

/** The same close on a fresh read of the market and of the position. */
export async function freshCloseTrade(rpc: AppRpc, math: MathModule, mint: Address, inp: CloseTradeInputs) {
    const [s, acc] = await Promise.all([
        freshSnapshot(rpc, math, mint),
        fetchMaybeShortPosition(rpc, inp.position, { commitment: 'confirmed' }),
    ]);
    if (!acc.exists) throw new PlanError('This position is already closed.');
    return closeTrade(math, s, acc.data, inp);
}
