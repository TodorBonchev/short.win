/**
 * Duplicate tickers (R-MINT §10.11; M9-SEC-5): a symbol is untrusted text and any number of mints
 * can carry it. The markets table marks duplicates among its rows; outside it (the market header,
 * search results) the indexer's search for the symbol tells whether another listed market uses it.
 */
import type { Address } from '@solana/kit';
import { useQuery } from '@tanstack/react-query';
import { indexerQueries } from '../../../api/queries.ts';

export { duplicateMints } from '../../../lib/displayText.ts';

/** Whether another listed market uses `symbol` (case-insensitive), per the indexer's search. */
export function useDuplicateSymbol(symbol: string | null, mint: Address): boolean {
    const q = symbol?.trim() ?? '';
    const res = useQuery({ ...indexerQueries.search(q), enabled: q.length > 0 && q.length <= 64, retry: false });
    const hits = res.data?.data.data ?? [];
    return hits.some((h) => h.mint !== mint && !!h.symbol && h.symbol.toUpperCase() === q.toUpperCase());
}
