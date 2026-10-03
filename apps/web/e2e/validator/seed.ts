/**
 * The E2E seed (FRONTEND.md §10.2): the scenario every spec starts from, built through real
 * instructions (Kit, the Codama client `@shortwin/client`, `@solana-program/*` and the mock
 * venue's interface) on the E2E validator, written to `.e2e/state.json`.
 *
 * 1. `initialize_config` signed by the generated upgrade authority: ECON §15 defaults (WASM
 *    `ParamSet`), the generated governance / Emergency / coverage / fee keys, the release venue bits
 *    plus the mock's (test-venue build), the live ProgramData slots as `guard_slot`.
 * 2. A protocol ALT (`@solana-program/address-lookup-table`) for the app's v0 path.
 * 3. Three SPL mints with mock pools and markets (`venue_hint = 6`): `main` (trading), `move` (the
 *    price-move specs), `paused` (coverage-paused by the generated coverage authority); a Token-2022
 *    mint with metadata only and its list-only market `t22` (CPI Guard and memo specs). Each lent by
 *    a seed lender; the `claimer` wallet lends into `main` before any position opens.
 * 4. Per-wallet token accounts: main-mint ATAs; for `guard` a plain T22 ATA, a CPI-Guarded account
 *    and an unguarded one with the CpiGuard extension (the race); for `memo` / `memoRace` a lender
 *    position in `t22` and an ATA that requires incoming memos (`memo`) or has the MemoTransfer
 *    extension off (`memoRace`).
 * 5. Mints without a market for create-market: safe (two), freeze authority, a disallowed extension.
 * 6. Two more mock markets for FE-18, each lent by one spec wallet alone: `wipe` (wallet `wiped`;
 *    lend-post-wipe drives its full wipe) and `pot` (wallet `potClaimer`; lend-claim-sync), with the
 *    tokens their other lenders deposit later (`rejoin`, `potLate`).
 * 7. After the 50 s market warm-up: five open positions (`margin` on main, `closer` and `holder`
 *    on move, `wipeBorrower` on wipe, `potBorrower` on pot) with generous bounds.
 *
 * Run standalone: `node e2e/validator/seed.ts` (against a running E2E validator).
 */
import { mkdirSync, writeFileSync } from 'node:fs';
import {
    getCreateMarketInstructionAsync,
    getDepositInstructionAsync,
    getInitializeConfigInstructionAsync,
    getOpenShortInstructionAsync,
    getSetCoveragePauseInstructionAsync,
    SHORTWIN_PROGRAM_ADDRESS,
} from '@shortwin/client';
import {
    type AccountMeta,
    AccountRole,
    type Address,
    generateKeyPairSigner,
    type Instruction,
    type KeyPairSigner,
} from '@solana/kit';
import {
    findAddressLookupTablePda,
    getCreateLookupTableInstruction,
    getExtendLookupTableInstruction,
} from '@solana-program/address-lookup-table';
import { getCreateAccountInstruction, getTransferSolInstruction } from '@solana-program/system';
import {
    getCreateAssociatedTokenIdempotentInstructionAsync,
    getInitializeMint2Instruction,
    getMintToInstruction,
    getSyncNativeInstruction,
} from '@solana-program/token';
import {
    type ExtensionArgs,
    ExtensionType,
    getEnableMemoTransfersInstruction,
    getInitializeAccount3Instruction as getInitializeAccount3Instruction2022,
    getInitializeMint2Instruction as getInitializeMint2Instruction2022,
    getMintSize,
    getMintToInstruction as getMintToInstruction2022,
    getPostInitializeInstructionsForMintExtensions,
    getPostInitializeInstructionsForTokenExtensions,
    getPreInitializeInstructionsForMintExtensions,
    getReallocateInstruction,
    getTokenSize,
} from '@solana-program/token-2022';
import { E2E_DIR, ENV, PATHS } from '../env.ts';
import {
    ata,
    clock,
    detectionSet,
    eventAuthority,
    getAccount,
    LAMPORTS_PER_SOL,
    marketKeys,
    PROGRAMS,
    pda,
    programDataOf,
    programDataSlot,
    rpc,
    send,
    sleep,
    WSOL_MINT,
    waitForClock,
} from './chain.ts';
import { fixtureAddress, rentExempt } from './genesis.ts';
import { KEY_NAMES, keyPath, signerFor, type WalletName } from './keys.ts';
import { addLiquidityIx, derivePool, initializePoolIx, type MockPool } from './mock-cpamm.ts';
import { defaultParams } from './params.ts';
import type { E2EState, MarketInfo, PositionInfo } from './state.ts';

const U64_MAX = 0xffff_ffff_ffff_ffffn;
const PYTH_SOL_USD = '7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE' as Address;
const PYTH_FEED_ID = Uint8Array.from(
    Buffer.from('ef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d', 'hex'),
);
/** `programs/shortwin/src/constants.rs`: `DEFAULT_TIMELOCK_S`, the venue bits, the v0.4.1 deploy slots. */
const DEFAULT_TIMELOCK_S = 172_800;
const VENUE_BITS_RELEASE_DEFAULT = 0b1111; // pump curve, PumpSwap, Raydium AMM v4, CPMM (Env::default_init_args)
const VENUE_BIT_MOCK = 1 << 5;
const VENUE_HINT_AUTO = 0;
const VENUE_HINT_MOCK = 6;
const JUPITER_GUARD_SLOT_2026_09_27 = 449_280_566n;
const COVERAGE_REASON_KEEPER = 1;
/** `market_warmup_s` (ECON §15; its compiled minimum). */
const MARKET_WARMUP_S = 50n;
const DECIMALS = 6;
const TOKEN = 10n ** BigInt(DECIMALS);

const log = (...a: unknown[]) => console.log('[seed]', ...a);

interface Ctx {
    payer: KeyPairSigner;
    mintAuthority: KeyPairSigner;
    poolAdmin: KeyPairSigner;
    seedLender: KeyPairSigner;
    eventAuthority: Address;
}

// ---- mints and token accounts ----------------------------------------------------------------

async function createSplMint(c: Ctx, freezeAuthority: Address | null = null): Promise<Address> {
    const mint = await generateKeyPairSigner();
    await send(c.payer, [
        getCreateAccountInstruction({
            payer: c.payer,
            newAccount: mint,
            lamports: rentExempt(82),
            space: 82,
            programAddress: PROGRAMS.token,
        }),
        getInitializeMint2Instruction({
            mint: mint.address,
            decimals: DECIMALS,
            mintAuthority: c.mintAuthority.address,
            freezeAuthority,
        }),
    ]);
    return mint.address;
}

/**
 * A Token-2022 mint whose extensions `ext(mint)` returns: `fixed` ones sized into the account
 * (initialised before `InitializeMint2`), `variable` ones (TokenMetadata) paid for in its lamports
 * and initialised after it.
 */
async function createT22Mint(
    c: Ctx,
    ext: (mint: Address) => { fixed: ExtensionArgs[]; variable?: ExtensionArgs[] },
): Promise<Address> {
    const mint = await generateKeyPairSigner();
    const { fixed, variable = [] } = ext(mint.address);
    const space = getMintSize(fixed);
    const full = getMintSize([...fixed, ...variable]);
    await send(c.payer, [
        getCreateAccountInstruction({
            payer: c.payer,
            newAccount: mint,
            lamports: rentExempt(full),
            space,
            programAddress: PROGRAMS.token2022,
        }),
        ...getPreInitializeInstructionsForMintExtensions(mint.address, fixed),
        getInitializeMint2Instruction2022({
            mint: mint.address,
            decimals: DECIMALS,
            mintAuthority: c.mintAuthority.address,
            freezeAuthority: null,
        }),
        ...getPostInitializeInstructionsForMintExtensions(mint.address, c.mintAuthority, variable),
    ]);
    return mint.address;
}

async function ensureAta(c: Ctx, owner: Address, mint: Address, tokenProgram: Address): Promise<Address> {
    const ix = await getCreateAssociatedTokenIdempotentInstructionAsync({
        payer: c.payer,
        owner,
        mint,
        tokenProgram,
    });
    await send(c.payer, [ix]);
    return ata(owner, mint, tokenProgram);
}

function mintToIx(c: Ctx, mint: Address, token: Address, tokenProgram: Address, amount: bigint): Instruction {
    return tokenProgram === PROGRAMS.token2022
        ? getMintToInstruction2022({ mint, token, mintAuthority: c.mintAuthority, amount })
        : getMintToInstruction({ mint, token, mintAuthority: c.mintAuthority, amount });
}

async function mintTo(c: Ctx, mint: Address, token: Address, tokenProgram: Address, amount: bigint): Promise<void> {
    await send(c.payer, [mintToIx(c, mint, token, tokenProgram, amount)]);
}

/** A non-ATA Token-2022 account of `owner` with `extensions` initialised (owner-signed where needed). */
async function createT22Account(
    c: Ctx,
    owner: KeyPairSigner,
    mint: Address,
    extensions: ExtensionArgs[],
    amount: bigint,
): Promise<Address> {
    const acct = await generateKeyPairSigner();
    const space = getTokenSize(extensions);
    await send(
        c.payer,
        [
            getCreateAccountInstruction({
                payer: c.payer,
                newAccount: acct,
                lamports: rentExempt(space),
                space,
                programAddress: PROGRAMS.token2022,
            }),
            getInitializeAccount3Instruction2022({ account: acct.address, mint, owner: owner.address }),
            ...getPostInitializeInstructionsForTokenExtensions(acct.address, owner, extensions),
            mintToIx(c, mint, acct.address, PROGRAMS.token2022, amount),
        ],
        { signers: [owner] },
    );
    return acct.address;
}

/** The owner's Token-2022 ATA with room for MemoTransfer, required (`enable`) or not. */
async function memoAta(c: Ctx, owner: KeyPairSigner, mint: Address, enable: boolean): Promise<Address> {
    const a = await ensureAta(c, owner.address, mint, PROGRAMS.token2022);
    const ixs: Instruction[] = [
        getReallocateInstruction({
            token: a,
            payer: c.payer,
            owner,
            newExtensionTypes: [ExtensionType.MemoTransfer],
        }),
    ];
    if (enable) ixs.push(getEnableMemoTransfersInstruction({ token: a, owner }));
    await send(c.payer, ixs, { signers: [owner] });
    return a;
}

/** `amount` lamports wrapped into the owner's WSOL ATA. */
async function wrapSol(c: Ctx, owner: KeyPairSigner, amount: bigint): Promise<Address> {
    const w = await ensureAta(c, owner.address, WSOL_MINT, PROGRAMS.token);
    await send(c.payer, [
        getTransferSolInstruction({ source: c.payer, destination: w, amount }),
        getSyncNativeInstruction({ account: w }),
    ]);
    return w;
}

// ---- protocol --------------------------------------------------------------------------------

async function initializeConfig(): Promise<void> {
    const authority = await signerFor('authority');
    const p = defaultParams();
    const venueGuardOrder = [
        PROGRAMS.pump,
        PROGRAMS.pumpAmm,
        PROGRAMS.pumpFees,
        PROGRAMS.raydiumV4,
        PROGRAMS.raydiumCpmm,
    ];
    const guardSlot: bigint[] = [];
    for (const prog of venueGuardOrder) guardSlot.push((await programDataSlot(prog)) ?? 0n);
    guardSlot.push((await programDataSlot(PROGRAMS.jupiter)) ?? JUPITER_GUARD_SLOT_2026_09_27);
    const ix = await getInitializeConfigInstructionAsync({
        authority,
        programData: await programDataOf(PROGRAMS.shortwin),
        governance: (await signerFor('governance')).address,
        pauseAuthority: (await signerFor('pause')).address,
        coverageAuthority: (await signerFor('coverage')).address,
        feeRecipient: (await signerFor('feeRecipient')).address,
        pythSolUsd: PYTH_SOL_USD,
        pythFeedId: PYTH_FEED_ID,
        timelockS: DEFAULT_TIMELOCK_S,
        venueEnabled: VENUE_BITS_RELEASE_DEFAULT | (ENV.program === 'test-venue' ? VENUE_BIT_MOCK : 0),
        params: p.params,
        tiers: p.tiers,
        ema: p.ema,
        utilKinkBps: p.utilKinkBps,
        guardSlot,
    });
    await send(authority, [ix]);
    log('initialize_config ok; guard slots', guardSlot.join(','));
}

async function createAlt(c: Ctx, addresses: readonly Address[]): Promise<Address> {
    const recentSlot = await rpc().getSlot({ commitment: 'finalized' }).send();
    const [table, bump] = await findAddressLookupTablePda({ authority: c.payer.address, recentSlot });
    await send(c.payer, [getCreateLookupTableInstruction({ address: [table, bump], authority: c.payer, recentSlot })]);
    const unique = [...new Set(addresses)];
    for (let i = 0; i < unique.length; i += 24) {
        await send(c.payer, [
            getExtendLookupTableInstruction({
                address: table,
                authority: c.payer,
                payer: c.payer,
                addresses: unique.slice(i, i + 24),
            }),
        ]);
    }
    return table;
}

// ---- markets ---------------------------------------------------------------------------------

async function createMarket(
    c: Ctx,
    creator: KeyPairSigner,
    mint: Address,
    tokenProgram: Address,
    pool: MockPool | null,
): Promise<void> {
    const k = await marketKeys(mint, tokenProgram);
    const det = await detectionSet(mint);
    const ix = await getCreateMarketInstructionAsync({
        creator,
        mint,
        traderBaseAta: k.traderBaseAta,
        tokenProgram,
        eventAuthority: c.eventAuthority,
        program: SHORTWIN_PROGRAM_ADDRESS,
        venueHint: pool ? VENUE_HINT_MOCK : VENUE_HINT_AUTO,
        maxCreationFee: U64_MAX,
    });
    const ro = (address: Address): AccountMeta => ({ address, role: AccountRole.READONLY });
    const remaining = [ro(det.bondingCurve), ro(det.canonicalPool)];
    if (pool) remaining.push(ro(pool.pool), ro(pool.baseVault), ro(pool.quoteVault));
    await send(creator, [{ ...ix, accounts: [...ix.accounts, ...remaining] }]);
}

async function deposit(
    c: Ctx,
    owner: KeyPairSigner,
    mint: Address,
    tokenProgram: Address,
    source: Address,
    amount: bigint,
): Promise<void> {
    const ix = await getDepositInstructionAsync({
        owner,
        mint,
        ownerTokenAccount: source,
        tokenProgram,
        eventAuthority: c.eventAuthority,
        program: SHORTWIN_PROGRAM_ADDRESS,
        amount,
        minShares: 0n,
    });
    await send(owner, [ix]);
}

/**
 * A mock market: mint, pool with `base` tokens against `quoteSol` SOL, listing (by the seed lender),
 * and the first deposit of `lend` tokens by `lender` (default: the seed lender).
 */
async function mockMarket(
    c: Ctx,
    label: { symbol: string; name: string },
    base: bigint,
    quoteSol: bigint,
    lend: bigint,
    lender: KeyPairSigner = c.seedLender,
): Promise<MarketInfo> {
    const mint = await createSplMint(c);
    const k = await marketKeys(mint, PROGRAMS.token);
    let pool: MockPool | null = null;
    if (ENV.program === 'test-venue') {
        pool = await derivePool(mint, PROGRAMS.token);
        const feePpm = 2_500;
        await send(c.payer, [initializePoolIx(pool, c.poolAdmin.address, feePpm)], { signers: [c.poolAdmin] });
        const baseSrc = await ensureAta(c, c.poolAdmin.address, mint, PROGRAMS.token);
        await mintTo(c, mint, baseSrc, PROGRAMS.token, base);
        const quoteSrc = await wrapSol(c, c.poolAdmin, quoteSol * LAMPORTS_PER_SOL);
        await send(
            c.payer,
            [addLiquidityIx(pool, c.poolAdmin.address, baseSrc, quoteSrc, base, quoteSol * LAMPORTS_PER_SOL)],
            { signers: [c.poolAdmin] },
        );
    }
    await createMarket(c, c.seedLender, mint, PROGRAMS.token, pool);
    const lenderSrc = await ensureAta(c, lender.address, mint, PROGRAMS.token);
    await mintTo(c, mint, lenderSrc, PROGRAMS.token, lend);
    await deposit(c, lender, mint, PROGRAMS.token, lenderSrc, lend);
    log(`market ${label.symbol} ${k.market} (mint ${mint}${pool ? `, mock pool ${pool.pool}` : ', list-only'})`);
    return {
        ...label,
        mint,
        tokenProgram: PROGRAMS.token,
        decimals: DECIMALS,
        market: k.market,
        marketVault: k.marketVault,
        poolVault: k.poolVault,
        trader: k.trader,
        traderWsol: k.traderWsol,
        venue: pool ? 'mock' : 'none',
        pool: pool
            ? {
                  pool: pool.pool,
                  baseVault: pool.baseVault,
                  quoteVault: pool.quoteVault,
                  authority: pool.authority,
                  feePpm: 2_500,
              }
            : null,
        creator: c.seedLender.address,
    };
}

/** The mock market's trading remaining accounts: ReadSet (W) ++ TradeExtras (venues/mock.rs). */
export function mockTradeAccounts(m: MarketInfo): AccountMeta[] {
    if (!m.pool) throw new Error(`${m.symbol} has no mock pool`);
    const w = (address: Address): AccountMeta => ({ address, role: AccountRole.WRITABLE });
    const r = (address: Address): AccountMeta => ({ address, role: AccountRole.READONLY });
    return [
        w(m.pool.pool),
        w(m.pool.baseVault),
        w(m.pool.quoteVault),
        r(PROGRAMS.mock),
        r(m.pool.authority),
        r(WSOL_MINT),
        r(PROGRAMS.token),
        w(m.traderWsol),
    ];
}

/** `open_short` from Node with generous bounds (the seeded positions; the specs open through the UI). */
export async function openShortFromNode(
    borrower: KeyPairSigner,
    m: MarketInfo,
    q: bigint,
    nonce: bigint,
): Promise<PositionInfo> {
    const ix = await getOpenShortInstructionAsync({
        borrower,
        mint: m.mint,
        trader: m.trader,
        tokenProgram: m.tokenProgram,
        eventAuthority: await eventAuthority(),
        program: SHORTWIN_PROGRAM_ADDRESS,
        nonce,
        q,
        minProceeds: 0n,
        maxMargin: U64_MAX,
        maxOpenFee: U64_MAX,
        extras: { set: 0, bumps: new Uint8Array(4) },
    });
    await send(borrower, [{ ...ix, accounts: [...ix.accounts, ...mockTradeAccounts(m)] }], { computeUnits: 1_000_000 });
    const position = ix.accounts[5]?.address;
    if (!position) throw new Error('open_short without a position account');
    return {
        position,
        market: m.market,
        mint: m.mint,
        borrower: borrower.address,
        nonce: nonce.toString(),
        q: q.toString(),
    };
}

// ---- the scenario ----------------------------------------------------------------------------

export async function seed(): Promise<E2EState> {
    const t0 = Date.now();
    const c: Ctx = {
        payer: await signerFor('payer'),
        mintAuthority: await signerFor('mintAuthority'),
        poolAdmin: await signerFor('poolAdmin'),
        seedLender: await signerFor('seedLender'),
        eventAuthority: await eventAuthority(),
    };
    const w = async (n: WalletName) => signerFor(n);

    await initializeConfig();

    // Markets: three mock markets (list-only on a release-build run) and a list-only Token-2022 one.
    const main = await mockMarket(
        c,
        { symbol: 'E2EMAIN', name: 'ShortWin E2E Main' },
        1_000_000_000n * TOKEN,
        1_000n,
        100_000_000n * TOKEN,
    );
    const move = await mockMarket(
        c,
        { symbol: 'E2EMOVE', name: 'ShortWin E2E Mover' },
        100_000_000n * TOKEN,
        100n,
        10_000_000n * TOKEN,
    );
    const paused = await mockMarket(
        c,
        { symbol: 'E2EPAUSE', name: 'ShortWin E2E Paused' },
        100_000_000n * TOKEN,
        100n,
        10_000_000n * TOKEN,
    );
    // FE-18: lent by one spec wallet each. `wipe` at `move`'s depth (1e-6 SOL a token); `pot` ten
    // times dearer and ten times deeper (its wallet cap admits a 5 SOL position), its lender's 600k tokens
    // mostly borrowed (high utilisation: rent accrues fast).
    const wipe = await mockMarket(
        c,
        { symbol: 'E2EWIPE', name: 'ShortWin E2E Wipe' },
        100_000_000n * TOKEN,
        100n,
        10_000_000n * TOKEN,
        await w('wiped'),
    );
    const pot = await mockMarket(
        c,
        { symbol: 'E2EPOT', name: 'ShortWin E2E Pot' },
        1_000_000_000n * TOKEN,
        10_000n,
        600_000n * TOKEN,
        await w('potClaimer'),
    );
    const createdAt = (await clock()).unixTimestamp;

    const t22Mint = await createT22Mint(c, (mint) => ({
        fixed: [{ __kind: 'MetadataPointer', authority: c.mintAuthority.address, metadataAddress: mint }],
        variable: [
            {
                __kind: 'TokenMetadata',
                updateAuthority: c.mintAuthority.address,
                mint,
                name: 'ShortWin E2E Token-2022',
                symbol: 'E2ET22',
                uri: '',
                additionalMetadata: new Map(),
            },
        ],
    }));
    await createMarket(c, c.seedLender, t22Mint, PROGRAMS.token2022, null);
    const t22Keys = await marketKeys(t22Mint, PROGRAMS.token2022);
    const seedT22 = await ensureAta(c, c.seedLender.address, t22Mint, PROGRAMS.token2022);
    await mintTo(c, t22Mint, seedT22, PROGRAMS.token2022, 1_000_000n * TOKEN);
    await deposit(c, c.seedLender, t22Mint, PROGRAMS.token2022, seedT22, 1_000_000n * TOKEN);
    const t22: MarketInfo = {
        symbol: 'E2ET22',
        name: 'ShortWin E2E Token-2022',
        mint: t22Mint,
        tokenProgram: PROGRAMS.token2022,
        decimals: DECIMALS,
        market: t22Keys.market,
        marketVault: t22Keys.marketVault,
        poolVault: t22Keys.poolVault,
        trader: t22Keys.trader,
        traderWsol: t22Keys.traderWsol,
        venue: 'none',
        pool: null,
        creator: c.seedLender.address,
    };
    log(`market E2ET22 ${t22.market} (Token-2022 mint ${t22Mint}, list-only)`);

    // Coverage pause on `paused` (O17; the generated coverage authority).
    const coverage = await signerFor('coverage');
    await send(c.payer, [
        await getSetCoveragePauseInstructionAsync({
            coverageAuthority: coverage,
            market: paused.market,
            eventAuthority: c.eventAuthority,
            program: SHORTWIN_PROGRAM_ADDRESS,
            paused: true,
            reason: COVERAGE_REASON_KEEPER,
        }),
    ]);
    log('coverage pause set on E2EPAUSE');

    // Wallet token accounts.
    const mainAta: Partial<Record<WalletName, Address>> = {};
    for (const n of ['lender', 'keyboard', 'errors', 'claimer', 'trader'] as const) {
        const a = await ensureAta(c, (await w(n)).address, main.mint, PROGRAMS.token);
        await mintTo(c, main.mint, a, PROGRAMS.token, (n === 'claimer' ? 200_000_000n : 1_000_000n) * TOKEN);
        mainAta[n] = a;
    }
    // The claimer lends into `main` before any position opens (rent accrues to it).
    const claimer = await w('claimer');
    await deposit(c, claimer, main.mint, PROGRAMS.token, mainAta.claimer as Address, 200_000_000n * TOKEN);

    const guard = await w('guard');
    const guardAta = await ensureAta(c, guard.address, t22Mint, PROGRAMS.token2022);
    await mintTo(c, t22Mint, guardAta, PROGRAMS.token2022, 1_000n * TOKEN);
    const guarded = await createT22Account(c, guard, t22Mint, [{ __kind: 'CpiGuard', lockCpi: true }], 1_000n * TOKEN);
    const guardRace = await createT22Account(
        c,
        guard,
        t22Mint,
        [{ __kind: 'CpiGuard', lockCpi: false }],
        1_000n * TOKEN,
    );

    const memoAccounts = async (n: 'memo' | 'memoRace') => {
        const owner = await w(n);
        const source = await createT22Account(c, owner, t22Mint, [], 10_000n * TOKEN);
        await deposit(c, owner, t22Mint, PROGRAMS.token2022, source, 5_000n * TOKEN);
        const a = await memoAta(c, owner, t22Mint, n === 'memo');
        return { ata: a, source };
    };
    const memo = await memoAccounts('memo');
    const memoRace = await memoAccounts('memoRace');

    const errorsW = await w('errors');
    const errorsT22 = await ensureAta(c, errorsW.address, t22Mint, PROGRAMS.token2022);
    await mintTo(c, t22Mint, errorsT22, PROGRAMS.token2022, 1_000n * TOKEN);
    const errorsRace = await createT22Account(
        c,
        errorsW,
        t22Mint,
        [{ __kind: 'CpiGuard', lockCpi: false }],
        1_000n * TOKEN,
    );
    // FE-18: the wiped market's only lender withdraws into its ATA; `rejoin` and `potLate` lend later.
    const wipeAccounts = {
        lender: await ata((await w('wiped')).address, wipe.mint, PROGRAMS.token),
        rejoin: await ensureAta(c, (await w('rejoin')).address, wipe.mint, PROGRAMS.token),
    };
    await mintTo(c, wipe.mint, wipeAccounts.rejoin, PROGRAMS.token, 1_000_000n * TOKEN);
    const potAccounts = { late: await ensureAta(c, (await w('potLate')).address, pot.mint, PROGRAMS.token) };
    await mintTo(c, pot.mint, potAccounts.late, PROGRAMS.token, 5_400_000n * TOKEN);
    log('wallet token accounts ready');

    // create-market mints (§3.5): two safe, one with a freeze authority, one with a disallowed extension.
    const createMints = {
        ok: await createSplMint(c),
        listable: await createSplMint(c),
        freeze: await createSplMint(c, c.mintAuthority.address),
        badExtension: await createT22Mint(c, () => ({
            fixed: [{ __kind: 'PermanentDelegate', delegate: c.mintAuthority.address }],
        })),
    };

    // The protocol ALT: A28's static accounts plus the seeded markets' (FRONTEND.md §5.2).
    const altAddresses: Address[] = [
        PROGRAMS.shortwin,
        PROGRAMS.system,
        PROGRAMS.token,
        PROGRAMS.token2022,
        PROGRAMS.ata,
        PROGRAMS.memo,
        PROGRAMS.computeBudget,
        WSOL_MINT,
        c.eventAuthority,
    ];
    for (const seedName of ['config', 'venue_guard', 'sol_usd'])
        altAddresses.push(await pda(PROGRAMS.shortwin, [seedName]));
    if (ENV.program === 'test-venue') altAddresses.push(PROGRAMS.mock);
    for (const m of [main, move, paused, t22, wipe, pot]) {
        altAddresses.push(m.market, m.marketVault, m.poolVault, m.trader, m.traderWsol, m.mint);
        if (m.pool) altAddresses.push(m.pool.pool, m.pool.baseVault, m.pool.quoteVault, m.pool.authority);
    }
    const protocolAlt = await createAlt(c, altAddresses);
    log(`protocol ALT ${protocolAlt} (${new Set(altAddresses).size} addresses)`);

    // Seeded positions, once the markets are past `market_warmup_s`.
    let positions: E2EState['positions'] = null;
    if (ENV.program === 'test-venue') {
        log(`waiting for the ${MARKET_WARMUP_S}s market warm-up…`);
        await waitForClock(createdAt + MARKET_WARMUP_S + 2n);
        positions = {
            margin: await openShortFromNode(await w('margin'), main, 2_000_000n * TOKEN, 1n),
            closer: await openShortFromNode(await w('closer'), move, 300_000n * TOKEN, 1n),
            holder: await openShortFromNode(await w('holder'), move, 300_000n * TOKEN, 1n),
            // ≈ 0.3 SOL of notional at 3 % utilisation (as `move`'s: inside the wallet cap of a
            // 100M-token pool, ≈ 469k tokens); ≈ 5 SOL at 83 %.
            wipe: await openShortFromNode(await w('wipeBorrower'), wipe, 300_000n * TOKEN, 1n),
            pot: await openShortFromNode(await w('potBorrower'), pot, 500_000n * TOKEN, 1n),
        };
        log('seeded positions open');
    }

    const venueMints = ENV.venues
        ? {
              pumpCurve: fixtureAddress('pump_fresh_mint'),
              pumpSwap: fixtureAddress('pamm_mature_mint'),
              raydiumV4: fixtureAddress('mint_fartcoin'),
              raydiumCpmm: fixtureAddress('cpmm_useless_token_mint'),
          }
        : null;
    if (venueMints) {
        for (const [k, v] of Object.entries(venueMints)) {
            if (!(await getAccount(v))) throw new Error(`fixture mint ${k} ${v} missing on the validator`);
        }
    }

    const wallets = Object.fromEntries(
        await Promise.all(
            KEY_NAMES.wallets.map(async (n) => [n, { address: (await signerFor(n)).address, keyPath: keyPath(n) }]),
        ),
    ) as E2EState['wallets'];
    const admin = Object.fromEntries(
        await Promise.all(
            (
                [
                    'authority',
                    'governance',
                    'pause',
                    'coverage',
                    'feeRecipient',
                    'payer',
                    'mintAuthority',
                    'poolAdmin',
                    'seedLender',
                ] as const
            ).map(async (n) => [n, (await signerFor(n)).address]),
        ),
    ) as E2EState['admin'];

    const state: E2EState = {
        version: 1,
        createdAt: new Date().toISOString(),
        program: ENV.program,
        venues: ENV.venues,
        protocolAlt,
        admin,
        wallets,
        markets: { main, move, paused, t22, wipe, pot },
        createMints,
        positions,
        tokenAccounts: {
            mainAta,
            guard: { ata: guardAta, guarded, race: guardRace },
            memo,
            memoRace,
            errors: { t22Ata: errorsT22, race: errorsRace },
            wipe: wipeAccounts,
            pot: potAccounts,
        },
        venueMints,
    };
    mkdirSync(E2E_DIR, { recursive: true });
    writeFileSync(PATHS.state, JSON.stringify(state, null, 2));
    log(`state written to .e2e/state.json in ${((Date.now() - t0) / 1000).toFixed(1)}s`);
    // Let the ALT's last extension become usable (one slot) before any client compiles against it.
    await sleep(1_000);
    return state;
}

if (import.meta.url === `file://${process.argv[1]}`) {
    seed().then(
        () => process.exit(0),
        (e: unknown) => {
            console.error(e);
            process.exit(1);
        },
    );
}
