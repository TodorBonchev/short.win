/**
 * OPTIONAL live check (skipped unless `SHORTWIN_LIVE_RPC` is set): the Short flows end to end on a
 * local `solana-test-validator` that runs the TEST-ONLY test-venue build and the mock venue with a
 * market loaded from `vectors/program-vectors.json` (case `spl-open-0`; see `vectors/live-validator.sh`).
 * Through the real RPC: the snapshot (two `getMultipleAccounts` and the `Clock`), the open preview,
 * the plan, the scaffold's `simulatePlan` and `buildMessage`, a key-pair signer, `sendTransaction`
 * (confirmation polled: jsdom's `AbortSignal` cannot drive Kit's WebSocket subscriptions), the
 * position as the program wrote it against the preview, then add margin and the close against its
 * preview.
 */
import { fetchMaybeShortPosition } from '@shortwin/client';
import {
    type Address,
    type Blockhash,
    generateKeyPairSigner,
    getBase64EncodedWireTransaction,
    getSignatureFromTransaction,
    lamports,
    type Signature,
    signTransactionMessageWithSigners,
} from '@solana/kit';
import { describe, expect, it } from 'vitest';
import { config } from '../../config/env.ts';
import { createAppClient } from '../../rpc/client.ts';
import { buildMessage } from '../../tx/message.ts';
import { simulatePlan } from '../../tx/simulate.ts';
import type { TxPlan } from '../../tx/types.ts';
import { NO_EXTRAS } from '../../venues/lists.ts';
import { planAddMargin, planCloseShort, planOpenShort } from './plans.ts';
import { assignTier, openBlocked, positionMetrics, previewCloseFor, previewOpen } from './previews.ts';
import { loadSnapshot } from './snapshot.ts';
import { testMath, vectorCase } from './testing.ts';

const RPC = process.env.SHORTWIN_LIVE_RPC;

describe.skipIf(!RPC)('Short flows on a live local validator', () => {
    it('opens, adds margin and closes exactly as previewed', { timeout: 120_000 }, async () => {
        const rpcUrl = RPC ?? '';
        const client = createAppClient({
            ...config,
            rpcUrl,
            rpcWsUrl: rpcUrl.replace('http', 'ws').replace(/:(\d+)$/, (_, p) => `:${Number(p) + 1}`),
        });
        const math = testMath();
        const mint = vectorCase('spl-open-0').mint as Address;
        const signer = await generateKeyPairSigner();
        const confirmed = async (sig: Signature) => {
            for (let i = 0; i < 120; i++) {
                const { value } = await client.rpc.getSignatureStatuses([sig]).send();
                const st = value[0];
                if (st?.err)
                    throw new Error(
                        `transaction failed: ${JSON.stringify(st.err, (_, v) => (typeof v === 'bigint' ? v.toString() : v))}`,
                    );
                if (st?.confirmationStatus === 'confirmed' || st?.confirmationStatus === 'finalized') return;
                await new Promise((r) => setTimeout(r, 250));
            }
            throw new Error('not confirmed');
        };
        await confirmed(await client.rpc.requestAirdrop(signer.address, lamports(50_000_000_000n)).send());

        const run = async (plan: TxPlan) => {
            const { report } = await simulatePlan({
                rpc: client.rpc,
                plan,
                feePayer: signer,
                lookupTables: {},
                version: 0,
                priorityFeeMicroLamports: 0n,
            });
            expect(report.error).toBeNull();
            expect(report.ok).toBe(true);
            const { value: lifetime } = await client.rpc.getLatestBlockhash({ commitment: 'confirmed' }).send();
            const message = buildMessage({
                instructions: plan.instructions,
                feePayer: signer,
                lifetime: lifetime as { blockhash: Blockhash; lastValidBlockHeight: bigint },
                computeUnitLimit: report.computeUnitLimit,
                computeUnitPrice: 0n,
                lookupTables: {},
                version: 0,
            });
            const signed = await signTransactionMessageWithSigners(message);
            await client.rpc
                .sendTransaction(getBase64EncodedWireTransaction(signed), {
                    encoding: 'base64',
                    preflightCommitment: 'confirmed',
                })
                .send();
            await confirmed(getSignatureFromTransaction(signed));
            return report;
        };

        // ---- open ----
        let { snapshot: s } = await loadSnapshot(client.rpc, math, mint, [], undefined, { allowTestVenue: true });
        if (!s) throw new Error('no market on the validator');
        expect(s.kind).toBe('mock');
        expect(openBlocked(math, s)).toBeNull();
        const q = 100_000_000_000n;
        const preview = previewOpen(math, s, { q, slippageBps: 100, walletQ: 0n });
        if (!preview) throw new Error('preview');
        expect(preview.refusal).toBeNull();
        const tier = assignTier(math, s);
        const tp = tier === null ? undefined : s.config.tiers[tier];
        if (!tp) throw new Error('tier');
        const open = await planOpenShort({
            snapshot: s,
            signer,
            symbol: 'MOCK',
            q,
            minProceeds: preview.minProceeds,
            maxMargin: preview.maxMargin,
            maxOpenFee: preview.maxOpenFee,
            tier: tp,
            extras: NO_EXTRAS,
        });
        const report = await run(open);
        const d = (f: string) => report.deltas.find((x) => x.key === `${open.position}:${f}`)?.after;
        expect(d('q')).toBe(q);
        expect(d('s')).toBe(preview.proceeds);
        let pos = await fetchMaybeShortPosition(client.rpc, open.position, { commitment: 'confirmed' });
        if (!pos.exists) throw new Error('position not opened');
        // The sale is quoted on the live reserves: exact. The margin's EMA leg reads the newest ring
        // entry after the open's own prologue roll, which no keeper cranked here since the preview's
        // read (production cranks every second): within the plan's +2 % bounds, which the program
        // enforced (`MarginAboveMax`, `OpenFeeAboveMax`).
        expect(pos.data.s).toBe(preview.proceeds);
        expect(pos.data.m <= preview.maxMargin).toBe(true);
        expect(pos.data.feeOpen <= preview.maxOpenFee).toBe(true);
        expect(pos.data.m * 1_000n >= preview.margin * 990n).toBe(true);

        // ---- add margin ----
        const add = await planAddMargin({
            signer,
            market: s.marketAddress,
            mint,
            position: open.position,
            amount: 250_000_000n,
            decimals: s.decimals,
            symbol: 'MOCK',
        });
        await run(add);
        pos = await fetchMaybeShortPosition(client.rpc, open.position, { commitment: 'confirmed' });
        if (!pos.exists) throw new Error('position');
        expect(pos.data.v).toBe(pos.data.s + pos.data.m);
        expect(pos.data.m >= 250_000_000n).toBe(true);

        // ---- close ----
        s = (await loadSnapshot(client.rpc, math, mint, [], undefined, { allowTestVenue: true })).snapshot;
        if (!s) throw new Error('snapshot');
        const close = previewCloseFor(math, s, pos.data, 100);
        if (!close) throw new Error('close preview');
        const mx = positionMetrics(math, s, pos.data);
        expect(mx.payout).toBe(close.payout);
        const plan = await planCloseShort({
            snapshot: s,
            signer,
            symbol: 'MOCK',
            position: open.position,
            q,
            maxBuybackCost: close.maxBuybackCost,
            extras: NO_EXTRAS,
        });
        const closed = await run(plan);
        const sol = closed.deltas.find((x) => x.key === `${signer.address}:lamports`);
        // The wallet receives the payout and the position account's rent, less the network fee.
        expect(
            sol && sol.after !== null && sol.before !== null && sol.after - sol.before > close.payout - 1_000_000n,
        ).toBe(true);
        const after = await fetchMaybeShortPosition(client.rpc, open.position, { commitment: 'confirmed' });
        expect(after.exists).toBe(false);
    });
});
