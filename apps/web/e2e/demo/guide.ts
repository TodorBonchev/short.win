import { type BrowserContext, expect, type Locator, type Page } from '@playwright/test';

/**
 * A DOM pointer is visible in Playwright's video, including above native dialogs.
 * Its position and click rings come from the browser's actual pointer events.
 */
export async function installPointer(context: BrowserContext, touch = false): Promise<void> {
    await context.addInitScript((touch) => {
        const install = () => {
            if (document.getElementById('shortwin-demo-pointer')) return;

            const layer = document.createElement('div');
            layer.id = 'shortwin-demo-pointer';
            layer.setAttribute('popover', 'manual');
            layer.setAttribute('aria-hidden', 'true');
            Object.assign(layer.style, {
                position: 'fixed',
                inset: '0',
                width: '100vw',
                height: '100vh',
                margin: '0',
                padding: '0',
                border: '0',
                background: 'transparent',
                overflow: 'visible',
                pointerEvents: 'none',
                zIndex: '2147483647',
            });

            const pointer = document.createElement('div');
            Object.assign(pointer.style, {
                position: 'absolute',
                left: '0',
                top: '0',
                width: '0',
                height: '0',
                visibility: 'hidden',
                pointerEvents: 'none',
            });
            const halo = document.createElement('div');
            Object.assign(halo.style, {
                position: 'absolute',
                left: '-22px',
                top: '-22px',
                width: '44px',
                height: '44px',
                borderRadius: '50%',
                border: '2px solid #f0b90bbf',
                background: '#f0b90b30',
                boxShadow: '0 0 18px #f0b90b35',
                boxSizing: 'border-box',
            });
            const arrow = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
            arrow.setAttribute('width', '25');
            arrow.setAttribute('height', '33');
            arrow.setAttribute('viewBox', '-2 -2 25 33');
            Object.assign(arrow.style, {
                position: 'absolute',
                left: '-2px',
                top: '-2px',
                overflow: 'visible',
                filter: 'drop-shadow(0 2px 2px #0009)',
            });
            const shape = document.createElementNS('http://www.w3.org/2000/svg', 'path');
            shape.setAttribute('d', 'M0 0 L0 24 L6.8 17.7 L11.9 28 L16.5 25.7 L11.5 15.8 L21 15 Z');
            shape.setAttribute('fill', '#fff');
            shape.setAttribute('stroke', '#1b2029');
            shape.setAttribute('stroke-width', '1.8');
            shape.setAttribute('stroke-linejoin', 'round');
            arrow.append(shape);
            pointer.append(halo, arrow);
            layer.append(pointer);
            document.body.append(layer);

            let seenPointer = false;
            let x = 0;
            let y = 0;
            let dialogs: HTMLDialogElement[] = [];
            const visible = () => {
                const card = document.getElementById('shortwin-demo-overlay');
                const cardVisible = !!card && card.checkVisibility();
                layer.style.visibility = cardVisible ? 'hidden' : 'visible';
                pointer.style.visibility = !touch && seenPointer && !cardVisible ? 'visible' : 'hidden';
            };
            const raise = () => {
                const open = [...document.querySelectorAll<HTMLDialogElement>('dialog[open]')];
                if (open.length !== dialogs.length || open.some((dialog, index) => dialog !== dialogs[index])) {
                    dialogs = open;
                    if (layer.matches(':popover-open')) layer.hidePopover();
                }
                if (!layer.matches(':popover-open')) layer.showPopover();
                visible();
            };
            raise();
            new MutationObserver(raise).observe(document.body, {
                subtree: true,
                childList: true,
                attributes: true,
                attributeFilter: ['open', 'hidden'],
            });
            document.addEventListener(
                'pointermove',
                (event) => {
                    x = event.clientX;
                    y = event.clientY;
                    seenPointer = true;
                    pointer.style.transform = `translate(${x}px, ${y}px)`;
                    visible();
                },
                true,
            );
            document.addEventListener(
                'pointerdown',
                (event) => {
                    x = event.clientX;
                    y = event.clientY;
                    const ring = document.createElement('div');
                    Object.assign(ring.style, {
                        position: 'absolute',
                        left: `${x - 20}px`,
                        top: `${y - 20}px`,
                        width: '40px',
                        height: '40px',
                        borderRadius: '50%',
                        border: '3px solid #f0b90b',
                        boxSizing: 'border-box',
                        background: '#f0b90b25',
                        pointerEvents: 'none',
                    });
                    layer.append(ring);
                    const pulse = ring.animate(
                        [
                            { transform: 'scale(.5)', opacity: 1 },
                            { transform: 'scale(2.2)', opacity: 0 },
                        ],
                        { duration: 800, easing: 'ease-out' },
                    );
                    void pulse.finished.then(() => ring.remove());
                    halo.animate([{ transform: 'scale(1)' }, { transform: 'scale(.8)' }, { transform: 'scale(1)' }], {
                        duration: 240,
                    });
                },
                true,
            );
        };
        if (document.body) install();
        else document.addEventListener('DOMContentLoaded', install, { once: true });
    }, touch);
}

/** Deliberate cursor travel and short pauses make each recorded action readable. */
export class DemoGuide {
    private readonly page: Page;
    private readonly cue: (text: string) => void;
    private readonly touch: boolean;
    private position = { x: 0, y: 0 };

    constructor(page: Page, cue: (text: string) => void, touch = false) {
        this.page = page;
        this.cue = cue;
        this.touch = touch;
    }

    async move(target: Locator, label: string): Promise<void> {
        await target.waitFor({ state: 'visible' });
        if (this.touch) {
            this.cue(label);
            await target.evaluate((element) =>
                element.scrollIntoView({ behavior: 'smooth', block: 'center', inline: 'nearest' }),
            );
            await this.page.waitForTimeout(650);
            await target.scrollIntoViewIfNeeded();
            await expect(target).toBeEnabled();
            await this.page.waitForTimeout(350);
            return;
        }
        await target.scrollIntoViewIfNeeded();
        await expect(target).toBeEnabled();
        const box = await target.boundingBox();
        if (!box) throw new Error(`demo target is not visible: ${label}`);
        const destination = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
        const start = this.position;
        this.cue(label);
        for (let step = 1; step <= 24; step++) {
            const eased = (1 - Math.cos((Math.PI * step) / 24)) / 2;
            await this.page.mouse.move(
                start.x + (destination.x - start.x) * eased,
                start.y + (destination.y - start.y) * eased,
            );
            await this.page.waitForTimeout(25);
        }
        this.position = destination;
        await this.page.waitForTimeout(300);
    }

    async click(target: Locator, label: string): Promise<void> {
        await this.move(target, label);
        if (this.touch) await target.tap();
        else await target.click({ delay: 80 });
        await this.page.waitForTimeout(500);
    }

    async check(target: Locator, label: string): Promise<void> {
        if (await target.isChecked()) return;
        await this.click(target, label);
        if (!(await target.isChecked())) throw new Error(`demo checkbox did not become checked: ${label}`);
    }

    async fill(target: Locator, value: string, label: string): Promise<void> {
        await this.move(target, label);
        if (this.touch) await target.tap();
        else await target.click({ delay: 80 });
        await target.fill(value);
        await this.page.waitForTimeout(500);
    }
}
