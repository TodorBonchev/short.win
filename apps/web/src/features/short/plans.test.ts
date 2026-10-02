/**
 * The borrower's transaction plans (FRONTEND.md §5.1, §5.10): the Codama instructions with the
 * venue's remaining accounts in the program's order, the bounds of the preview encoded exactly,
 * only pinned programs at the top level (§5.8), the review's watched accounts and disclosures, and a
 * message within the 1,232-byte limit — built on the program-derived snapshots of
 * `vectors/program-vectors.json` (the mock venue).
 */
import {
    findExposurePda,
    getAddMarginInstructionDataDecoder,
    getCloseShortInstructionDataDecoder,
    getOpenShortInstructionDataDecoder,
    SHORTWIN_PROGRAM_ADDRESS,
} from '@shortwin/client';
import { AccountRole, type Address, type Blockhash, createNoopSigner, type Instruction } from '@solana/kit';
import { describe, expect, it } from 'vitest';
import { renderDisclosure } from '../../config/disclosures.ts';
import { assertAllowedInstructions } from '../../tx/integrity.ts';
import { buildMessage, MAX_TRANSACTION_BYTES, SIMULATION_CU_LIMIT, wireOf } from '../../tx/message.ts';
import { MOCK_CPAMM_PROGRAM } from '../../venues/constants.ts';
import { mockListingFrom } from '../../venues/index.ts';
import { keeperRecipientIndex, NO_EXTRAS } from '../../venues/lists.ts';
import { mockPool } from '../../venues/pda.ts';
import {
    borrowerDisclosureVars,
    extrasFor,
    planAddMargin,
    planCloseShort,
    planOpenShort,
    proseDuration,
    randomNonce,
} from './plans.ts';
import { assignTier, previewCloseFor, previewOpen } from './previews.ts';
import { decodePosition, exposureQ } from './snapshot.ts';
import { programVectors, testMath, vectorCase, vectorSnapshot } from './testing.ts';

const lifetime = { blockhash: '11111111111111111111111111111111' as Blockhash, lastValidBlockHeight: 1n };

function sizeOf(
    instructions: Parameters<typeof buildMessage>[0]['instructions'],
    payer: Address,
    version: 0 | 'legacy',
) {
    const message = buildMessage({
        instructions,
        feePayer: createNoopSigner(payer),
        lifetime,
        computeUnitLimit: SIMULATION_CU_LIMIT,
        computeUnitPrice: 0n,
        lookupTables: {},
        version,
    });
    return wireOf(message).size;
}

describe('open_short plan', () => {
    it('encodes the preview bounds and appends ReadSet ++ TradeExtras in the program order', async () => {
        const c = vectorCase('spl-open-1');
        const { s, get } = await vectorSnapshot(c);
        const signer = createNoopSigner(c.borrower as Address);
        const [ex] = await findExposurePda({ market: s.marketAddress, borrower: signer.address });
        const walletQ = exposureQ(get(ex));
        const preview = previewOpen(testMath(), s, { q: BigInt(c.q ?? 0), slippageBps: 100, walletQ });
        if (!preview) throw new Error('preview');
        const tier = assignTier(testMath(), s);
        const tp = tier === null ? undefined : s.config.tiers[tier];
        if (!tp) throw new Error('tier');
        const extras = await extrasFor(s, 'sell');
        expect(extras.map((e) => e.label)).toEqual(['none']);
        const plan = await planOpenShort({
            snapshot: s,
            signer,
            symbol: 'MOCK',
            q: preview.q,
            minProceeds: preview.minProceeds,
            maxMargin: preview.maxMargin,
            maxOpenFee: preview.maxOpenFee,
            tier: tp,
            extras: extras[0],
            nonce: 7n,
            pick: keeperRecipientIndex,
        });
        expect(plan.kind).toBe('open_short');
        expect(plan.nonce).toBe(7n);
        expect(plan.instructions).toHaveLength(1);
        const ix = plan.instructions[0];
        if (!ix?.accounts || !ix.data) throw new Error('instruction');
        expect(ix.programAddress).toBe(SHORTWIN_PROGRAM_ADDRESS);
        const data = getOpenShortInstructionDataDecoder().decode(ix.data);
        expect(data.nonce).toBe(7n);
        expect(data.q).toBe(preview.q);
        expect(data.minProceeds).toBe(preview.minProceeds);
        expect(data.maxMargin).toBe(preview.maxMargin);
        expect(data.maxOpenFee).toBe(preview.maxOpenFee);
        expect(data.extras.set).toBe(0);
        // 15 named accounts, then the mock's ReadSet (3, writable) and TradeExtras (5).
        const named = ix.accounts.slice(0, 15);
        expect(named[0]?.address).toBe(signer.address);
        expect(named[0]?.role).toBe(AccountRole.WRITABLE_SIGNER);
        expect(named[5]?.address).toBe(plan.position);
        expect(named[7]?.address).toBe(s.mint);
        expect(named[10]?.address).toBe(s.market.trader);
        expect(named[11]?.address).toBe(s.tokenProgram);
        const remaining = ix.accounts.slice(15);
        expect(remaining.map((a) => a.address)).toEqual([
            ...s.readKeys,
            MOCK_CPAMM_PROGRAM,
            'FchvTASEWCcGETHi6QMLW7v64Jzn66W8Z3yqZjY9iqLA',
            'So11111111111111111111111111111111111111112',
            'TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA',
            s.market.traderWsol,
        ]);
        expect(remaining.map((a) => a.role)).toEqual([1, 1, 1, 0, 0, 0, 0, 1]);
        // Only pinned programs at the top level; the message fits.
        expect(() => assertAllowedInstructions(plan.instructions)).not.toThrow();
        expect(sizeOf(plan.instructions, signer.address, 0)).toBeLessThanOrEqual(MAX_TRANSACTION_BYTES);
        expect(sizeOf(plan.instructions, signer.address, 'legacy')).toBeLessThanOrEqual(MAX_TRANSACTION_BYTES);
        // The review: wallet SOL, the new position, the market's free inventory; the borrower disclosure.
        expect(plan.watch.map((w) => w.kind)).toEqual(['lamports', 'program', 'program']);
        expect(plan.watch[1]?.address).toBe(plan.position);
        expect(plan.disclosures).toEqual(['borrower.liquidation']);
        const vars = plan.disclosureVars?.['borrower.liquidation'];
        expect(() => renderDisclosure('borrower.liquidation', vars)).not.toThrow();
        expect(plan.invalidate.length).toBeGreaterThan(0);
    });

    it('fills the borrower disclosure from the live tier terms', async () => {
        const { s } = await vectorSnapshot(vectorCase('spl-open-0'));
        const tp = s.config.tiers[2];
        if (!tp) throw new Error('tier');
        const vars = borrowerDisclosureVars(tp, s.config.params);
        const text = renderDisclosure('borrower.liquidation', vars).join(' ');
        expect(text).toContain(`below ${vars.liqCr} on both`);
        expect(text).not.toMatch(/\{\w+\}/);
        // Governable terms come from the live configuration, not the copy (M9-UX-3).
        expect(vars.expiryFee).toBe(`${s.config.params.expiryFeeBps / 100}%`);
        expect(renderDisclosure('borrower.residuals', vars).join(' ')).not.toMatch(/\{\w+\}/);
        expect(proseDuration(3_600)).toBe('1 hour');
        expect(proseDuration(10 * 86_400)).toBe('10 days');
        expect(proseDuration(72 * 3_600)).toBe('3 days');
        expect(proseDuration(90)).toBe('90 seconds');
    });

    it('draws a fresh u64 nonce', () => {
        const a = randomNonce();
        const b = randomNonce();
        expect(a).not.toBe(b);
        expect(a >= 0n && a < 1n << 64n).toBe(true);
    });
});

describe('close_short and add_margin plans', () => {
    it('close: max_buyback_cost from the preview, the venue accounts, the position closing', async () => {
        const c = vectorCase('spl-close-after-rise');
        const { s, get } = await vectorSnapshot(c);
        const pos = decodePosition(get(c.position as Address));
        if (!pos) throw new Error('position');
        const preview = previewCloseFor(testMath(), s, pos, 100);
        if (!preview) throw new Error('preview');
        const signer = createNoopSigner(c.borrower as Address);
        const plan = await planCloseShort({
            snapshot: s,
            signer,
            symbol: 'MOCK',
            position: c.position as Address,
            q: pos.q,
            maxBuybackCost: preview.maxBuybackCost,
            extras: NO_EXTRAS,
        });
        const ix = plan.instructions[0];
        if (!ix?.accounts || !ix.data) throw new Error('instruction');
        const data = getCloseShortInstructionDataDecoder().decode(ix.data);
        expect(data.maxBuybackCost).toBe(preview.maxBuybackCost);
        // `⌈B̂·1.01⌉` admits the program's executed buyback.
        expect(data.maxBuybackCost >= BigInt(c.result.event?.buybackCost ?? 0)).toBe(true);
        expect(ix.accounts[3]?.address).toBe(c.position);
        expect(ix.accounts.slice(13).map((a) => a.address)).toHaveLength(8);
        expect(() => assertAllowedInstructions(plan.instructions)).not.toThrow();
        expect(plan.disclosures).toEqual([]);
        expect(sizeOf(plan.instructions, signer.address, 'legacy')).toBeLessThanOrEqual(MAX_TRANSACTION_BYTES);
    });

    it('add margin: the amount, the market read-only, no venue accounts', async () => {
        const c = vectorCase('spl-add-margin');
        const { s } = await vectorSnapshot(c);
        const signer = createNoopSigner(c.borrower as Address);
        const plan = await planAddMargin({
            signer,
            market: s.marketAddress,
            mint: s.mint,
            position: c.position as Address,
            amount: 1_500_000_000n,
            decimals: s.decimals,
            symbol: 'MOCK',
        });
        const ix = plan.instructions[0];
        if (!ix?.accounts || !ix.data) throw new Error('instruction');
        expect(getAddMarginInstructionDataDecoder().decode(ix.data).amount).toBe(1_500_000_000n);
        expect(ix.accounts.map((a) => a.address).slice(0, 3)).toEqual([signer.address, s.marketAddress, c.position]);
        expect(ix.accounts[1]?.role).toBe(AccountRole.READONLY);
        expect(plan.watch.map((w) => w.label)).toEqual(['Your SOL', 'Position']);
    });
});

describe('the plans build exactly the instructions the program executed', () => {
    const U64_MAX = (1n << 64n) - 1n;
    const flat = (ix: Instruction) => ({
        program: ix.programAddress,
        accounts: (ix.accounts ?? []).map((a) => ({ key: a.address, signer: a.role >= 2, writable: a.role % 2 === 1 })),
        data: Buffer.from(ix.data ?? new Uint8Array()).toString('hex'),
    });

    for (const c of programVectors()) {
        it(`${c.name} (${c.kind})`, async () => {
            const { s } = await vectorSnapshot(c);
            const signer = createNoopSigner(c.borrower as Address);
            const tp = s.config.tiers[2];
            if (!tp) throw new Error('tier');
            let plan: { readonly instructions: readonly Instruction[] };
            if (c.kind === 'open') {
                plan = await planOpenShort({
                    snapshot: s,
                    signer,
                    symbol: 'MOCK',
                    q: BigInt(c.q ?? 0),
                    minProceeds: 0n,
                    maxMargin: U64_MAX,
                    maxOpenFee: U64_MAX,
                    tier: tp,
                    extras: NO_EXTRAS,
                    nonce: BigInt(c.nonce),
                });
            } else if (c.kind === 'close') {
                plan = await planCloseShort({
                    snapshot: s,
                    signer,
                    symbol: 'MOCK',
                    position: c.position as Address,
                    q: 0n,
                    maxBuybackCost: U64_MAX,
                    extras: NO_EXTRAS,
                });
            } else {
                plan = await planAddMargin({
                    signer,
                    market: s.marketAddress,
                    mint: s.mint,
                    position: c.position as Address,
                    amount: BigInt(c.amount ?? 0),
                    decimals: s.decimals,
                    symbol: 'MOCK',
                });
            }
            expect(plan.instructions).toHaveLength(1);
            const ix = plan.instructions[0];
            if (!ix) throw new Error('instruction');
            expect(flat(ix)).toEqual((c as unknown as { ix: unknown }).ix);
        });
    }
});

describe('the mock listing (TEST-ONLY)', () => {
    it('names the mint’s mock pool and its vaults after the detection set', async () => {
        const c = vectorCase('spl-open-0');
        const { s, get } = await vectorSnapshot(c);
        const pool = await mockPool(s.mint);
        expect(pool).toBe(s.readKeys[0]);
        const l = mockListingFrom('11111111111111111111111111111111' as Address, s.market.canonicalPool, get(pool));
        expect(l?.remainingAccounts.map((a) => a.address).slice(2)).toEqual([...s.readKeys]);
        expect(l?.remainingAccounts.every((a) => a.role === AccountRole.READONLY)).toBe(true);
    });
});
