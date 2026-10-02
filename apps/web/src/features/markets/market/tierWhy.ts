/**
 * "Why this tier" (FRONTEND.md §3.4; SPEC §4, O13): the tier inputs a user can check — the venue
 * rule, the pool's age and where it is measured from, the liquidity thresholds in USD and SOL, and
 * the SOL/USD price the program converts them at. The tier itself is computed on-chain at every
 * trade (`tier::assign`, ARCH §4.7); this explains it, it never recomputes it.
 */
import type { TierName, VenueName } from '../../../api/types.ts';
import { formatDuration, formatFixed } from '../../../lib/format.ts';
import { formatUtc } from '../../../lib/time.ts';

/** `tier::AGE_C_S`: below this age a pool tiers C (SPEC §4 "graduated < 24 h ago"). */
export const AGE_C_S = 86_400;
/** `tier::AGE_A_S`: Tier A needs at least this age (SPEC §4 "age ≥ 7 days"). */
export const AGE_A_S = 604_800;
/** Listing and graduation within this many seconds count as "measured from the listing". */
const SAME_TIME_S = 120;

export interface TierWhyInput {
    readonly venue: VenueName;
    readonly tier: TierName;
    readonly now: number;
    readonly graduatedAt: number | null;
    readonly createdTs: number | null;
    /** `GlobalConfig.params.theta_{a,b}_usd_e6`. */
    readonly thetaAUsdE6: bigint | null;
    readonly thetaBUsdE6: bigint | null;
    /** `OracleCache`: SOL/USD mantissa, confidence, exponent, publish time. */
    readonly oracle: {
        readonly price: bigint;
        readonly conf: bigint;
        readonly expo: number;
        readonly publishTime: number;
    } | null;
    /** `GlobalConfig.params.oracle_stale_s`. */
    readonly oracleStaleS: number | null;
}

export interface TierWhyLine {
    readonly key: 'venue' | 'age' | 'liquidity' | 'oracle';
    readonly label: string;
    readonly value: string;
    readonly effect: string;
}

export interface TierWhy {
    readonly lines: readonly TierWhyLine[];
    /** The best tier the pool's age alone allows right now (`null`: no age cap). */
    readonly ageCap: TierName | null;
    readonly ageS: number | null;
    readonly ageBasis: 'listing' | 'graduation' | 'pool-creation' | 'unknown';
}

/** A Pyth mantissa × 10^expo as a decimal string with ≤ 2 fraction digits. */
export function oracleUsd(price: bigint, expo: number): string {
    if (expo >= 0) return formatFixed(price * 10n ** BigInt(expo), 0, 0);
    return formatFixed(price, -expo, 2);
}

function usdE6(v: bigint): string {
    return `$${formatFixed(v, 6, 0)}`;
}

/** θ (USD·10⁶) in SOL at the conservative price `price − conf` (as the program converts it). */
function thetaSol(thetaE6: bigint, price: bigint, conf: bigint, expo: number): string | null {
    const p = price - conf;
    if (p <= 0n) return null;
    // SOL = θ_e6 / 10⁶ / (p · 10^expo)  → lamports = θ_e6 · 10³ · 10^(−expo) / p
    const lamports =
        expo <= 0 ? (thetaE6 * 1_000n * 10n ** BigInt(-expo)) / p : (thetaE6 * 1_000n) / (p * 10n ** BigInt(expo));
    return `≈ ${formatFixed(lamports, 9, 0)} SOL`;
}

export function explainTier(i: TierWhyInput): TierWhy {
    const lines: TierWhyLine[] = [];
    if (i.venue === 'pump_curve') {
        lines.push({
            key: 'venue',
            label: 'Venue',
            value: 'Pump.fun bonding curve',
            effect: 'Every token still on its bonding curve is Tier D.',
        });
    } else if (i.venue === 'none' || i.venue === 'pump_migrating' || i.venue === 'unknown') {
        lines.push({
            key: 'venue',
            label: 'Venue',
            value: i.venue === 'pump_migrating' ? 'Migrating to its pool' : 'No priceable pool',
            effect: 'New shorts are not possible on this venue now; the tier applies once they are.',
        });
    } else {
        lines.push({
            key: 'venue',
            label: 'Venue',
            value: 'Graduated pool',
            effect: 'Tiers A–C by the pool’s age and its locked liquidity.',
        });
    }

    let ageS: number | null = null;
    let ageBasis: TierWhy['ageBasis'] = 'unknown';
    let ageCap: TierName | null = null;
    if (i.venue !== 'pump_curve') {
        if (i.graduatedAt === null || i.graduatedAt <= 0) {
            ageCap = 'C';
            lines.push({
                key: 'age',
                label: 'Pool age',
                value: 'Unknown',
                effect: 'Without a provable start time the pool tiers C.',
            });
        } else {
            ageS = Math.max(0, i.now - i.graduatedAt);
            const created = i.createdTs ?? i.graduatedAt;
            ageBasis =
                Math.abs(i.graduatedAt - created) <= SAME_TIME_S
                    ? 'listing'
                    : i.graduatedAt < created
                      ? 'pool-creation'
                      : 'graduation';
            const basisText =
                ageBasis === 'listing'
                    ? 'measured from this market’s listing'
                    : ageBasis === 'pool-creation'
                      ? 'measured from the pool’s recorded creation time'
                      : 'measured from the graduation short.win observed';
            let effect: string;
            if (ageS < AGE_C_S) {
                ageCap = 'C';
                effect = `At most Tier C until ${formatUtc(i.graduatedAt + AGE_C_S)}.`;
            } else if (ageS < AGE_A_S) {
                ageCap = 'B';
                effect = `At most Tier B until ${formatUtc(i.graduatedAt + AGE_A_S)}.`;
            } else {
                effect = 'Old enough for Tier A (7 days).';
            }
            lines.push({
                key: 'age',
                label: 'Pool age',
                value: `${formatDuration(ageS)}, ${basisText} (${formatUtc(i.graduatedAt)})`,
                effect,
            });
        }

        if (i.thetaAUsdE6 !== null && i.thetaBUsdE6 !== null) {
            const solA = i.oracle ? thetaSol(i.thetaAUsdE6, i.oracle.price, i.oracle.conf, i.oracle.expo) : null;
            const solB = i.oracle ? thetaSol(i.thetaBUsdE6, i.oracle.price, i.oracle.conf, i.oracle.expo) : null;
            lines.push({
                key: 'liquidity',
                label: 'Locked liquidity',
                value: `Tier A ≥ ${usdE6(i.thetaAUsdE6)}${solA ? ` (${solA})` : ''}, Tier B ≥ ${usdE6(i.thetaBUsdE6)}${solB ? ` (${solB})` : ''}`,
                effect: 'Pool liquidity counts only what is locked (burned LP) and is measured on-chain at every trade.',
            });
        }

        if (i.oracle && (i.oracle.publishTime <= 0 || i.oracle.price <= 0n)) {
            // `OracleCache` exists but has never been cranked (`crank_sol_usd`): no reading to age.
            lines.push({
                key: 'oracle',
                label: 'SOL/USD',
                value: 'No reading yet',
                effect: 'Until the SOL/USD price is cranked, tiers stay at C.',
            });
        } else if (i.oracle) {
            const age = Math.max(0, i.now - i.oracle.publishTime);
            const stale = i.oracleStaleS !== null && age > i.oracleStaleS;
            lines.push({
                key: 'oracle',
                label: 'SOL/USD',
                value: `$${oracleUsd(i.oracle.price, i.oracle.expo)}, ${formatDuration(age)} old`,
                effect: stale
                    ? `Older than ${formatDuration(i.oracleStaleS ?? 0)}: tiers fall back to C until it is refreshed.`
                    : 'The thresholds convert at the low end of its confidence interval.',
            });
        }
    }
    return { lines, ageCap, ageS, ageBasis };
}
