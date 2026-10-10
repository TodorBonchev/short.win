/**
 * Toasts (FRONTEND.md §8): a polite live region for success / info, assertive for errors; each
 * toast stays 6 s in view — time under a dialog or a sheet does not count — (errors until
 * dismissed) and can be dismissed by keyboard. The kind is a word
 * (Confirmed, Failed, Note), not only a colour; a toast may carry links (Explorer, View position).
 * The region is never made inert by a sheet (`data-sheet-keep`, FRONTEND.md §7.6).
 */
import { createContext, type ReactNode, useCallback, useContext, useMemo, useState } from 'react';
import { T } from '../config/testids.ts';

export type ToastKind = 'success' | 'error' | 'info';

export interface ToastLink {
    readonly href: string;
    readonly text: string;
    readonly testId?: string;
}

export interface ToastInput {
    readonly kind: ToastKind;
    readonly title: string;
    readonly body?: ReactNode;
    readonly links?: readonly ToastLink[];
}

const KIND_WORD: Record<ToastKind, string> = { success: 'Confirmed', error: 'Failed', info: 'Note' };

interface ToastItem extends ToastInput {
    readonly id: number;
}

interface ToastApi {
    push(t: ToastInput): void;
    dismiss(id: number): void;
}

const Ctx = createContext<ToastApi | null>(null);

const TOAST_MS = 6_000;
const TOAST_TICK_MS = 500;

/** Whether a toast is (partly) hidden now: under a native dialog's top layer, or an open sheet. */
function covered(): boolean {
    if (typeof document === 'undefined') return false;
    return document.documentElement.hasAttribute('data-sheet-open') || document.querySelector('dialog[open]') !== null;
}
let nextId = 1;

export function ToastProvider({ children }: { children: ReactNode }) {
    const [items, setItems] = useState<ToastItem[]>([]);
    const dismiss = useCallback((id: number) => setItems((xs) => xs.filter((x) => x.id !== id)), []);
    const push = useCallback(
        (t: ToastInput) => {
            const id = nextId++;
            setItems((xs) => [...xs.slice(-4), { ...t, id }]);
            if (t.kind === 'error') return;
            // 6 s in view: time under a review dialog or an open sheet (where a phone shows a toast
            // as a one-line strip without its links) does not count, so the whole toast — View
            // position, Explorer — is still there when the user is back on the page.
            let shown = 0;
            const tick = setInterval(() => {
                if (!covered()) shown += TOAST_TICK_MS;
                if (shown >= TOAST_MS) {
                    clearInterval(tick);
                    dismiss(id);
                }
            }, TOAST_TICK_MS);
        },
        [dismiss],
    );
    const api = useMemo(() => ({ push, dismiss }), [push, dismiss]);
    const polite = items.filter((t) => t.kind !== 'error');
    const assertive = items.filter((t) => t.kind === 'error');
    const render = (t: ToastItem) => (
        <div key={t.id} className="toast" data-kind={t.kind} data-testid={T.toast}>
            <div className="grow">
                <span className="toast-kind">{KIND_WORD[t.kind]}</span>
                <strong>{t.title}</strong>
                {t.body && <div className="secondary">{t.body}</div>}
                {t.links && t.links.length > 0 && (
                    <div className="toast-links">
                        {t.links.map((l) => (
                            <a
                                key={l.href}
                                href={l.href}
                                data-testid={l.testId}
                                {...(/^https?:/.test(l.href) ? { target: '_blank', rel: 'noopener noreferrer' } : {})}
                            >
                                {l.text}
                            </a>
                        ))}
                    </div>
                )}
            </div>
            <button
                type="button"
                className="btn toast-dismiss"
                data-variant="ghost"
                data-size="sm"
                onClick={() => dismiss(t.id)}
                aria-label="Dismiss"
            >
                <span className="toast-dismiss-x" aria-hidden="true">
                    ✕
                </span>
                <span className="toast-dismiss-word" aria-hidden="true">
                    Dismiss
                </span>
            </button>
        </div>
    );
    return (
        <Ctx.Provider value={api}>
            {children}
            <div className="toast-region" data-testid={T.toastRegion} data-sheet-keep="">
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
