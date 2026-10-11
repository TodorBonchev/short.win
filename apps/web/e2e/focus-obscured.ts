/**
 * Focus Not Obscured (Minimum), WCAG 2.2 2.4.11 (FRONTEND.md §8, §10.2): Tab walks over a real
 * page that, for every control that takes focus, check the control is not entirely hidden by
 * author content — the sticky header, the status bar, the phone's tab bar and status strip, a
 * market's action bar. The build keeps room for a focused control with `scroll-padding-top/bottom`
 * on html (styles/base.css) and `scroll-margin` on `:focus-visible`; this holds them to it.
 *
 * Two walks per page: Tab from the top (each stop below the last: the bars at the foot) and
 * Shift+Tab from the end (each stop above the last: the sticky header). A walk ends when focus
 * comes back to a control it already judged.
 *
 * For each focused element: each of its client rects (a link wrapped over two lines has two),
 * clipped to the viewport, is sampled at a 5 × 5 grid (15 % to 85 % of each side, clear of rounded
 * corners, which hit testing honours) and `document.elementFromPoint` must land on the element, one
 * of its descendants or a `<label>` of it at one point at least. No point at all (or nothing on
 * screen) is a failure; some points covered is a note (an annotation and a log line), not a failure
 * — unless a point lands on a **different interactive control** outside the page's bars
 * (the thirteenth round: an ⓘ's enlarged tap area over the Copy button or the ⓘ beside it, so a tap
 * there hits the wrong control), which fails. Covers by the header, the tab bar, the status bar, the
 * action bar or a sticky form foot stay notes (a page scrolls under them by design).
 *
 * Skipped: visually hidden elements (1 px or `clip: rect(0 0 0 0)` — a skip link before it shows,
 * 1 px clipped text; a clipped native input whose `<label>` is the visible control is checked
 * through that label instead), and, while a modal `<dialog>` or a sheet is open, the inert page
 * behind it. `elementFromPoint` ignores `pointer-events: none`, so a control drawn under such an
 * overlay counts as visible, as it is.
 */
import { expect, type Page, test } from '@playwright/test';

/** One focus stop as the page reports it. */
interface Stop {
    /** A short description: tag, test id, text. */
    readonly what: string;
    /**
     * `ok` — every point hits; `partial` — some do; `hidden` — none (or nothing on screen);
     * `overlap` — a point lands on another control outside the page's bars.
     */
    readonly verdict: 'ok' | 'partial' | 'hidden' | 'overlap' | 'skip' | 'end';
    readonly hits: number;
    readonly points: number;
    readonly rect: { x: number; y: number; w: number; h: number };
    readonly why?: string;
}

export interface FocusWalk {
    readonly stops: number;
    readonly failures: readonly string[];
    readonly notes: readonly string[];
}

/** Waits two animation frames: the scroll the focus move started has been laid out and painted. */
async function settle(page: Page): Promise<void> {
    await page.evaluate(() => new Promise<void>((r) => requestAnimationFrame(() => requestAnimationFrame(() => r()))));
}

/** The verdict on the focused element (runs in the page: self-contained). */
function inspectFocus(): Stop {
    const W = window as unknown as { __fnoSeen?: WeakSet<Element> };
    W.__fnoSeen ??= new WeakSet();
    const none = { x: 0, y: 0, w: 0, h: 0 };
    const el = document.activeElement;
    if (!el || el === document.body || el === document.documentElement)
        return { what: 'body', verdict: 'end', hits: 0, points: 0, rect: none, why: 'focus left the page' };
    const tid = el.getAttribute('data-testid');
    const text = (el.getAttribute('aria-label') ?? el.textContent ?? '').replace(/\s+/g, ' ').trim().slice(0, 40);
    const what = `<${el.tagName.toLowerCase()}${tid ? ` ${tid}` : ''}> "${text}"`;
    if (W.__fnoSeen.has(el)) return { what, verdict: 'end', hits: 0, points: 0, rect: none, why: 'the walk wrapped' };
    W.__fnoSeen.add(el);

    // A modal dialog or a sheet makes the page behind it inert: nothing there is a stop to judge.
    const modal = document.querySelector('dialog[open]:modal, .sheet-host[data-sheet="open"]');
    if (modal && !modal.contains(el))
        return { what, verdict: 'skip', hits: 0, points: 0, rect: none, why: 'behind a modal' };

    const clipped = (e: Element): boolean => {
        for (let n: Element | null = e; n; n = n.parentElement) {
            if (/^rect\(0(px)?,? 0(px)?,? 0(px)?,? 0(px)?\)$/.test(getComputedStyle(n).clip)) return true;
        }
        return false;
    };
    const tiny = (r: DOMRect): boolean => r.width <= 1 || r.height <= 1;

    // The element that shows the control: the element itself, or (a clipped native input) its label.
    let targets: Element[] = [el];
    if (tiny(el.getBoundingClientRect()) || clipped(el)) {
        const labels = 'labels' in el ? Array.from((el as HTMLInputElement).labels ?? []) : [];
        const shown = labels.filter((l) => !tiny(l.getBoundingClientRect()) && !clipped(l));
        if (shown.length === 0)
            return { what, verdict: 'skip', hits: 0, points: 0, rect: none, why: 'visually hidden' };
        targets = shown;
    }

    const INTERACTIVE =
        'a[href], button, input:not([type="hidden"]), select, textarea, summary, [role="button"], [role="tab"], ' +
        '[role="menuitem"], [role="link"], [role="checkbox"], [role="radio"], [role="switch"], [role="option"], ' +
        '[tabindex]:not([tabindex="-1"])';
    /**
     * Inside one of the page's bars while it is over the page: the header (and the phone's tab bar),
     * the status bar, an action bar, a form foot, a sheet's head, the toasts — each only while fixed,
     * or sticky and stuck away from its place in the flow (the second approval pass's review: a form
     * foot in the flow is judged). Only these: a control in another fixed or sticky box (an open
     * trade or Manage sheet, the sticky trade panel, a sticky table header, a modal dialog) is
     * judged like any other (the round-13 review). The same list and rule as `collectTapCovers`
     * (responsive/metrics.ts). No walk currently lands a point on a bar's control (FRONTEND.md §7.6).
     */
    const BARS = '.app-header, .nav, .statusbar, .action-bar, .form-foot, .sheet-head, .toast-region';
    const overPage = new Map<Element, boolean>();
    const isOverPage = (bar: Element): boolean => {
        const known = overPage.get(bar);
        if (known !== undefined) return known;
        const pos = getComputedStyle(bar).position;
        let over = pos === 'fixed';
        if (pos === 'sticky' && bar instanceof HTMLElement) {
            const at = bar.getBoundingClientRect();
            const was = bar.style.getPropertyValue('position');
            const prio = bar.style.getPropertyPriority('position');
            bar.style.setProperty('position', 'static', 'important');
            const flow = bar.getBoundingClientRect();
            if (was) bar.style.setProperty('position', was, prio);
            else bar.style.removeProperty('position');
            over = Math.abs(at.top - flow.top) > 0.5 || Math.abs(at.left - flow.left) > 0.5;
        }
        overPage.set(bar, over);
        return over;
    };
    const inBar = (e: Element): boolean => {
        for (let b = e.closest(BARS); b; b = b.parentElement?.closest(BARS) ?? null) if (isOverPage(b)) return true;
        return false;
    };
    /** Another control (not this one, its ancestor or its descendant) outside the page's bars. */
    const otherControl = (hit: Element | null): Element | null => {
        const other = hit?.closest(INTERACTIVE) ?? null;
        if (!other || other === el || other.contains(el) || el.contains(other) || inBar(other)) return null;
        return other;
    };
    const isSame = (hit: Element | null): boolean => {
        if (!hit) return false;
        if (hit === el || el.contains(hit)) return true;
        const label = hit.closest('label');
        return !!label && (label.contains(el) || (label as HTMLLabelElement).control === el);
    };
    const name = (e: Element | null): string => {
        const by = e?.closest('[class], [data-testid]') ?? e;
        if (!by) return 'nothing';
        const cls = by.getAttribute('class');
        const id = by.getAttribute('data-testid');
        return `<${by.tagName.toLowerCase()}${id ? ` ${id}` : ''}${cls ? ` class="${cls}"` : ''}>`;
    };

    const vw = document.documentElement.clientWidth;
    const vh = window.innerHeight;
    const F = [0.15, 0.325, 0.5, 0.675, 0.85];
    let hits = 0;
    let points = 0;
    let cover: Element | null = null;
    let onOther = 0;
    let other: Element | null = null;
    const b = targets[0]?.getBoundingClientRect();
    const box = b
        ? { x: Math.round(b.left), y: Math.round(b.top), w: Math.round(b.width), h: Math.round(b.height) }
        : none;
    for (const t of targets)
        for (const r of Array.from(t.getClientRects())) {
            const x0 = Math.max(r.left, 0);
            const x1 = Math.min(r.right, vw);
            const y0 = Math.max(r.top, 0);
            const y1 = Math.min(r.bottom, vh);
            if (x1 - x0 < 1 || y1 - y0 < 1) continue;
            for (const fx of F)
                for (const fy of F) {
                    const hit = document.elementFromPoint(x0 + (x1 - x0) * fx, y0 + (y1 - y0) * fy);
                    points++;
                    if (isSame(hit)) hits++;
                    else {
                        cover ??= hit;
                        const o = otherControl(hit);
                        if (o) {
                            onOther++;
                            other ??= o;
                        }
                    }
                }
        }
    if (points === 0) return { what, verdict: 'hidden', hits, points, rect: box, why: 'no part on screen' };
    if (hits === 0) return { what, verdict: 'hidden', hits, points, rect: box, why: `covered by ${name(cover)}` };
    if (onOther > 0)
        return {
            what,
            verdict: 'overlap',
            hits,
            points,
            rect: box,
            why: `${onOther} on another control, ${name(other)}`,
        };
    if (hits < points)
        return { what, verdict: 'partial', hits, points, rect: box, why: `partly covered by ${name(cover)}` };
    return { what, verdict: 'ok', hits, points, rect: box };
}

/**
 * Walks focus with Tab (`back`: Shift+Tab) until it comes back to a control already judged (at
 * most `max` stops) and judges every stop. Focus leaves the page past its last control and comes
 * back at its first, so a walk that starts mid-page still covers the whole page, once.
 */
export async function walkFocus(page: Page, back = false, max = 250): Promise<FocusWalk> {
    await page.evaluate(() => {
        (window as unknown as { __fnoSeen?: WeakSet<Element> }).__fnoSeen = new WeakSet();
    });
    const key = back ? 'Shift+Tab' : 'Tab';
    const failures: string[] = [];
    const notes: string[] = [];
    let stops = 0;
    let left = 0;
    for (let i = 0; i < max; i++) {
        await page.keyboard.press(key);
        await settle(page);
        const s = await page.evaluate(inspectFocus);
        if (s.verdict === 'end') {
            if (s.why === 'focus left the page' && left++ === 0) continue;
            break;
        }
        stops++;
        const where = `${key} ${s.what} at (${s.rect.x}, ${s.rect.y}) ${s.rect.w}×${s.rect.h}`;
        if (s.verdict === 'hidden') failures.push(`${where}: entirely hidden (${s.why ?? ''})`);
        else if (s.verdict === 'overlap')
            failures.push(`${where}: ${s.hits}/${s.points} points visible, ${s.why ?? ''} (a tap there hits it)`);
        else if (s.verdict === 'partial') notes.push(`${where}: ${s.hits}/${s.points} points visible (${s.why ?? ''})`);
    }
    return { stops, failures, notes };
}

/**
 * The Tab walk from the top of the current page, then the Shift+Tab walk from its end: no focused
 * control entirely hidden (WCAG 2.2 2.4.11); partial covers are annotated and logged.
 */
export async function expectFocusNeverObscured(page: Page, label: string): Promise<void> {
    await page.evaluate(() => {
        (document.activeElement as HTMLElement | null)?.blur?.();
        window.scrollTo(0, 0);
    });
    const fwd = await walkFocus(page);
    const bwd = await walkFocus(page, true);
    const failures = [...fwd.failures, ...bwd.failures];
    const notes = [...fwd.notes, ...bwd.notes];
    for (const n of notes)
        test.info().annotations.push({ type: 'focus partly covered', description: `${label}: ${n}` });
    const tally = `${label}: ${fwd.stops} stops by Tab, ${bwd.stops} by Shift+Tab; ${failures.length} failing (hidden, or under another control), ${notes.length} partly covered`;
    test.info().annotations.push({ type: 'focus walk', description: tally });
    // The tally in the run's log too (the list reporter shows no annotations).
    console.log(`[focus] ${tally}`);
    for (const f of [...failures, ...notes]) console.log(`[focus]   ${f}`);
    expect(fwd.stops, `${label}: the Tab walk reached no control`).toBeGreaterThan(0);
    expect(bwd.stops, `${label}: the Shift+Tab walk reached no control`).toBeGreaterThan(0);
    expect(
        failures,
        `${label}: focused controls entirely hidden (WCAG 2.2 2.4.11) or partly under another control's tap area`,
    ).toEqual([]);
}
