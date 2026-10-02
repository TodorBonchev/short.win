/**
 * The tier badge and its "Why this tier" popover (FRONTEND.md §3.4): the tier inputs from
 * `explainTier` and the `market.tierAge` disclosure. A non-modal popover: Escape or a click outside
 * closes it and focus returns to the badge.
 */
import { useEffect, useId, useRef, useState } from 'react';
import { TierBadge } from '../../../components/Badge.tsx';
import { Disclosure } from '../../../components/Disclosure.tsx';
import { T } from '../../../config/testids.ts';
import { explainTier, type TierWhyInput } from './tierWhy.ts';

export function TierWhyPopover({ input }: { input: TierWhyInput }) {
    const [open, setOpen] = useState(false);
    const id = useId();
    const box = useRef<HTMLSpanElement>(null);
    const button = useRef<HTMLButtonElement>(null);
    const panel = useRef<HTMLDivElement>(null);

    useEffect(() => {
        if (!open) return;
        panel.current?.focus();
        const onDown = (e: MouseEvent) => {
            if (box.current && !box.current.contains(e.target as Node)) setOpen(false);
        };
        document.addEventListener('mousedown', onDown);
        return () => document.removeEventListener('mousedown', onDown);
    }, [open]);

    const why = explainTier(input);
    return (
        <span className="mk-anchor" ref={box}>
            <button
                ref={button}
                type="button"
                className="mk-tier-button"
                aria-expanded={open}
                aria-controls={id}
                aria-label={`Tier ${input.tier}: why this tier`}
                data-testid={T.marketTier}
                data-value={input.tier}
                onClick={() => setOpen((o) => !o)}
            >
                <TierBadge tier={input.tier} />
            </button>
            {open && (
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
                            setOpen(false);
                            button.current?.focus();
                        }
                    }}
                >
                    <h3>Why Tier {input.tier}</h3>
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
            )}
        </span>
    );
}
