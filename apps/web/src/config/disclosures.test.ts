import { describe, expect, it } from 'vitest';
import { readRepo } from '../test/repo.ts';
import { DISCLOSURES, type DisclosureId, LENDER_SOL_REPAYMENT, placeholders, renderDisclosure } from './disclosures.ts';

describe('disclosures', () => {
    it('carries SPEC §5.3’s lender sentence verbatim and requires it as the acknowledgement', () => {
        const spec = readRepo('docs/SPEC.md');
        expect(spec).toContain(`"${LENDER_SOL_REPAYMENT}"`);
        const d = DISCLOSURES['lender.solRepayment'];
        expect(d.body).toEqual([LENDER_SOL_REPAYMENT]);
        expect(d.ack).toBe(LENDER_SOL_REPAYMENT);
    });

    it('keys every entry by its own id and gives every entry a title, a body and a source', () => {
        for (const [key, d] of Object.entries(DISCLOSURES)) {
            expect(d.id).toBe(key);
            expect(d.title.length).toBeGreaterThan(0);
            expect(d.body.length).toBeGreaterThan(0);
            expect(d.source.length).toBeGreaterThan(0);
            expect(d.ack === null || d.ack.length > 0).toBe(true);
        }
    });

    it('requires an acknowledgement for the lender and borrower risk disclosures', () => {
        const acked = (Object.keys(DISCLOSURES) as DisclosureId[]).filter((id) => DISCLOSURES[id].ack !== null);
        expect(acked).toEqual(['lender.solRepayment', 'lender.exitForfeitsVest', 'borrower.liquidation']);
    });

    it('fills placeholders and refuses to render a missing one', () => {
        expect(placeholders('borrower.liquidation')).toEqual([
            'liqCr',
            'penalty',
            'maxDuration',
            'expiryFee',
            'maxSlippage',
            's3Wait',
            'minRent',
        ]);
        // Governable numbers are placeholders, never literals in the copy (FRONTEND.md §4.1 rule 5).
        expect(placeholders('borrower.residuals')).toEqual(['keeperReward', 'maxDuration']);
        expect(placeholders('lender.risks')).toEqual(['lockGrace', 'vestPeriod']);
        expect(placeholders('market.routeBroken')).toEqual(['s3Wait']);
        expect(() => renderDisclosure('borrower.liquidation', { liqCr: '125%' })).toThrow(/missing \{penalty\}/);
        const body = renderDisclosure('lender.exitForfeitsVest', { vesting: '0.42' });
        expect(body[0]).toContain('You have 0.42 SOL of income still vesting.');
        for (const id of Object.keys(DISCLOSURES) as DisclosureId[]) {
            const vars = Object.fromEntries(placeholders(id).map((p) => [p, 'x']));
            for (const line of renderDisclosure(id, vars)) expect(line).not.toMatch(/\{\w+\}/);
        }
    });

    it('is documented verbatim in FRONTEND.md §4: id, title, every paragraph and the acknowledgement', () => {
        const doc = readRepo('docs/design/FRONTEND.md');
        for (const d of Object.values(DISCLOSURES)) {
            expect(doc, d.id).toContain(`#### \`${d.id}\` — ${d.title}`);
            for (const p of d.body) expect(doc, `${d.id}: ${p.slice(0, 40)}`).toContain(`> ${p}`);
            if (d.ack) expect(doc, `${d.id} ack`).toContain(`Checkbox: “${d.ack}”`);
        }
    });
});
