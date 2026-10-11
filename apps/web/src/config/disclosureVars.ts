/**
 * Values for the disclosures' placeholders that governance can change (FRONTEND.md §4.1 rule 5;
 * ECON §15 bounds; M9-UX-3): filled from the live `GlobalConfig` on the panels and from the terms
 * a position locked at open on its own pages — never a number written into the copy.
 */
import type { Params, TierParams } from '@shortwin/client';
import { formatBps, formatSol } from '../lib/format.ts';

/** A duration in plain words for the disclosures ("10 days", "72 hours", "1 hour"). */
export function proseDuration(seconds: number): string {
    const unit = (n: number, w: string) => `${n} ${w}${n === 1 ? '' : 's'}`;
    if (seconds % 86_400 === 0) return unit(seconds / 86_400, 'day');
    if (seconds % 3_600 === 0) return unit(seconds / 3_600, 'hour');
    if (seconds % 60 === 0) return unit(seconds / 60, 'minute');
    return unit(seconds, 'second');
}

/** The SOL-settlement keeper reward: "0.01 SOL + 1% of the vault (capped at 0.5 SOL)". */
export function keeperRewardText(fixed: bigint, bps: number, cap: bigint): string {
    return `${formatSol(fixed, 9)} SOL + ${formatBps(bps, 2).replace(/\.00%$/, '%')} of the vault (capped at ${formatSol(cap, 9)} SOL)`;
}

/** A percentage without trailing zero decimals ("2%", "2.5%", "50%"). */
export function percent(bps: number): string {
    // Only zeros after the decimal point go: "50%" stays "50%" (thirteenth round: it read "5%").
    return formatBps(bps, 2)
        .replace(/(\.\d*?)0+%$/, '$1%')
        .replace(/\.%$/, '%');
}

type LenderParams = Pick<Params, 'lockGraceS' | 'lumpVestS'>;

/** `lender.risks`: `{lockGrace}`, `{vestPeriod}` (GlobalConfig `lock_grace_s`, `lump_vest_s`). */
export function lenderRiskVars(p: LenderParams | null | undefined): Record<string, string> {
    return {
        lockGrace: p ? proseDuration(p.lockGraceS) : 'the lock grace period',
        vestPeriod: p ? proseDuration(p.lumpVestS) : 'the vesting period',
    };
}

/** `market.routeBroken`: `{s3Wait}` (GlobalConfig `s3_wait_s`). */
export function routeBrokenVars(p: Pick<Params, 's3WaitS'> | null | undefined): Record<string, string> {
    return { s3Wait: p ? proseDuration(p.s3WaitS) : 'the S3 wait' };
}

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
