/**
 * Untrusted display text (FE-D8, R-MINT §10.11; M9-SEC-5): token symbols and names are whatever a
 * mint's Metaplex or Token-2022 metadata says, relayed by the indexer. They are normalised once,
 * where indexer data enters the app (`api/client.ts`, `api/ws.ts`): control characters, bidi
 * embeddings / overrides / isolates, zero-width and other invisible format characters are removed,
 * the text is NFC-normalised, whitespace collapsed and the length capped (symbols 16, names 64 code
 * points). Rendering isolates it (`<bdi>`, `unicode-bidi: isolate`), and wherever a symbol names the
 * token of a transaction the short mint stands next to it (`tokenLabel`): a homoglyph ("USDС" with a
 * Cyrillic С) survives normalisation, the mint does not lie.
 */
import { shortAddress } from './format.ts';

/** Max code points shown of a symbol / name. */
export const SYMBOL_MAX = 16;
export const NAME_MAX = 64;

/**
 * C0 / C1 controls, soft hyphen, Arabic letter mark, Hangul fillers, Mongolian / variation
 * selectors, zero-width and bidi marks (U+200B–U+200F), line / paragraph separators and bidi
 * embeddings / overrides (U+2028–U+202E), word joiner / invisible operators / bidi isolates
 * (U+2060–U+206F), BOM, interlinear annotation, tag characters. The indexer strips the same set
 * (`services/indexer/src/ingest/refresher.rs` `invisible`).
 */
export function invisible(cp: number): boolean {
    return (
        cp <= 0x1f ||
        (cp >= 0x7f && cp <= 0x9f) ||
        cp === 0xad ||
        cp === 0x61c ||
        cp === 0x115f ||
        cp === 0x1160 ||
        cp === 0x17b4 ||
        cp === 0x17b5 ||
        (cp >= 0x180b && cp <= 0x180f) ||
        (cp >= 0x200b && cp <= 0x200f) ||
        (cp >= 0x2028 && cp <= 0x202e) ||
        (cp >= 0x2060 && cp <= 0x206f) ||
        cp === 0x3164 ||
        (cp >= 0xfe00 && cp <= 0xfe0f) ||
        cp === 0xfeff ||
        cp === 0xffa0 ||
        (cp >= 0xfff9 && cp <= 0xfffb) ||
        (cp >= 0xe0000 && cp <= 0xe007f)
    );
}

const SPACE = /^\s$/u;

/** The cleaned text, or `null` when nothing visible is left. */
export function cleanText(raw: unknown, max: number): string | null {
    if (typeof raw !== 'string') return null;
    let out = '';
    let pendingSpace = false;
    for (const ch of raw.normalize('NFC')) {
        if (SPACE.test(ch)) {
            pendingSpace = out.length > 0;
            continue;
        }
        if (invisible(ch.codePointAt(0) ?? 0)) continue;
        if (pendingSpace) {
            out += ' ';
            pendingSpace = false;
        }
        out += ch;
    }
    if (!out) return null;
    const cps = Array.from(out);
    return cps.length > max ? `${cps.slice(0, max - 1).join('')}…` : out;
}

export const cleanSymbol = (raw: unknown): string | null => cleanText(raw, SYMBOL_MAX);
export const cleanName = (raw: unknown): string | null => cleanText(raw, NAME_MAX);

/**
 * Cleans the `symbol` and `name` string fields of an indexer payload in place (any depth), so no
 * component ever sees the raw text. Returns the same value.
 */
export function cleanIndexerText<T>(value: T): T {
    const walk = (v: unknown, depth: number): void => {
        if (depth > 8 || v === null || typeof v !== 'object') return;
        if (Array.isArray(v)) {
            for (const x of v) walk(x, depth + 1);
            return;
        }
        const o = v as Record<string, unknown>;
        for (const k of Object.keys(o)) {
            const x = o[k];
            if (k === 'symbol' && typeof x === 'string') o[k] = cleanSymbol(x);
            else if (k === 'name' && typeof x === 'string') o[k] = cleanName(x);
            else walk(x, depth + 1);
        }
    };
    walk(value, 0);
    return value;
}

/**
 * A token as text for titles, labels and units: the (clean) symbol and the short mint, or the
 * short mint alone — "BONK · DezX…B263".
 */
export function tokenLabel(symbol: string | null | undefined, mint: string): string {
    const s = cleanSymbol(symbol);
    const m = shortAddress(mint, 4, 4);
    return s && s !== m ? `${s} · ${m}` : m;
}

/** The mints among `hits` whose symbol (case-insensitive) another hit — another mint — also carries. */
export function duplicateMints(hits: readonly { mint: string; symbol: string | null }[]): Set<string> {
    const bySymbol = new Map<string, Set<string>>();
    for (const h of hits) {
        if (!h.symbol) continue;
        const key = h.symbol.toUpperCase();
        const set = bySymbol.get(key) ?? new Set<string>();
        set.add(h.mint);
        bySymbol.set(key, set);
    }
    const out = new Set<string>();
    for (const set of bySymbol.values()) if (set.size > 1) for (const m of set) out.add(m);
    return out;
}
