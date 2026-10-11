/**
 * The other lenders a claim may sync first (FRONTEND.md §3.6, KI66-R4, FE-30), for Lend → Claim
 * and the dashboard's claims alike: the indexer's `/v1/markets/{mint}/lenders` (its default
 * `active` list, lenders with shares > 0), filtered by `epoch` — `behind` the market's share epoch
 * or `current` — a page at a time (`gatherPotSyncs` reads the behind lenders first).
 */
import { isAddress } from '@solana/kit';
import type { QueryClient } from '@tanstack/react-query';
import { indexerQueries } from '../../../api/queries.ts';
import type { OtherLenders } from '../quote.ts';

/**
 * `once`: each page asked for once, without the indexer queries' retries (the Claim button's
 * background search: a failing indexer then frees its queue slot within one request, and the
 * search counts as cut short, so the review searches again with the retries).
 */
export function indexerLenders(qc: QueryClient, mint: string, once = false): OtherLenders {
    return async (q) => {
        const query = indexerQueries.marketLendersPage(mint, q);
        const page = await qc.fetchQuery(once ? { ...query, retry: false } : query);
        return {
            lenders: page.data.data.flatMap((l) => (isAddress(l.owner) ? [{ owner: l.owner, epoch: l.epoch }] : [])),
            next: page.data.next_cursor,
        };
    };
}
