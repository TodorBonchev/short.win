/**
 * Header token search (SPEC §8: by mint or ticker; FRONTEND.md §3.1). Combobox pattern: results
 * from `/v1/search`; a valid mint always offers "Open <mint>" even before the indexer knows it
 * (the market page then reads the chain or offers to create the market).
 *
 * Under 1024 px (FRONTEND.md §7.6) the box folds into a Search button (`search-toggle`) that opens
 * it as a full-screen overlay (the same element, a sheet: `useSheetMode`), with Cancel
 * (`search-close`); Escape or Cancel close it and focus returns to the button.
 */
import type { Address } from '@solana/kit';
import { useQuery } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { useEffect, useId, useRef, useState } from 'react';
import { indexerQueries } from '../api/queries.ts';
import type { SearchHit } from '../api/types.ts';
import { addressInputProps } from '../components/Field.tsx';
import { useSheetMode } from '../components/Sheet.tsx';
import { T } from '../config/testids.ts';
import { parseAddress } from '../lib/address.ts';
import { duplicateMints } from '../lib/displayText.ts';
import { shortAddress } from '../lib/format.ts';
import { COMPACT, matches } from '../styles/breakpoints.ts';
import { NavIcon } from './icons.tsx';

function useDebounced<V>(v: V, ms: number): V {
    const [x, setX] = useState(v);
    useEffect(() => {
        const t = setTimeout(() => setX(v), ms);
        return () => clearTimeout(t);
    }, [v, ms]);
    return x;
}

export function SearchBox() {
    const [text, setText] = useState('');
    const [open, setOpen] = useState(false);
    const [active, setActive] = useState(0);
    /** The compact overlay (no effect from 1024 px up, where the box is always in the header). */
    const [overlay, setOverlay] = useState(false);
    const input = useRef<HTMLInputElement>(null);
    const box = useRef<HTMLDivElement>(null);
    const toggle = useRef<HTMLButtonElement>(null);
    const titleId = useId();
    const closeOverlay = () => {
        setOverlay(false);
        setOpen(false);
    };
    useSheetMode(box, {
        open: overlay,
        onClose: closeOverlay,
        labelledBy: titleId,
        initialFocus: () => input.current,
    });
    // "/" focuses the search from anywhere outside a text field (terminal convention).
    useEffect(() => {
        const onKey = (e: KeyboardEvent) => {
            const t = e.target as HTMLElement | null;
            const typing =
                t &&
                (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.tagName === 'SELECT' || t.isContentEditable);
            if (e.key === '/' && !typing && !e.metaKey && !e.ctrlKey && !e.altKey) {
                e.preventDefault();
                if (matches(COMPACT)) setOverlay(true);
                else input.current?.focus();
            }
        };
        document.addEventListener('keydown', onKey);
        return () => document.removeEventListener('keydown', onKey);
    }, []);
    const q = useDebounced(text.trim(), 150);
    const listId = useId();
    const navigate = useNavigate();
    const res = useQuery({ ...indexerQueries.search(q), enabled: q.length > 0 && q.length <= 64 });
    const mint = parseAddress(q);
    const hits: readonly SearchHit[] = res.data?.data.data ?? [];
    // A symbol is untrusted text (M9-SEC-5): every result shows the short mint, and a symbol two
    // results share is marked.
    const dups = duplicateMints(hits);
    const items: { key: string; mint: string; label: string; name: string | null; dup: boolean }[] = hits.map((h) => ({
        key: h.mint,
        mint: h.mint,
        label: h.symbol ?? shortAddress(h.mint),
        name: h.name,
        dup: dups.has(h.mint),
    }));
    if (mint && !items.some((i) => i.mint === mint))
        items.unshift({ key: `go-${mint}`, mint, label: 'Open mint', name: null, dup: false });

    // In the overlay the list stays while the keyboard is down (a tap outside the input blurs it).
    const showList = (open || overlay) && items.length > 0;
    const go = (m: string) => {
        setOpen(false);
        setOverlay(false);
        setText('');
        void navigate({ to: '/m/$mint', params: { mint: m as Address } });
    };

    return (
        <>
            <button
                ref={toggle}
                type="button"
                className="btn search-toggle"
                data-variant="ghost"
                aria-label="Search"
                aria-expanded={overlay}
                data-testid={T.searchToggle}
                onClick={() => setOverlay(true)}
            >
                <NavIcon name="search" className="search-toggle-icon" />
            </button>
            <div ref={box} className="search" data-open={overlay ? '' : undefined}>
                <span id={titleId} className="visually-hidden">
                    Search markets
                </span>
                <div className="search-row">
                    <div className="input-wrap">
                        <input
                            ref={input}
                            className="input"
                            role="combobox"
                            aria-label="Search token by mint or ticker"
                            aria-expanded={showList}
                            aria-controls={listId}
                            aria-activedescendant={showList && items[active] ? `${listId}-${active}` : undefined}
                            placeholder="Jump to a market…"
                            value={text}
                            maxLength={64}
                            {...addressInputProps}
                            enterKeyHint="search"
                            data-testid={T.searchInput}
                            onChange={(e) => {
                                setText(e.target.value);
                                setOpen(true);
                                setActive(0);
                            }}
                            onFocus={() => {
                                setOpen(true);
                                if (matches(COMPACT)) setOverlay(true);
                            }}
                            onBlur={() => setTimeout(() => setOpen(false), 120)}
                            onKeyDown={(e) => {
                                if (e.key === 'ArrowDown') setActive((a) => Math.min(a + 1, items.length - 1));
                                else if (e.key === 'ArrowUp') setActive((a) => Math.max(a - 1, 0));
                                else if (e.key === 'Enter' && items[active]) go(items[active].mint);
                                else if (e.key === 'Escape') {
                                    if (overlay) closeOverlay();
                                    else setOpen(false);
                                } else return;
                                e.preventDefault();
                            }}
                        />
                        <span className="search-kbd" aria-hidden="true">
                            /
                        </span>
                    </div>
                    <button
                        type="button"
                        className="btn search-close"
                        data-variant="ghost"
                        data-testid={T.searchClose}
                        onClick={closeOverlay}
                    >
                        Cancel
                    </button>
                </div>
                {showList && (
                    <div id={listId} className="search-results" role="listbox" data-testid={T.searchResults}>
                        {items.map((it, i) => (
                            <div
                                key={it.key}
                                id={`${listId}-${i}`}
                                role="option"
                                tabIndex={-1}
                                className="search-option"
                                aria-selected={i === active}
                                data-testid={T.searchResult(it.mint)}
                                onMouseDown={(e) => {
                                    e.preventDefault();
                                    go(it.mint);
                                }}
                            >
                                <strong>
                                    <bdi>{it.label}</bdi>
                                </strong>
                                <span className="num" title={it.mint}>
                                    {shortAddress(it.mint, 4, 4)}
                                </span>
                                {it.dup && (
                                    <span className="badge" data-tone="warn" title="Another result uses this symbol">
                                        same symbol
                                    </span>
                                )}
                                {it.name && (
                                    <bdi className="muted" title={it.name}>
                                        {it.name}
                                    </bdi>
                                )}
                            </div>
                        ))}
                    </div>
                )}
            </div>
        </>
    );
}
