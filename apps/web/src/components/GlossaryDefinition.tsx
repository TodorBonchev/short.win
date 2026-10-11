/**
 * A glossary definition's text (FRONTEND.md §4.3), loaded with the first definition opened: the
 * entries' own text or the approved paragraphs they quote (`glossaryText.ts`, which brings the
 * disclosures' text), filled. An entry with placeholders (`{vestPeriod}`, `{lockGrace}`) reads the
 * live config through `GlossaryLive` (its own chunk: the chain-account readers); until it has, the
 * approved fallback words ("the vesting period").
 */
import { lazy, Suspense } from 'react';
import type { GlossaryId } from '../config/glossary.ts';
import { glossaryHasVars, glossaryText, glossaryVars } from '../config/glossaryText.ts';

const GlossaryLive = lazy(() => import('./GlossaryLive.tsx'));

export default function GlossaryDefinition({ id }: { id: GlossaryId }) {
    const plain = glossaryText(id, glossaryVars(null));
    if (!glossaryHasVars(id)) return <>{plain}</>;
    return (
        <Suspense fallback={plain}>
            <GlossaryLive id={id} />
        </Suspense>
    );
}
