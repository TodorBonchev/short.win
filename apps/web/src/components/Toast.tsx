/**
 * Toasts (FRONTEND.md §8): a polite live region for success / info, assertive for errors; each
 * toast stays 6 s (errors until dismissed) and can be dismissed by keyboard.
 */
import { createContext, type ReactNode, useCallback, useContext, useMemo, useState } from 'react';
import { T } from '../config/testids.ts';

export type ToastKind = 'success' | 'error' | 'info';

export interface ToastInput {
    readonly kind: ToastKind;
    readonly title: string;
    readonly body?: ReactNode;
}

interface ToastItem extends ToastInput {
    readonly id: number;
}

interface ToastApi {
    push(t: ToastInput): void;
    dismiss(id: number): void;
}

const Ctx = createContext<ToastApi | null>(null);
let nextId = 1;

export function ToastProvider({ children }: { children: ReactNode }) {
    const [items, setItems] = useState<ToastItem[]>([]);
    const dismiss = useCallback((id: number) => setItems((xs) => xs.filter((x) => x.id !== id)), []);
    const push = useCallback(
        (t: ToastInput) => {
            const id = nextId++;
            setItems((xs) => [...xs.slice(-4), { ...t, id }]);
            if (t.kind !== 'error') setTimeout(() => dismiss(id), 6_000);
        },
        [dismiss],
    );
    const api = useMemo(() => ({ push, dismiss }), [push, dismiss]);
    const polite = items.filter((t) => t.kind !== 'error');
    const assertive = items.filter((t) => t.kind === 'error');
    const render = (t: ToastItem) => (
        <div key={t.id} className="toast" data-kind={t.kind} data-testid={T.toast}>
            <div className="grow">
                <strong>{t.title}</strong>
                {t.body && <div className="secondary">{t.body}</div>}
            </div>
            <button
                type="button"
                className="btn"
                data-variant="ghost"
                data-size="sm"
                onClick={() => dismiss(t.id)}
                aria-label="Dismiss"
            >
                ✕
            </button>
        </div>
    );
    return (
        <Ctx.Provider value={api}>
            {children}
            <div className="toast-region" data-testid={T.toastRegion}>
                <div role="status" aria-live="polite" className="stack">
                    {polite.map(render)}
                </div>
                <div role="alert" aria-live="assertive" className="stack">
                    {assertive.map(render)}
                </div>
            </div>
        </Ctx.Provider>
    );
}

export function useToast(): ToastApi {
    const c = useContext(Ctx);
    if (!c) throw new Error('useToast outside ToastProvider');
    return c;
}
