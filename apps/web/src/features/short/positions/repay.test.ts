import { getRepayShortInstructionDataDecoder } from '@shortwin/client';
import { AccountRole, type Address, createNoopSigner, getAddressEncoder } from '@solana/kit';
import { findAssociatedTokenPda } from '@solana-program/token-2022';
import { describe, expect, it, vi } from 'vitest';
import { renderDisclosure } from '../../../config/disclosures.ts';
import { programError } from '../../../config/errors.ts';
import { SYSTEM_PROGRAM, TOKEN_PROGRAM } from '../../../config/programs.ts';
import type { AppRpc } from '../../../rpc/client.ts';
import { parseMint } from '../../../rpc/mint.ts';
import { assertAllowedInstructions } from '../../../tx/integrity.ts';
import type { SimulationReport } from '../../../tx/types.ts';
import { CLOCK_SYSVAR } from '../../../venues/fetch.ts';
import { positionMetrics } from '../previews.ts';
import { decodePosition } from '../snapshot.ts';
import { testMath, vectorCase, vectorSnapshot } from '../testing.ts';
import {
    planRepay,
    previewRepay,
    type RepayState,
    readRepayState,
    repaymentReason,
    repaymentSimulationHold,
    repaySource,
} from './repay.ts';

async function fixture(name = 'spl-close-after-fall') {
    const c = vectorCase(name);
    const { s, get } = await vectorSnapshot(c);
    const position = decodePosition(get(c.position as Address));
    if (!position) throw new Error('position');
    const mint = get(s.mint);
    if (!mint) throw new Error('mint');
    const mintInfo = parseMint(s.mint, mint.owner, mint.data);
    if (!mintInfo) throw new Error('mintInfo');
    const [source] = await findAssociatedTokenPda({
        owner: position.borrower,
        mint: s.mint,
        tokenProgram: s.tokenProgram,
    });
    const state: RepayState = {
        positionAddress: c.position as Address,
        position,
        marketAddress: s.marketAddress,
        market: s.market,
        mintInfo,
        sources: [
            {
                address: source,
                programAddress: s.tokenProgram,
                mint: s.mint,
                owner: position.borrower,
                amount: position.q + 1n,
                cpiGuard: false,
                frozen: false,
            },
        ],
        walletLamports: 2_000_000_000n,
        now: s.now,
        slot: s.slot,
        readAt: Date.now(),
    };
    return { c, s, get, state };
}

function accountBytes(mint: Address, owner: Address, amount: bigint) {
    const bytes = new Uint8Array(165);
    const encode = getAddressEncoder();
    bytes.set(encode.encode(mint), 0);
    bytes.set(encode.encode(owner), 32);
    new DataView(bytes.buffer).setBigUint64(64, amount, true);
    bytes[108] = 1;
    return bytes;
}

function report(source: Address, pool: Address, q: bigint): SimulationReport {
    return {
        ok: true,
        unitsConsumed: 20_000n,
        computeUnitLimit: 22_000,
        priorityFeeMicroLamports: 0n,
        feeLamports: 5_000n,
        sizeBytes: 500,
        logs: [],
        error: null,
        simulatedAt: Date.now(),
        deltas: [
            { key: `${source}:amount`, label: 'source', before: q + 1n, after: 1n, unit: 'TOK', scale: 6 },
            { key: `${pool}:amount`, label: 'pool', before: 10n, after: 10n + q, unit: 'TOK', scale: 6 },
        ],
    };
}

describe('whole-debt token repayment', () => {
    it('reuses the program rent basis without any live venue/quote input', async () => {
        const { c, s, state } = await fixture();
        const preview = previewRepay(testMath(), state, s.now);
        expect(preview.rentDue).toBe(positionMetrics(testMath(), s, state.position).rentDue);
        expect(preview.rentDue).toBe(BigInt(c.result.event?.rent ?? 0));
        expect(preview.estimatedVaultReturn).toBe(state.position.v - preview.rentDue);
        const thin = { ...state, position: { ...state.position, v: preview.rentDue - 1n } };
        expect(previewRepay(testMath(), thin, s.now)).toMatchObject({ vaultCanPay: false, estimatedVaultReturn: null });
    });

    it.each(['spl-close-after-fall', 't22-close'])(
        'builds only SDK RepayShort with exact rent bound and actual token program (%s)',
        async (name) => {
            const { state } = await fixture(name);
            const signer = createNoopSigner(state.position.borrower);
            const preview = previewRepay(testMath(), state, state.now);
            const maxRent = preview.rentDue + 123n;
            const source = state.sources[0];
            if (!source) throw new Error('source');
            const plan = await planRepay(testMath(), state, { signer, source: source.address, maxRent, symbol: 'TOK' });
            expect(plan.kind).toBe('repay_short');
            expect(plan.instructions).toHaveLength(1);
            const ix = plan.instructions[0];
            if (!ix?.accounts || !ix.data) throw new Error('instruction');
            expect(getRepayShortInstructionDataDecoder().decode(ix.data).maxRent).toBe(maxRent);
            expect(ix.data).toHaveLength(16); // no partial-debt input: the program returns all position.q
            expect(ix.accounts).toHaveLength(12);
            expect(ix.accounts[0]).toMatchObject({ address: signer.address, role: AccountRole.WRITABLE_SIGNER });
            expect(ix.accounts[6]?.address).toBe(source.address);
            expect(ix.accounts[7]?.address).toBe(state.market.poolVault);
            expect(ix.accounts[9]?.address).toBe(state.market.tokenProgram);
            expect(() => assertAllowedInstructions(plan.instructions)).not.toThrow();
            expect(ix.accounts.some((meta) => meta.address === state.market.trader)).toBe(false);
            expect(plan.watch.filter((watch) => watch.kind === 'token').map((watch) => watch.decimals)).toEqual([
                state.mintInfo.decimals,
                state.mintInfo.decimals,
            ]);
            const disclosure = renderDisclosure('borrower.tokenRepay', plan.disclosureVars?.['borrower.tokenRepay']);
            expect(disclosure.join(' ')).toContain(`${maxRent} lamports`);
            expect(disclosure.join(' ')).toContain(`${state.position.q} raw units`);
            expect(disclosure.join(' ')).toContain('economic P&L is unavailable');
            expect(plan.disclosures).toContain('borrower.tokenRepay');
        },
    );

    it('rejects another wallet, an underfunded source and a different mint/program before review', async () => {
        const { state } = await fixture();
        const source = state.sources[0];
        if (!source) throw new Error('source');
        const preview = previewRepay(testMath(), state, state.now);
        const maxRent = preview.rentDue + 1n;
        expect(repaymentReason(state, SYSTEM_PROGRAM, source, maxRent, preview)).toMatch(/another wallet/);
        for (const changed of [
            { ...source, amount: state.position.q - 1n },
            { ...source, mint: SYSTEM_PROGRAM },
            { ...source, owner: SYSTEM_PROGRAM },
            { ...source, programAddress: SYSTEM_PROGRAM },
            { ...source, frozen: true },
            { ...source, cpiGuard: true },
        ]) {
            const bad = { ...state, sources: [changed] };
            await expect(
                planRepay(testMath(), bad, {
                    signer: createNoopSigner(state.position.borrower),
                    source: source.address,
                    maxRent,
                    symbol: 'TOK',
                }),
            ).rejects.toThrow();
        }
        await expect(
            planRepay(testMath(), state, {
                signer: createNoopSigner(SYSTEM_PROGRAM),
                source: source.address,
                maxRent,
                symbol: 'TOK',
            }),
        ).rejects.toThrow(/another wallet/);
    });

    it('enforces the explicit collateral-capped rent limit and does not replace it with u64MAX', async () => {
        const { state } = await fixture();
        const source = state.sources[0];
        if (!source) throw new Error('source');
        const due = previewRepay(testMath(), state, state.now).rentDue;
        const signer = createNoopSigner(state.position.borrower);
        for (const maxRent of [-1n, (1n << 64n) - 1n, state.position.v + 1n, due - 1n]) {
            await expect(
                planRepay(testMath(), state, { signer, source: source.address, maxRent, symbol: 'TOK' }),
            ).rejects.toThrow();
        }
        expect(
            repaymentReason(state, signer.address, source, null, previewRepay(testMath(), state, state.now)),
        ).toMatch(/explicit maximum/);
        const wrongDecimals = { ...state, mintInfo: { ...state.mintInfo, decimals: state.mintInfo.decimals + 1 } };
        await expect(
            planRepay(testMath(), wrongDecimals, { signer, source: source.address, maxRent: due, symbol: 'TOK' }),
        ).rejects.toThrow(/decimals/);
        const noGas = { ...state, walletLamports: 0n };
        await expect(
            planRepay(testMath(), noGas, { signer, source: source.address, maxRent: due, symbol: 'TOK' }),
        ).rejects.toThrow(/network fee/);
        expect(programError(6111)).toMatchObject({ name: 'RepaymentRentAboveMax', action: 'adjust' });
    });

    it('holds Sign unless simulation shows the exact source debit and pool principal credit', async () => {
        const { state } = await fixture();
        const source = state.sources[0];
        if (!source) throw new Error('source');
        const good = report(source.address, state.market.poolVault, state.position.q);
        expect(repaymentSimulationHold(good, source.address, state.market.poolVault, state.position.q)).toBeNull();
        const [inputDelta, outputDelta] = good.deltas;
        if (!inputDelta || !outputDelta) throw new Error('delta fixture');
        for (const deltas of [
            [],
            [inputDelta],
            [inputDelta, { ...outputDelta, after: state.position.q + 9n }],
            [{ ...inputDelta, after: 0n }, outputDelta],
            [{ ...inputDelta, before: null }, outputDelta],
        ]) {
            expect(
                repaymentSimulationHold({ ...good, deltas }, source.address, state.market.poolVault, state.position.q),
            ).toMatch(/full token repayment/);
        }
    });

    it('allows a small gas balance when the simulated base and priority fee fits, before refunds', async () => {
        const { state } = await fixture();
        const source = state.sources[0];
        if (!source) throw new Error('source');
        const small = { ...state, walletLamports: 5_000n };
        const due = previewRepay(testMath(), small, small.now).rentDue;
        const plan = await planRepay(testMath(), small, {
            signer: createNoopSigner(state.position.borrower),
            source: source.address,
            maxRent: due,
            symbol: 'TOK',
        });
        const good = report(source.address, state.market.poolVault, state.position.q);
        expect(plan.holdSign?.(good)).toBeNull();
        const mapped = plan.refineError?.({
            kind: 'program',
            code: 6066,
            name: 'InsufficientVault',
            action: 'blocked',
            message: 'buyback error',
        });
        expect(mapped).toMatchObject({ kind: 'program', code: 6066, action: 'blocked' });
        expect(mapped?.message).toContain('full accrued rent');
        expect(plan.holdSign?.({ ...good, feeLamports: 5_001n })).toMatch(/before account refunds/);
    });

    it('checks raw token owner/mint/program/frozen state and preserves >2^53 balances', async () => {
        const { state } = await fixture();
        const source = state.sources[0];
        if (!source) throw new Error('source');
        const amount = 9_007_199_254_740_993n;
        const bytes = accountBytes(state.market.mint, state.position.borrower, amount);
        const account = { address: source.address, owner: TOKEN_PROGRAM, data: bytes };
        expect(repaySource(account, state.position.borrower, state.market.mint, TOKEN_PROGRAM)?.amount).toBe(amount);
        expect(repaySource(account, SYSTEM_PROGRAM, state.market.mint, TOKEN_PROGRAM)).toBeNull();
        expect(repaySource(account, state.position.borrower, SYSTEM_PROGRAM, TOKEN_PROGRAM)).toBeNull();
        expect(
            repaySource(
                { ...account, owner: SYSTEM_PROGRAM },
                state.position.borrower,
                state.market.mint,
                TOKEN_PROGRAM,
            ),
        ).toBeNull();
        const frozen = bytes.slice();
        frozen[108] = 2;
        expect(
            repaySource({ ...account, data: frozen }, state.position.borrower, state.market.mint, TOKEN_PROGRAM)
                ?.frozen,
        ).toBe(true);
    });

    it('reads only position/market/mint/Clock and wallet funds; missing Clock is never priced from device time', async () => {
        const { state, get } = await fixture();
        const source = state.sources[0];
        if (!source) throw new Error('source');
        const clockBytes = new Uint8Array(40);
        new DataView(clockBytes.buffer).setBigInt64(32, state.now, true);
        const clock = {
            address: CLOCK_SYSVAR,
            owner: 'Sysvar1111111111111111111111111111111111111' as Address,
            lamports: 1n,
            executable: false,
            data: clockBytes,
        };
        const rpcAccount = (raw: ReturnType<typeof get>) =>
            raw
                ? {
                      owner: raw.owner,
                      lamports: raw.lamports,
                      executable: raw.executable,
                      rentEpoch: 0n,
                      space: BigInt(raw.data.length),
                      data: [Buffer.from(raw.data).toString('base64'), 'base64'],
                  }
                : null;
        let missingClock = false;
        const addresses: Address[] = [];
        const rpc = {
            getAccountInfo: vi.fn((address: Address) => ({
                send: async () => {
                    addresses.push(address);
                    return { context: { slot: state.slot }, value: rpcAccount(get(address)) };
                },
            })),
            getMultipleAccounts: vi.fn((keys: Address[]) => ({
                send: async () => {
                    addresses.push(...keys);
                    return {
                        context: { slot: state.slot },
                        value: keys.map((key) =>
                            rpcAccount(key === CLOCK_SYSVAR ? (missingClock ? null : clock) : get(key)),
                        ),
                    };
                },
            })),
            getTokenAccountsByOwner: vi.fn(() => ({
                send: async () => ({
                    value: [
                        {
                            pubkey: source.address,
                            account: {
                                owner: state.market.tokenProgram,
                                data: [
                                    Buffer.from(
                                        accountBytes(state.market.mint, state.position.borrower, state.position.q),
                                    ).toString('base64'),
                                    'base64',
                                ],
                            },
                        },
                    ],
                }),
            })),
            getBalance: vi.fn(() => ({ send: async () => ({ value: 2_000_000_000n }) })),
        } as unknown as AppRpc;
        const loaded = await readRepayState(rpc, state.positionAddress, state.position.borrower);
        expect(loaded.sources[0]?.amount).toBe(state.position.q);
        expect(loaded.now).toBe(state.now);
        expect(new Set(addresses)).toEqual(
            new Set([state.positionAddress, state.marketAddress, state.market.mint, CLOCK_SYSVAR]),
        );
        missingClock = true;
        await expect(readRepayState(rpc, state.positionAddress, state.position.borrower)).rejects.toThrow(
            /chain clock/,
        );
    });
});
