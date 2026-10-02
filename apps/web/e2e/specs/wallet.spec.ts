/**
 * The injected test wallet itself (harness self-test; `@scaffold`): it registers through Wallet
 * Standard, its in-browser Ed25519 signatures land on the E2E validator, and its test controls do
 * what the pipeline specs rely on — `rejectNext` rejects with 4001, `tamperNext` signs a changed
 * instruction, `raiseFeeNext` changes only the compute-unit price.
 */

import {
    type Address,
    appendTransactionMessageInstructions,
    compileTransaction,
    createNoopSigner,
    createTransactionMessage,
    decompileTransactionMessage,
    getBase64EncodedWireTransaction,
    getBase64Encoder,
    getCompiledTransactionMessageDecoder,
    getTransactionDecoder,
    pipe,
    setTransactionMessageComputeUnitLimit,
    setTransactionMessageComputeUnitPrice,
    setTransactionMessageFeePayerSigner,
    setTransactionMessageLifetimeUsingBlockhash,
} from '@solana/kit';
import { getTransferSolInstruction } from '@solana-program/system';
import { expect, test } from '../fixtures.ts';
import { getBalance, PROGRAMS, rpc } from '../validator/chain.ts';

test.use({ wallet: 'trader' });

/** An unsigned v0 transfer from the wallet (fee payer) with a compute-unit price, as base64 wire. */
async function unsignedTransfer(from: Address, to: Address, lamports: bigint): Promise<string> {
    const { value: blockhash } = await rpc().getLatestBlockhash({ commitment: 'confirmed' }).send();
    const payer = createNoopSigner(from);
    const msg = pipe(
        createTransactionMessage({ version: 0 }),
        (m) => setTransactionMessageFeePayerSigner(payer, m),
        (m) => setTransactionMessageLifetimeUsingBlockhash(blockhash, m),
        (m) => setTransactionMessageComputeUnitLimit(50_000, m),
        (m) => setTransactionMessageComputeUnitPrice(0n, m),
        (m) =>
            appendTransactionMessageInstructions(
                [getTransferSolInstruction({ source: payer, destination: to, amount: lamports })],
                m,
            ),
    );
    return getBase64EncodedWireTransaction(compileTransaction(msg));
}

function messageOf(wireBase64: string) {
    const tx = getTransactionDecoder().decode(getBase64Encoder().encode(wireBase64));
    return decompileTransactionMessage(getCompiledTransactionMessageDecoder().decode(tx.messageBytes));
}

test.describe('injected wallet', { tag: '@scaffold' }, () => {
    test('registers, connects and signs a transaction that lands', async ({ app, page, state }) => {
        await app.goto('/markets', 'markets');
        await app.connectWallet();
        const to = state.wallets.creator.address;
        const before = await getBalance(to);
        const wire = await unsignedTransfer(app.walletAddress, to, 12_345n);
        const signed = await page.evaluate((w) => window.__shortwinE2E?.signWireForTest(w), wire);
        expect(signed).toBeTruthy();
        const sig = await rpc()
            .sendTransaction(signed as never, { encoding: 'base64', preflightCommitment: 'confirmed' })
            .send();
        await expect.poll(async () => getBalance(to), { timeout: 30_000 }).toBe(before + 12_345n);
        const records = await app.walletSigned();
        expect(records.at(-1)?.signature).toBe(sig);
    });

    test('rejectNext rejects with 4001', async ({ app, page, state }) => {
        await app.goto('/markets', 'markets');
        await app.connectWallet();
        const wire = await unsignedTransfer(app.walletAddress, state.wallets.creator.address, 1n);
        await app.walletRejectNext();
        const r = await page.evaluate(async (w) => {
            try {
                await window.__shortwinE2E?.signWireForTest(w);
                return null;
            } catch (e) {
                return { message: (e as Error).message, code: (e as { code?: number }).code };
            }
        }, wire);
        expect(r).toEqual({ message: 'User rejected the request.', code: 4001 });
    });

    test('tamperNext changes an instruction; raiseFeeNext changes only the compute-unit price', async ({
        app,
        page,
        state,
    }) => {
        await app.goto('/markets', 'markets');
        await app.connectWallet();
        const wire = await unsignedTransfer(app.walletAddress, state.wallets.creator.address, 1_000n);
        const original = messageOf(wire);

        await app.walletTamperNext('data');
        const tampered = messageOf((await page.evaluate((w) => window.__shortwinE2E?.signWireForTest(w), wire)) ?? '');
        const lastData = (m: typeof original) => m.instructions.at(-1)?.data;
        expect(Buffer.from(lastData(tampered) ?? [])).not.toEqual(Buffer.from(lastData(original) ?? []));

        await app.walletRaiseFeeNext(77_777);
        const raised = messageOf((await page.evaluate((w) => window.__shortwinE2E?.signWireForTest(w), wire)) ?? '');
        const nonBudget = (m: typeof original) =>
            m.instructions
                .filter((ix) => ix.programAddress !== PROGRAMS.computeBudget)
                .map((ix) => [ix.programAddress, Buffer.from(ix.data ?? []).toString('hex')]);
        expect(nonBudget(raised)).toEqual(nonBudget(original));
        const price = raised.instructions.find(
            (ix) => ix.programAddress === PROGRAMS.computeBudget && ix.data?.[0] === 3,
        )?.data;
        expect(price && Buffer.from(price).readBigUInt64LE(1)).toBe(77_777n);
        const records = await app.walletSigned();
        expect(records.map((r) => [r.tampered, r.raisedFeeTo])).toEqual([
            [true, null],
            [false, '77777'],
        ]);
    });
});
