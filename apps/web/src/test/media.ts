/**
 * A `matchMedia` for jsdom (which has none): the desktop with a mouse by default, so components
 * render their desktop behaviour; `setMedia` switches to a tablet / phone / touch screen and fires
 * `change` on every live query (FRONTEND.md §7.6, §10.1).
 */

export interface MediaState {
    /** Under 1024 px (tablet or phone). */
    readonly compact: boolean;
    /** Under 640 px (implies `compact`). */
    readonly phone: boolean;
    /** A coarse, non-hovering pointer. */
    readonly coarse: boolean;
}

const DESKTOP: MediaState = { compact: false, phone: false, coarse: false };
let state: MediaState = DESKTOP;

interface Live {
    readonly query: string;
    readonly listeners: Set<(e: MediaQueryListEvent) => void>;
    last: boolean;
}
const live = new Set<Live>();

/** One condition (no `and`): true when the current state matches it. */
function one(c: string): boolean {
    const t = c.trim();
    if (/max-width:\s*399\.98px/.test(t)) return false;
    if (/max-width:\s*639\.98px/.test(t)) return state.phone;
    if (/max-width:\s*(1023\.98|1179\.98|1279\.98)px/.test(t)) return state.compact || state.phone;
    if (/min-width:\s*640px/.test(t)) return !state.phone;
    if (/min-width:\s*(1024|1280)px/.test(t)) return !state.compact && !state.phone;
    if (/min-width:\s*1680px/.test(t)) return false;
    if (/pointer:\s*coarse/.test(t)) return state.coarse;
    if (/hover:\s*hover/.test(t)) return !state.coarse;
    if (/hover:\s*none/.test(t)) return state.coarse;
    return false;
}

function evaluate(query: string): boolean {
    return query.split(/\band\b/).every(one);
}

function matchMedia(query: string): MediaQueryList {
    const entry: Live = { query, listeners: new Set(), last: evaluate(query) };
    live.add(entry);
    const mql = {
        media: query,
        get matches() {
            return evaluate(query);
        },
        onchange: null,
        addEventListener: (_: string, l: (e: MediaQueryListEvent) => void) => entry.listeners.add(l),
        removeEventListener: (_: string, l: (e: MediaQueryListEvent) => void) => entry.listeners.delete(l),
        addListener: (l: (e: MediaQueryListEvent) => void) => entry.listeners.add(l),
        removeListener: (l: (e: MediaQueryListEvent) => void) => entry.listeners.delete(l),
        dispatchEvent: () => true,
    };
    return mql as unknown as MediaQueryList;
}

function notify(): void {
    for (const e of live) {
        const now = evaluate(e.query);
        if (now === e.last) continue;
        e.last = now;
        for (const l of e.listeners) l({ matches: now, media: e.query } as MediaQueryListEvent);
    }
}

/** Emulates a width and input (fires `change` on the queries that flip). */
export function setMedia(next: Partial<MediaState>): void {
    const phone = next.phone ?? state.phone;
    state = { phone, compact: phone || (next.compact ?? state.compact), coarse: next.coarse ?? state.coarse };
    notify();
}

/** Back to the desktop (after each test). */
export function resetMedia(): void {
    state = DESKTOP;
    notify();
}

Object.defineProperty(window, 'matchMedia', { configurable: true, writable: true, value: matchMedia });
