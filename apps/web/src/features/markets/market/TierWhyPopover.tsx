/**
 * The tier badge and its "Why this tier" popover (FRONTEND.md §3.4): the tier inputs from
 * `explainTier` and the `market.tierAge` disclosure. A non-modal popover: Escape, its close button
 * or a tap / click outside closes it and focus returns to the badge. The badge reads "Tier C ⓘ" (it
 * opens something). On a phone the popover is a bottom sheet over a scrim (M10-RESP §7.8).
 */
import { useEffect, useId, useRef, useState } from 'react';
import { Disclosure } from '../../../components/Disclosure.tsx';
import { T } from '../../../config/testids.ts';
import { NavIcon } from '../../../shell/icons.tsx';
import { explainTier, type TierWhyInput } from './tierWhy.ts';

export function TierWhyPopover({ input }: { input: TierWhyInput }) {
    const [open, setOpen] = useState(false);
    const id = useId();
    const button = useRef<HTMLButtonElement>(null);
    const panel = useRef<HTMLDivElement>(null);

    useEffect(() => {
        if (!open) return;
        panel.current?.focus();
        const onDown = (e: PointerEvent) => {
            const t = e.target as Node;
            // Outside the popover and its badge (the scrim counts as outside).
            if (!panel.current?.contains(t) && !button.current?.contains(t)) setOpen(false);
        };
        document.addEventListener('pointerdown', onDown);
        return () => document.removeEventListener('pointerdown', onDown);
    }, [open]);

    const close = () => {
        setOpen(false);
        button.current?.focus();
    };
    const why = explainTier(input);
    return (
        <span className="mk-anchor">
            <button
                ref={button}
                type="button"
                className="mk-tier-button hit"
                aria-expanded={open}
                aria-controls={id}
                aria-label={`Tier ${input.tier}: why this tier`}
                data-testid={T.marketTier}
                data-value={input.tier}
                onClick={() => setOpen((o) => !o)}
            >
                <span className="badge tier" data-tier={input.tier}>
                    Tier {input.tier}
                    <NavIcon name="info" className="mk-tier-info" />
                </span>
            </button>
            {open && (
                <>
                    {/* A phone's dimmed page under the sheet: a tap outside closes it (Escape and ✕ for keys). */}
                    <span className="mk-popover-scrim" aria-hidden="true" />
                    <div
                        id={id}
                        ref={panel}
                        role="dialog"
                        aria-label="Why this tier"
                        tabIndex={-1}
                        className="mk-popover"
                        data-testid={T.marketTierWhy}
                        onKeyDown={(e) => {
                            if (e.key === 'Escape') {
                                e.stopPropagation();
                                close();
                            }
                        }}
                    >
                        <div className="mk-popover-head">
                            <h3>Why Tier {input.tier}</h3>
                            <button
                                type="button"
                                className="btn mk-popover-close"
                                data-variant="ghost"
                                aria-label="Close"
                                data-testid={T.marketTierWhyClose}
                                onClick={close}
                            >
                                ✕
                            </button>
                        </div>
                        <ul className="mk-why-list">
                            {why.lines.map((l) => (
                                <li key={l.key} data-key={l.key}>
                                    <span className="mk-why-key">{l.label}</span>
                                    <span>
                                        {l.value}
                                        <span className="mk-why-effect">{l.effect}</span>
                                    </span>
                                </li>
                            ))}
                        </ul>
                        <Disclosure id="market.tierAge" />
                    </div>
                </>
            )}
        </span>
    );
}
