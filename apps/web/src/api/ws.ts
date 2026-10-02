/**
 * The indexer WebSocket client (INDEXER.md §9.3; FRONTEND.md §6.4): reference-counted channel
 * subscriptions, resume by event id after a reconnect, a heartbeat watchdog (the server sends a
 * heartbeat every 20 s), exponential back-off with jitter, and rollback / lagged signalling so the
 * caller can refetch. Framework-free; `live.tsx` binds it to TanStack Query.
 */
import { cleanIndexerText } from '../lib/displayText.ts';
import type { WsChannel, WsServerMessage } from './types.ts';

export type WsState = 'idle' | 'connecting' | 'open' | 'reconnecting' | 'closed';

const CHANNEL_RE =
    /^(markets|liquidations|admin|coverage|stats|(market|position|lender|trades):[1-9A-HJ-NP-Za-km-z]{32,44})$/;

export function isWsChannel(c: string): c is WsChannel {
    return CHANNEL_RE.test(c);
}

export interface WebSocketLike {
    readonly readyState: number;
    onopen: ((ev: Event) => void) | null;
    onclose: ((ev: CloseEvent) => void) | null;
    onerror: ((ev: Event) => void) | null;
    onmessage: ((ev: MessageEvent) => void) | null;
    send(data: string): void;
    close(code?: number, reason?: string): void;
}

export interface IndexerSocketOptions {
    readonly url: string;
    readonly onMessage: (msg: WsServerMessage) => void;
    readonly onState?: (state: WsState) => void;
    /** Called after a reconnect whose replay may be incomplete (lagged close, or > 1,000 missed events). */
    readonly onResync?: () => void;
    readonly createSocket?: (url: string) => WebSocketLike;
    readonly heartbeatTimeoutMs?: number;
    readonly maxBackoffMs?: number;
    readonly random?: () => number;
    readonly setTimer?: (fn: () => void, ms: number) => unknown;
    readonly clearTimer?: (t: unknown) => void;
}

const OPEN = 1;
/** INDEXER.md §9.3: the server closes a connection whose outbound buffer filled. */
export const WS_CLOSE_LAGGED = 4008;
const MAX_CHANNELS = 50;

export class IndexerSocket {
    private ws: WebSocketLike | null = null;
    private readonly refs = new Map<WsChannel, number>();
    private lastEventId: number | null = null;
    private attempt = 0;
    private watchdog: unknown = null;
    private retryTimer: unknown = null;
    private stopped = false;
    private state: WsState = 'idle';
    private readonly o: Required<Omit<IndexerSocketOptions, 'onState' | 'onResync'>> &
        Pick<IndexerSocketOptions, 'onState' | 'onResync'>;

    constructor(opts: IndexerSocketOptions) {
        this.o = {
            createSocket: (u) => new WebSocket(u) as unknown as WebSocketLike,
            heartbeatTimeoutMs: 45_000,
            maxBackoffMs: 30_000,
            random: Math.random,
            setTimer: (fn, ms) => setTimeout(fn, ms),
            clearTimer: (t) => clearTimeout(t as ReturnType<typeof setTimeout>),
            ...opts,
        };
    }

    get currentState(): WsState {
        return this.state;
    }

    get channels(): WsChannel[] {
        return [...this.refs.keys()];
    }

    /** Adds a reference to each channel; connects on the first one. */
    subscribe(channels: readonly WsChannel[]): void {
        const fresh: WsChannel[] = [];
        for (const c of channels) {
            if (!isWsChannel(c)) throw new Error(`invalid channel ${c}`);
            const n = this.refs.get(c) ?? 0;
            if (n === 0) {
                if (this.refs.size >= MAX_CHANNELS) continue; // server limit; the page falls back to polling
                fresh.push(c);
            }
            this.refs.set(c, n + 1);
        }
        if (this.stopped) return;
        if (!this.ws) this.connect();
        else if (fresh.length && this.ws.readyState === OPEN) this.send({ op: 'subscribe', channels: fresh });
    }

    /** Drops a reference to each channel; unsubscribes a channel at zero. */
    unsubscribe(channels: readonly WsChannel[]): void {
        const gone: WsChannel[] = [];
        for (const c of channels) {
            const n = this.refs.get(c);
            if (n === undefined) continue;
            if (n <= 1) {
                this.refs.delete(c);
                gone.push(c);
            } else this.refs.set(c, n - 1);
        }
        if (gone.length && this.ws?.readyState === OPEN) this.send({ op: 'unsubscribe', channels: gone });
    }

    close(): void {
        this.stopped = true;
        this.clearTimers();
        this.ws?.close(1000, 'client closed');
        this.ws = null;
        this.setState('closed');
    }

    private setState(s: WsState): void {
        if (this.state === s) return;
        this.state = s;
        this.o.onState?.(s);
    }

    private send(msg: unknown): void {
        this.ws?.send(JSON.stringify(msg));
    }

    private clearTimers(): void {
        if (this.watchdog !== null) this.o.clearTimer(this.watchdog);
        if (this.retryTimer !== null) this.o.clearTimer(this.retryTimer);
        this.watchdog = null;
        this.retryTimer = null;
    }

    private armWatchdog(): void {
        if (this.watchdog !== null) this.o.clearTimer(this.watchdog);
        this.watchdog = this.o.setTimer(() => {
            this.ws?.close(4000, 'heartbeat timeout');
        }, this.o.heartbeatTimeoutMs);
    }

    private connect(): void {
        this.setState(this.attempt === 0 ? 'connecting' : 'reconnecting');
        const ws = this.o.createSocket(this.o.url);
        this.ws = ws;
        ws.onopen = () => {
            const resumed = this.attempt > 0;
            this.attempt = 0;
            this.setState('open');
            this.armWatchdog();
            const channels = this.channels;
            if (channels.length) {
                this.send({
                    op: 'subscribe',
                    channels,
                    ...(resumed && this.lastEventId !== null ? { since: this.lastEventId } : {}),
                });
            }
        };
        ws.onmessage = (ev) => {
            this.armWatchdog();
            let msg: WsServerMessage;
            try {
                msg = cleanIndexerText(JSON.parse(String(ev.data)) as WsServerMessage);
            } catch {
                return;
            }
            if (msg.type === 'event') this.lastEventId = Math.max(this.lastEventId ?? 0, msg.id);
            if (msg.type === 'error' && (msg.code === 'unavailable' || msg.code === 'rate_limited'))
                this.o.onResync?.();
            this.o.onMessage(msg);
        };
        ws.onerror = () => {
            // onclose follows
        };
        ws.onclose = (ev) => {
            if (this.ws !== ws) return;
            this.ws = null;
            if (this.watchdog !== null) this.o.clearTimer(this.watchdog);
            this.watchdog = null;
            if (this.stopped) return;
            if (ev.code === WS_CLOSE_LAGGED) this.o.onResync?.();
            this.scheduleReconnect(ev.code === 1008 ? 4 : 0);
        };
    }

    private scheduleReconnect(extraSteps: number): void {
        this.attempt += 1;
        this.setState('reconnecting');
        const exp = Math.min(this.o.maxBackoffMs, 1000 * 2 ** Math.min(this.attempt - 1 + extraSteps, 10));
        const delay = Math.round(exp / 2 + (this.o.random() * exp) / 2);
        this.retryTimer = this.o.setTimer(() => {
            this.retryTimer = null;
            if (!this.stopped && this.refs.size > 0) this.connect();
        }, delay);
    }
}
