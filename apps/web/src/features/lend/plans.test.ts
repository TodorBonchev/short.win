/**
 * The lender plans (FRONTEND.md §10.1, LEND): the CPI Guard and memo branches, the bracket's
 * instruction order, `min_shares` / `min_tokens` / `min_sol` in the instruction data, the
 * `ShareEpoch` remaining accounts, the sync and sweep prepends, the watched accounts and the
 * disclosures each plan asks to acknowledge.
 */
import {
    findConfigPda,
    findShareEpochPda,
    getClaimSolInstructionDataDecoder,
    getCoverDeficitInstructionDataDecoder,
    getDepositInstructionDataDecoder,
    getLenderPositionEncoder,
    getSyncLenderInstructionDataDecoder,
    getWithdrawInstructionDataDecoder,
    identifyShortwinInstruction,
    ShortwinInstruction,
} from '@shortwin/client';
import { AccountRole, address, createNoopSigner, type Instruction } from '@solana/kit';
import { describe, expect, it } from 'vitest';
import {
    ASSOCIATED_TOKEN_PROGRAM,
    MEMO_PROGRAM,
    SHORTWIN_PROGRAM,
    TOKEN_2022_PROGRAM,
    TOKEN_PROGRAM,
} from '../../config/programs.ts';
import { eventAuthority } from '../../rpc/pda.ts';
import { assertAllowedInstructions } from '../../tx/integrity.ts';
import type { WatchedAccount } from '../../tx/types.ts';
import { MAX_RECORDS } from './chain.ts';
import {
    buildClaimPlan,
    buildCoverPlan,
    buildDepositPlan,
    buildSyncPlan,
    buildWithdrawPlan,
    type LendContext,
    lendInvalidations,
    MAX_SYNCS_PER_CLAIM,
    PlanError,
    recordMetas,
    solPaidFrom,
} from './plans.ts';

const OWNER = address('9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin');
const MINT = address('EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v');
const MARKET = address('7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE');
const VAULT = address('Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB');
const LENDER = address('mSoLzYCxHdYgdzU16g5QSh3i5K3z3KZK7ytfqcJm7So');
const SOURCE = address('4k3Dyjzvzp8eMZWUXbBCjEvwSkkk59S5iCNLY3QrkX6R');
const OTHER = address('JUPyiwrYJFskUPiHa7hkeR8VUtAeFoSYbKedZNsDvCN');
const OTHER_OWNER = address('So11111111111111111111111111111111111111112');
const signer = createNoopSigner(OWNER);

const ctx = (o: Partial<LendContext> = {}): LendContext => ({
    signer,
    mint: MINT,
    marketAddress: MARKET,
    vaultAddress: VAULT,
    tokenProgram: TOKEN_2022_PROGRAM,
    decimals: 6,
    symbol: 'TKN',
    lenderAddress: LENDER,
    lenderEpoch: 0,
    shareEpoch: 0,
    ...o,
});

const kinds = (ixs: readonly Instruction[]) =>
    ixs.map((ix) =>
        ix.programAddress === SHORTWIN_PROGRAM
            ? ShortwinInstruction[identifyShortwinInstruction(ix.data as Uint8Array)]
            : ix.programAddress,
    );
const data = (ix: Instruction | undefined) => (ix?.data ?? new Uint8Array()) as Uint8Array;
const tail = (ix: Instruction | undefined, n: number) => (ix?.accounts ?? []).slice(-n);

describe('deposit plans', () => {
    it('build a plain deposit with the bounds, the market’s token program and the watched accounts', async () => {
        const plan = await buildDepositPlan({
            ...ctx(),
            source: SOURCE,
            sourceCpiGuard: false,
            amount: 1_000_000n,
            minShares: 995_000_000n,
            bracket: false,
            sweep: [],
        });
        expect(kinds(plan.instructions)).toEqual(['Deposit']);
        const ix = plan.instructions[0];
        expect(getDepositInstructionDataDecoder().decode(data(ix))).toMatchObject({
            amount: 1_000_000n,
            minShares: 995_000_000n,
        });
        const accounts = ix?.accounts ?? [];
        expect(accounts[0]).toMatchObject({ address: OWNER, role: AccountRole.WRITABLE_SIGNER });
        expect(accounts[1]?.address).toBe((await findConfigPda())[0]);
        expect(accounts.map((a) => a.address)).toEqual(
            expect.arrayContaining([MARKET, VAULT, LENDER, MINT, SOURCE, TOKEN_2022_PROGRAM, await eventAuthority()]),
        );
        expect(accounts).toHaveLength(12);
        expect(plan.kind).toBe('deposit');
        expect(plan.disclosures).toEqual(['lender.solRepayment']);
        expect(plan.watch.map((w: WatchedAccount) => [w.kind, w.address])).toEqual([
            ['lamports', OWNER],
            ['token', SOURCE],
            ['program', LENDER],
        ]);
        expect(plan.invalidate).toEqual(lendInvalidations(MINT, OWNER));
        expect(() => assertAllowedInstructions(plan.instructions)).not.toThrow();
    });

    it('refuses a plain deposit from a CPI-Guarded account', async () => {
        await expect(
            buildDepositPlan({
                ...ctx(),
                source: SOURCE,
                sourceCpiGuard: true,
                amount: 1n,
                minShares: 0n,
                bracket: false,
                sweep: [],
            }),
        ).rejects.toBeInstanceOf(PlanError);
    });

    it('brackets the deposit with DisableCpiGuard / EnableCpiGuard, all signed by the owner (FE-1)', async () => {
        const plan = await buildDepositPlan({
            ...ctx(),
            source: SOURCE,
            sourceCpiGuard: true,
            amount: 5n,
            minShares: 1n,
            bracket: true,
            sweep: [],
        });
        const [off, dep, on] = plan.instructions;
        expect(plan.instructions).toHaveLength(3);
        expect(off?.programAddress).toBe(TOKEN_2022_PROGRAM);
        expect(on?.programAddress).toBe(TOKEN_2022_PROGRAM);
        // Token-2022 CpiGuardExtension (34): Disable = 1, Enable = 0.
        expect([...data(off)]).toEqual([34, 1]);
        expect([...data(on)]).toEqual([34, 0]);
        for (const ix of [off, on]) {
            expect(ix?.accounts?.[0]?.address).toBe(SOURCE);
            expect(ix?.accounts?.[1]).toMatchObject({ address: OWNER, role: AccountRole.READONLY_SIGNER });
        }
        expect(kinds([dep as Instruction])).toEqual(['Deposit']);
        expect(plan.title).toContain('CPI Guard off → deposit → CPI Guard on');
        expect(plan.disclosures).toEqual(['lender.solRepayment', 'token.cpiGuard']);
        expect(() => assertAllowedInstructions(plan.instructions)).not.toThrow();
    });

    it('ignores the bracket for an unguarded account and prepends the sweep bundle first', async () => {
        const sweep: Instruction = { programAddress: SHORTWIN_PROGRAM, accounts: [], data: new Uint8Array([1]) };
        const plan = await buildDepositPlan({
            ...ctx(),
            source: SOURCE,
            sourceCpiGuard: false,
            amount: 5n,
            minShares: 1n,
            bracket: true,
            sweep: [sweep],
        });
        expect(plan.instructions[0]).toBe(sweep);
        expect(kinds(plan.instructions.slice(1))).toEqual(['Deposit']);
        expect(plan.title).toContain('post-wipe sweep');
        expect(plan.disclosures).toEqual(['lender.solRepayment']);
    });

    it('passes the ShareEpoch records a lender is behind on, in order and writable (KI-65)', async () => {
        const plan = await buildDepositPlan({
            ...ctx({ lenderEpoch: 2, shareEpoch: 5 }),
            source: SOURCE,
            sourceCpiGuard: false,
            amount: 5n,
            minShares: 1n,
            bracket: false,
            sweep: [],
        });
        const expected = await Promise.all(
            [2, 3, 4].map(async (epoch) => (await findShareEpochPda({ market: MARKET, epoch }))[0]),
        );
        expect(tail(plan.instructions[0], 3)).toEqual(
            expected.map((a) => ({ address: a, role: AccountRole.WRITABLE })),
        );
        expect(await recordMetas(MARKET, null, 5)).toEqual([]);
        expect(await recordMetas(MARKET, 5, 5)).toEqual([]);
        expect(await recordMetas(MARKET, 0, 40)).toHaveLength(MAX_RECORDS);
    });
});

describe('withdraw plans', () => {
    const base = {
        ...ctx(),
        destination: SOURCE,
        createDestination: false,
        memoRequired: false,
        shares: 1_000n,
        minTokens: 99n,
        minSol: 12_345n,
        vestingForfeit: 0n,
        vesting: 0n,
    };

    it('encode shares, min_tokens and min_sol; spl-memo for a plain Token-2022 destination, unannounced', async () => {
        const plan = await buildWithdrawPlan(base);
        expect(kinds(plan.instructions)).toEqual(['Withdraw']);
        const ix = plan.instructions[0];
        expect(getWithdrawInstructionDataDecoder().decode(data(ix))).toMatchObject({
            shares: 1_000n,
            minTokens: 99n,
            minSol: 12_345n,
        });
        // [owner, config, market, vault, lender, mint, destination, pool vault, token program, system, memo, event authority, program]
        // TMEMO-9: spl-memo for every Token-2022 destination (CPI'd only if it requires memos then).
        expect(ix?.accounts?.[10]?.address).toBe(MEMO_PROGRAM);
        expect(ix?.accounts?.[6]).toMatchObject({ address: SOURCE, role: AccountRole.WRITABLE });
        expect(ix?.accounts?.[8]?.address).toBe(TOKEN_2022_PROGRAM);
        expect(plan.disclosures).toEqual([]);
        expect(plan.title).not.toContain('with memo');
        expect(plan.watch.map((w) => w.address)).toEqual([OWNER, SOURCE, LENDER, VAULT]);
    });

    it('pass spl-memo for a memo-required destination and say so', async () => {
        const plan = await buildWithdrawPlan({ ...base, memoRequired: true });
        expect(plan.instructions[0]?.accounts?.[10]?.address).toBe(MEMO_PROGRAM);
        expect(plan.disclosures).toEqual(['token.memoRequired']);
        expect(plan.title).toContain('with memo');
    });

    it('pass the "none" memo program for an SPL Token destination', async () => {
        const plan = await buildWithdrawPlan({ ...base, tokenProgram: TOKEN_PROGRAM });
        expect(plan.instructions[0]?.accounts?.[10]?.address).toBe(SHORTWIN_PROGRAM);
        expect(plan.disclosures).toEqual([]);
    });

    it('create a missing ATA destination idempotently under the market’s token program first', async () => {
        const plan = await buildWithdrawPlan({ ...base, createDestination: true, tokenProgram: TOKEN_PROGRAM });
        const [ata, wd] = plan.instructions;
        expect(ata?.programAddress).toBe(ASSOCIATED_TOKEN_PROGRAM);
        expect([...data(ata)]).toEqual([1]);
        expect(ata?.accounts?.[1]?.address).toBe(SOURCE);
        expect(ata?.accounts?.[5]?.address).toBe(TOKEN_PROGRAM);
        expect(kinds([wd as Instruction])).toEqual(['Withdraw']);
        expect(() => assertAllowedInstructions(plan.instructions)).not.toThrow();
    });

    it('ask for the vest-forfeit acknowledgement with the vesting amount', async () => {
        const plan = await buildWithdrawPlan({ ...base, vestingForfeit: 1_000n, vesting: 250_000_000n });
        expect(plan.disclosures).toEqual(['lender.exitForfeitsVest']);
        expect(plan.disclosureVars?.['lender.exitForfeitsVest']).toEqual({ vesting: '0.25' });
    });

    it('add lender.subUnitConversion after the vest-forfeit acknowledgement (KI-65)', async () => {
        const subUnit = { vestEnd: '2026-10-08 06:00 UTC' };
        const plan = await buildWithdrawPlan({ ...base, subUnit, vestingForfeit: 1_000n, vesting: 250_000_000n });
        expect(plan.disclosures).toEqual(['lender.exitForfeitsVest', 'lender.subUnitConversion']);
        expect(plan.disclosureVars).toEqual({
            'lender.exitForfeitsVest': { vesting: '0.25' },
            'lender.subUnitConversion': subUnit,
        });
    });

    it('refuse zero shares', async () => {
        await expect(buildWithdrawPlan({ ...base, shares: 0n })).rejects.toBeInstanceOf(PlanError);
    });

    it('read the SOL paid from the vault’s simulated lamports', () => {
        const key = `${VAULT}:lamports`;
        expect(solPaidFrom([{ key, before: 1_000n, after: 400n }], VAULT)).toBe(600n);
        expect(solPaidFrom([{ key, before: 1_000n, after: 1_000n }], VAULT)).toBe(0n);
        expect(solPaidFrom([{ key: 'x', before: 1n, after: 0n }], VAULT)).toBeNull();
    });
});

describe('claim, cover and sync plans', () => {
    it('prepend sync_lender(0) of other lenders (at most 4) before claim_sol', async () => {
        const syncs = Array.from({ length: 6 }, () => ({ lender: OTHER, owner: OTHER_OWNER }));
        const plan = await buildClaimPlan({ ...ctx(), syncs, expected: 5_000n });
        expect(kinds(plan.instructions)).toEqual([...Array(MAX_SYNCS_PER_CLAIM).fill('SyncLender'), 'ClaimSol']);
        const sync = plan.instructions[0];
        expect(getSyncLenderInstructionDataDecoder().decode(data(sync)).maxEpochs).toBe(0);
        expect(sync?.accounts?.map((a) => a.address)).toContain(OTHER);
        // The other lender's owner rides along unsigned: a plain touch reads no rebase record.
        expect(sync?.accounts?.[3]).toMatchObject({ address: OTHER_OWNER, role: AccountRole.READONLY });
        const claim = plan.instructions[MAX_SYNCS_PER_CLAIM];
        expect(getClaimSolInstructionDataDecoder().decode(data(claim))).toBeTruthy();
        expect(claim?.accounts?.[0]).toMatchObject({ address: OWNER, role: AccountRole.WRITABLE_SIGNER });
        expect(claim?.accounts?.[1]).toMatchObject({ address: MARKET, role: AccountRole.READONLY });
        expect(plan.title).toContain('4 lender syncs first');
        expect(() => assertAllowedInstructions(plan.instructions)).not.toThrow();
    });

    it('cover the exact deficit', async () => {
        const plan = await buildCoverPlan({ ...ctx(), lamports: 42n });
        expect(kinds(plan.instructions)).toEqual(['CoverDeficit']);
        expect(getCoverDeficitInstructionDataDecoder().decode(data(plan.instructions[0])).lamports).toBe(42n);
        await expect(buildCoverPlan({ ...ctx(), lamports: 0n })).rejects.toBeInstanceOf(PlanError);
    });

    it('sync the own position through the records one transaction holds', async () => {
        const plan = await buildSyncPlan(ctx({ lenderEpoch: 1, shareEpoch: 30 }));
        const ix = plan.instructions[0];
        expect(getSyncLenderInstructionDataDecoder().decode(data(ix)).maxEpochs).toBe(MAX_RECORDS);
        // Signed by the owner: it may convert through rebase records (KI65-REV-SEC1).
        expect(ix?.accounts?.[2]).toMatchObject({ address: LENDER, role: AccountRole.WRITABLE });
        expect(ix?.accounts?.[3]).toMatchObject({ address: OWNER, role: AccountRole.READONLY_SIGNER });
        expect(tail(ix, MAX_RECORDS)[0]?.address).toBe((await findShareEpochPda({ market: MARKET, epoch: 1 }))[0]);
    });

    it('show lender.subUnitConversion last, with its variables, on every flow that converts the position', async () => {
        const subUnit = { vestEnd: '2026-10-08 06:00 UTC' };
        const c = ctx({ lenderEpoch: 0, shareEpoch: 1, subUnit });
        const plans = [
            await buildClaimPlan({ ...c, syncs: [], expected: 1n }),
            await buildCoverPlan({ ...c, lamports: 1n }),
            await buildSyncPlan(c),
        ];
        for (const plan of plans) {
            expect(plan.disclosures, plan.kind).toEqual(['lender.subUnitConversion']);
            expect(plan.disclosureVars?.['lender.subUnitConversion'], plan.kind).toEqual(subUnit);
        }
        const deposit = await buildDepositPlan({
            ...c,
            source: SOURCE,
            sourceCpiGuard: false,
            amount: 1n,
            minShares: 0n,
            bracket: false,
            sweep: [],
            unpaidRent: { receivable: '1', exposure: '0.5', share: '50%' },
        });
        expect(deposit.disclosures).toEqual(['lender.solRepayment', 'lender.unpaidRent', 'lender.subUnitConversion']);
        expect(deposit.disclosureVars).toEqual({
            'lender.unpaidRent': { receivable: '1', exposure: '0.5', share: '50%' },
            'lender.subUnitConversion': subUnit,
        });
        // Without it nothing changes.
        const plain = await buildClaimPlan({ ...ctx(), syncs: [], expected: 1n });
        expect(plain.disclosures).toEqual([]);
        expect(plain.disclosureVars).toBeUndefined();
    });

    it('decode the lender position for the review', () => {
        const plan = buildCoverPlan({ ...ctx(), lamports: 1n });
        return plan.then((p) => {
            const w = p.watch.find((x) => x.kind === 'program');
            if (w?.kind !== 'program') throw new Error('no program watch');
            const bytes = getLenderPositionEncoder().encode({
                owner: OWNER,
                market: MARKET,
                shares: 7n,
                snap: 0n,
                pending: -3n,
                snapR: 0n,
                real: 0n,
                claimed: 9n,
                epoch: 0,
                lock: 2n,
                lockPi: 10n ** 18n,
                lockEpoch: 0,
                lockTerm: 0n,
                lockUntil: 0n,
                bump: 255,
                fwdShares: 0n,
                fwdEpoch: 0,
                fwd: 0,
                fwdLag: 0,
                reserved: new Uint8Array(10),
            });
            expect(w.decode(new Uint8Array(bytes))).toEqual({ shares: 7n, lock: 2n, claimed: 9n, pending: -3n });
            expect(Object.keys(w.fields)).toEqual(['pending']);
        });
    });
});
