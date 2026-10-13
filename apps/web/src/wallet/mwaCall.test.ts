/**
 * R-C1, the way out of a Mobile Wallet Adapter connect the wallet never answers (FRONTEND.md §5.4
 * (6), FE-35; mobile.md §1.10): the guard waits while the user is in the wallet, ends the connect
 * 10 s after the page is back in front (or 120 s after the tap), and resets the library's connect
 * lock through the targeted `standard:disconnect`.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
    guardMwaConnect,
    MWA_CONNECT_CEILING_MS,
    MWA_RETURN_GRACE_MS,
    MWA_SIGN_CANCEL_AFTER_MS,
    type MwaConnectClient,
    resetMwaConnect,
} from './mwaCall.ts';

const MWA = { name: 'Mobile Wallet Adapter' };
const OTHER = { name: 'Phantom' };

function stubClient(initial = 'connecting') {
    const listeners = new Set<() => void>();
    let state = { status: initial, wallets: [OTHER, MWA] };
    const disconnect = vi.fn(async (_wallet?: unknown) => {});
    const client = {
        wallet: {
            getState: () => state,
            subscribe: (l: () => void) => {
                listeners.add(l);
                return () => listeners.delete(l);
            },
            disconnect,
        },
    } as unknown as MwaConnectClient;
    const setStatus = (status: string) => {
        state = { ...state, status };
        for (const l of [...listeners]) l();
    };
    return { client, disconnect, listeners, setStatus };
}

let visibility: DocumentVisibilityState = 'visible';
beforeEach(() => {
    vi.useFakeTimers();
    visibility = 'visible';
    Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => visibility });
});
afterEach(() => {
    vi.useRealTimers();
    delete (document as unknown as Record<string, unknown>).visibilityState;
});

const leave = () => window.dispatchEvent(new Event('blur'));
const comeBack = () => window.dispatchEvent(new Event('focus'));

describe('guardMwaConnect (R-C1)', () => {
    it('the bounds: 5 s before Cancel at Sign, 10 s after a return, 120 s at most', () => {
        expect(MWA_SIGN_CANCEL_AFTER_MS).toBe(5_000);
        expect(MWA_RETURN_GRACE_MS).toBe(10_000);
        expect(MWA_CONNECT_CEILING_MS).toBe(120_000);
    });

    it('never resets while the page never left (a focus alone is not a return)', async () => {
        const s = stubClient();
        guardMwaConnect(s.client);
        comeBack();
        await vi.advanceTimersByTimeAsync(MWA_CONNECT_CEILING_MS - 1);
        expect(s.disconnect).not.toHaveBeenCalled();
    });

    it('blur then focus: resets exactly MWA_RETURN_GRACE_MS after the return', async () => {
        const s = stubClient();
        guardMwaConnect(s.client);
        await vi.advanceTimersByTimeAsync(3_000);
        leave();
        await vi.advanceTimersByTimeAsync(20_000); // in the wallet: no timer runs
        expect(s.disconnect).not.toHaveBeenCalled();
        comeBack();
        await vi.advanceTimersByTimeAsync(MWA_RETURN_GRACE_MS - 1);
        expect(s.disconnect).not.toHaveBeenCalled();
        await vi.advanceTimersByTimeAsync(1);
        expect(s.disconnect.mock.calls).toEqual([[], [MWA]]);
    });

    it('a second blur / focus restarts the grace', async () => {
        const s = stubClient();
        guardMwaConnect(s.client);
        leave();
        comeBack();
        await vi.advanceTimersByTimeAsync(6_000);
        leave();
        await vi.advanceTimersByTimeAsync(30_000);
        comeBack();
        await vi.advanceTimersByTimeAsync(MWA_RETURN_GRACE_MS - 1);
        expect(s.disconnect).not.toHaveBeenCalled();
        await vi.advanceTimersByTimeAsync(1);
        expect(s.disconnect).toHaveBeenCalledTimes(2);
    });

    it('visibilitychange hidden → visible counts as a leave and a return', async () => {
        const s = stubClient();
        guardMwaConnect(s.client);
        visibility = 'hidden';
        document.dispatchEvent(new Event('visibilitychange'));
        await vi.advanceTimersByTimeAsync(15_000);
        visibility = 'visible';
        document.dispatchEvent(new Event('visibilitychange'));
        await vi.advanceTimersByTimeAsync(MWA_RETURN_GRACE_MS);
        expect(s.disconnect.mock.calls).toEqual([[], [MWA]]);
    });

    it('the ceiling ends it MWA_CONNECT_CEILING_MS after the tap without any focus event', async () => {
        const s = stubClient();
        guardMwaConnect(s.client);
        leave();
        await vi.advanceTimersByTimeAsync(MWA_CONNECT_CEILING_MS - 1);
        expect(s.disconnect).not.toHaveBeenCalled();
        await vi.advanceTimersByTimeAsync(1);
        expect(s.disconnect.mock.calls).toEqual([[], [MWA]]);
        // Once: nothing is left to fire.
        comeBack();
        await vi.advanceTimersByTimeAsync(MWA_CONNECT_CEILING_MS);
        expect(s.disconnect).toHaveBeenCalledTimes(2);
        expect(vi.getTimerCount()).toBe(0);
    });

    it('a status away from connecting removes every listener and timer (no reset later)', async () => {
        const s = stubClient();
        const add = vi.spyOn(window, 'addEventListener');
        const remove = vi.spyOn(window, 'removeEventListener');
        guardMwaConnect(s.client);
        expect(vi.getTimerCount()).toBe(1);
        leave();
        comeBack();
        expect(vi.getTimerCount()).toBe(2);
        s.setStatus('connected');
        expect(s.listeners.size).toBe(0);
        expect(vi.getTimerCount()).toBe(0);
        const added = add.mock.calls.map((c) => c[0]).filter((t) => t === 'blur' || t === 'focus');
        const removed = remove.mock.calls.map((c) => c[0]).filter((t) => t === 'blur' || t === 'focus');
        expect(removed.sort()).toEqual(added.sort());
        leave();
        comeBack();
        await vi.advanceTimersByTimeAsync(MWA_CONNECT_CEILING_MS);
        expect(s.disconnect).not.toHaveBeenCalled();
        add.mockRestore();
        remove.mockRestore();
    });

    it('a connect that already failed (not connecting after the dispatch) is not guarded', async () => {
        const s = stubClient('disconnected');
        guardMwaConnect(s.client);
        await Promise.resolve();
        expect(vi.getTimerCount()).toBe(0);
        expect(s.listeners.size).toBe(0);
    });

    it('stop() ends it too', async () => {
        const s = stubClient();
        const stop = guardMwaConnect(s.client);
        stop();
        expect(vi.getTimerCount()).toBe(0);
        await vi.advanceTimersByTimeAsync(MWA_CONNECT_CEILING_MS);
        expect(s.disconnect).not.toHaveBeenCalled();
    });
});

describe('resetMwaConnect', () => {
    it('calls disconnect() and then disconnect(the MWA wallet), in that order', async () => {
        const s = stubClient();
        const order: string[] = [];
        s.disconnect.mockImplementation(async (w?: unknown) => {
            order.push(w ? (w as { name: string }).name : 'untargeted');
        });
        await resetMwaConnect(s.client);
        expect(order).toEqual(['untargeted', 'Mobile Wallet Adapter']);
        expect(s.disconnect.mock.calls[1]?.[0]).toBe(MWA);
    });

    it('a failing disconnect is reported, not thrown', async () => {
        const s = stubClient();
        s.disconnect.mockRejectedValueOnce(new Error('no'));
        const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
        await expect(resetMwaConnect(s.client)).resolves.toBeUndefined();
        expect(warn).toHaveBeenCalledOnce();
        warn.mockRestore();
    });
});
