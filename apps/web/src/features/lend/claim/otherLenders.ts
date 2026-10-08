/**
 * The other lenders a claim may sync first (FRONTEND.md §3.6, KI66-R4), for Lend → Claim and the
 * dashboard's claims alike: the owners on the first page of the indexer's
 * `/v1/markets/{mint}/lenders` — its default `active` list, lenders with shares > 0, largest raw
 * share count first (`claimPlanFrom` reads at most `MAX_SYNC_CANDIDATES` of them).
 */
import { type Address, isAddress } from '@solana/kit';
import type { QueryClient } from '@tanstack/react-query';
import { indexerQueries } from '../../../api/queries.ts';
import type { OtherLenders } from '../build.ts';

export function indexerLenders(qc: QueryClient, mint: string): OtherLenders {
    return async () => {
        const page = await qc.fetchQuery(indexerQueries.marketLenders(mint));
        return page.data.data.map((l) => l.owner).filter((o): o is Address => isAddress(o));
    };
}
