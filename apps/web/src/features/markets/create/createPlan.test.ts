import {
    findMarketPda,
    findMarketVaultPda,
    findPoolVaultPda,
    findTraderPda,
    getCreateMarketInstructionDataDecoder,
} from '@shortwin/client';
import { AccountRole, address, createNoopSigner, getProgramDerivedAddress, type Instruction } from '@solana/kit';
import { findAssociatedTokenPda } from '@solana-program/token';
import { describe, expect, it } from 'vitest';
import { qk } from '../../../api/keys.ts';
import { PUMP_PROGRAM, SHORTWIN_PROGRAM, TOKEN_2022_PROGRAM, TOKEN_PROGRAM } from '../../../config/programs.ts';
import { eventAuthority as sharedEventAuthority } from '../../../rpc/pda.ts';
import { readRepo } from '../../../test/repo.ts';
import type { PoolCandidate, VenueDetection } from '../../../venues/types.ts';
import {
    ATA_LEN,
    buildCreateMarketPlan,
    creationAccounts,
    creationCost,
    listingVenue,
    POOL_VAULT_LEN,
    UVA_LEN,
} from './createPlan.ts';

const MINT = address('7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU');
const WALLET = createNoopSigner(address('9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin'));
const BC = address('BPAZFPkmVtYEWEZHCzFUns7oybh1VWfAvmu3ofM2mQi7');
const CP = address('4B5mGykX9EWwaYdbTvsp8qXCUTbc7caQourz16Gat6LJ');
const POOL = address('So11111111111111111111111111111111111111112');

const detection = (kind: VenueDetection['kind'], pre: Instruction[] = []): VenueDetection => ({
    kind,
    bondingCurve: BC,
    canonicalPool: CP,
    refusal: null,
    remainingAccounts: [
        { address: BC, role: AccountRole.READONLY },
        { address: CP, role: AccountRole.READONLY },
    ],
    preInstructions: pre,
});

const candidate: PoolCandidate = {
    pool: POOL,
    kind: 'raydium_cpmm',
    solReserve: 10n ** 12n,
    lockedDepth: 10n ** 9n,
    lambdaBps: 9_000,
    feePpm: 2_500,
    eligible: true,
    reasons: [],
    remainingAccounts: [
        { address: BC, role: AccountRole.READONLY },
        { address: CP, role: AccountRole.READONLY },
        { address: POOL, role: AccountRole.READONLY },
    ],
};

const cost = creationCost(20_000_000n, [22_500_000n, 16_200_000n, 2_039_280n]);

describe('creation cost', () => {
    it('lists the accounts the program creates and their sizes', () => {
        const src = readRepo('programs/shortwin/src/instructions/create_market.rs');
        expect(src).toContain(`pub const POOL_VAULT_LEN: usize = ${POOL_VAULT_LEN};`);
        expect(readRepo('crates/shortwin-venue-state/src/pump.rs')).toContain(
            `pub const USER_VOLUME_ACCUMULATOR_LEN: usize = ${UVA_LEN};`,
        );
        expect(creationAccounts('none', 'token').map((i) => [i.key, i.size])).toEqual([
            ['market', 3_104],
            ['vault', 2_200],
            ['pool-vault', 165],
        ]);
        expect(creationAccounts('raydium_v4', 'token-2022')).toHaveLength(3);
        expect(creationAccounts('pump_curve', 'token-2022').map((i) => i.size)).toEqual([
            3_104,
            2_200,
            165,
            ATA_LEN['token-2022'],
            UVA_LEN,
        ]);
        expect(creationAccounts('pump_swap', 'token').map((i) => i.key)).toContain('trader-uva');
        expect(cost).toEqual({ fee: 20_000_000n, rent: 40_739_280n, total: 60_739_280n });
    });
});

describe('buildCreateMarketPlan', () => {
    it('builds an Auto listing: Codama accounts, the detection set, the read fee as the maximum', async () => {
        const plan = await buildCreateMarketPlan({
            signer: WALLET,
            mint: MINT,
            tokenProgram: TOKEN_2022_PROGRAM,
            choice: { kind: 'auto', detection: detection('none') },
            creationFee: 20_000_000n,
            cost,
            label: 'TEST',
        });
        expect(plan.kind).toBe('create_market');
        expect(plan.instructions).toHaveLength(1);
        const ix = plan.instructions[0];
        expect(ix?.programAddress).toBe(SHORTWIN_PROGRAM);
        const [market] = await findMarketPda({ mint: MINT });
        const [marketVault] = await findMarketVaultPda({ market });
        const [poolVault] = await findPoolVaultPda({ market });
        const [trader] = await findTraderPda({ market });
        const [traderAta] = await findAssociatedTokenPda({
            owner: trader,
            mint: MINT,
            tokenProgram: TOKEN_2022_PROGRAM,
        });
        const [eventAuthority] = await getProgramDerivedAddress({
            programAddress: SHORTWIN_PROGRAM,
            seeds: ['__event_authority'],
        });
        expect(await sharedEventAuthority()).toBe(eventAuthority);
        const accounts = ix?.accounts ?? [];
        expect(accounts.map((m) => m.address)).toEqual([
            WALLET.address,
            expect.any(String), // config PDA
            expect.any(String), // venue guard PDA
            market,
            marketVault,
            poolVault,
            MINT,
            trader,
            traderAta,
            TOKEN_2022_PROGRAM,
            expect.any(String), // ATA program
            expect.any(String), // System
            eventAuthority,
            SHORTWIN_PROGRAM,
            BC,
            CP,
        ]);
        expect(accounts[0]?.role).toBe(AccountRole.WRITABLE_SIGNER);
        expect(accounts[3]?.role).toBe(AccountRole.WRITABLE);
        const data = getCreateMarketInstructionDataDecoder().decode(ix?.data ?? new Uint8Array());
        expect(data.venueHint).toBe(0);
        expect(data.maxCreationFee).toBe(20_000_000n);
        expect(plan.disclosures).toEqual(['market.create']);
        expect(plan.disclosureVars?.['market.create']).toEqual({ fee: '0.02 SOL', rent: '0.040739 SOL' });
        expect(plan.watch.map((w) => w.address)).toEqual([WALLET.address, market]);
        expect(plan.invalidate).toContainEqual(qk.marketAccount(MINT));
        expect(plan.invalidate).toContainEqual(['indexer', 'markets']);
    });

    it('prepends the venue layer’s instructions (pump extend_account) on an Auto pump listing', async () => {
        const extend: Instruction = { programAddress: PUMP_PROGRAM, accounts: [], data: new Uint8Array([1]) };
        const plan = await buildCreateMarketPlan({
            signer: WALLET,
            mint: MINT,
            tokenProgram: TOKEN_PROGRAM,
            choice: { kind: 'auto', detection: detection('pump_curve', [extend]) },
            creationFee: 0n,
            cost,
            label: 'PUMP',
        });
        expect(plan.instructions.map((i) => i.programAddress)).toEqual([PUMP_PROGRAM, SHORTWIN_PROGRAM]);
    });

    it('lists on a Raydium candidate with its hint and ReadSet', async () => {
        const choice = { kind: 'raydium', candidate } as const;
        expect(listingVenue(choice)).toBe('raydium_cpmm');
        const plan = await buildCreateMarketPlan({
            signer: WALLET,
            mint: MINT,
            tokenProgram: TOKEN_PROGRAM,
            choice,
            creationFee: 1n,
            cost,
            label: 'RAY',
        });
        const ix = plan.instructions[0];
        expect(getCreateMarketInstructionDataDecoder().decode(ix?.data ?? new Uint8Array()).venueHint).toBe(5);
        expect(ix?.accounts?.slice(-3).map((m) => m.address)).toEqual([BC, CP, POOL]);
    });
});
