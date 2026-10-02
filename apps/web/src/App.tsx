/** Providers (FRONTEND.md §6): Kit client → TanStack Query → indexer live feed → toasts → router. */
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { RouterProvider } from '@tanstack/react-router';
import { IndexerLiveProvider } from './api/live.tsx';
import { ToastProvider } from './components/Toast.tsx';
import { config } from './config/env.ts';
import { createAppRouter } from './router.tsx';
import { createAppClient } from './rpc/client.ts';
import { AppClientProvider } from './rpc/context.tsx';

const queryClient = new QueryClient({
    defaultOptions: {
        queries: { staleTime: 2_000, refetchOnWindowFocus: false },
    },
});
const client = createAppClient(config);
const router = createAppRouter(queryClient);

export function App() {
    return (
        <AppClientProvider client={client}>
            <QueryClientProvider client={queryClient}>
                <IndexerLiveProvider>
                    <ToastProvider>
                        <RouterProvider router={router} />
                    </ToastProvider>
                </IndexerLiveProvider>
            </QueryClientProvider>
        </AppClientProvider>
    );
}
