/**
 * The route tree (FRONTEND.md §3.2) — frozen by the scaffold: tracks implement the page components
 * this file imports (their props are part of the contract) and never edit the tree itself. Search
 * parameters are validated here, so a page receives typed, allow-listed values only.
 *
 * Every page is its own chunk (`lazyRouteComponent`): the router loads it before it renders the
 * route (and on hover / focus of a link, `defaultPreload: 'intent'`), so a page never suspends.
 */

import type { Address } from '@solana/kit';
import type { QueryClient } from '@tanstack/react-query';
import {
    createRootRouteWithContext,
    createRoute,
    createRouter,
    lazyRouteComponent,
    redirect,
    useParams,
} from '@tanstack/react-router';
import type { MarketSort, PositionStatus, SortOrder, TierName, VenueName } from './api/types.ts';
import { parseAddress } from './lib/address.ts';
import { AppShell } from './shell/AppShell.tsx';
import { NotFound } from './shell/NotFound.tsx';

export interface RouterContext {
    readonly queryClient: QueryClient;
}

const pick = <V extends string>(v: unknown, allowed: readonly V[]): V | undefined =>
    typeof v === 'string' && (allowed as readonly string[]).includes(v) ? (v as V) : undefined;

const VENUES: readonly VenueName[] = [
    'none',
    'pump_curve',
    'pump_migrating',
    'pump_swap',
    'raydium_v4',
    'raydium_cpmm',
];
const TIERS: readonly TierName[] = ['A', 'B', 'C', 'D'];
const SORTS: readonly MarketSort[] = ['tvl', 'oi', 'util', 'created', 'open_positions'];
const ORDERS: readonly SortOrder[] = ['asc', 'desc'];

export interface MarketsSearch {
    readonly q?: string;
    readonly venue?: VenueName;
    readonly tier?: TierName;
    readonly sort?: MarketSort;
    readonly order?: SortOrder;
    readonly active?: boolean;
}

export type MarketTab = 'overview' | 'positions' | 'lenders' | 'liquidations' | 'pools' | 'coverage' | 'events';
export type MarketPanel = 'short' | 'lend';
export type LendAction = 'deposit' | 'withdraw' | 'claim';

export interface MarketSearch {
    readonly tab?: MarketTab;
    readonly panel?: MarketPanel;
    readonly action?: LendAction;
}

export interface PositionsSearch {
    readonly status?: PositionStatus | 'all';
}

export interface CreateMarketSearch {
    readonly mint?: string;
}

const rootRoute = createRootRouteWithContext<RouterContext>()({
    component: AppShell,
    notFoundComponent: NotFound,
});

const indexRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/',
    beforeLoad: () => {
        throw redirect({ to: '/markets' });
    },
});

export const marketsRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/markets',
    validateSearch: (s: Record<string, unknown>): MarketsSearch => ({
        q: typeof s.q === 'string' && s.q.length <= 64 ? s.q : undefined,
        venue: pick(s.venue, VENUES),
        tier: pick(s.tier, TIERS),
        sort: pick(s.sort, SORTS),
        order: pick(s.order, ORDERS),
        active: s.active === true || s.active === 'true' ? true : undefined,
    }),
    component: lazyRouteComponent(() => import('./features/markets/MarketsPage.tsx'), 'MarketsPage'),
});

export const createMarketRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/markets/new',
    validateSearch: (s: Record<string, unknown>): CreateMarketSearch => ({
        mint: typeof s.mint === 'string' ? (parseAddress(s.mint) ?? undefined) : undefined,
    }),
    component: lazyRouteComponent(() => import('./features/markets/CreateMarketPage.tsx'), 'CreateMarketPage'),
});

const MarketRouteComponent = lazyRouteComponent(() =>
    import('./features/markets/MarketPage.tsx').then(({ MarketPage }) => ({
        default: function MarketRouteComponent() {
            const { mint } = useParams({ from: '/m/$mint' });
            return <MarketPage mint={mint as Address} />;
        },
    })),
);

export const marketRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/m/$mint',
    params: {
        parse: (p: { mint: string }) => {
            const mint = parseAddress(p.mint);
            if (!mint) throw new Error('not a mint address');
            return { mint };
        },
        stringify: (p: { mint: string }) => ({ mint: p.mint }),
    },
    validateSearch: (s: Record<string, unknown>): MarketSearch => ({
        tab: pick(s.tab, ['overview', 'positions', 'lenders', 'liquidations', 'pools', 'coverage', 'events'] as const),
        panel: pick(s.panel, ['short', 'lend'] as const),
        action: pick(s.action, ['deposit', 'withdraw', 'claim'] as const),
    }),
    component: MarketRouteComponent,
    errorComponent: NotFound,
});

export const positionsRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/positions',
    validateSearch: (s: Record<string, unknown>): PositionsSearch => ({
        status: pick(s.status, ['open', 'closed', 'liquidated', 'expired', 'settled', 'all'] as const),
    }),
    component: lazyRouteComponent(() => import('./features/short/PositionsPage.tsx'), 'PositionsPage'),
});

const PositionRouteComponent = lazyRouteComponent(() =>
    import('./features/short/PositionDetailPage.tsx').then(({ PositionDetailPage }) => ({
        default: function PositionRouteComponent() {
            const { position } = useParams({ from: '/positions/$position' });
            return <PositionDetailPage position={position as Address} />;
        },
    })),
);

export const positionRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/positions/$position',
    params: {
        parse: (p: { position: string }) => {
            const position = parseAddress(p.position);
            if (!position) throw new Error('not a position address');
            return { position };
        },
        stringify: (p: { position: string }) => ({ position: p.position }),
    },
    component: PositionRouteComponent,
    errorComponent: NotFound,
});

export const lendRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/lend',
    component: lazyRouteComponent(() => import('./features/lend/LenderDashboardPage.tsx'), 'LenderDashboardPage'),
});

export const statusRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/status',
    component: lazyRouteComponent(() => import('./features/markets/ProtocolStatusPage.tsx'), 'ProtocolStatusPage'),
});

export const routeTree = rootRoute.addChildren([
    indexRoute,
    marketsRoute,
    createMarketRoute,
    marketRoute,
    positionsRoute,
    positionRoute,
    lendRoute,
    statusRoute,
]);

export function createAppRouter(queryClient: QueryClient) {
    return createRouter({
        routeTree,
        context: { queryClient },
        defaultPreload: 'intent',
        scrollRestoration: true,
    });
}

declare module '@tanstack/react-router' {
    interface Register {
        router: ReturnType<typeof createAppRouter>;
    }
}
