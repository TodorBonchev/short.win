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

export function indexerLenders(qc: QueryClient, mint: string): OtherLenders {
    return async (q) => {
        const page = await qc.fetchQuery(indexerQueries.marketLendersPage(mint, q));
        return {
            lenders: page.data.data.flatMap((l) => (isAddress(l.owner) ? [{ owner: l.owner, epoch: l.epoch }] : [])),
            next: page.data.next_cursor,
        };
    };
}
