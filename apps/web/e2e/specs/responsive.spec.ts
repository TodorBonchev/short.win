/**
 * responsive — every page and state at six viewports (FRONTEND.md §7, §10.5): phone-s 360×780 and
 * phone 390×844 (mobile, touch, DPR 2 / 3), tablet 768×1024 (touch), laptop-s 1024×768, laptop
 * 1280×800 and desktop 1440×900. For each state it writes a full-page PNG to
 * `$SHORTWIN_SHOTS_DIR/<viewport>/<name>.png` (on a phone also `-screen`, the first screen at load;
 * an open dialog: the viewport, plus `-full` with the dialog's own scroll area unrolled) and records
 * layout metrics (`e2e/responsive/metrics.ts`) and the axe violations per viewport in
 * `$SHORTWIN_SHOTS_DIR/metrics.json`. A control a tap cannot hit (Playwright's hit test) is clicked
 * through the DOM so the states behind it are still captured, and noted on the next shot.
 *
 * A measuring pass by default: findings never fail it (a scenario step that cannot run is recorded
 * with its error and a PNG of where it stopped); only the fixtures' CSP check can. With
 * `SHORTWIN_RESPONSIVE_GATE=1` it is the M10-RESP gate (FRONTEND.md §10.5): each test fails on any
 * state with sideways document overflow, a layout wider than the screen, a zoomed-out phone, an
 * element off screen, an axe violation, a target under 24 px, a primary control under 44 px with a
 * coarse pointer, text under 12 px (at every viewport since the thirteenth round's O-4), a control
 * under another control's tap area (`collectTapCovers`, the thirteenth round's last review), a failed
 * step or a DOM-click fallback. The
 * viewports also include 320 × 640 and a landscape phone (844 × 390). Excluded from the default
 * run (`@responsive`; playwright.config.ts) — it takes ≈ 6 × the screens spec:
 *
 *   SHORTWIN_E2E_RESPONSIVE=1 SHORTWIN_SHOTS_DIR=<dir> pnpm --filter web e2e --grep @responsive
 *
 * `SHORTWIN_SHOTS_DIR` defaults to `apps/web/.e2e/responsive`; `SHORTWIN_SHOTS_VIEWPORTS=phone,desktop`
 * limits the viewports. `node e2e/responsive/summary.ts <dir>` prints the per-viewport counts.
 *
 * Chain writes: the lend screens' `accrue` (as the screens spec; on the pot market too, for the
 * shortfall states), and one 1-token deposit by the `errors` wallet per viewport (the confirmed review
 * and its toast); once every viewport has captured the pot market with its only lender, a second
 * pass (all viewports again, after the first) has `potLate` lend there and `potBorrower` close, so a
 * second lender's surplus can fund `potClaimer`'s shortfall (the two-line Claim label); everything
 * else is opened and cancelled.
 */
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import AxeBuilder from '@axe-core/playwright';
import { type CDPSession, devices, type Locator, type Page, type Route } from '@playwright/test';
import { SHORT_PHONE } from '../../src/styles/breakpoints.ts';
import { E2E_DIR, URLS } from '../env.ts';
import { type App, E2E_WALLET_NAME, expect, need, T, test } from '../fixtures.ts';
import {
    type AxeSummary,
    collectLayout,
    collectTapCovers,
    type MetricsFile,
    type ShotRecord,
    summarise,
    type TapCovers,
    type Viewport,
} from '../responsive/metrics.ts';
import {
    accrue,
    closeShortFromNode,
    lenderPosition,
    lendFromNode,
    shortPosition,
    simulatePotSync,
} from '../validator/actions.ts';
import { signerFor } from '../validator/keys.ts';
import type { E2EState } from '../validator/state.ts';

export const VIEWPORTS: readonly Viewport[] = [
    {
        name: 'phone-s',
        width: 360,
        height: 780,
        isMobile: true,
        hasTouch: true,
        deviceScaleFactor: 2,
        userAgent: devices['Pixel 7'].userAgent,
    },
    {
        name: 'phone',
        width: 390,
        height: 844,
        isMobile: true,
        hasTouch: true,
        deviceScaleFactor: 3,
        userAgent: devices['iPhone 13'].userAgent,
    },
    {
        name: 'tablet',
        width: 768,
        height: 1024,
        isMobile: false,
        hasTouch: true,
        deviceScaleFactor: 2,
        userAgent: devices['iPad Mini'].userAgent,
    },
    {
        name: 'phone-xs',
        width: 320,
        height: 640,
        isMobile: true,
        hasTouch: true,
        deviceScaleFactor: 2,
        userAgent: devices['Pixel 7'].userAgent,
    },
    {
        name: 'phone-land',
        width: 844,
        height: 390,
        isMobile: true,
        hasTouch: true,
        deviceScaleFactor: 3,
        userAgent: devices['iPhone 13'].userAgent,
    },
    { name: 'laptop-s', width: 1024, height: 768, isMobile: false, hasTouch: false, deviceScaleFactor: 1 },
    { name: 'laptop', width: 1280, height: 800, isMobile: false, hasTouch: false, deviceScaleFactor: 1 },
    { name: 'desktop', width: 1440, height: 900, isMobile: false, hasTouch: false, deviceScaleFactor: 1 },
];

const DIR = process.env.SHORTWIN_SHOTS_DIR || join(E2E_DIR, 'responsive');
const ONLY = (process.env.SHORTWIN_SHOTS_VIEWPORTS ?? '')
    .split(',')
    .map((s) => s.trim())
    .filter(Boolean);
const METRICS = join(DIR, 'metrics.json');
/** M10-RESP: fail on findings (see the header). */
const GATE = process.env.SHORTWIN_RESPONSIVE_GATE === '1';
/** The gate's findings in the running test (one worker: a module-level list is safe). */
const gateFindings: string[] = [];
/** Chromium's capture limit is ~16k device px: the tallest full-page PNG per DPR. */
const MAX_DEVICE_PX = 16_000;
/**
 * The media queries that name a height or an orientation (`src/styles/breakpoints.ts` SHORT_PHONE
 * and the landscape-phone rules of components.css and markets.css, as they are written: the
 * orientation only counts in the tablet tier with a coarse pointer): the unrolled capture must
 * leave them as they were. A laptop or desktop page taller than wide is still unrolled.
 */
const LANDSCAPE_PHONE =
    '(min-width: 640px) and (max-width: 1023.98px) and (pointer: coarse) and (orientation: landscape)';
const HEIGHT_QUERIES = [SHORT_PHONE, LANDSCAPE_PHONE];
const heightQueries = (qs: readonly string[]): string => qs.map((q) => (matchMedia(q).matches ? 1 : 0)).join('');
/** In the page: an open sheet (`Sheet.tsx`) whose body scrolls (its lower part is out of view). */
const openSheetScrolls = (): boolean => {
    const sheet = document.querySelector('.sheet-host[data-sheet="open"]');
    return !!sheet && sheet.scrollHeight > sheet.clientHeight + 1;
};

/**
 * In the page: a device-view slice's step, 75 % of what the scrolled box shows between its bars —
 * the open sheet (else the viewport) less the page's bars (the list of `collectTapCovers`) fixed
 * or stuck over its top or its bottom.
 */
const sliceStep = (): number => {
    const sheet = document.querySelector('.sheet-host[data-sheet="open"]');
    const box = sheet?.getBoundingClientRect() ?? { top: 0, bottom: window.innerHeight };
    let top = box.top;
    let bottom = box.bottom;
    const BARS = '.app-header, .nav, .statusbar, .action-bar, .form-foot, .sheet-head, .toast-region';
    for (const b of document.querySelectorAll(BARS)) {
        const pos = getComputedStyle(b).position;
        if ((pos !== 'fixed' && pos !== 'sticky') || !b.checkVisibility()) continue;
        const r = b.getBoundingClientRect();
        if (r.height < 1 || r.bottom <= box.top || r.top >= box.bottom) continue;
        if (r.top <= box.top + 1) top = Math.max(top, r.bottom);
        else if (r.bottom >= box.bottom - 1) bottom = Math.min(bottom, r.top);
    }
    return Math.max(40, Math.floor((bottom - top) * 0.75));
};

/** In the page: scrolls the open sheet's body (else the document) to `y`, clamped. */
const scrollSliceTo = (y: number): { at: number; max: number } => {
    const sheet = document.querySelector('.sheet-host[data-sheet="open"]');
    if (sheet) {
        const max = Math.max(0, sheet.scrollHeight - sheet.clientHeight);
        sheet.scrollTo({ top: Math.min(y, max), behavior: 'instant' });
        return { at: sheet.scrollTop, max };
    }
    const max = Math.max(0, document.documentElement.scrollHeight - window.innerHeight);
    window.scrollTo({ top: Math.min(y, max), behavior: 'instant' });
    return { at: window.scrollY, max };
};

const AXE_TAGS = ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'];

function loadMetrics(): MetricsFile {
    if (existsSync(METRICS)) {
        try {
            return JSON.parse(readFileSync(METRICS, 'utf8')) as MetricsFile;
        } catch {
            // a torn file from an aborted run: start over
        }
    }
    return { version: 1, updatedAt: '', viewports: {}, summary: {}, worst: [], shots: {} };
}

/** Records one shot (one worker, serial: a read-modify-write is safe). */
function record(vp: Viewport, r: ShotRecord): void {
    const m = loadMetrics();
    const shots = { ...m.shots, [vp.name]: { ...(m.shots[vp.name] ?? {}), [r.name]: r } };
    const next: MetricsFile = {
        version: 1,
        updatedAt: new Date().toISOString(),
        viewports: { ...m.viewports, [vp.name]: vp },
        ...summarise(shots),
        shots,
    };
    writeFileSync(METRICS, `${JSON.stringify(next, null, 2)}\n`);
}

/** The first line of an error, without the terminal colours of Playwright's messages. */
function firstLine(e: unknown): string {
    const m = e instanceof Error ? e.message : String(e);
    // biome-ignore lint/suspicious/noControlCharactersInRegex: strips ANSI colour codes
    return (m.replace(/\u001b\[[0-9;]*m/g, '').split('\n')[0] ?? 'error').slice(0, 300);
}

interface ShotOptions {
    /** Wait for the skeleton rows to go (default true; a loading state passes false). */
    readonly settle?: boolean;
    /** An overlay (menu, popover, toast): the viewport only, not the unrolled page. */
    readonly viewportOnly?: boolean;
}

/** The capture of one viewport: PNGs, layout metrics and axe, recorded per state. */
class Capture {
    readonly page: Page;
    readonly vp: Viewport;

    constructor(page: Page, vp: Viewport) {
        this.page = page;
        this.vp = vp;
        mkdirSync(join(DIR, vp.name), { recursive: true });
    }

    private png(name: string): string {
        return join(DIR, this.vp.name, `${name}.png`);
    }

    /** Screenshots first (a toast or a loading state does not wait for axe), then measures. */
    async shot(name: string, opts: ShotOptions = {}, error: string | null = null): Promise<void> {
        const { page } = this;
        let unsettled = false;
        if (opts.settle !== false && !error) {
            try {
                await expect(page.locator('.skeleton')).toHaveCount(0, { timeout: 20_000 });
            } catch {
                unsettled = true;
            }
        }
        await page.evaluate(() => document.fonts.ready.then(() => undefined));
        const size = page.viewportSize() ?? { width: this.vp.width, height: this.vp.height };
        const maxH = Math.floor(MAX_DEVICE_PX / this.vp.deviceScaleFactor);
        const extraPngs: string[] = [];
        let truncated = false;
        const dialogOpen = (await page.locator('dialog[open]').count()) > 0;
        let unrolledAxe: AxeSummary[] | null = null;
        let tapCovers: TapCovers | null = null;

        /**
         * A dialog or an overlay, or a page that fits its screen: tap covers at the real size, each
         * box that scrolls its own content stepped (below).
         */
        let stepScrollers = false;
        if (dialogOpen || opts.viewportOnly) {
            stepScrollers = true;
            await page.screenshot({ path: this.png(name), animations: 'disabled' });
            if (dialogOpen) {
                // The dialog scrolls its body: unroll it into a second, taller capture.
                const extra = await page.evaluate(() => {
                    const b = document.querySelector('dialog[open] .modal-body');
                    return b ? Math.max(0, b.scrollHeight - b.clientHeight) : 0;
                });
                if (extra > 0) {
                    const h = size.height + extra + 64;
                    truncated = h > maxH;
                    await page.setViewportSize({ width: size.width, height: Math.min(h, maxH) });
                    // The whole layout, not the visual viewport: on a phone whose layout is wider
                    // than the screen, the visual viewport stays scrolled to the focused control.
                    await page.evaluate(() => document.querySelector('dialog[open] .modal-body')?.scrollTo(0, 0));
                    await page.screenshot({ path: this.png(`${name}-full`), animations: 'disabled', fullPage: true });
                    await page.setViewportSize(size);
                    extraPngs.push(`${name}-full.png`);
                }
            } else if (await page.evaluate(openSheetScrolls)) {
                // An open sheet's lower part (its form foot, its submit), as the device shows it.
                extraPngs.push(...(await this.slices(name)));
            }
        } else {
            // A phone or a tablet: also what the first screen shows at load (the visual viewport).
            if (this.vp.isMobile || this.vp.hasTouch) {
                await page.screenshot({ path: this.png(`${name}-screen`), animations: 'disabled' });
                extraPngs.push(`${name}-screen.png`);
            }
            // The document scrolls (FRONTEND.md §7.6; before M10-RESP the shell scrolled `main`):
            // grow the viewport to the scroll height of either, so the sticky header and bottom bars
            // land where they are on screen, then capture the full document.
            const extra = await page.evaluate(() => {
                const main = document.querySelector('main');
                const inMain = main ? Math.max(0, main.scrollHeight - main.clientHeight) : 0;
                return Math.max(inMain, document.documentElement.scrollHeight - window.innerHeight, 0);
            });
            const h = size.height + extra;
            truncated = h > maxH;
            if (extra === 0) stepScrollers = true;
            // Unrolling must not change the layout: a landscape phone grown taller than wide turns
            // portrait, a short phone grown past 700 px stops being short (the second approval
            // pass's review). There the page is captured and measured at its real size instead.
            const before = extra > 0 ? await page.evaluate(heightQueries, HEIGHT_QUERIES) : '';
            if (extra > 0) await page.setViewportSize({ width: size.width, height: Math.min(h, maxH) });
            const unrolled = extra > 0 && (await page.evaluate(heightQueries, HEIGHT_QUERIES)) === before;
            if (extra > 0 && !unrolled) await page.setViewportSize(size);
            const docH = await page.evaluate(() => document.documentElement.scrollHeight);
            truncated ||= docH > maxH;
            // Not unrolled: Chromium's full-page capture keeps the layout viewport (the orientation
            // and height queries) but drops the touch emulation, so it draws the fine pointer's
            // sizes, and the fixed bars sit where they are on the first screen. The device's own view
            // follows as `-scroll-N` slices (below).
            await page.screenshot({ path: this.png(name), animations: 'disabled', fullPage: docH <= maxH });
            // axe on the unrolled page: every target in view and the fixed bars past the content's
            // end, so a target is never judged by what happens to scroll under a bar at one moment
            // (a page scrolls under its fixed bars by design; 2.4.11 keeps focus clear of them).
            if (unrolled) {
                await this.retouch();
                // On a failure the page is measured again below, at its own size.
                unrolledAxe = await this.axe().catch(() => null);
                // Tap areas likewise: every control in view, none under a fixed bar; each box that
                // scrolls its own content (the side trade panel from 1,024 px) stepped, so its end,
                // where its form foot is no longer stuck, is judged too.
                tapCovers = await this.tapCoversInSteps(false).catch(() => null);
                await page.setViewportSize(size);
            } else if (extra > 0) {
                await this.retouch();
                extraPngs.push(...(await this.slices(name)));
                tapCovers = await this.tapCoversInSteps().catch(() => null);
            }
        }
        await this.retouch();
        // A dialog's or a sheet's body below its fold (the unrolled `-full` capture is a picture
        // only: its taller viewport changes the dvh sizes), or a box that scrolls in a page that fits
        // its screen: stepped at the real size.
        if (stepScrollers) tapCovers = await this.tapCoversInSteps(false).catch(() => null);

        let layout: ShotRecord['layout'] = null;
        let axe: AxeSummary[] = unrolledAxe ?? [];
        try {
            layout = await page.evaluate(collectLayout, this.vp.width);
            tapCovers ??= await page.evaluate(collectTapCovers);
            layout = { ...layout, tapCovers };
        } catch (e) {
            error = `${error ? `${error}; ` : ''}metrics: ${(e as Error).message}`;
        }
        try {
            if (!unrolledAxe) axe = await this.axe();
        } catch (e) {
            error = `${error ? `${error}; ` : ''}axe: ${(e as Error).message}`;
        }
        if (GATE) this.gate(name, layout, axe, error);
        record(this.vp, {
            name,
            viewport: this.vp.name,
            png: `${this.vp.name}/${name}.png`,
            extraPngs: extraPngs.map((p) => `${this.vp.name}/${p}`),
            takenAt: new Date().toISOString(),
            truncated,
            unsettled,
            error,
            notes: this.notes.splice(0),
            layout,
            axe,
        });
    }

    private notes: string[] = [];

    /**
     * The page at its real size, one screen at a time (`<name>-scroll-N.png`, N from 1): what the
     * device shows where the unrolled capture would change the layout, or an open sheet's lower
     * part. The open sheet's body (else the document) scrolls 75 % of its content's height — the
     * box less the bars fixed or stuck over its top and bottom — per slice, so slices overlap by a
     * quarter of what is visible between the bars; the scroll position is restored.
     */
    private async slices(name: string): Promise<string[]> {
        const { page } = this;
        const out: string[] = [];
        const step = await page.evaluate(sliceStep);
        for (let y = 0, k = 1; k <= 40; y += step, k++) {
            const { at, max } = await page.evaluate(scrollSliceTo, y);
            await page.evaluate(
                () => new Promise<void>((r) => requestAnimationFrame(() => requestAnimationFrame(() => r()))),
            );
            const file = `${name}-scroll-${k}.png`;
            await page.screenshot({ path: this.png(`${name}-scroll-${k}`), animations: 'disabled' });
            out.push(file);
            if (at >= max - 1) break;
        }
        await page.evaluate(scrollSliceTo, 0);
        return out;
    }

    /**
     * Tap covers scrolled in steps of half a screen: the document (unless `withDocument` is false:
     * a dialog or an overlay, which the page does not scroll under, the unrolled page, every control
     * already in view, or a page that fits its screen), then each box that scrolls its own content
     * (an open sheet's body, the side trade panel; in an open dialog only its own, the dialog's
     * body), so every control is wholly in view at one step at least; the covers of all steps
     * merged (the same pair once, its most points), each control judged counted once. Scroll
     * positions are restored.
     */
    private async tapCoversInSteps(withDocument = true): Promise<TapCovers> {
        const { page } = this;
        await page.evaluate(() => {
            (window as unknown as { __tapCounted?: WeakSet<Element> }).__tapCounted = new WeakSet();
        });
        try {
            return await this.tapCoverSteps(withDocument);
        } finally {
            await page.evaluate(() => {
                delete (window as unknown as { __tapCounted?: WeakSet<Element> }).__tapCounted;
            });
        }
    }

    private async tapCoverSteps(withDocument: boolean): Promise<TapCovers> {
        const { page } = this;
        const pairs = new Map<string, readonly [string, string, number]>();
        let judged = 0;
        let most = 0;
        const measure = async (): Promise<void> => {
            await page.evaluate(
                () => new Promise<void>((r) => requestAnimationFrame(() => requestAnimationFrame(() => r()))),
            );
            const t = await page.evaluate(collectTapCovers);
            judged += t.judged;
            most = Math.max(most, t.count);
            for (const it of t.items) {
                const k = `${it[0]} | ${it[1]}`;
                const was = pairs.get(k);
                if (!was || was[2] < it[2]) pairs.set(k, it);
            }
        };
        const scrollers = await page.evaluate(() => {
            const out: [number, number][] = [];
            const all = [...document.querySelectorAll('body *')];
            const dialog = document.querySelector('dialog[open]');
            all.forEach((el, i) => {
                const cs = getComputedStyle(el);
                if (!/auto|scroll/.test(cs.overflowY) || el.scrollHeight <= el.clientHeight + 1) return;
                if (!el.checkVisibility() || el.closest('[inert]')) return;
                if (dialog && !dialog.contains(el)) return;
                out.push([i, el.clientHeight]);
            });
            return out;
        });
        const half = Math.max(80, Math.floor(this.vp.height / 2));
        /**
         * Scrolls to `y` (clamped) and measures, until the furthest position has been measured: half
         * a screen at a time, or half the box when it is lower (a dialog's body on a landscape phone).
         */
        const steps = async (
            scroll: (y: number) => Promise<{ at: number; max: number }>,
            step = half,
        ): Promise<void> => {
            for (let y = 0, k = 0; k < 200; y += step, k++) {
                const { at, max } = await scroll(y);
                await measure();
                if (at >= max - 1) break;
            }
            await scroll(0);
        };
        // The document, then each box that scrolls its own content (instant: no smooth scrolling).
        if (withDocument)
            await steps((y) =>
                page.evaluate((v) => {
                    const max = Math.max(0, document.documentElement.scrollHeight - window.innerHeight);
                    window.scrollTo({ top: Math.min(v, max), behavior: 'instant' });
                    return { at: window.scrollY, max };
                }, y),
            );
        else if (!scrollers.length) await measure();
        for (const [i, h] of scrollers)
            await steps(
                (y) =>
                    page.evaluate(
                        ([idx, v]) => {
                            const el = document.querySelectorAll('body *')[idx];
                            if (!el) return { at: 0, max: 0 };
                            const max = Math.max(0, el.scrollHeight - el.clientHeight);
                            el.scrollTo({ top: Math.min(v, max), behavior: 'instant' });
                            return { at: el.scrollTop, max };
                        },
                        [i, y] as const,
                    ),
                Math.max(40, Math.min(half, Math.floor(h / 2))),
            );
        const items = [...pairs.values()];
        return { judged, count: Math.max(most, items.length), items: items.slice(0, 40) };
    }

    /** axe (WCAG 2.2 A/AA) on the page as it is now. */
    private async axe(): Promise<AxeSummary[]> {
        const r = await new AxeBuilder({ page: this.page }).withTags(AXE_TAGS).analyze();
        return r.violations.map((v) => ({
            id: v.id,
            impact: v.impact ?? 'n/a',
            help: v.help,
            nodes: v.nodes.length,
            targets: v.nodes.slice(0, 5).map((n) => n.target.join(' ')),
        }));
    }
    private cdp: CDPSession | null = null;

    /**
     * A full-page screenshot clears Chromium's touch emulation: `(pointer: coarse)` reads false for
     * the rest of the page's life, so a tablet would be measured — and its later PNGs drawn — with a
     * mouse's density. Turned back on after every capture on a touch viewport.
     */
    private async retouch(): Promise<void> {
        if (!this.vp.hasTouch) return;
        this.cdp ??= await this.page.context().newCDPSession(this.page);
        await this.cdp.send('Emulation.setTouchEmulationEnabled', { enabled: true, maxTouchPoints: 5 });
    }

    /** The M10-RESP gate's findings for one state (SHORTWIN_RESPONSIVE_GATE=1). */
    private gate(name: string, l: ShotRecord['layout'], axe: readonly AxeSummary[], error: string | null): void {
        const at = `${this.vp.name}/${name}`;
        const f = (msg: string): void => {
            gateFindings.push(`${at}: ${msg}`);
        };
        if (error) f(`step failed: ${error}`);
        for (const n of this.notes) f(`DOM-click fallback: ${n}`);
        for (const v of axe) f(`axe ${v.id} ×${v.nodes} (${v.targets.join(', ')})`);
        if (!l) {
            f('no layout metrics');
            return;
        }
        if (l.docOverflowX > 0) f(`document overflows sideways by ${l.docOverflowX}px`);
        if (l.innerWidth !== l.deviceWidth) f(`layout ${l.innerWidth}px wide on a ${l.deviceWidth}px screen`);
        if (l.zoom !== 1) f(`zoomed to ${l.zoom}`);
        if (l.offscreen.count > 0)
            f(`${l.offscreen.count} off screen: ${l.offscreen.items.map((i) => i.sel).join(', ')}`);
        if (l.targets.under24 > 0) {
            const small = l.targets.items.filter((t) => t.w < 24 || t.h < 24);
            f(`${l.targets.under24} targets under 24px: ${small.map((t) => `${t.sel} ${t.w}×${t.h}`).join(', ')}`);
        }
        if (l.primaryUnder44 && l.primaryUnder44.count > 0)
            f(
                `${l.primaryUnder44.count} primary controls under 44px: ${l.primaryUnder44.items.map((t) => `${t.sel} ${t.w}×${t.h}`).join(', ')}`,
            );
        // Text under 12 px is a finding at every viewport (the owner's thirteenth round, O-4: the
        // 12 px floor from 640 px up too; FRONTEND.md §7.2). Until then it was phones only.
        if (l.smallText.nodes > 0)
            f(`text under 12px: ${l.smallText.groups.map((g) => `${g.sel} ${g.px}px`).join(', ')}`);
        // A control's tap area over another control (the thirteenth round's last review: an ⓘ's
        // 44 px area over the Copy button or the ⓘ beside it; FRONTEND.md §7.6).
        if (l.tapCovers && l.tapCovers.count > 0)
            f(
                `${l.tapCovers.count} controls under another's tap area: ${l.tapCovers.items.map(([a, b, n]) => `${a} under ${b} (${n}/25)`).join(', ')}`,
            );
    }

    /** A note for the next shot (what a step had to work around). */
    note(msg: string): void {
        this.notes.push(msg);
    }

    /**
     * Clicks (or checks) like a user. When the element cannot be hit at this viewport (a phone that
     * zoomed a too-wide page out: Playwright's hit test misses), records a note and falls back to a
     * DOM click so the capture can carry on to the states behind it.
     */
    async tap(l: Locator, action: 'click' | 'check' = 'click'): Promise<void> {
        try {
            if (action === 'check') await l.check({ timeout: 5_000 });
            else await l.click({ timeout: 5_000 });
        } catch (e) {
            const what = (await l.getAttribute('data-testid').catch(() => null)) ?? l.toString();
            this.note(`${action} on ${what} failed (${firstLine(e)}); DOM click used`);
            await l.evaluate((el) => (el as HTMLElement).click());
            if (action === 'check') await expect(l).toBeChecked({ timeout: 5_000 });
        }
    }

    /** `App.connectWallet` through `tap`. */
    async connect(app: App): Promise<void> {
        const address = app.id(T.walletAddress);
        const approved = await app.page.evaluate(
            (a) => window.localStorage.getItem('shortwin-e2e-wallet:authorized') === a,
            app.walletAddress,
        );
        if (approved) {
            await expect(address).toHaveAttribute('data-value', app.walletAddress, { timeout: 15_000 });
            return;
        }
        if (await address.isVisible()) return;
        await this.tap(app.id(T.walletButton));
        await expect(app.id(T.walletMenu)).toBeVisible();
        await this.tap(app.id(T.walletOption(E2E_WALLET_NAME)));
        await expect(address).toHaveAttribute('data-value', app.walletAddress);
    }

    /** `App.closeModal` through `tap`. */
    async closeModal(app: App): Promise<void> {
        if ((await app.stage()) === 'confirmed') await this.tap(app.modal.getByRole('button', { name: 'Done' }));
        else await this.tap(app.inModal(T.txCancel));
        await expect(app.modal).toHaveCount(0);
    }

    /** `App.signAndConfirm` through `tap`. */
    async signAndConfirm(app: App): Promise<void> {
        await expect(app.inModal(T.txSign)).toBeEnabled();
        await this.tap(app.inModal(T.txSign));
        await app.expectStage('confirmed', 90_000);
    }

    /**
     * One scenario step ending in `shot(name)`: a failure is recorded against `name` with a PNG of
     * where it stopped, and the scenario carries on with its next step.
     */
    async step(name: string, fn: () => Promise<void>): Promise<void> {
        try {
            await fn();
        } catch (e) {
            const msg = firstLine(e);
            try {
                await this.page.keyboard.press('Escape');
                await this.shot(name, { settle: false }, msg);
            } catch {
                // the page is gone: nothing to capture
            }
        }
    }
}

async function marketTab(cap: Capture, app: App, tab: Parameters<typeof T.marketTab>[0]): Promise<void> {
    await cap.tap(app.id(T.marketTab(tab)));
    await expect(app.id(T.marketTab(tab))).toHaveAttribute('aria-selected', 'true');
}

/** A route that holds every indexer request until released (the loading skeletons). */
async function holdIndexer(page: Page): Promise<() => Promise<void>> {
    let release: () => void = () => {};
    const held = new Promise<void>((r) => {
        release = r;
    });
    const pattern = `${URLS.indexer}/**`;
    // Same glob as the indexer mock's route: unroute this handler only, never the mock's.
    const handler = async (route: Route) => {
        await held;
        await route.fallback().catch(() => undefined);
    };
    await page.route(pattern, handler);
    return async () => {
        release();
        await page.unroute(pattern, handler);
    };
}

/** An indexer that answers `path` with a 500 problem (the page's error state). */
async function failIndexer(page: Page, path: string): Promise<() => Promise<void>> {
    const pattern = new RegExp(`^${URLS.indexer.replace(/[.]/g, '\\.')}${path}(\\?.*)?$`);
    const handler = (route: Route) =>
        route.fulfill({
            status: 500,
            headers: { 'content-type': 'application/problem+json', 'access-control-allow-origin': '*' },
            body: JSON.stringify({
                type: 'about:blank',
                title: 'Internal Server Error',
                status: 500,
                code: 'internal',
                detail: 'The indexer failed (responsive capture).',
            }),
        });
    await page.route(pattern, handler);
    return async () => page.unroute(pattern, handler);
}

/** Under 1024 px the trade panel and a position's Manage panel are sheets (FRONTEND.md §7.6). */
const compact = (vp: Viewport): boolean => vp.width < 1024;

/**
 * A market's trade panel at `/m/<mint>?<query>`: in the page from 1024 px; under it, the page first
 * (the header's wallet button is outside the sheet, which makes the rest of the page inert), then
 * the action bar opens the sheet as a user would.
 */
async function tradePanel(
    cap: Capture,
    app: App,
    mint: string,
    query: string,
    panel: 'short' | 'lend',
    opts: { connect?: boolean } = {},
): Promise<void> {
    await app.goto(`/m/${mint}?${query}`, 'market');
    if (opts.connect !== false) await cap.connect(app);
    if (compact(cap.vp)) {
        await cap.tap(app.id(panel === 'short' ? T.marketActionShort : T.marketActionLend));
        await expect(app.id(T.tradeSheetClose)).toBeVisible();
    }
    await expect(app.id(panel === 'short' ? T.shortPanel : T.lendPanel)).toBeVisible();
}

async function depositForm(cap: Capture, app: App, mint: string, source: string, amount: string): Promise<void> {
    await tradePanel(cap, app, mint, 'panel=lend&action=deposit', 'lend');
    const panel = app.id(T.lendPanel);
    await app.selectTokenAccount(panel.getByTestId(T.lendTokenAccount), source as never);
    await panel.getByTestId(T.lendAmountInput).fill(amount);
    await cap.tap(panel.getByTestId(T.disclosureAck('lender.solRepayment')), 'check');
}

const tradingSeeded = (state: E2EState) => state.program === 'test-venue' && !!state.positions;

/** A viewport's tests (inside its describe): the gate's check after each, the device, the timeout. */
function atViewport(vp: Viewport): void {
    test.afterEach(() => {
        const found = gateFindings.splice(0);
        if (GATE) expect(found, `M10-RESP responsive gate at ${vp.name}`).toEqual([]);
    });
    test.use({
        viewport: { width: vp.width, height: vp.height },
        isMobile: vp.isMobile,
        hasTouch: vp.hasTouch,
        deviceScaleFactor: vp.deviceScaleFactor,
        ...(vp.userAgent ? { userAgent: vp.userAgent } : {}),
    });
    test.describe.configure({ timeout: 900_000 });
}

for (const vp of VIEWPORTS) {
    if (ONLY.length && !ONLY.includes(vp.name)) continue;

    test.describe(`responsive ${vp.name}`, { tag: '@responsive' }, () => {
        atViewport(vp);

        test.describe('browsing', () => {
            test.use({ wallet: 'trader' });

            test('markets, search, states, status, not found, security, create market, wallet menu', async ({
                app,
                page,
                state,
            }) => {
                const cap = new Capture(page, vp);
                await cap.step('markets', async () => {
                    await app.goto('/markets', 'markets');
                    await expect(app.id(T.marketsRow(state.markets.main.mint))).toBeVisible();
                    // The intro strip stays compact (O-3 (a)): the first market shows on the first
                    // screen — at least its first 16 px above the bars fixed at the screen's foot
                    // (the phone's tab bar and status strip). A phone held sideways (390 px tall)
                    // has no room for a row under the page head and the list's own filter and sort
                    // bars even without the strip; there the list's filter must show.
                    await expect(app.id(T.marketsIntro)).toBeVisible();
                    const room = await page.evaluate(() => {
                        const vh = window.innerHeight;
                        const short = vh < 500;
                        const first = short
                            ? document.querySelector('[data-testid="markets-search"]')
                            : document.querySelector('[data-testid^="markets-row-"]');
                        let floor = vh;
                        for (const el of document.querySelectorAll<HTMLElement>('body *')) {
                            const cs = getComputedStyle(el);
                            if (cs.position !== 'fixed' && cs.position !== 'sticky') continue;
                            const r = el.getBoundingClientRect();
                            if (r.height > 0 && r.height < vh / 2 && Math.round(r.bottom) >= vh)
                                floor = Math.min(floor, r.top);
                        }
                        return { short, top: first?.getBoundingClientRect().top ?? Number.POSITIVE_INFINITY, floor };
                    });
                    expect(
                        room.top + 16,
                        `the first market row (sideways: the filter) on the first screen (${JSON.stringify(room)})`,
                    ).toBeLessThanOrEqual(room.floor);
                    await cap.shot('markets');
                });
                // The thirteenth round (O-3): the strip hidden, and How it works (its own page, and at
                // the intro's risk anchor).
                await cap.step('markets-intro-hidden', async () => {
                    await app.goto('/markets', 'markets');
                    await cap.tap(app.id(T.marketsIntroDismiss));
                    await expect(app.id(T.marketsIntro)).toHaveCount(0);
                    await cap.shot('markets-intro-hidden');
                    // The next visits show the strip again (it is per browser; this one remembers it).
                    await page.evaluate(() => window.localStorage.removeItem('shortwin:markets-intro:hidden'));
                });
                await cap.step('how-it-works', async () => {
                    await app.goto('/how-it-works', 'how-it-works');
                    await expect(app.id(T.howItWorksSection('fees'))).toBeVisible();
                    await cap.shot('how-it-works');
                });
                await cap.step('how-it-works-risks', async () => {
                    await app.goto('/how-it-works#short-risks', 'how-it-works');
                    await expect(app.id(T.howItWorksSection('short-risks'))).toBeFocused();
                    await cap.shot('how-it-works-risks', { viewportOnly: true });
                });
                await cap.step('markets-search-no-market', async () => {
                    await app.goto(`/markets?q=${state.createMints.ok}`, 'markets');
                    await expect(app.id(T.marketsCreateLink)).toBeVisible();
                    await cap.shot('markets-search-no-market');
                });
                await cap.step('markets-empty-filter', async () => {
                    await app.goto('/markets?q=zzzz-no-such-token', 'markets');
                    await expect(page.locator('.skeleton')).toHaveCount(0, { timeout: 20_000 });
                    await cap.shot('markets-empty-filter');
                });
                await cap.step('markets-loading', async () => {
                    const release = await holdIndexer(page);
                    try {
                        await app.goto('/markets', 'markets');
                        await expect(page.locator('.skeleton').first()).toBeVisible({ timeout: 10_000 });
                        await cap.shot('markets-loading', { settle: false });
                    } finally {
                        await release();
                    }
                });
                await cap.step('markets-error', async () => {
                    const restore = await failIndexer(page, '/v1/markets');
                    try {
                        await app.goto('/markets', 'markets');
                        await expect(page.locator('.error-state').first()).toBeVisible({ timeout: 20_000 });
                        await cap.shot('markets-error');
                    } finally {
                        await restore();
                    }
                });
                await cap.step('header-search-results', async () => {
                    await app.goto('/markets', 'markets');
                    // Under 1024 px the search is behind its Search button (an overlay).
                    if (await app.id(T.searchToggle).isVisible()) await cap.tap(app.id(T.searchToggle));
                    await app.id(T.searchInput).fill('E2E');
                    await expect(app.id(T.searchResults)).toBeVisible();
                    await cap.shot('header-search-results', { viewportOnly: true });
                });
                await cap.step('status', async () => {
                    await app.goto('/status', 'status');
                    await expect(app.id(T.statusDegradedRow(state.markets.paused.mint))).toBeVisible();
                    await cap.shot('status');
                });
                await cap.step('not-found-mint', async () => {
                    await app.goto('/m/not-a-mint');
                    await expect(page.getByText(/Not found/)).toBeVisible();
                    await cap.shot('not-found-mint');
                });
                await cap.step('not-found-route', async () => {
                    await app.goto('/no-such-page');
                    await expect(page.getByText(/Not found/)).toBeVisible();
                    // A short page does not scroll at all: not even the 1 px a bar's border once
                    // added (the status bar's Security link under its top border).
                    const overY = await page.evaluate(() => document.documentElement.scrollHeight - window.innerHeight);
                    expect(overY, 'a short page scrolls vertically').toBe(0);
                    await cap.shot('not-found-route');
                });
                await cap.step('market-not-listed', async () => {
                    await app.goto(`/m/${state.createMints.ok}`, 'market');
                    await expect(app.id(T.marketNotFound)).toBeVisible();
                    await cap.shot('market-not-listed');
                });
                await cap.step('security', async () => {
                    await app.goto('/security', 'security');
                    await expect(app.id(T.page('security')).locator('a[href^="mailto:"]').first()).toBeVisible();
                    await cap.shot('security');
                });
                await cap.step('create-market-empty', async () => {
                    await app.goto('/markets/new', 'create-market');
                    await expect(app.id(T.createMintInput)).toBeVisible();
                    await cap.shot('create-market-empty');
                });
                await cap.step('wallet-menu-disconnected', async () => {
                    await app.goto('/markets', 'markets');
                    await cap.tap(app.id(T.walletButton));
                    await expect(app.id(T.walletMenu)).toBeVisible();
                    await cap.shot('wallet-menu-disconnected', { viewportOnly: true });
                    await page.keyboard.press('Escape');
                });
                await cap.step('create-market-ok', async () => {
                    await app.goto(`/markets/new?mint=${state.createMints.ok}`, 'create-market');
                    await cap.connect(app);
                    await expect(app.id(T.createVerdict)).toHaveAttribute('data-verdict', 'ok');
                    await expect(app.id(T.createCost)).toHaveAttribute('data-value', /^\d+$/);
                    await cap.shot('create-market-ok');
                });
                await cap.step('create-market-rejected', async () => {
                    await app.goto(`/markets/new?mint=${state.createMints.badExtension}`, 'create-market');
                    await expect(app.id(T.createVerdict)).toHaveAttribute('data-verdict', 'reject');
                    await cap.shot('create-market-rejected');
                });
                if (state.venueMints) {
                    const pump = state.venueMints.pumpCurve;
                    await cap.step('create-market-pump-curve', async () => {
                        await app.goto(`/markets/new?mint=${pump}`, 'create-market');
                        await expect(app.id(T.createVenue)).toBeVisible();
                        await cap.shot('create-market-pump-curve');
                    });
                }
                await cap.step('wallet-menu-connected', async () => {
                    await app.goto('/markets', 'markets');
                    await cap.connect(app);
                    await cap.tap(app.id(T.walletButton));
                    await expect(app.id(T.walletDisconnect)).toBeVisible();
                    await cap.shot('wallet-menu-connected', { viewportOnly: true });
                    await page.keyboard.press('Escape');
                });
            });

            test('market page: tabs, the tier popover, paused and list-only markets', async ({ app, page, state }) => {
                test.skip(!tradingSeeded(state), 'needs the test-venue build (mock venue)');
                const cap = new Capture(page, vp);
                const main = state.markets.main;
                await cap.step('market-overview', async () => {
                    await app.goto(`/m/${main.mint}`, 'market');
                    await expect(app.id(T.marketSource)).toHaveAttribute('data-value', 'indexer');
                    // Under 1024 px the trade panel waits behind the action bar.
                    await expect(app.id(compact(vp) ? T.marketActionShort : T.shortPanel)).toBeVisible();
                    await cap.shot('market-overview');
                    // No stat alone on a row (seven stats: one row at 1,440 px, else 4 + 3; 2 + 2 on
                    // a phone), and the Overview's columns reach the card's right edge.
                    const layout = await page.evaluate(() => {
                        const rows = new Map<number, number>();
                        for (const s of document.querySelectorAll<HTMLElement>('.mk-stats > .stat')) {
                            if (s.offsetParent === null) continue;
                            const top = Math.round(s.getBoundingClientRect().top);
                            rows.set(top, (rows.get(top) ?? 0) + 1);
                        }
                        const grid = document.querySelector('.mk-overview')?.getBoundingClientRect();
                        const col = document.querySelector('.mk-overview-col')?.getBoundingClientRect();
                        return { rows: [...rows.values()], slack: grid && col ? grid.right - col.right : 0 };
                    });
                    expect(layout.rows.length === 1 || !layout.rows.includes(1), `stat rows ${layout.rows}`).toBe(true);
                    expect(layout.slack, 'space right of the Overview columns').toBeLessThanOrEqual(1);
                });
                if (compact(vp))
                    await cap.step('market-trade-sheet', async () => {
                        await cap.tap(app.id(T.marketActionShort));
                        await expect(app.id(T.shortPanel)).toBeVisible();
                        await cap.shot('market-trade-sheet', { viewportOnly: true });
                        await cap.tap(app.id(T.tradeSheetClose));
                        await expect(app.id(T.tradeSheetClose)).toBeHidden();
                    });
                for (const tab of ['positions', 'lenders', 'liquidations', 'pools', 'coverage', 'events'] as const) {
                    await cap.step(`market-tab-${tab}`, async () => {
                        await marketTab(cap, app, tab);
                        await cap.shot(`market-tab-${tab}`);
                    });
                }
                await cap.step('market-tier-why', async () => {
                    await app.goto(`/m/${main.mint}`, 'market');
                    await cap.tap(app.id(T.marketTier));
                    await expect(app.id(T.marketTierWhy)).toBeVisible();
                    await cap.shot('market-tier-why', { viewportOnly: true });
                    await page.keyboard.press('Escape');
                });
                await cap.step('glossary-open', async () => {
                    await app.goto(`/m/${main.mint}`, 'market');
                    await cap.tap(app.id(T.glossaryTip('rentApr')).first());
                    await expect(app.id(T.glossaryPopover('rentApr'))).toBeVisible();
                    await cap.shot('glossary-open', { viewportOnly: true });
                    await page.keyboard.press('Escape');
                    await expect(app.id(T.glossaryPopover('rentApr'))).toHaveCount(0);
                });
                await cap.step('market-coverage-paused', async () => {
                    await app.goto(`/m/${state.markets.paused.mint}`, 'market');
                    await expect(app.id(T.marketBanner('coverage-paused'))).toBeVisible();
                    await cap.shot('market-coverage-paused');
                });
                await cap.step('market-list-only', async () => {
                    await app.goto(`/m/${state.markets.t22.mint}`, 'market');
                    await expect(app.id(T.marketFlag('list-only'))).toBeVisible();
                    await cap.shot('market-list-only');
                });
            });

            test('short: preview, review, blocked', async ({ app, page, state }) => {
                test.skip(!tradingSeeded(state), 'needs the test-venue build (mock venue)');
                const cap = new Capture(page, vp);
                const main = state.markets.main;
                await cap.step('short-preview', async () => {
                    await tradePanel(cap, app, main.mint, 'panel=short', 'short');
                    const panel = app.id(T.shortPanel);
                    await panel.getByTestId(T.shortQInput).fill('250000');
                    await expect(panel.getByTestId(T.shortPreview('margin'))).toHaveAttribute('data-value', /^\d+$/);
                    await cap.shot('short-preview');
                });
                await cap.step('short-review', async () => {
                    const panel = app.id(T.shortPanel);
                    await cap.tap(panel.getByTestId(T.disclosureAck('borrower.liquidation')), 'check');
                    await cap.tap(panel.getByTestId(T.shortSubmit));
                    expect(await app.simulated()).toBe('review');
                    await cap.shot('short-review');
                    await cap.closeModal(app);
                });
                await cap.step('short-blocked', async () => {
                    await tradePanel(cap, app, state.markets.paused.mint, 'panel=short', 'short');
                    await expect(app.id(T.shortBlocked)).toHaveAttribute('data-code', 'CoveragePaused');
                    await cap.shot('short-blocked');
                });
                // O-2: the panel links to the page's banner (from the sheet: it closes it first).
                await cap.step('short-coverage-link', async () => {
                    await expect(app.id(T.shortCoverageLink)).toBeVisible();
                    await cap.shot('short-coverage-link', { viewportOnly: true });
                });
                await cap.step('coverage-banner-focused', async () => {
                    await cap.tap(app.id(T.shortCoverageLink));
                    await expect(app.id(T.marketBanner('coverage-paused'))).toBeFocused();
                    await cap.shot('coverage-banner-focused', { viewportOnly: true });
                });
            });
        });

        test.describe('no wallet', () => {
            test.use({ wallet: null });

            test('the wallet menu without a wallet, and the pages that need one', async ({ app, page, state }) => {
                const cap = new Capture(page, vp);
                await cap.step('wallet-menu-no-wallet', async () => {
                    await app.goto('/markets', 'markets');
                    await cap.tap(app.id(T.walletButton));
                    await expect(app.id(T.walletMenu)).toBeVisible();
                    await cap.shot('wallet-menu-no-wallet', { viewportOnly: true });
                    await page.keyboard.press('Escape');
                });
                await cap.step('positions-no-wallet', async () => {
                    await app.goto('/positions', 'positions');
                    await cap.shot('positions-no-wallet');
                });
                await cap.step('lend-no-wallet', async () => {
                    await app.goto('/lend', 'lend');
                    await cap.shot('lend-no-wallet');
                });
                await cap.step('market-short-no-wallet', async () => {
                    await tradePanel(cap, app, state.markets.main.mint, 'panel=short', 'short', { connect: false });
                    await cap.shot('market-short-no-wallet');
                });
                await cap.step('market-lend-no-wallet', async () => {
                    await tradePanel(cap, app, state.markets.main.mint, 'panel=lend&action=deposit', 'lend', {
                        connect: false,
                    });
                    await cap.shot('market-lend-no-wallet');
                });
            });
        });

        test.describe('empty wallet', () => {
            test.use({ wallet: 'creator' });

            test('no positions, no lending, a position that does not exist', async ({ app, page, state }) => {
                const cap = new Capture(page, vp);
                await cap.step('positions-empty', async () => {
                    await app.goto('/positions', 'positions');
                    await cap.connect(app);
                    await cap.shot('positions-empty');
                });
                await cap.step('lend-empty', async () => {
                    await app.goto('/lend', 'lend');
                    await cap.connect(app);
                    await cap.shot('lend-empty');
                });
                await cap.step('position-not-found', async () => {
                    // A valid address that is no position: the seeded market account.
                    await app.goto(`/positions/${state.markets.main.market}`);
                    await cap.shot('position-not-found');
                });
            });
        });

        test.describe('positions', () => {
            test.use({ wallet: 'holder' });

            test('the dashboard and a position', async ({ app, page, state }) => {
                test.skip(!tradingSeeded(state), 'needs the test-venue build (mock venue)');
                const cap = new Capture(page, vp);
                const holder = need(state.positions, 'positions').holder;
                await cap.step('positions', async () => {
                    await app.goto('/positions', 'positions');
                    await cap.connect(app);
                    await expect(app.id(T.positionRow(holder.position))).toBeVisible();
                    await cap.shot('positions');
                });
                await cap.step('position-detail', async () => {
                    await app.goto(`/positions/${holder.position}`, 'position');
                    await expect(app.id(T.positionDetail)).toHaveAttribute('data-value', holder.position);
                    // Under 1024 px Manage is a sheet behind the action bar.
                    await expect(app.id(compact(vp) ? T.positionActionClose : T.closePreview('payout'))).toBeVisible();
                    await cap.shot('position-detail');
                });
                if (compact(vp))
                    await cap.step('position-manage-sheet', async () => {
                        await cap.tap(app.id(T.positionActionClose));
                        await expect(app.id(T.closePreview('payout'))).toBeVisible();
                        await cap.shot('position-manage-sheet', { viewportOnly: true });
                        await cap.tap(app.id(T.manageSheetClose));
                        await expect(app.id(T.manageSheetClose)).toBeHidden();
                    });
            });
        });

        test.describe('lend', () => {
            test.use({ wallet: 'claimer' });

            test('deposit, withdraw, claim and the lender dashboard', async ({ app, page, state }) => {
                const cap = new Capture(page, vp);
                const main = state.markets.main;
                if (state.positions) await accrue(main);
                for (const action of ['deposit', 'withdraw', 'claim'] as const) {
                    await cap.step(`lend-${action}`, async () => {
                        await tradePanel(cap, app, main.mint, `panel=lend&action=${action}`, 'lend');
                        await expect(app.id(T.lendPanel)).toHaveAttribute('data-action', action);
                        if (action === 'deposit') await app.id(T.lendAmountInput).fill('1000');
                        if (action === 'claim') {
                            await expect(app.id(T.claimValue('claimable'))).toHaveAttribute('data-value', /^\d+$/);
                            // The search for syncs has ended: the settled label (owner, 2026-10-11).
                            await expect(app.id(T.claimSubmit)).not.toHaveAttribute('aria-busy', 'true', {
                                timeout: 30_000,
                            });
                        }
                        await cap.shot(`lend-${action}`);
                    });
                }
                await cap.step('lender-dashboard', async () => {
                    await app.goto('/lend', 'lend');
                    await expect(app.id(T.lenderRow(main.market))).toBeVisible();
                    await expect(app.id(T.lenderClaim(main.market))).not.toHaveAttribute('aria-busy', 'true', {
                        timeout: 30_000,
                    });
                    await cap.shot('lender-dashboard');
                });
            });
        });

        test.describe('lend: a shortfall', () => {
            test.use({ wallet: 'potClaimer' });

            // The thirteenth round (the owner's decisions of 2026-10-11, FRONTEND.md §3.6, §3.8): rent
            // credited to the pot market's only lender and not paid in yet is a shortfall no sync can
            // fund ("only when syncs can pay") — the Lending page's row and Lend → Claim are the
            // disabled "Review claim · 0 SOL" with "Nothing to claim yet" once the search has run.
            test('the Lending page and Lend → Claim with credit not paid in yet', async ({ app, page, state }) => {
                test.skip(!tradingSeeded(state), 'needs the test-venue build (mock venue)');
                const cap = new Capture(page, vp);
                const pot = state.markets.pot;
                // The market's prologue books the rent credited so far (the previews read the stored
                // index): at least 20,000 lamports, well above the 10,000-lamport dust threshold.
                await expect
                    .poll(
                        async () => {
                            await accrue(pot);
                            return BigInt((await simulatePotSync('potClaimer', pot, [])).lender.pending);
                        },
                        { timeout: 120_000, intervals: [5_000] },
                    )
                    .toBeGreaterThanOrEqual(20_000n);
                await cap.step('lender-dashboard-shortfall', async () => {
                    await app.goto('/lend', 'lend');
                    await cap.connect(app);
                    const row = app.id(T.lenderRow(pot.market));
                    await expect(row.getByTestId(T.lenderClaimReason(pot.market))).toHaveText('Nothing to claim yet', {
                        timeout: 90_000,
                    });
                    await expect(row.getByTestId(T.lenderClaim(pot.market))).toBeDisabled();
                    await expect(row.getByTestId(T.claimSubmitSyncs)).toHaveCount(0);
                    await cap.shot('lender-dashboard-shortfall');
                });
                await cap.step('lend-claim-shortfall', async () => {
                    await tradePanel(cap, app, pot.mint, 'panel=lend&action=claim', 'lend');
                    await expect(app.id(T.lendPanel)).toHaveAttribute('data-action', 'claim');
                    await expect(app.id(T.lendClaimReason)).toHaveText('Nothing to claim yet', { timeout: 30_000 });
                    await expect(app.id(T.claimSubmit)).toBeDisabled();
                    await cap.shot('lend-claim-shortfall');
                });
            });
        });

        test.describe('CPI Guard', () => {
            test.use({ wallet: 'guard' });

            test('the CPI Guard notice and the bracket review', async ({ app, page, state }) => {
                const cap = new Capture(page, vp);
                const t22 = state.markets.t22;
                await cap.step('lend-cpi-guard-notice', async () => {
                    await tradePanel(cap, app, t22.mint, 'panel=lend&action=deposit', 'lend');
                    const panel = app.id(T.lendPanel);
                    await app.selectTokenAccount(
                        panel.getByTestId(T.lendTokenAccount),
                        state.tokenAccounts.guard.guarded,
                    );
                    await panel.getByTestId(T.lendAmountInput).fill('5');
                    await expect(panel.getByTestId(T.lendCpiGuardNotice)).toHaveAttribute('data-state', 'blocked');
                    await cap.shot('lend-cpi-guard-notice');
                });
                await cap.step('lend-cpi-guard-bracket-review', async () => {
                    const panel = app.id(T.lendPanel);
                    await cap.tap(panel.getByTestId(T.lendCpiGuardBracket), 'check');
                    await cap.tap(panel.getByTestId(T.disclosureAck('lender.solRepayment')), 'check');
                    await cap.tap(panel.getByTestId(T.lendSubmit));
                    expect(await app.simulated()).toBe('review');
                    await cap.shot('lend-cpi-guard-bracket-review');
                    await cap.closeModal(app);
                });
            });
        });

        test.describe('transaction states', () => {
            test.use({ wallet: 'errors' });

            test('rejected, review, confirmed and the success toast', async ({ app, page, state }) => {
                const cap = new Capture(page, vp);
                const m = state.markets.main;
                const source = need(state.tokenAccounts.mainAta.errors, 'errors main ATA');
                await cap.step('tx-wallet-rejected', async () => {
                    await depositForm(cap, app, m.mint, source, '1');
                    await cap.tap(app.id(T.lendPanel).getByTestId(T.lendSubmit));
                    expect(await app.simulated()).toBe('review');
                    await cap.tap(app.inModal(T.disclosureAck('lender.solRepayment')), 'check');
                    await app.walletRejectNext();
                    await cap.tap(app.inModal(T.txSign));
                    await app.expectStage('failed');
                    await cap.shot('tx-wallet-rejected');
                });
                await cap.step('tx-review-deposit', async () => {
                    await cap.tap(app.inModal(T.txRetry));
                    expect(await app.simulated()).toBe('review');
                    await cap.tap(app.inModal(T.disclosureAck('lender.solRepayment')), 'check');
                    await cap.shot('tx-review-deposit');
                });
                await cap.step('tx-confirmed', async () => {
                    await cap.signAndConfirm(app);
                    await cap.shot('tx-confirmed');
                });
                await cap.step('toast-success', async () => {
                    // The toast lives 6 s: close the confirmed dialog by Escape (no hit test to wait
                    // out), and by Done only when Escape did not close it.
                    await page.keyboard.press('Escape');
                    if ((await app.modal.count()) > 0) await cap.closeModal(app);
                    await expect(app.modal).toHaveCount(0);
                    await expect(app.id(T.toast).first()).toBeVisible();
                    await cap.shot('toast-success', { viewportOnly: true });
                });
            });
        });

        test.describe('indexer down', () => {
            test.use({ wallet: 'claimer', indexer: 'down' });

            test('the market page, the markets list and the lender dashboard from the chain', async ({
                app,
                page,
                state,
            }) => {
                const cap = new Capture(page, vp);
                await cap.step('fallback-market', async () => {
                    await app.goto(`/m/${state.markets.main.mint}`, 'market');
                    await expect(app.id(T.fallbackBanner)).toBeVisible();
                    await expect(app.id(T.marketSource)).toHaveAttribute('data-value', 'chain');
                    await cap.shot('fallback-market');
                });
                await cap.step('fallback-markets', async () => {
                    await app.goto('/markets', 'markets');
                    await expect(app.id(T.marketsOpenMint)).toBeVisible();
                    await cap.shot('fallback-markets');
                });
                await cap.step('fallback-lender-dashboard', async () => {
                    await app.goto('/lend', 'lend');
                    await cap.connect(app);
                    await expect(app.id(T.lenderRow(state.markets.main.market))).toBeVisible();
                    await cap.shot('fallback-lender-dashboard');
                });
            });
        });
    });
}

/**
 * The second pass (after every viewport's first): `potLate` lends nine times `potClaimer`'s tokens
 * into the pot market and `potBorrower` closes, so the rent paid in is realised across both
 * lenders' shares — `potLate` holds a surplus its sync moves to the pot, `potClaimer` a shortfall it
 * can fund (as lend-claim-sync). Nothing claims here, so the state holds for every viewport: the
 * two-line label ("… now · syncs may add up to …") on the Lending page's row and Lend → Claim.
 */
for (const vp of VIEWPORTS) {
    if (ONLY.length && !ONLY.includes(vp.name)) continue;

    test.describe(`responsive ${vp.name}: a second lender`, { tag: '@responsive' }, () => {
        atViewport(vp);
        test.use({ wallet: 'potClaimer' });

        test('the Lending page and Lend → Claim when another lender’s sync can pay', async ({ app, page, state }) => {
            test.skip(!tradingSeeded(state), 'needs the test-venue build (mock venue)');
            const cap = new Capture(page, vp);
            const pot = state.markets.pot;
            if (!(await lenderPosition(pot.market, state.wallets.potLate.address))) {
                // potClaimer's credit first (rent accrues to it alone), well above the dust threshold.
                await expect
                    .poll(
                        async () => {
                            await accrue(pot);
                            return BigInt((await simulatePotSync('potClaimer', pot, [])).lender.pending);
                        },
                        { timeout: 120_000, intervals: [5_000] },
                    )
                    .toBeGreaterThanOrEqual(40_000n);
                await lendFromNode(
                    'potLate',
                    pot,
                    state.tokenAccounts.pot.late,
                    5_400_000n * 10n ** BigInt(pot.decimals),
                );
                const p = need(state.positions?.pot, 'pot position');
                if (await shortPosition(p.position)) await closeShortFromNode(await signerFor('potBorrower'), pot, p);
            }
            await cap.step('lender-dashboard-syncs', async () => {
                await app.goto('/lend', 'lend');
                await cap.connect(app);
                const row = app.id(T.lenderRow(pot.market));
                await expect(row.getByTestId(T.claimSubmitSyncs)).toBeVisible({ timeout: 90_000 });
                await expect(row.getByTestId(T.lenderClaim(pot.market))).toBeEnabled();
                await cap.shot('lender-dashboard-syncs');
            });
            await cap.step('lend-claim-syncs', async () => {
                await tradePanel(cap, app, pot.mint, 'panel=lend&action=claim', 'lend');
                await expect(app.id(T.lendPanel)).toHaveAttribute('data-action', 'claim');
                await expect(app.id(T.claimSubmitSyncs)).toBeVisible({ timeout: 30_000 });
                await expect(app.id(T.claimSubmit)).toBeEnabled();
                await cap.shot('lend-claim-syncs');
            });
        });
    });
}
