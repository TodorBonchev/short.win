/** Wall-clock helpers. Chain time comes from the `Clock` the indexer and the accounts report. */
export function nowSeconds(): number {
    return Math.floor(Date.now() / 1000);
}

/** Seconds until `deadline` (unix seconds), never negative. */
export function secondsUntil(deadline: number | bigint, now = nowSeconds()): number {
    return Math.max(0, Number(deadline) - now);
}

/** "2026-10-01 14:03 UTC" */
export function formatUtc(unixSeconds: number | bigint): string {
    const d = new Date(Number(unixSeconds) * 1000);
    return `${d.toISOString().slice(0, 16).replace('T', ' ')} UTC`;
}
