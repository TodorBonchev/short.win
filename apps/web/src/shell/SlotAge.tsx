/**
 * The age of a preview's chain reads (FRONTEND.md §6.6; M9-UX-6): previews are computed on
 * accounts read at some slot; when that read is more than 2 slots behind the confirmed tip (the
 * RPC's slot less the usual 2-slot confirmation lag), the panel says so, and past
 * `STALE_READ_SLOTS` / `STALE_READ_MS` its Submit is disabled until a fresh read arrives — a dead
 * WebSocket or a stalled poll is never silent behind a money number.
 */
import { T } from '../config/testids.ts';
import { useStore } from '../lib/store.ts';
import { rpcSlot } from '../rpc/slot.ts';
import { useNow } from './freshness.tsx';

/** Shown above this many slots of lag. */
export const SHOW_READ_SLOTS = 2n;
/** How far `confirmed` normally trails the slot the RPC reports. */
const CONFIRMED_LAG = 2n;
/** Shown above this age when the read has no slot (a polled list). */
export const SHOW_READ_MS = 10_000;
/** Submit is disabled above this lag (≈ 30 s at 0.4 s a slot) … */
export const STALE_READ_SLOTS = 75n;
/** … or above this age of the read when the slot lag is unknown or the read is older anyway. */
export const STALE_READ_MS = 30_000;

export type ReadAgeState = 'fresh' | 'aging' | 'stale';

export interface ReadAge {
    readonly state: ReadAgeState;
    /** Slots between the read and the RPC's current slot, when both are known. */
    readonly slots: bigint | null;
    readonly seconds: number;
}

/** Pure: the age of a read at `slot` / `readAt` given the RPC's slot and the wall clock. */
export function readAge(
    read: { readonly slot: bigint | null; readonly readAt: number | null },
    rpc: bigint | null,
    nowMs: number,
    live = false,
): ReadAge {
    // A live account subscription keeps the read current while nothing changes.
    if (live) return { state: 'fresh', slots: 0n, seconds: 0 };
    const seconds = read.readAt === null ? 0 : Math.max(0, Math.round((nowMs - read.readAt) / 1000));
    const ms = read.readAt === null ? 0 : Math.max(0, nowMs - read.readAt);
    // The RPC's slot is its newest (processed) one; a `confirmed` read trails it by ~2 slots normally.
    const tip = rpc !== null ? rpc - CONFIRMED_LAG : null;
    const slots = read.slot !== null && tip !== null ? (tip > read.slot ? tip - read.slot : 0n) : null;
    const stale = ms > STALE_READ_MS || (slots !== null && slots > STALE_READ_SLOTS);
    const shown = slots !== null ? slots > SHOW_READ_SLOTS : ms > SHOW_READ_MS;
    return { state: stale ? 'stale' : shown ? 'aging' : 'fresh', slots, seconds };
}

/** The age of a read, ticking every second. */
export function useReadAge(slot: bigint | null, readAt: number | null, live = false): ReadAge {
    const rpc = useStore(rpcSlot);
    const now = useNow(1_000);
    return readAge({ slot, readAt }, rpc.slot, now, live);
}

function ageText(a: ReadAge): string {
    const slots = a.slots !== null ? `${a.slots} slots, ` : '';
    return `${slots}${a.seconds}s old`;
}

/**
 * The indicator: an empty placeholder while fresh (no layout shift as reads land), a muted age
 * while aging, a danger badge (announced) once stale.
 */
export function ReadAgeView({ age, what = 'Chain data' }: { age: ReadAge; what?: string }) {
    if (age.state === 'fresh') {
        return <span className="slot-age" data-testid={T.slotAge} data-state="fresh" aria-hidden="true" />;
    }
    return (
        <span
            className={age.state === 'stale' ? 'badge slot-age' : 'slot-age muted num'}
            data-tone={age.state === 'stale' ? 'danger' : undefined}
            data-testid={T.slotAge}
            data-state={age.state}
            data-slots={age.slots?.toString()}
            role={age.state === 'stale' ? 'status' : undefined}
            title={
                age.state === 'stale'
                    ? 'These numbers come from an old read of the chain; actions wait for a fresh one.'
                    : 'These numbers come from a read of the chain a few slots ago.'
            }
        >
            {what} {ageText(age)}
            {age.state === 'stale' ? ' · waiting for a fresh read' : ''}
        </span>
    );
}

/** A preview's slot age (`slot` / `readAt` of its chain read). */
export function SlotAgeBadge({
    slot,
    readAt,
    live = false,
    what,
}: {
    slot: bigint | null;
    readAt: number | null;
    live?: boolean;
    what?: string;
}) {
    const age = useReadAge(slot, readAt, live);
    return <ReadAgeView age={age} {...(what ? { what } : {})} />;
}

/** The age of a chain read that has no slot (a polled list of accounts). */
export function ReadAgeBadge({ readAt, what }: { readAt: number | null; what?: string }) {
    const age = useReadAge(null, readAt);
    return <ReadAgeView age={age} {...(what ? { what } : {})} />;
}
