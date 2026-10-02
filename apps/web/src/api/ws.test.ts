import { describe, expect, it } from 'vitest';
import type { WsServerMessage } from './types.ts';
import { IndexerSocket, isWsChannel, type WebSocketLike, WS_CLOSE_LAGGED, type WsState } from './ws.ts';

const MINT = 'So11111111111111111111111111111111111111112';

class FakeSocket implements WebSocketLike {
    readyState = 0;
    sent: unknown[] = [];
    onopen: ((ev: Event) => void) | null = null;
    onclose: ((ev: CloseEvent) => void) | null = null;
    onerror: ((ev: Event) => void) | null = null;
    onmessage: ((ev: MessageEvent) => void) | null = null;
    send(data: string) {
        this.sent.push(JSON.parse(data));
    }
    close(code = 1000) {
        this.readyState = 3;
        this.onclose?.({ code } as CloseEvent);
    }
    open() {
        this.readyState = 1;
        this.onopen?.({} as Event);
    }
    message(m: WsServerMessage) {
        this.onmessage?.({ data: JSON.stringify(m) } as MessageEvent);
    }
}

function harness() {
    const sockets: FakeSocket[] = [];
    const timers: { fn: () => void; ms: number }[] = [];
    const messages: WsServerMessage[] = [];
    const states: WsState[] = [];
    let resyncs = 0;
    const s = new IndexerSocket({
        url: 'ws://x/v1/ws',
        onMessage: (m) => messages.push(m),
        onState: (st) => states.push(st),
        onResync: () => {
            resyncs++;
        },
        createSocket: () => {
            const f = new FakeSocket();
            sockets.push(f);
            return f;
        },
        random: () => 0,
        setTimer: (fn, ms) => {
            const t = { fn, ms };
            timers.push(t);
            return t;
        },
        clearTimer: (t) => {
            const i = timers.indexOf(t as { fn: () => void; ms: number });
            if (i >= 0) timers.splice(i, 1);
        },
    });
    return { s, sockets, timers, messages, states, resyncs: () => resyncs };
}

describe('IndexerSocket', () => {
    it('validates channel names', () => {
        expect(isWsChannel('markets')).toBe(true);
        expect(isWsChannel(`market:${MINT}`)).toBe(true);
        expect(isWsChannel('market:not-a-key')).toBe(false);
        expect(isWsChannel('everything')).toBe(false);
    });

    it('connects lazily, subscribes once per channel and reference-counts', () => {
        const h = harness();
        h.s.subscribe(['markets']);
        h.s.subscribe(['markets', `market:${MINT}`]);
        expect(h.sockets).toHaveLength(1);
        h.sockets[0]?.open();
        expect(h.sockets[0]?.sent).toEqual([{ op: 'subscribe', channels: ['markets', `market:${MINT}`] }]);
        h.s.unsubscribe(['markets']);
        expect(h.sockets[0]?.sent).toHaveLength(1);
        h.s.unsubscribe(['markets']);
        expect(h.sockets[0]?.sent.at(-1)).toEqual({ op: 'unsubscribe', channels: ['markets'] });
    });

    it('resumes after a reconnect from the last event id and backs off', () => {
        const h = harness();
        h.s.subscribe([`market:${MINT}`]);
        h.sockets[0]?.open();
        h.sockets[0]?.message({
            type: 'event',
            channel: `market:${MINT}`,
            id: 41,
            data: {} as never,
        });
        h.sockets[0]?.close(1006);
        expect(h.states).toContain('reconnecting');
        const retry = h.timers.find((t) => t.ms === 500);
        expect(retry).toBeDefined();
        retry?.fn();
        h.sockets[1]?.open();
        expect(h.sockets[1]?.sent).toEqual([{ op: 'subscribe', channels: [`market:${MINT}`], since: 41 }]);
        expect(h.messages).toHaveLength(1);
    });

    it('asks for a resync after a lagged close, and closes for good', () => {
        const h = harness();
        h.s.subscribe(['stats']);
        h.sockets[0]?.open();
        h.sockets[0]?.close(WS_CLOSE_LAGGED);
        expect(h.resyncs()).toBe(1);
        h.s.close();
        expect(h.s.currentState).toBe('closed');
    });

    it('closes a silent connection after the heartbeat timeout', () => {
        const h = harness();
        h.s.subscribe(['stats']);
        h.sockets[0]?.open();
        const watchdog = h.timers.find((t) => t.ms === 45_000);
        watchdog?.fn();
        expect(h.sockets[0]?.readyState).toBe(3);
        expect(h.states.at(-1)).toBe('reconnecting');
    });
});
