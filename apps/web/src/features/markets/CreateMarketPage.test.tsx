/**
 * Create market (FRONTEND.md §3.5, §10.1) with the chain, the venue layer (SHORT, mocked until it
 * lands), the wallet and the transaction pipeline mocked: the safety verdict for each rule, an
 * existing market, venue detection and Raydium candidates, the cost, and the plan handed to the
 * pipeline (which simulates it before any signature), then the navigation on confirmation.
 */
import { getCreateMarketInstructionDataDecoder } from '@shortwin/client';
import { AccountRole, type Address, createNoopSigner, type Instruction } from '@solana/kit';
import { act, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { programError } from '../../config/errors.ts';
import { PUMP_PROGRAM, SHORTWIN_PROGRAM, WSOL_MINT } from '../../config/programs.ts';
import { T } from '../../config/testids.ts';
import * as accounts from '../../rpc/accounts.ts';
import type { TxPlan } from '../../tx/types.ts';
import * as venuesLayer from '../../venues/index.ts';
import type { PoolCandidate, VenueDetection } from '../../venues/types.ts';
import * as chainReads from './shared/chainReads.ts';
import {
    CREATOR,
    chainMarket,
    exists,
    globalConfig,
    LENDER,
    MINT_A,
    MINT_B,
    MINT_C,
    mint2022Bytes,
    mintBytes,
    missing,
    TOKEN_2022_PROGRAM,
    TOKEN_PROGRAM,
} from './test/fixtures.ts';
import { mockIndexer, renderAt } from './test/harness.tsx';

const h = vi.hoisted(() => ({
    wallet: null as unknown,
    start: [] as unknown[],
    setStage: null as null | ((stage: string) => void),
    mathError: null as Error | null,
}));

vi.mock('../../rpc/accounts.ts', async (orig) => ({
    ...(await orig<typeof import('../../rpc/accounts.ts')>()),
    readMarket: vi.fn(),
    readGlobalConfig: vi.fn(),
}));
vi.mock('./shared/chainReads.ts', async (orig) => ({
    ...(await orig<typeof import('./shared/chainReads.ts')>()),
    readRawAccount: vi.fn(),
    readRentMinimums: vi.fn(),
}));
vi.mock('../../venues/index.ts', async (orig) => ({
    ...(await orig<typeof import('../../venues/index.ts')>()),
    detectVenue: vi.fn(),
    raydiumCandidates: vi.fn(),
}));
vi.mock('../../math/useMath.ts', async (orig) => {
    const actual = await orig<typeof import('../../math/useMath.ts')>();
    return {
        ...actual,
        useMath: () => {
            const real = actual.useMath();
            return h.mathError ? { math: undefined, error: h.mathError } : real;
        },
    };
});
vi.mock('../../wallet/useWallet.ts', () => ({
    useWallet: () => ({ ready: true, status: h.wallet ? 'connected' : 'disconnected', connected: h.wallet }),
}));
vi.mock('../../tx/useTxFlow.ts', async (orig) => {
    const actual = await orig<typeof import('../../tx/useTxFlow.ts')>();
    const { useState } = await import('react');
    return {
        ...actual,
        useTxFlow: () => {
            const [state, setState] = useState({
                stage: 'idle',
                plan: null as unknown,
                report: null,
                acks: new Set(),
                speed: 'normal',
                signature: null,
                error: null,
                refreshed: false,
            });
            h.setStage = (stage) => setState((s) => ({ ...s, stage }));
            return {
                state,
                start: (plan: unknown) => {
                    h.start.push(plan);
                    setState((s) => ({ ...s, stage: 'review', plan }));
                },
                refresh() {},
                setAck() {},
                setSpeed() {},
                sign() {},
                close: () => setState((s) => ({ ...s, stage: 'idle', plan: null })),
            };
        },
    };
});

const BC = LENDER;
const CP = CREATOR;
const POOL_OK = MINT_B;
const POOL_THIN = MINT_C;
const RENT = { 3104: 22_494_960n, 2200: 16_203_200n, 165: 2_039_280n, 170: 2_074_080n, 137: 1_844_400n } as const;

function detection(kind: VenueDetection['kind'], over: Partial<VenueDetection> = {}): VenueDetection {
    return {
        kind,
        bondingCurve: BC,
        canonicalPool: CP,
        refusal: null,
        remainingAccounts: [
            { address: BC, role: AccountRole.READONLY },
            { address: CP, role: AccountRole.READONLY },
        ],
        preInstructions: [],
        ...over,
    };
}

function candidate(pool: Address, over: Partial<PoolCandidate> = {}): PoolCandidate {
    return {
        pool,
        kind: 'raydium_cpmm',
        solReserve: 1_500_000_000_000n,
        lockedDepth: 123_456_789n,
        lambdaBps: 9_500,
        feePpm: 2_500,
        eligible: true,
        reasons: [],
        remainingAccounts: [
            { address: BC, role: AccountRole.READONLY },
            { address: CP, role: AccountRole.READONLY },
            { address: pool, role: AccountRole.READONLY },
        ],
        ...over,
    };
}

function mintAccount(owner: Address, data: Uint8Array, address: Address = MINT_A): chainReads.RawAccount {
    return { address, owner, lamports: 1_461_600n, data, executable: false };
}

beforeEach(() => {
    h.wallet = {
        address: CREATOR,
        signer: createNoopSigner(CREATOR),
        walletName: 'Test wallet',
        supportsV0: true,
    };
    h.start = [];
    h.mathError = null;
    mockIndexer({});
    vi.mocked(accounts.readMarket).mockResolvedValue(missing(MINT_A));
    vi.mocked(accounts.readGlobalConfig).mockResolvedValue(exists(MINT_A, globalConfig()));
    vi.mocked(chainReads.readRawAccount).mockResolvedValue(mintAccount(TOKEN_PROGRAM, mintBytes()));
    vi.mocked(chainReads.readRentMinimums).mockImplementation(async (_rpc, sizes) =>
        sizes.map((s) => RENT[s as keyof typeof RENT] ?? 0n),
    );
    vi.mocked(venuesLayer.detectVenue).mockResolvedValue(detection('none'));
    vi.mocked(venuesLayer.raydiumCandidates).mockResolvedValue([]);
});

afterEach(() => {
    vi.unstubAllGlobals();
    vi.clearAllMocks();
});

const verdict = () => screen.getByTestId(T.createVerdict).dataset.verdict;
const submit = () => screen.getByTestId(T.createSubmit) as HTMLButtonElement;

describe('CreateMarketPage', () => {
    it('checks a safe mint, detects a list-only venue, prices it and hands a simulated plan to the pipeline', async () => {
        const { router, queryClient } = await renderAt(`/markets/new?mint=${MINT_A}`);
        expect((screen.getByTestId(T.createMintInput) as HTMLInputElement).value).toBe(MINT_A);
        await waitFor(() => expect(verdict()).toBe('ok'));
        expect(screen.getByTestId(T.createMintInfo).textContent).toContain('SPL Token');
        await waitFor(() => expect(screen.getByTestId(T.createVenue).dataset.value).toBe('none'));
        expect((screen.getByTestId(T.createChoice('auto')) as HTMLInputElement).checked).toBe(true);
        expect(await screen.findByText(/No Raydium AMM v4 or CPMM SOL pool/)).toBeTruthy();
        const total = 20_000_000n + RENT[3104] + RENT[2200] + RENT[165];
        await waitFor(() => expect(screen.getByTestId(T.createCost).dataset.value).toBe(total.toString()));
        expect(vi.mocked(chainReads.readRentMinimums).mock.calls.at(-1)?.[1]).toEqual([3_104, 2_200, 165]);
        expect(screen.getByTestId(T.disclosure('market.create')).textContent).toContain('creation fee (0.02 SOL)');
        await waitFor(() => expect(submit().disabled).toBe(false));
        await act(async () => {
            fireEvent.click(submit());
        });
        await waitFor(() => expect(h.start).toHaveLength(1));
        const plan = h.start[0] as TxPlan;
        expect(plan.kind).toBe('create_market');
        expect(plan.disclosures).toEqual(['market.create']);
        const ix = plan.instructions[0] as Instruction;
        expect(ix.programAddress).toBe(SHORTWIN_PROGRAM);
        const data = getCreateMarketInstructionDataDecoder().decode(ix.data ?? new Uint8Array());
        expect(data).toMatchObject({ venueHint: 0, maxCreationFee: 20_000_000n });
        expect(ix.accounts?.[0]?.address).toBe(CREATOR);
        expect(ix.accounts?.slice(-2).map((m) => m.address)).toEqual([BC, CP]);
        expect(await screen.findByTestId(T.txModal)).toBeTruthy();
        // Confirmed: the new market's page.
        await act(async () => {
            h.setStage?.('confirmed');
        });
        await waitFor(() => expect(router.state.location.pathname).toBe(`/m/${MINT_A}`));
        expect(screen.getByTestId(T.toast).textContent).toContain('Market created');
        queryClient.clear();
    });

    it('rejects a mint with a freeze authority, with the program’s reason, and never detects a venue', async () => {
        vi.mocked(chainReads.readRawAccount).mockResolvedValue(mintAccount(TOKEN_PROGRAM, mintBytes({ freeze: true })));
        const { queryClient } = await renderAt(`/markets/new?mint=${MINT_A}`);
        await waitFor(() => expect(verdict()).toBe('reject'));
        const reason = screen.getByTestId(T.createReason('MintFreezeAuthority'));
        expect(reason.textContent).toContain(programError(6018)?.message ?? 'missing');
        expect(reason.textContent).toContain(CREATOR);
        expect(screen.queryByTestId(T.createVenue)).toBeNull();
        expect(venuesLayer.detectVenue).not.toHaveBeenCalled();
        // D14: the cost says it cannot be listed, and there is no Review at all.
        expect(screen.queryByTestId(T.createSubmit)).toBeNull();
        expect(screen.getByText('Cannot be listed')).toBeTruthy();
        expect(screen.getByText(/This token cannot be listed/).parentElement?.textContent).toContain(
            programError(6018)?.message ?? 'missing',
        );
        queryClient.clear();
    });

    it('lists every disallowed Token-2022 extension by name', async () => {
        vi.mocked(chainReads.readRawAccount).mockResolvedValue(
            mintAccount(
                TOKEN_2022_PROGRAM,
                mint2022Bytes([
                    [18, 64],
                    [12, 32],
                    [14, 64],
                ]),
            ),
        );
        const { queryClient } = await renderAt(`/markets/new?mint=${MINT_A}`);
        await waitFor(() => expect(verdict()).toBe('reject'));
        const reason = screen.getByTestId(T.createReason('MintDisallowedExtension'));
        expect(reason.textContent).toContain('PermanentDelegate');
        expect(reason.textContent).toContain('TransferHook');
        const info = screen.getByTestId(T.createMintInfo);
        expect(info.textContent).toContain('Token-2022');
        expect(info.textContent).toContain('✓ MetadataPointer');
        expect(info.textContent).toContain('✕ PermanentDelegate');
        queryClient.clear();
    });

    it('rejects the native mint and an address without an account', async () => {
        vi.mocked(chainReads.readRawAccount).mockResolvedValue(mintAccount(TOKEN_PROGRAM, mintBytes(), WSOL_MINT));
        const first = await renderAt(`/markets/new?mint=${WSOL_MINT}`);
        await waitFor(() => expect(verdict()).toBe('reject'));
        expect(screen.getByTestId(T.createReason('MintNative'))).toBeTruthy();
        first.queryClient.clear();
        first.view?.unmount();
        vi.mocked(chainReads.readRawAccount).mockResolvedValue(null);
        const second = await renderAt(`/markets/new?mint=${MINT_A}`);
        await waitFor(() => expect(verdict()).toBe('reject'));
        expect(screen.getByTestId(T.createReason('MintUnsupportedProgram')).textContent).toContain('No account');
        second.queryClient.clear();
    });

    it('shows the math module’s load error instead of checking forever (the verdict is the facade’s)', async () => {
        h.mathError = new Error('math module failed to load');
        const { queryClient } = await renderAt(`/markets/new?mint=${MINT_A}`);
        await waitFor(() => expect(screen.getByText(/math module failed to load/).className).toBe('error-state'));
        expect(screen.queryByTestId(T.createVerdict)).toBeNull();
        expect(submit().disabled).toBe(true);
        expect(vi.mocked(venuesLayer.detectVenue)).not.toHaveBeenCalled();
        queryClient.clear();
    });

    it('links to the existing market of a mint', async () => {
        vi.mocked(accounts.readMarket).mockResolvedValue(exists(MINT_A, chainMarket()));
        const { queryClient } = await renderAt(`/markets/new?mint=${MINT_A}`);
        await waitFor(() => expect(verdict()).toBe('existing'));
        expect(screen.getByTestId(T.createExisting).getAttribute('href')).toBe(`/m/${MINT_A}`);
        expect(submit().disabled).toBe(true);
        expect(screen.getByTestId(T.createSubmitReason).textContent).toBe('This token already has a market.');
        queryClient.clear();
    });

    it('offers eligible Raydium pools and lists on the chosen one', async () => {
        vi.mocked(venuesLayer.raydiumCandidates).mockResolvedValue([
            candidate(POOL_OK),
            candidate(POOL_THIN, { kind: 'raydium_v4', eligible: false, reasons: ['PoolTooThin'] }),
        ]);
        const { queryClient } = await renderAt('/markets/new');
        await act(async () => {
            fireEvent.change(screen.getByTestId(T.createMintInput), { target: { value: MINT_A } });
        });
        const ok = await screen.findByTestId(T.createPoolRow(POOL_OK));
        expect(ok.dataset.eligible).toBe('true');
        const thin = screen.getByTestId(T.createPoolRow(POOL_THIN));
        expect(thin.dataset.eligible).toBe('false');
        expect(thin.textContent).toContain('PoolTooThin');
        expect((within(thin).getByRole('radio') as HTMLInputElement).disabled).toBe(true);
        await act(async () => {
            fireEvent.click(screen.getByTestId(T.createChoice(POOL_OK)));
        });
        await waitFor(() => expect(submit().textContent).toContain('Raydium CPMM'));
        await waitFor(() => expect(submit().disabled).toBe(false));
        await act(async () => {
            fireEvent.click(submit());
        });
        await waitFor(() => expect(h.start).toHaveLength(1));
        const ix = (h.start[0] as TxPlan).instructions[0] as Instruction;
        expect(getCreateMarketInstructionDataDecoder().decode(ix.data ?? new Uint8Array()).venueHint).toBe(5);
        expect(ix.accounts?.slice(-3).map((m) => m.address)).toEqual([BC, CP, POOL_OK]);
        queryClient.clear();
    });

    it('prices a pump listing with the trader accounts and prepends the venue layer’s instructions', async () => {
        const extend: Instruction = { programAddress: PUMP_PROGRAM, accounts: [], data: new Uint8Array([7]) };
        vi.mocked(venuesLayer.detectVenue).mockResolvedValue(detection('pump_curve', { preInstructions: [extend] }));
        const { queryClient } = await renderAt(`/markets/new?mint=${MINT_A}`);
        await waitFor(() => expect(screen.getByTestId(T.createVenue).dataset.value).toBe('pump_curve'));
        const total = 20_000_000n + RENT[3104] + RENT[2200] + RENT[165] + RENT[165] + RENT[137];
        await waitFor(() => expect(screen.getByTestId(T.createCost).dataset.value).toBe(total.toString()));
        expect(screen.queryByTestId(T.createPools)).toBeNull();
        expect(venuesLayer.raydiumCandidates).not.toHaveBeenCalled();
        await waitFor(() => expect(submit().disabled).toBe(false));
        await act(async () => {
            fireEvent.click(submit());
        });
        await waitFor(() => expect(h.start).toHaveLength(1));
        expect((h.start[0] as TxPlan).instructions.map((i) => i.programAddress)).toEqual([
            PUMP_PROGRAM,
            SHORTWIN_PROGRAM,
        ]);
        queryClient.clear();
    });

    it('blocks a pump coin the program refuses, with the reason', async () => {
        vi.mocked(venuesLayer.detectVenue).mockResolvedValue(
            detection('pump_curve', { refusal: 'CurveMayhemUnsupported' }),
        );
        const { queryClient } = await renderAt(`/markets/new?mint=${MINT_A}`);
        const reason = await screen.findByTestId(T.createReason('CurveMayhemUnsupported'));
        expect(reason.textContent).toContain(programError(6025)?.message ?? 'missing');
        expect(screen.queryByTestId(T.createChoice('auto'))).toBeNull();
        expect(screen.queryByTestId(T.createSubmit)).toBeNull();
        expect(screen.getByText('Cannot be listed')).toBeTruthy();
        queryClient.clear();
    });

    it('asks for a wallet before the review', async () => {
        h.wallet = null;
        const { queryClient } = await renderAt(`/markets/new?mint=${MINT_A}`);
        await waitFor(() => expect(verdict()).toBe('ok'));
        await waitFor(() => expect(screen.getByTestId(T.createCost)).toBeTruthy());
        expect(submit().disabled).toBe(true);
        expect(screen.getByTestId(T.createSubmitReason).textContent).toBe(
            'Connect a wallet that can sign to create the market.',
        );
        queryClient.clear();
    });
});
