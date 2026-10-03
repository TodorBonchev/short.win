/**
 * lend-sweep-cashback (FRONTEND.md §3.6 Deposit, FE-18; ARCH §3.9; M5-SEC-1, M6-SEC-2). NEEDS the
 * dumped pump and PumpSwap programs (SHORTWIN_E2E_VENUES=1): `@lend`.
 *
 * The venue part of the post-wipe sweep bundle on the dumped mainnet ELFs. lend-post-wipe lands
 * the bundle on a mock market, where it is `sweep_trader` alone; on a pump-venue market the app
 * first claims the trader's venue cashback (`sweep.ts`, a port of the keeper's `sweep.rs`). Those
 * claims are hand-encoded (no Codama client for pump / PumpSwap), so here the app's own builders —
 * `readSweepFacts` and `cashbackClaimIxs` — run against the real programs, for the trader PDA of a
 * market address no market uses (nothing else is touched):
 *
 * - its pump UVA (created by pump's `init_user_volume_accumulator`) holds lamports above rent, and
 *   `ATA(pAMM UVA(trader), WSOL)` holds WSOL while the PumpSwap UVA does not exist yet;
 * - `readSweepFacts` reads exactly those amounts and the missing UVA;
 * - `cashbackClaimIxs` gives `[pump claim_cashback, pAMM init_user_volume_accumulator,
 *   create_idempotent(ATA(trader, WSOL)), pAMM claim_cashback]`, which lands signed by the fee payer
 *   alone (the trader, a PDA, cannot sign) and moves both cashbacks to the trader's side, where the
 *   bundle's `sweep_trader` collects them.
 */
import { type AccountMeta, AccountRole, type Address, generateKeyPairSigner, type Instruction } from '@solana/kit';
import { getTransferSolInstruction } from '@solana-program/system';
import { getCreateAssociatedTokenIdempotentInstructionAsync, getSyncNativeInstruction } from '@solana-program/token';
import { PUMP_AMM_PROGRAM, PUMP_PROGRAM, SYSTEM_PROGRAM } from '../../src/config/programs.ts';
import {
    cashbackClaimIxs,
    IX_INIT_USER_VOLUME_ACCUMULATOR,
    PUMP_EVENT_AUTHORITY,
    pammCashbackAta,
    pammUva,
    readSweepFacts,
    userVolumeAccumulator,
    wsolAta,
} from '../../src/features/lend/sweep.ts';
import type { AppRpc } from '../../src/rpc/client.ts';
import { expect, need, requireVenueFixtures, test } from '../fixtures.ts';
import { landedTransaction } from '../validator/actions.ts';
import {
    getAccount,
    getBalance,
    getTokenAmount,
    LAMPORTS_PER_SOL,
    PROGRAMS,
    pda,
    rpc,
    send,
    WSOL_MINT,
} from '../validator/chain.ts';
import { rentExempt } from '../validator/genesis.ts';
import { signerFor } from '../validator/keys.ts';

const DEFAULT_ADDRESS = '11111111111111111111111111111111' as Address;
/** `VenueKind::PumpSwap` (both claims apply: curve-era pump cashback and PumpSwap's). */
const VENUE_PUMP_SWAP = 3;
const PUMP_CASHBACK = LAMPORTS_PER_SOL / 50n;
const PAMM_CASHBACK = LAMPORTS_PER_SOL / 20n;

/** pump `init_user_volume_accumulator(payer, user)` (keeper `graduation::pump_init_uva_ix`). */
async function pumpInitUvaIx(payer: Address, user: Address): Promise<Instruction> {
    const m = (address: Address, role: AccountRole): AccountMeta => ({ address, role });
    return {
        programAddress: PUMP_PROGRAM,
        accounts: [
            m(payer, AccountRole.WRITABLE_SIGNER),
            m(user, AccountRole.READONLY),
            m(await userVolumeAccumulator(PUMP_PROGRAM, user), AccountRole.WRITABLE),
            m(SYSTEM_PROGRAM, AccountRole.READONLY),
            m(PUMP_EVENT_AUTHORITY, AccountRole.READONLY),
            m(PUMP_PROGRAM, AccountRole.READONLY),
        ],
        data: IX_INIT_USER_VOLUME_ACCUMULATOR,
    };
}

test.use({ wallet: null });

test.describe("lend: the post-wipe bundle's venue cashback claims on the dumped ELFs (FE-18)", { tag: '@lend' }, () => {
    test('readSweepFacts and cashbackClaimIxs claim pump and PumpSwap cashback for a trader PDA, fee payer only', async ({
        state,
    }) => {
        requireVenueFixtures(state);
        const payer = await signerFor('payer');
        // A trader PDA as short.win derives it, of a market address no market uses.
        const unused = (await generateKeyPairSigner()).address;
        const trader = await pda(PROGRAMS.shortwin, ['trader', unused]);
        const pumpUva = await userVolumeAccumulator(PUMP_PROGRAM, trader);
        const market = {
            venue: VENUE_PUMP_SWAP,
            trader,
            pumpUva,
            poolCache: Array.from({ length: 10 }, () => DEFAULT_ADDRESS),
            traderWsolAta: DEFAULT_ADDRESS,
        } as unknown as Parameters<typeof readSweepFacts>[1];
        const uva = await pammUva(market);
        const uvaWsol = await pammCashbackAta(market);
        expect(uvaWsol).toBe(await wsolAta(uva));

        // The trader side as a wipe leaves it on a cashback coin: pump cashback above the UVA's rent,
        // PumpSwap cashback in ATA(pAMM UVA, WSOL) with the PumpSwap UVA not created yet.
        await send(payer, [await pumpInitUvaIx(payer.address, trader)]);
        const uvaAccount = need(await getAccount(pumpUva), 'pump UVA');
        expect(uvaAccount.owner).toBe(PUMP_PROGRAM);
        const floor = uvaAccount.lamports;
        expect(floor).toBe(rentExempt(uvaAccount.data.length));
        await send(payer, [
            getTransferSolInstruction({ source: payer, destination: pumpUva, amount: PUMP_CASHBACK }),
            await getCreateAssociatedTokenIdempotentInstructionAsync({
                payer,
                owner: uva,
                mint: WSOL_MINT,
                tokenProgram: PROGRAMS.token,
            }),
            getTransferSolInstruction({ source: payer, destination: uvaWsol, amount: PAMM_CASHBACK }),
            getSyncNativeInstruction({ account: uvaWsol }),
        ]);
        expect(await getAccount(uva)).toBeNull();

        const app = rpc() as unknown as AppRpc;
        expect(await readSweepFacts(app, market)).toEqual({
            pumpClaimable: PUMP_CASHBACK,
            pammClaimable: PAMM_CASHBACK,
            pammUvaExists: false,
        });
        const ixs = await cashbackClaimIxs(payer, market, await readSweepFacts(app, market));
        expect(ixs.map((ix) => ix.programAddress)).toEqual([
            PUMP_PROGRAM,
            PUMP_AMM_PROGRAM,
            PROGRAMS.ata,
            PUMP_AMM_PROGRAM,
        ]);
        // Nothing in the bundle asks the trader to sign.
        for (const ix of ixs) {
            for (const a of ix.accounts ?? []) {
                if (a.address === trader) expect([AccountRole.READONLY, AccountRole.WRITABLE]).toContain(a.role);
            }
        }
        const signature = await send(payer, ixs, { computeUnits: 400_000 });
        const landed = need(await landedTransaction(signature), 'landed claims');
        expect(landed.err).toBeNull();

        // Both cashbacks reached the trader's side: pump's as lamports, PumpSwap's as WSOL in
        // ATA(trader, WSOL) (sweep_trader closes it into the trader); the venue accounts are drained.
        expect(await getBalance(trader)).toBe(PUMP_CASHBACK);
        expect(need(await getAccount(pumpUva), 'pump UVA').lamports).toBe(floor);
        expect(await getTokenAmount(await wsolAta(trader))).toBe(PAMM_CASHBACK);
        expect(await getTokenAmount(uvaWsol)).toBe(0n);
        expect(need(await getAccount(uva), 'PumpSwap UVA').owner).toBe(PUMP_AMM_PROGRAM);
        expect(await readSweepFacts(app, market)).toEqual({
            pumpClaimable: 0n,
            pammClaimable: 0n,
            pammUvaExists: true,
        });
    });
});
