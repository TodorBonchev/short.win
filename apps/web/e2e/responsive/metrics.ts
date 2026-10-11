/**
 * The responsive capture's layout metrics (FRONTEND.md §10.5; `e2e/specs/responsive.spec.ts`): what
 * one page state measures at one viewport, the in-page collector that measures it, and the summary
 * of a whole run (`metrics.json`). Plain DOM measurement — no app code is involved, so the same
 * collector measures the app before and after a layout change.
 *
 * `collectLayout` runs inside the page (`page.evaluate`): it is serialised on its own, so it must not
 * reference anything outside its body.
 */

/** A viewport of the capture (Chromium emulation). */
export interface Viewport {
    readonly name: string;
    readonly width: number;
    readonly height: number;
    readonly isMobile: boolean;
    readonly hasTouch: boolean;
    readonly deviceScaleFactor: number;
    /** Phones and the tablet send a mobile user agent (as the real devices would). */
    readonly userAgent?: string;
}

export interface SmallTarget {
    readonly sel: string;
    readonly w: number;
    readonly h: number;
    /** A link inside running text (WCAG 2.5.8's inline exception). */
    readonly inline: boolean;
    readonly disabled: boolean;
}

export interface Offender {
    readonly sel: string;
    readonly left: number;
    readonly right: number;
    readonly width: number;
}

export interface Scroller {
    readonly sel: string;
    readonly scrollWidth: number;
    readonly clientWidth: number;
}

export interface SmallText {
    readonly sel: string;
    readonly px: number;
    readonly count: number;
    readonly sample: string;
}

export interface LayoutMetrics {
    readonly url: string;
    /** The emulated screen's width in CSS px (the viewport the capture asked for). */
    readonly deviceWidth: number;
    /**
     * The layout viewport (`innerWidth`). Wider than `deviceWidth` when the page does not fit a
     * phone: the mobile browser lays it out wider and shows the left part of it at scale 1 (the
     * user pans sideways or pinches out).
     */
    readonly innerWidth: number;
    /**
     * The visual viewport's scale at capture (`visualViewport.scale`). Target and font sizes below
     * are as seen (CSS px × zoom); overflow is against `deviceWidth`.
     */
    readonly zoom: number;
    readonly innerHeight: number;
    readonly dpr: number;
    /** `document.documentElement.scrollWidth - deviceWidth` (> 0: the page scrolls or is zoomed out). */
    readonly docOverflowX: number;
    /** `main`'s own sideways scroll (the shell scrolls `main`, FRONTEND.md §3.1). */
    readonly mainOverflowX: number;
    /** Visible elements that reach past the viewport's left or right edge, outermost first. */
    readonly offscreen: { readonly count: number; readonly items: readonly Offender[] };
    /** Elements that scroll sideways on their own (tables, `main`). */
    readonly hScrollers: readonly Scroller[];
    /** `overflow-x: hidden | clip` boxes whose content is cut (ellipsis or not). */
    readonly clipped: { readonly count: number; readonly ellipsis: number; readonly items: readonly Scroller[] };
    /** Interactive elements under 44 × 44 CSS px (and the subset under WCAG 2.5.8's 24 × 24). */
    readonly targets: {
        readonly total: number;
        readonly under44: number;
        readonly under24: number;
        readonly inlineExempt: number;
        readonly items: readonly SmallTarget[];
    };
    /** Pairs of interactive elements whose boxes overlap (neither contains the other). */
    readonly overlaps: { readonly count: number; readonly items: readonly (readonly [string, string])[] };
    /**
     * The thirteenth round: controls whose tap area another control covers — a point of the 5 × 5
     * grid (`collectTapCovers`) that hits a different interactive control outside the page's bars
     * (header, tab bar, status bar, action bar, form foot, sheet / dialog / menu head, toasts). Measured on the unrolled page with the touch emulation on (absent before).
     */
    readonly tapCovers?: TapCovers;
    /** Text nodes rendered under 12 px, grouped by element and size. */
    readonly smallText: { readonly nodes: number; readonly groups: readonly SmallText[] };
    /** Affordances that need a hover: `title` tooltips, `cursor: help`, `:hover`-only reveals. */
    readonly hover: {
        readonly titles: readonly { readonly sel: string; readonly title: string }[];
        readonly cursorHelp: number;
        readonly hoverRevealRules: readonly string[];
        readonly hoverRules: number;
    };
    /** The fixed shell chrome and the height left for content. */
    readonly chrome: {
        readonly header: number;
        readonly statusBar: number;
        readonly content: number;
        /** M10-RESP: the phone's tab bar and a page's action bar (0 when not shown). */
        readonly tabBar?: number;
        readonly actionBar?: number;
    };
    /** M10-RESP: `matchMedia('(pointer: coarse)')` — the emulation drives the touch tokens. */
    readonly coarse?: boolean;
    /**
     * M10-RESP (FRONTEND.md §9 of the responsive spec): under a coarse pointer, primary controls
     * (buttons other than `data-size="sm"`, inputs, selects, tabs, nav, menu items, summaries,
     * checkbox rows, search options, close buttons, action-bar and form-foot buttons) under 44 px.
     */
    readonly primaryUnder44?: { readonly count: number; readonly items: readonly SmallTarget[] };
    /** Measurement was scoped to the open dialog (the background is inert). */
    readonly scopedToDialog: boolean;
}

export interface TapCovers {
    /**
     * Controls judged (visible, in the open dialog when there is one, wholly in the viewport, a
     * point in view); over scroll steps, each control once.
     */
    readonly judged: number;
    /** Controls with at least one point on another control. */
    readonly count: number;
    /** `[covered, covering, points]` (points of 25 per line box), at most 40. */
    readonly items: readonly (readonly [string, string, number])[];
}

export interface AxeSummary {
    readonly id: string;
    readonly impact: string;
    readonly help: string;
    readonly nodes: number;
    readonly targets: readonly string[];
}

export interface ShotRecord {
    readonly name: string;
    readonly viewport: string;
    readonly png: string;
    readonly extraPngs: readonly string[];
    readonly takenAt: string;
    /** The capture height hit the per-DPR cap: the bottom of the page is not in the PNG. */
    readonly truncated: boolean;
    /** Skeleton rows were still on the page after the wait (a loading state, or a slow read). */
    readonly unsettled: boolean;
    /** The scenario step failed before or while capturing (the PNG shows where it stopped). */
    readonly error: string | null;
    /** What the step had to work around on the way (e.g. a control a tap could not hit). */
    readonly notes?: readonly string[];
    readonly layout: LayoutMetrics | null;
    readonly axe: readonly AxeSummary[];
}

export interface ViewportSummary {
    readonly shots: number;
    /** Page states laid out wider than the screen (`innerWidth > deviceWidth`: sideways panning). */
    readonly widerLayout: number;
    readonly minZoom: number;
    readonly notes: number;
    readonly errors: number;
    readonly pagesWithDocOverflow: number;
    readonly pagesWithMainOverflow: number;
    readonly maxOverflowX: number;
    readonly offscreen: number;
    readonly hScrollers: number;
    readonly targetsUnder44: number;
    readonly targetsUnder24: number;
    readonly smallTextNodes: number;
    readonly titles: number;
    readonly overlaps: number;
    /** Controls whose tap area another control covers (`LayoutMetrics.tapCovers`). */
    readonly tapCovers: number;
    readonly axeViolations: number;
    readonly axeNodes: number;
    readonly axeRules: readonly string[];
}

export interface MetricsFile {
    readonly version: 1;
    readonly updatedAt: string;
    readonly viewports: Readonly<Record<string, Viewport>>;
    readonly summary: Readonly<Record<string, ViewportSummary>>;
    /** The ten worst page states over all viewports (by `score`). */
    readonly worst: readonly { readonly viewport: string; readonly name: string; readonly score: number }[];
    readonly shots: Readonly<Record<string, Readonly<Record<string, ShotRecord>>>>;
}

/** A rough badness score of one shot, for ranking only. */
export function score(r: ShotRecord): number {
    const l = r.layout;
    if (!l) return r.error ? 50 : 0;
    return (
        (l.docOverflowX > 0 || l.mainOverflowX > 0
            ? 20 + Math.min(80, Math.max(l.docOverflowX, l.mainOverflowX) / 10)
            : 0) +
        l.offscreen.count * 2 +
        l.targets.under24 * 2 +
        l.targets.under44 * 0.5 +
        l.overlaps.count * 3 +
        (l.tapCovers?.count ?? 0) * 5 +
        l.smallText.nodes * 0.1 +
        r.axe.reduce((s, v) => s + 5 * v.nodes, 0) +
        (r.error ? 50 : 0)
    );
}

export function summarise(shots: MetricsFile['shots']): Pick<MetricsFile, 'summary' | 'worst'> {
    const summary: Record<string, ViewportSummary> = {};
    const all: { viewport: string; name: string; score: number }[] = [];
    for (const [vp, byName] of Object.entries(shots)) {
        const rs = Object.values(byName);
        const ls = rs.flatMap((r) => (r.layout ? [r.layout] : []));
        const rules = new Set<string>();
        for (const r of rs) for (const v of r.axe) rules.add(v.id);
        summary[vp] = {
            shots: rs.length,
            widerLayout: ls.filter((l) => l.innerWidth > l.deviceWidth).length,
            minZoom: Math.min(1, ...ls.map((l) => l.zoom)),
            notes: rs.reduce((n, r) => n + (r.notes?.length ?? 0), 0),
            errors: rs.filter((r) => r.error).length,
            pagesWithDocOverflow: ls.filter((l) => l.docOverflowX > 0).length,
            pagesWithMainOverflow: ls.filter((l) => l.mainOverflowX > 0).length,
            maxOverflowX: Math.max(0, ...ls.map((l) => Math.max(l.docOverflowX, l.mainOverflowX))),
            offscreen: ls.reduce((s, l) => s + l.offscreen.count, 0),
            hScrollers: ls.reduce((s, l) => s + l.hScrollers.length, 0),
            targetsUnder44: ls.reduce((s, l) => s + l.targets.under44, 0),
            targetsUnder24: ls.reduce((s, l) => s + l.targets.under24, 0),
            smallTextNodes: ls.reduce((s, l) => s + l.smallText.nodes, 0),
            titles: ls.reduce((s, l) => s + l.hover.titles.length, 0),
            overlaps: ls.reduce((s, l) => s + l.overlaps.count, 0),
            tapCovers: ls.reduce((s, l) => s + (l.tapCovers?.count ?? 0), 0),
            axeViolations: rs.reduce((s, r) => s + r.axe.length, 0),
            axeNodes: rs.reduce((s, r) => s + r.axe.reduce((t, v) => t + v.nodes, 0), 0),
            axeRules: [...rules].sort(),
        };
        for (const r of rs) all.push({ viewport: vp, name: r.name, score: Math.round(score(r)) });
    }
    all.sort((a, b) => b.score - a.score);
    return { summary, worst: all.slice(0, 10) };
}

/**
 * Measures the current page (runs in the browser). Interactive elements and text are measured in
 * the open `<dialog>` when there is one (the page behind it is inert); overflow over the whole page.
 */
export function collectLayout(deviceWidth: number): LayoutMetrics {
    const vw = window.innerWidth;
    const zoom = window.visualViewport?.scale ?? 1;
    const vh = window.innerHeight;
    const MAX_ITEMS = 40;
    const round = (n: number) => Math.round(n * 10) / 10;

    const testIdOf = (el: Element | null): string | null => el?.getAttribute('data-testid') ?? null;
    const describe = (el: Element): string => {
        const tag = el.tagName.toLowerCase();
        const tid = testIdOf(el);
        let s = tid ? `${tag}[data-testid="${tid}"]` : tag;
        if (!tid) {
            if (el.id) s += `#${el.id}`;
            const cls = [...el.classList].slice(0, 3);
            if (cls.length) s += `.${cls.join('.')}`;
            let a = el.parentElement;
            while (a && !testIdOf(a)) a = a.parentElement;
            if (a) s = `[data-testid="${testIdOf(a)}"] ${s}`;
        }
        const label = (el.getAttribute('aria-label') ?? el.textContent ?? '').replace(/\s+/g, ' ').trim();
        if (label && /^(a|button|summary|label|select|th|td|span)$/.test(tag)) s += ` «${label.slice(0, 32)}»`;
        return s;
    };
    const isVisible = (el: Element): boolean => {
        if (!el.checkVisibility({ opacityProperty: true, visibilityProperty: true })) return false;
        const r = el.getBoundingClientRect();
        return r.width > 0 && r.height > 0;
    };
    const ignored = (el: Element): boolean =>
        !!el.closest('.skip-link, .visually-hidden, [inert]') ||
        (el.getBoundingClientRect().width <= 1 && el.getBoundingClientRect().height <= 1);

    const dialog = document.querySelector('dialog[open]');
    const scope: ParentNode = dialog ?? document;
    const main = document.querySelector('main');
    const PAGE_SCROLLERS = new Set<Element | null>([document.documentElement, document.body, main]);

    // ---- overflow -------------------------------------------------------------------------
    const docOverflowX = Math.max(0, document.documentElement.scrollWidth - deviceWidth);
    const mainOverflowX = main ? Math.max(0, main.scrollWidth - main.clientWidth) : 0;

    const all = [...document.body.querySelectorAll('*')].filter(
        (el) => !(el instanceof SVGElement && !(el instanceof SVGSVGElement)) && isVisible(el) && !ignored(el),
    );
    const hClip = (el: Element): boolean => {
        const ox = getComputedStyle(el).overflowX;
        return ox !== 'visible';
    };
    /** The nearest ancestor (below the page scrollers) that clips or scrolls sideways. */
    const container = (el: Element): Element | null => {
        for (let a = el.parentElement; a && !PAGE_SCROLLERS.has(a); a = a.parentElement) if (hClip(a)) return a;
        return null;
    };
    const offenders = new Set<Element>();
    for (const el of all) {
        const r = el.getBoundingClientRect();
        if ((r.right > vw + 1 || r.left < -1) && !container(el)) offenders.add(el);
    }
    const outermost = [...offenders].filter((el) => !(el.parentElement && offenders.has(el.parentElement)));
    const offscreen = {
        count: outermost.length,
        items: outermost.slice(0, MAX_ITEMS).map((el) => {
            const r = el.getBoundingClientRect();
            return { sel: describe(el), left: round(r.left), right: round(r.right), width: round(r.width) };
        }),
    };

    const hScrollers: Scroller[] = [];
    const clippedItems: Scroller[] = [];
    let clippedCount = 0;
    let ellipsis = 0;
    for (const el of [document.documentElement, document.body, ...all]) {
        if (el.scrollWidth <= el.clientWidth + 1 || el.clientWidth === 0) continue;
        const cs = getComputedStyle(el);
        if (cs.overflowX === 'auto' || cs.overflowX === 'scroll') {
            hScrollers.push({ sel: describe(el), scrollWidth: el.scrollWidth, clientWidth: el.clientWidth });
        } else if (cs.overflowX === 'hidden' || cs.overflowX === 'clip') {
            clippedCount++;
            if (cs.textOverflow === 'ellipsis') ellipsis++;
            if (clippedItems.length < MAX_ITEMS)
                clippedItems.push({ sel: describe(el), scrollWidth: el.scrollWidth, clientWidth: el.clientWidth });
        }
    }

    // ---- touch targets ------------------------------------------------------------------------
    const INTERACTIVE =
        'a[href], button, input:not([type="hidden"]), select, textarea, summary, [role="button"], [role="tab"], ' +
        '[role="menuitem"], [role="link"], [role="checkbox"], [role="radio"], [role="switch"], [role="option"], ' +
        '[tabindex]:not([tabindex="-1"])';
    const interactive = [...scope.querySelectorAll(INTERACTIVE)].filter((el) => isVisible(el) && !ignored(el));
    const targetRect = (el: Element): DOMRect => {
        const r = el.getBoundingClientRect();
        if (el instanceof HTMLInputElement && (el.type === 'checkbox' || el.type === 'radio')) {
            const label = el.closest('label') ?? (el.id ? document.querySelector(`label[for="${el.id}"]`) : null);
            if (label) {
                const lr = label.getBoundingClientRect();
                if (lr.width * lr.height > r.width * r.height) return lr;
            }
        }
        return r;
    };
    const items: SmallTarget[] = [];
    let under44 = 0;
    let under24 = 0;
    let inlineExempt = 0;
    for (const el of interactive) {
        const raw = targetRect(el);
        const r = { width: raw.width * zoom, height: raw.height * zoom };
        if (r.width >= 44 && r.height >= 44) continue;
        const inline =
            el.tagName === 'A' &&
            getComputedStyle(el).display === 'inline' &&
            (el.parentElement?.textContent ?? '').trim().length > (el.textContent ?? '').trim().length + 8;
        if (inline) {
            inlineExempt++;
            continue;
        }
        under44++;
        if (r.width < 24 || r.height < 24) under24++;
        if (items.length < MAX_ITEMS)
            items.push({
                sel: describe(el),
                w: round(r.width),
                h: round(r.height),
                inline,
                disabled: (el as HTMLButtonElement).disabled === true,
            });
    }
    // Under-24 first: the WCAG 2.5.8 candidates matter most.
    items.sort((a, b) => Math.min(a.w, a.h) - Math.min(b.w, b.h));

    const overlapsItems: [string, string][] = [];
    let overlapCount = 0;
    const rects = interactive.map((el) => [el, el.getBoundingClientRect()] as const);
    for (let i = 0; i < rects.length; i++) {
        const [a, ra] = rects[i] as (typeof rects)[number];
        for (let j = i + 1; j < rects.length; j++) {
            const [b, rb] = rects[j] as (typeof rects)[number];
            if (a.contains(b) || b.contains(a)) continue;
            const w = Math.min(ra.right, rb.right) - Math.max(ra.left, rb.left);
            const h = Math.min(ra.bottom, rb.bottom) - Math.max(ra.top, rb.top);
            if (w > 2 && h > 2) {
                overlapCount++;
                if (overlapsItems.length < 20) overlapsItems.push([describe(a), describe(b)]);
            }
        }
    }

    // ---- small text ---------------------------------------------------------------------------
    const groups = new Map<string, SmallText & { count: number }>();
    let smallNodes = 0;
    const walker = document.createTreeWalker(dialog ?? document.body, NodeFilter.SHOW_TEXT);
    for (let n = walker.nextNode(); n; n = walker.nextNode()) {
        const text = (n.nodeValue ?? '').trim();
        const el = n.parentElement;
        if (!text || !el || el.closest('script, style, noscript, .visually-hidden, .skip-link')) continue;
        if (!isVisible(el)) continue;
        const px = round(Number.parseFloat(getComputedStyle(el).fontSize) * zoom);
        if (!(px < 12)) continue;
        smallNodes++;
        const sel = describe(el).replace(/ «.*»$/, '');
        const key = `${sel}|${px}`;
        const g = groups.get(key);
        if (g) g.count++;
        else groups.set(key, { sel, px, count: 1, sample: text.slice(0, 40) });
    }
    const smallGroups = [...groups.values()].sort((a, b) => b.count - a.count).slice(0, MAX_ITEMS);

    // ---- hover-only affordances ---------------------------------------------------------------
    const titles = [...scope.querySelectorAll('[title]')]
        .filter((el) => isVisible(el) && (el.getAttribute('title') ?? '').trim())
        .map((el) => ({ sel: describe(el), title: (el.getAttribute('title') ?? '').slice(0, 80) }));
    const cursorHelp = all.filter((el) => getComputedStyle(el).cursor === 'help').length;
    const REVEAL = ['display', 'visibility', 'opacity', 'max-height', 'height', 'transform', 'clip', 'clip-path'];
    const hoverRevealRules: string[] = [];
    let hoverRules = 0;
    const walk = (rules: CSSRuleList) => {
        for (const rule of rules) {
            if (rule instanceof CSSStyleRule) {
                if (!rule.selectorText.includes(':hover')) continue;
                hoverRules++;
                const props = REVEAL.filter((p) => rule.style.getPropertyValue(p) !== '');
                const alsoFocus = /:focus/.test(rule.selectorText);
                if (props.length && !alsoFocus) hoverRevealRules.push(`${rule.selectorText} { ${props.join(', ')} }`);
            } else if ('cssRules' in rule) {
                walk((rule as CSSGroupingRule).cssRules);
            }
        }
    };
    for (const sheet of document.styleSheets) {
        try {
            walk(sheet.cssRules);
        } catch {
            // a cross-origin sheet: not readable
        }
    }

    // ---- chrome -------------------------------------------------------------------------------
    const h = (sel: string) => {
        const el = document.querySelector(sel);
        return el && isVisible(el) ? round(el.getBoundingClientRect().height) : 0;
    };
    const header = h('[data-testid="app-header"]');
    const statusBar = h('[data-testid="status-bar"]');
    const coarse = window.matchMedia('(pointer: coarse)').matches;
    const tabBar =
        getComputedStyle(document.querySelector('.nav') ?? document.body).position === 'fixed' ? h('.nav') : 0;
    const actionBar = h('.action-bar');
    const PRIMARY =
        '.btn:not([data-size="sm"]), input:not([type="checkbox"]):not([type="radio"]):not([type="hidden"]), select, ' +
        '.tab, .nav a, .nav-more, [role="menuitem"], summary, label.checkbox, .search-option, .modal-close, ' +
        '.sheet-close, .action-bar .btn, .form-foot .btn';
    const primaryItems: SmallTarget[] = [];
    let primaryCount = 0;
    if (coarse) {
        for (const el of scope.querySelectorAll(PRIMARY)) {
            if (!isVisible(el) || ignored(el)) continue;
            const r = el.getBoundingClientRect();
            if (r.height * zoom >= 44 - 0.5) continue;
            primaryCount++;
            if (primaryItems.length < MAX_ITEMS)
                primaryItems.push({
                    sel: describe(el),
                    w: round(r.width * zoom),
                    h: round(r.height * zoom),
                    inline: false,
                    disabled: (el as HTMLButtonElement).disabled === true,
                });
        }
    }

    return {
        url: location.pathname + location.search,
        deviceWidth,
        innerWidth: vw,
        zoom: round(zoom * 1000) / 1000,
        innerHeight: vh,
        dpr: window.devicePixelRatio,
        docOverflowX,
        mainOverflowX,
        offscreen,
        hScrollers: hScrollers.slice(0, MAX_ITEMS),
        clipped: { count: clippedCount, ellipsis, items: clippedItems },
        targets: { total: interactive.length, under44, under24, inlineExempt, items },
        overlaps: { count: overlapCount, items: overlapsItems },
        smallText: { nodes: smallNodes, groups: smallGroups },
        hover: { titles: titles.slice(0, MAX_ITEMS), cursorHelp, hoverRevealRules, hoverRules },
        chrome: {
            header,
            statusBar,
            content: round(vh - header - statusBar - tabBar - actionBar),
            tabBar,
            actionBar,
        },
        coarse,
        primaryUnder44: { count: primaryCount, items: primaryItems },
        scopedToDialog: !!dialog,
    };
}

/**
 * Tap areas that cover a neighbour (runs in the browser; the thirteenth round, FRONTEND.md §7.6,
 * §10.5): every visible interactive control wholly inside the viewport (in the open `<dialog>` when
 * there is one; not one clipped away, like a card table's header row) is sampled at the focus
 * check's 5 × 5 grid on each of its line boxes (15 % to 85 % of each side, `e2e/focus-obscured.ts`),
 * the points inside the box it can be seen in (`clipBox`: the boxes that clip it on its
 * containing-block chain, up to the top layer) judged, and `document.elementFromPoint` must not
 * land on a **different** interactive control — one that is neither the control, its ancestor or
 * descendant nor its `<label>` — outside the page's bars while they are over the page (the header,
 * the tab bar, the status bar, an action bar, a form foot, a sheet's head and the toasts, each
 * exempt only while fixed or sticky away from its place in the flow: the page scrolls under them by
 * design and 2.4.11 keeps focus clear of them; any other fixed or sticky box — an open sheet, the
 * sticky trade panel, a sticky table header, a dialog — is judged) and outside an open popup
 * layered over the page (a popover's or a menu's own controls). An enlarged hit area
 * (`.hit::after`) counts as its control's. The capture runs it on the unrolled page (every control
 * in view, the fixed bars past the content's end), or, where unrolling would change the layout (a
 * landscape phone grown taller than wide, a short phone grown past 700 px), at the real size in
 * scroll steps (`responsive.spec.ts`); in an open dialog or overlay, and in a page that fits its
 * screen, at the real size; in each case with every box that scrolls its own content (a dialog's
 * body, an open sheet, the side trade panel) stepped, and with the touch emulation on, so the
 * coarse-pointer hit areas are the ones measured.
 */
export function collectTapCovers(): TapCovers {
    const INTERACTIVE =
        'a[href], button, input:not([type="hidden"]), select, textarea, summary, [role="button"], [role="tab"], ' +
        '[role="menuitem"], [role="link"], [role="checkbox"], [role="radio"], [role="switch"], [role="option"], ' +
        '[tabindex]:not([tabindex="-1"])';
    const MAX_ITEMS = 40;
    const testIdOf = (el: Element | null): string | null => el?.getAttribute('data-testid') ?? null;
    const describe = (el: Element): string => {
        const tag = el.tagName.toLowerCase();
        const tid = testIdOf(el);
        let s = tid ? `${tag}[data-testid="${tid}"]` : tag;
        if (!tid) {
            const cls = [...el.classList].slice(0, 3);
            if (cls.length) s += `.${cls.join('.')}`;
            let a = el.parentElement;
            while (a && !testIdOf(a)) a = a.parentElement;
            if (a) s = `[data-testid="${testIdOf(a)}"] ${s}`;
        }
        const label = (el.getAttribute('aria-label') ?? el.textContent ?? '').replace(/\s+/g, ' ').trim();
        if (label) s += ` «${label.slice(0, 32)}»`;
        return s;
    };
    const isVisible = (el: Element): boolean => {
        if (!el.checkVisibility({ opacityProperty: true, visibilityProperty: true })) return false;
        const r = el.getBoundingClientRect();
        return r.width > 1 && r.height > 1;
    };
    // The page's bars only (the same list and rule as focus-obscured.ts): a control in another fixed
    // or sticky box — an open trade or Manage sheet, the sticky trade panel, a sticky table header, a
    // modal dialog — is judged like any other (the round-13 review). A bar is exempt only while it is
    // over the page: fixed, or sticky and stuck away from its place in the flow (the second approval
    // pass's review: a form foot in the flow, as in a landscape phone's sheet, is judged).
    const BARS = '.app-header, .nav, .statusbar, .action-bar, .form-foot, .sheet-head, .toast-region';
    const overPage = new Map<Element, boolean>();
    const isOverPage = (bar: Element): boolean => {
        const known = overPage.get(bar);
        if (known !== undefined) return known;
        const pos = getComputedStyle(bar).position;
        let over = pos === 'fixed';
        if (pos === 'sticky' && bar instanceof HTMLElement) {
            // Where the flow would put it: the same box without the sticky offset (CSSOM, not an
            // inline style attribute: the CSP allows it), then restored.
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
    const inBar = (el: Element): boolean => {
        for (let b = el.closest(BARS); b; b = b.parentElement?.closest(BARS) ?? null) if (isOverPage(b)) return true;
        return false;
    };
    const dialog = document.querySelector('dialog[open]');
    const scope: ParentNode = dialog ?? document;
    const vw = document.documentElement.clientWidth;
    const vh = window.innerHeight;
    /** Inside a `clip: rect(0 0 0 0)` box (a card table's header row, a skip link before it shows). */
    const clipped = (e: Element): boolean => {
        for (let n: Element | null = e; n; n = n.parentElement) {
            if (/^rect\(0(px)?,? 0(px)?,? 0(px)?,? 0(px)?\)$/.test(getComputedStyle(n).clip)) return true;
        }
        return false;
    };
    const controls = [...scope.querySelectorAll(INTERACTIVE)].filter(
        (el) => isVisible(el) && !el.closest('.skip-link, .visually-hidden, [inert]') && !clipped(el),
    );
    /**
     * The box the control can be seen in: the viewport cut by every box that clips it — the
     * ancestors with a non-visible overflow on its containing-block chain (an absolutely positioned
     * box escapes an unpositioned scroller, a fixed one every box without a transform, filter or
     * containment), up to the top layer (an open modal `<dialog>` or popover is clipped by its own
     * overflow, never by the panel or sheet it sits in in the DOM: the second approval pass's
     * review). Points outside it land on what is drawn there (a dialog's foot past its scrolled
     * body, a sideways strip's edge): a scroll position, not a touch area over the control, so only
     * the points inside are judged.
     */
    const clipBox = (e: Element): { left: number; top: number; right: number; bottom: number } => {
        const box = { left: 0, top: 0, right: vw, bottom: vh };
        const fixedCb = (cs: CSSStyleDeclaration): boolean =>
            cs.transform !== 'none' ||
            cs.filter !== 'none' ||
            cs.perspective !== 'none' ||
            /paint|layout|strict|content/.test(cs.contain) ||
            /transform|filter|perspective/.test(cs.willChange);
        if (e.matches(':modal, :popover-open')) return box;
        let pos = getComputedStyle(e).position;
        for (let n = e.parentElement; n && n !== document.documentElement; n = n.parentElement) {
            const cs = getComputedStyle(n);
            const contains =
                pos === 'fixed' ? fixedCb(cs) : pos === 'absolute' ? cs.position !== 'static' || fixedCb(cs) : true;
            if (contains) {
                if (cs.overflowX !== 'visible' || cs.overflowY !== 'visible') {
                    const b = n.getBoundingClientRect();
                    const left = b.left + n.clientLeft;
                    const top = b.top + n.clientTop;
                    box.left = Math.max(box.left, left);
                    box.top = Math.max(box.top, top);
                    box.right = Math.min(box.right, left + n.clientWidth);
                    box.bottom = Math.min(box.bottom, top + n.clientHeight);
                }
                pos = cs.position;
            }
            if (n.matches(':modal, :popover-open')) break;
        }
        return box;
    };
    const F = [0.15, 0.325, 0.5, 0.675, 0.85];
    // Across one capture's scroll steps (`responsive.spec.ts` sets it): each control counted once.
    const counted = (window as unknown as { __tapCounted?: WeakSet<Element> }).__tapCounted;
    const items: [string, string, number][] = [];
    let judged = 0;
    let count = 0;
    for (const el of controls) {
        // Each line box of the control (a link wrapped over two lines has two), as the focus check.
        const rects = Array.from(el.getClientRects()).filter((r) => r.width >= 1 && r.height >= 1);
        if (!rects.length || rects.some((r) => r.left < 0 || r.top < 0 || r.right > vw || r.bottom > vh)) continue;
        const clip = clipBox(el);
        let seen = 0;
        let on = 0;
        let by: Element | null = null;
        for (const r of rects)
            for (const fx of F)
                for (const fy of F) {
                    const x = r.left + r.width * fx;
                    const y = r.top + r.height * fy;
                    if (x < clip.left || x > clip.right || y < clip.top || y > clip.bottom) continue;
                    seen++;
                    const hit = document.elementFromPoint(x, y);
                    if (!hit || hit === el || el.contains(hit)) continue;
                    const label = hit.closest('label');
                    if (label && (label.contains(el) || (label as HTMLLabelElement).control === el)) continue;
                    const other = hit.closest(INTERACTIVE);
                    if (!other || other === el || other.contains(el) || el.contains(other) || inBar(other)) continue;
                    // An open popup over the page (the tier popover, a menu): a layer, not a touch area.
                    const layer = other.closest('[role="dialog"], [role="menu"], [role="listbox"], [popover]');
                    if (layer && !layer.contains(el)) continue;
                    on++;
                    by ??= other;
                }
        if (seen === 0) continue;
        if (!counted?.has(el)) judged++;
        counted?.add(el);
        if (on === 0 || !by) continue;
        count++;
        if (items.length < MAX_ITEMS) items.push([describe(el), describe(by), on]);
    }
    return { judged, count, items };
}
