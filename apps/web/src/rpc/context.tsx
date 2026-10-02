import { createContext, type ReactNode, useContext } from 'react';
import type { AppClient } from './client.ts';

const AppClientContext = createContext<AppClient | null>(null);

export function AppClientProvider({ client, children }: { client: AppClient; children: ReactNode }) {
    return <AppClientContext.Provider value={client}>{children}</AppClientContext.Provider>;
}

/** The app's Kit client (RPC, subscriptions, wallet). */
export function useAppClient(): AppClient {
    const c = useContext(AppClientContext);
    if (!c) throw new Error('useAppClient outside AppClientProvider');
    return c;
}
