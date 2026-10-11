/**
 * Regressions of the M9 review — the borrower side, on the program-derived vectors of
 * `features/short/vectors/program-vectors.json`: positions are priced on the chain's market only
 * (M9-SEC-2); a symbol never names the token of a transaction without its mint (M9-SEC-5); the
 * risk badge says "Liquidatable" only when the program would liquidate (M9-UX-4); a position's own
 * locked terms fill its copy (M9-UX-3).
 */
import { type Address, createNoopSigner } from '@solana/kit';
import { render, screen } from '@testing-library/react';
import { beforeAll, describe, expect, it } from 'vitest';
import { createApiClient } from '../api/client.ts';
import type { Position } from '../api/types.ts';
import { riskLevel } from '../components/Badge.tsx';
import { renderDisclosure } from '../config/disclosures.ts';
import { proseDuration } from '../config/disclosureVars.ts';
import { planOpenShort, positionDisclosureVars } from '../features/short/plans.ts';
import { buildRows } from '../features/short/positions/data.ts';
import { exitShort, exitState, SettlementRisk } from '../features/short/positions/SettlementRisk.tsx';
import { positionMetrics, previewOpen } from '../features/short/previews.ts';
import { decodePosition } from '../features/short/snapshot.ts';
import { testMath, vectorCase, vectorSnapshot } from '../features/short/testing.ts';
import { cleanIndexerText, cleanSymbol, duplicateMints, tokenLabel } from '../lib/displayText.ts';
import type { MathModule } from '../math/load.ts';

let math: MathModule;
beforeAll(() => {
    math = testMath();
});

async function closeCase(name: string) {
    const c = vectorCase(name);
    const { s, get } = await vectorSnapshot(c);
    const pos = decodePosition(get(c.position as Address));
    if (!pos) throw new Error('position');
    return { c, s, pos };
}

function apiRow(position: string, market: string, mint: string, borrower: string): Position {
    return {
        position,
        market,
        mint,
        borrower,
        nonce: '0',
        status: 'open',
        tier: 'C',
        venue: 'mock',
        q: '0',
        s: '0',
        m: '0',
        margin_added: '0',
        v: '0',
        n_open: '0',
        fee_open: '0',
        entry_price: '0',
        deadline: 0,
        opened_ts: 0,
        open_signature: '1111111111111111111111111111111111111111111111111111111111111111',
        armed: false,
        exit: null,
    };
}

describe('M9-SEC-2: an open position is priced on the market its chain account names', () => {
    it('an indexer row naming another mint and market is flagged; the numbers stay the chain’s', async () => {
        const { c, s: t22, pos } = await closeCase('t22-close');
        const wrong = (await vectorSnapshot(vectorCase('spl-close-after-fall'))).s;
        const honest = positionMetrics(math, t22, pos);
        const lie = apiRow(c.position as string, wrong.marketAddress, wrong.mint, c.borrower);
        const rows = buildRows({
            math,
            usingChain: false,
            api: [lie],
            chainOpen: [],
            accounts: new Map([[c.position as Address, pos]]),
            // From the chain: the Market at `pos.market` names the Token-2022 mint.
            marketMints: new Map([[pos.market, t22.mint]]),
            snapshots: new Map([
                [wrong.mint, wrong],
                [t22.mint, t22],
            ]),
            status: 'all',
        });
        const row = rows[0];
        expect(row?.mismatch).toBe(true);
        expect(row?.mint).toBe(t22.mint);
        expect(row?.snapshot?.marketAddress).toBe(pos.market);
        expect(row?.metrics?.crBps).toBe(honest.crBps);
        expect(riskLevel(row?.metrics?.crBps ?? 0n, row?.metrics?.crEmaBps ?? null, pos.crLiqBps)).not.toBe('critical');
    });

    it('without the chain’s mint, or with a snapshot of another market, no metric is shown', async () => {
        const { c, s: t22, pos } = await closeCase('t22-close');
        const wrong = (await vectorSnapshot(vectorCase('spl-close-after-fall'))).s;
        const lie = apiRow(c.position as string, wrong.marketAddress, wrong.mint, c.borrower);
        const base = {
            math,
            usingChain: false,
            api: [lie],
            chainOpen: [],
            accounts: new Map([[c.position as Address, pos]]),
            status: 'all' as const,
        };
        // The chain's mint is not known yet: the indexer's mint never selects a snapshot.
        const unknown = buildRows({ ...base, marketMints: null, snapshots: new Map([[wrong.mint, wrong]]) });
        expect(unknown[0]?.metrics).toBeNull();
        expect(unknown[0]?.mismatch).toBe(true);
        // A snapshot filed under the chain's mint but of another market is refused too.
        const misfiled = buildRows({
            ...base,
            marketMints: new Map([[pos.market, t22.mint]]),
            snapshots: new Map([[t22.mint, wrong]]),
        });
        expect(misfiled[0]?.metrics).toBeNull();
        expect(misfiled[0]?.snapshot).toBeNull();
    });
});

describe('M9-SEC-5: untrusted symbols are normalised and never name a token without its mint', () => {
    it('the review title and labels of an open carry the short mint; bidi characters are gone', async () => {
        const c = vectorCase('spl-open-0');
        const { s } = await vectorSnapshot(c);
        const q = BigInt(c.q ?? '0');
        const p = previewOpen(math, s, { q, slippageBps: 100, walletQ: 0n });
        if (!p) throw new Error('preview');
        const spoof = 'USDC‮​';
        const plan = await planOpenShort({
            snapshot: s,
            signer: createNoopSigner(c.borrower as Address),
            symbol: spoof,
            q,
            minProceeds: p.minProceeds,
            maxMargin: p.maxMargin,
            maxOpenFee: p.maxOpenFee,
            tier: p.terms,
        });
        const short = `${s.mint.slice(0, 4)}…${s.mint.slice(-4)}`;
        expect(plan.title).toContain(short);
        expect(plan.title).not.toMatch(/[‪-‮​-‏]/);
        const labels = plan.watch.flatMap((w) =>
            w.kind === 'program' ? Object.values(w.fields).map((f) => f.label) : [],
        );
        expect(labels).toContain(`Borrowed (USDC · ${short})`);
        for (const l of labels.filter((x) => x.includes('USDC'))) expect(l).toContain(short);
    });

    it('normalises display text once, at the indexer boundary (REST), and caps it', async () => {
        expect(cleanSymbol('USDC‮⁦​\u0007')).toBe('USDC');
        expect(cleanSymbol('ABCDEFGHIJKLMNOPQRSTU')).toHaveLength(16);
        expect(cleanSymbol('​‮')).toBeNull();
        expect(cleanSymbol('é')).toBe('é'); // NFC
        const nested = cleanIndexerText({ data: [{ symbol: 'BO‮NK', name: 'Bonk\u0000 Inu', mint: 'x' }] });
        expect(nested.data[0]).toEqual({ symbol: 'BONK', name: 'Bonk Inu', mint: 'x' });
        const api = createApiClient('http://indexer', {
            fetchImpl: (async () =>
                new Response(JSON.stringify({ mint: 'm', symbol: 'USDC‮', name: null }), {
                    status: 200,
                    headers: { 'content-type': 'application/json' },
                })) as typeof fetch,
        });
        const { data } = await api.get<{ symbol: string }>('/v1/markets/m');
        expect(data.symbol).toBe('USDC');
        expect(tokenLabel('USDC', 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v')).toBe('USDC · EPjF…Dt1v');
        expect(tokenLabel(null, 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v')).toBe('EPjF…Dt1v');
        expect([
            ...duplicateMints([
                { mint: 'a', symbol: 'BONK' },
                { mint: 'b', symbol: 'bonk' },
                { mint: 'c', symbol: 'WIF' },
            ]),
        ]).toEqual(['a', 'b']);
    });
});

describe('M9-UX-4: "Liquidatable" only when the program would liquidate (both legs, strictly below)', () => {
    it('agrees with the exit state at the line and on the spot leg alone', async () => {
        const { s, pos } = await closeCase('spl-close-after-fall');
        const mx = positionMetrics(math, s, pos);
        const liq = BigInt(pos.crLiqBps);
        // Exactly at the line on both legs: not liquidatable (strict <), and the badge agrees.
        expect(riskLevel(liq, liq, pos.crLiqBps)).not.toBe('critical');
        expect(exitState(pos, { ...mx, crBps: liq, crEmaBps: liq }, s.venue, s.now)).not.toBe('liquidatable');
        // Spot leg below, smoothed leg healthy: "At risk (live price below)", exit state spot-below.
        expect(riskLevel(liq - 1n, liq * 2n, pos.crLiqBps)).toBe('below');
        expect(exitState(pos, { ...mx, crBps: liq - 1n, crEmaBps: liq * 2n }, s.venue, s.now)).toBe('spot-below');
        // Both below: Liquidatable on both.
        expect(riskLevel(liq - 1n, liq - 1n, pos.crLiqBps)).toBe('critical');
        expect(exitState(pos, { ...mx, crBps: liq - 1n, crEmaBps: liq - 1n }, s.venue, s.now)).toBe('liquidatable');
    });
});

describe('M9-UX-3: a position’s copy uses the terms it locked at open', () => {
    it('a position that locked a 5% expiry fee is told 5%, and its own S3 wait and duration', async () => {
        const { s, pos } = await closeCase('spl-close-after-fall');
        const locked = { ...pos, expiryFeeBps: 500, s3WaitS: 1_200 };
        const mx = positionMetrics(math, s, locked);
        render(
            <SettlementRisk
                pos={locked}
                metrics={mx}
                venue={s.venue}
                market={s.market}
                now={locked.deadline}
                tier={s.config.tiers[locked.tier]}
            />,
        );
        const status = screen.getByRole('status').textContent ?? '';
        expect(status).toContain('5% expiry fee');
        expect(status).not.toContain('2% expiry fee');
        const vars = positionDisclosureVars(locked, s.config.tiers[locked.tier]);
        expect(vars.expiryFee).toBe('5%');
        expect(vars.s3Wait).toBe('20 minutes');
        // The position's own duration (deadline − t_open), not the tier's current max duration.
        expect(vars.maxDuration).toBe(proseDuration(Number(locked.deadline - locked.tOpen)));
        expect(
            positionDisclosureVars({ ...locked, deadline: locked.tOpen + 3n * 86_400n }, undefined).maxDuration,
        ).toBe('3 days');
        const text = renderDisclosure('borrower.liquidation', vars).join(' ');
        expect(text).toContain('Unless it can be liquidated at that point, you pay a 5% expiry fee');
        expect(text).toContain('no route to buy back for 20 minutes');
        expect(exitShort('no-route', locked)).toContain('20 minutes');
    });
});
