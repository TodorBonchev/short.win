/** Explicit, separately reviewed cranks to recover an idle market's price observations. */
import { getCrankPriceInstructionAsync, getMarketDecoder, podU128ToBigInt } from '@shortwin/client';
import type { Address } from '@solana/kit';
import { qk } from '../../api/keys.ts';
import { SHORTWIN_PROGRAM } from '../../config/programs.ts';
import { tokenLabel } from '../../lib/displayText.ts';
import type { MathModule } from '../../math/load.ts';
import type { AppRpc } from '../../rpc/client.ts';
import type { TxPlan } from '../../tx/types.ts';
import { COVERAGE_GAP_S } from '../../venues/constants.ts';
import { readMetas } from '../../venues/lists.ts';
import { eventAuthority } from '../../venues/pda.ts';
import { type MarketSnapshot, openPrerequisiteBlocked } from './previews.ts';
import { type ChainSnapshot, CLOCK_SYSVAR, decodeClock, loadSnapshot } from './snapshot.ts';

export type PriceRefreshStep = 'record' | 'wait' | 'integrate';

/**
 * Use confirmed Clock/account state, never elapsed browser time. A pending fresh bucket alone is
 * insufficient: the quote still uses the stored EMA until a later crank integrates that bucket.
 * The state survives reloads and also recognises work another caller has already completed.
 */
export function priceRefreshStep(math: MathModule, s: MarketSnapshot): PriceRefreshStep | null {
    if (s.clockVerified === false || openPrerequisiteBlocked(math, s)) return null;
    const m = s.market;
    const now = s.now > m.lastAccrualTs ? s.now : m.lastAccrualTs;
    const stale = now - m.emaTs > COVERAGE_GAP_S || (podU128ToBigInt(m.lbLo) !== 0n && now - m.lbSec > COVERAGE_GAP_S);
    if (!stale) return null;
    if (podU128ToBigInt(m.bLo) === 0n || now - m.bSec > COVERAGE_GAP_S) return 'record';
    // Both actual Clock and the monotone prologue clock must have advanced past the bucket.
    return s.now > m.bSec && now > m.bSec ? 'integrate' : 'wait';
}

/** One crank only: no trade, token transfer, or automatically scheduled second transaction. */
export async function priceRefreshPlan(
    math: MathModule,
    s: ChainSnapshot,
    owner: Address,
    symbol: string,
): Promise<TxPlan> {
    const step = priceRefreshStep(math, s);
    if (step === null) throw new Error('No price refresh is needed now. Check the market’s current status.');
    if (step === 'wait') throw new Error('Waiting for the chain clock to advance. Review the update again shortly.');
    const ix = await getCrankPriceInstructionAsync({
        mint: s.mint,
        eventAuthority: await eventAuthority(),
        program: SHORTWIN_PROGRAM,
    });
    const timestamp = (label: string) => ({ label, unit: '', scale: 0, kind: 'timestamp' as const });
    return {
        kind: 'crank_price',
        title: `${step === 'record' ? 'Record price observation' : 'Update smoothed price'} · ${tokenLabel(symbol, s.mint)}`,
        instructions: [{ ...ix, accounts: [...ix.accounts, ...readMetas(s.kind, s.readKeys, false)] }],
        watch: [
            { kind: 'lamports', address: owner, label: 'Wallet SOL' },
            {
                kind: 'program',
                address: s.marketAddress,
                label: 'Market price',
                decode: (bytes) => {
                    const m = getMarketDecoder().decode(bytes);
                    return { bSec: m.bSec, lbSec: m.lbSec, emaTs: m.emaTs };
                },
                fields: {
                    bSec: timestamp('Recorded observation'),
                    lbSec: timestamp('Completed observation'),
                    emaTs: timestamp('Smoothed price updated'),
                },
            },
        ],
        disclosures: ['market.priceRefresh'],
        invalidate: [qk.marketAccount(s.mint), qk.market(s.mint), qk.balance(owner)],
        holdSign: (report) => {
            const after = (field: string) =>
                report.deltas.find((d) => d.key === `${s.marketAddress}:${field}`)?.after ?? null;
            const recorded = after('bSec');
            if (recorded === null || recorded < s.now)
                return 'The simulation did not record a fresh price. Close and check the market’s status.';
            if (step === 'integrate') {
                const completed = after('lbSec');
                const updated = after('emaTs');
                if (completed === null || updated === null || completed < s.market.bSec || updated <= s.market.bSec)
                    return 'The fresh observation has not been integrated yet. Refresh the simulation after the chain clock advances.';
                if (recorded - completed > COVERAGE_GAP_S || recorded - updated > COVERAGE_GAP_S)
                    return 'This observation has expired. Close and review a new price observation.';
            }
            return null;
        },
    };
}

/** Re-read before each new review or plan rebuild; an aged review is also held by simulation. */
export async function freshPriceRefreshPlan(
    rpc: AppRpc,
    math: MathModule,
    mint: Address,
    owner: Address,
    symbol: string,
): Promise<TxPlan> {
    const { snapshot, get } = await loadSnapshot(rpc, math, mint);
    if (!decodeClock(get(CLOCK_SYSVAR))) throw new Error('The chain clock could not be read. Try again.');
    if (!snapshot) throw new Error('This market could not be read. Try again.');
    return {
        ...(await priceRefreshPlan(math, snapshot, owner, symbol)),
        rebuild: () => freshPriceRefreshPlan(rpc, math, mint, owner, symbol),
    };
}
