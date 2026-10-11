/**
 * A glossary entry's definition as shown (`glossary.ts`; FRONTEND.md §4.3): its own text, or the
 * approved disclosure paragraphs it quotes, verbatim, with its placeholders filled. A module of its
 * own so the disclosures' text loads with the first opened definition, not with the app shell.
 */
import type { Params } from '@shortwin/client';
import { DISCLOSURES } from './disclosures.ts';
import { lenderRiskVars } from './disclosureVars.ts';
import { GLOSSARY, type GlossaryEntry, type GlossaryId } from './glossary.ts';

/** The placeholders an entry may use, from the live `GlobalConfig` (fallback words while unread). */
export function glossaryVars(params: Pick<Params, 'lockGraceS' | 'lumpVestS'> | null | undefined) {
    return lenderRiskVars(params);
}

/** An entry's definition: its own text, or the quoted disclosure paragraphs verbatim, filled. */
export function glossaryText(id: GlossaryId, vars: Readonly<Record<string, string>> = glossaryVars(null)): string {
    const e: GlossaryEntry = GLOSSARY[id];
    const raw = e.quote
        ? e.quote.paragraphs.map((i) => DISCLOSURES[e.quote?.disclosure ?? 'tx.simulation'].body[i] ?? '').join(' ')
        : (e.text ?? '');
    return raw.replace(/\{(\w+)\}/g, (_, k: string) => {
        const v = vars[k];
        if (v === undefined) throw new Error(`glossary ${id}: missing {${k}}`);
        return v;
    });
}

/** Whether an entry's text has placeholders (it then reads the live config). */
export function glossaryHasVars(id: GlossaryId): boolean {
    const e: GlossaryEntry = GLOSSARY[id];
    const raw = e.quote
        ? e.quote.paragraphs.map((i) => DISCLOSURES[e.quote?.disclosure ?? 'tx.simulation'].body[i]).join(' ')
        : e.text;
    return /\{\w+\}/.test(raw ?? '');
}
