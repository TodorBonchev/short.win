/**
 * SHORT track — the transaction flow of a trade (`open_short`, `close_short`) with the venue's
 * extras fallback (ARCH §7.1 duty 10, A41; FRONTEND.md §5.1): the plan is built with the `current`
 * extras set; when its simulation fails with `BadExtras` / `BadRemainingAccounts`, it is rebuilt
 * with the next set (`none`, `previous`, `stable`) and simulated again — each set once. Everything
 * else is the scaffold's `useTxFlow` (simulate, review, sign, confirm).
 *
 * With `refresh` (the same trade from a fresh chain read), every plan carries `rebuild`: the
 * review's Retry after a moved quote re-reads, re-quotes and restarts the extras sets (M9-UX-1).
 */
import { useCallback, useEffect, useRef, useState } from 'react';
import type { TxPlan } from '../../tx/types.ts';
import { type TxFlow, useTxFlow } from '../../tx/useTxFlow.ts';
import type { ExtrasChoice } from '../../venues/lists.ts';
import { EXTRAS_RETRY_CODES } from './plans.ts';

export type PlanBuilder = (extras: ExtrasChoice) => Promise<TxPlan>;

/** The same trade built from a fresh chain read (`trades.ts`). */
export type TradeRefresh = () => Promise<{ build: PlanBuilder; candidates: readonly ExtrasChoice[] }>;

export interface TradeFlow {
    readonly flow: TxFlow;
    /** Builds the plan with the first extras set and starts the flow. */
    start(build: PlanBuilder, candidates: readonly ExtrasChoice[], refresh?: TradeRefresh): void;
    readonly building: boolean;
    /** Why the plan could not be built (e.g. the venue cannot trade), else `null`. */
    readonly buildError: string | null;
    /** The extras set the current attempt uses when it is not the first one tried. */
    readonly fallback: ExtrasChoice['label'] | null;
}

export function useTradeFlow(): TradeFlow {
    const flow = useTxFlow();
    const [building, setBuilding] = useState(false);
    const [buildError, setBuildError] = useState<string | null>(null);
    const [fallback, setFallback] = useState<ExtrasChoice['label'] | null>(null);
    const ctx = useRef<{
        build: PlanBuilder;
        candidates: readonly ExtrasChoice[];
        index: number;
        refresh?: TradeRefresh | undefined;
    } | null>(null);
    const { start: startFlow } = flow;

    /** Attaches `rebuild`: a fresh trade, its first extras set, and `rebuild` again. */
    const withRebuild = useCallback((plan: TxPlan): TxPlan => {
        const refresh = ctx.current?.refresh;
        if (!refresh) return plan;
        const rebuild = async (): Promise<TxPlan> => {
            const next = await refresh();
            const first = next.candidates[0];
            if (!first) throw new Error('no venue accounts for this trade');
            ctx.current = { build: next.build, candidates: next.candidates, index: 0, refresh };
            setFallback(null);
            return { ...(await next.build(first)), rebuild };
        };
        return { ...plan, rebuild };
    }, []);

    const run = useCallback(
        async (index: number) => {
            const c = ctx.current;
            const extras = c?.candidates[index];
            if (!c || !extras) return;
            c.index = index;
            setBuilding(true);
            setBuildError(null);
            try {
                const plan = withRebuild(await c.build(extras));
                setFallback(index === 0 ? null : extras.label);
                startFlow(plan);
            } catch (e) {
                setBuildError(e instanceof Error ? e.message : String(e));
            } finally {
                setBuilding(false);
            }
        },
        [startFlow, withRebuild],
    );

    const start = useCallback(
        (build: PlanBuilder, candidates: readonly ExtrasChoice[], refresh?: TradeRefresh) => {
            ctx.current = { build, candidates, index: 0, refresh };
            void run(0);
        },
        [run],
    );

    const { stage, error } = flow.state;
    useEffect(() => {
        const c = ctx.current;
        if (stage !== 'failed' || !c || error?.kind !== 'program' || error.code === undefined) return;
        if (!EXTRAS_RETRY_CODES.has(error.code) || c.index + 1 >= c.candidates.length) return;
        void run(c.index + 1);
    }, [stage, error, run]);

    return { flow, start, building, buildError, fallback };
}
