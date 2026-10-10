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

/** M10-RESP: the market page's trade panel open as a sheet (under 1024 px; ignored above). */
export type MarketSheet = 'trade';
/** M10-RESP: a position's Manage panel open as a sheet, at that section. */
export type PositionSheet = 'add-margin' | 'close';

export interface MarketSearch {
    readonly tab?: MarketTab;
    readonly panel?: MarketPanel;
    readonly action?: LendAction;
    readonly sheet?: MarketSheet;
}

export interface PositionSearch {
    readonly sheet?: PositionSheet;
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
        sheet: pick(s.sheet, ['trade'] as const),
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
    validateSearch: (s: Record<string, unknown>): PositionSearch => ({
        sheet: pick(s.sheet, ['add-margin', 'close'] as const),
    }),
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

/** The vulnerability disclosure policy (M10 Q3): security.txt's `Policy`. */
export const securityRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/security',
    component: lazyRouteComponent(() => import('./features/security/SecurityPolicyPage.tsx'), 'SecurityPolicyPage'),
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
    securityRoute,
]);

export function createAppRouter(queryClient: QueryClient) {
    return createRouter({
        routeTree,
        context: { queryClient },
        defaultPreload: 'intent',
        scrollRestoration: true,
        // The page used to scroll inside `#main`: reset it on navigation as well as the document
        // (M10-RESP §5.1; harmless once the document scrolls).
        scrollToTopSelectors: ['#main'],
    });
}

declare module '@tanstack/react-router' {
    interface Register {
        router: ReturnType<typeof createAppRouter>;
    }
    /** M10-RESP: this history entry was pushed by opening a sheet (closing it goes back). */
    interface HistoryState {
        sheet?: boolean;
        /** The header wallet popup's own entry on a phone (Back closes it; WalletButton). */
        walletSheet?: boolean;
    }
}
