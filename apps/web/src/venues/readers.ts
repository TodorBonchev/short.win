/**
 * Venue-state readers: a TypeScript port of `crates/shortwin-venue-state` (the readers the
 * program, the keeper and this frontend share bit for bit). Length-tolerant, bytes in / values
 * out: every reader checks the owner, the length and (where one exists) the discriminator, and
 * zero-pads reads past the end of a shorter account (ARCH §4.1). A reader failure on a correctly
 * keyed venue account means "not executable" (A63), never a revert; key checks are the caller's.
 *
 * `readers.test.ts` holds every reader to the dumped mainnet fixtures of `tests/fixtures/accounts`
 * and the decoded values `tests/fixtures/manifest.json` records for them.
 */
import type { Address } from '@solana/kit';
import { boolAt, DEFAULT_KEY, hasDiscriminator, i128At, keyAt, u8At, u16At, u32At, u64At, u128At } from './bytes.ts';
import {
    MOCK_CPAMM_PROGRAM,
    PUMP_AMM_PROGRAM,
    PUMP_FEES_PROGRAM,
    PUMP_PROGRAM,
    RAYDIUM_CPMM_PROGRAM,
    RAYDIUM_V4_PROGRAM,
    TOKEN_2022_PROGRAM,
    TOKEN_PROGRAM,
    WSOL_MINT,
} from './constants.ts';

export type ReadError = 'WrongOwner' | 'TooShort' | 'WrongTag' | 'Malformed';
export type Read<T> = { readonly ok: true; readonly value: T } | { readonly ok: false; readonly error: ReadError };

const ok = <T>(value: T): Read<T> => ({ ok: true, value });
const err = <T>(error: ReadError): Read<T> => ({ ok: false, error });

/** An account as the RPC returned it. `null` for an address with no account. */
export interface RawAccount {
    readonly address: Address;
    readonly owner: Address;
    readonly lamports: bigint;
    readonly executable: boolean;
    readonly data: Uint8Array;
}

/** The value of a successful read, else `null`. */
export function value<T>(r: Read<T>): T | null {
    return r.ok ? r.value : null;
}

// ---- token accounts and mints (token_account.rs, mint.rs) --------------------------------------

export const TOKEN_ACCOUNT_LEN = 165;
export const MINT_LEN = 82;

export interface TokenAccountState {
    readonly mint: Address;
    /** The token authority (`owner` field). */
    readonly owner: Address;
    readonly amount: bigint;
    readonly delegate: Address | null;
    readonly delegatedAmount: bigint;
    readonly closeAuthority: Address | null;
    readonly isNative: bigint | null;
    readonly frozen: boolean;
}

function coptionKey(d: Uint8Array, off: number): Read<Address | null> {
    const tag = u32At(d, off);
    return tag === 0 ? ok(null) : tag === 1 ? ok(keyAt(d, off + 4)) : err('Malformed');
}

function coptionU64(d: Uint8Array, off: number): Read<bigint | null> {
    const tag = u32At(d, off);
    return tag === 0 ? ok(null) : tag === 1 ? ok(u64At(d, off + 4)) : err('Malformed');
}

/** An initialized SPL Token or Token-2022 token account (`token_account::read`). */
export function readTokenAccount(programOwner: Address, d: Uint8Array): Read<TokenAccountState> {
    if (programOwner !== TOKEN_PROGRAM && programOwner !== TOKEN_2022_PROGRAM) return err('WrongOwner');
    if (d.length < TOKEN_ACCOUNT_LEN) return err('TooShort');
    if (d.length > TOKEN_ACCOUNT_LEN && (programOwner !== TOKEN_2022_PROGRAM || u8At(d, TOKEN_ACCOUNT_LEN) !== 2)) {
        return err('WrongTag');
    }
    const state = u8At(d, 108);
    if (state !== 1 && state !== 2) return err('Malformed');
    const delegate = coptionKey(d, 72);
    const close = coptionKey(d, 129);
    const native = coptionU64(d, 109);
    if (!delegate.ok || !close.ok || !native.ok) return err('Malformed');
    return ok({
        mint: keyAt(d, 0),
        owner: keyAt(d, 32),
        amount: u64At(d, 64),
        delegate: delegate.value,
        delegatedAmount: u64At(d, 121),
        closeAuthority: close.value,
        isNative: native.value,
        frozen: state === 2,
    });
}

export interface MintBase {
    readonly mintAuthority: Address | null;
    readonly supply: bigint;
    readonly decimals: number;
    readonly freezeAuthority: Address | null;
}

/** The base of an initialized SPL Token or Token-2022 mint (`mint::read_base`). */
export function readMintBase(programOwner: Address, d: Uint8Array): Read<MintBase> {
    if (programOwner !== TOKEN_PROGRAM && programOwner !== TOKEN_2022_PROGRAM) return err('WrongOwner');
    if (d.length < MINT_LEN) return err('TooShort');
    const mintAuthority = coptionKey(d, 0);
    const init = u8At(d, 45);
    const freeze = coptionKey(d, 46);
    if (!mintAuthority.ok || !freeze.ok || init !== 1) return err('Malformed');
    return ok({
        mintAuthority: mintAuthority.value,
        supply: u64At(d, 36),
        decimals: u8At(d, 44),
        freezeAuthority: freeze.value,
    });
}

/** A token account of `program` (the raydium adapters' `token_of`). */
export function tokenOf(acc: RawAccount | null, program: Address): TokenAccountState | null {
    if (!acc || acc.owner !== program) return null;
    return value(readTokenAccount(acc.owner, acc.data));
}

// ---- pump (pump.rs) ------------------------------------------------------------------------------

export const BONDING_CURVE_DISCRIMINATOR = [0x17, 0xb7, 0xf8, 0x37, 0x60, 0xd8, 0xac, 0x60];
export const PUMP_GLOBAL_DISCRIMINATOR = [0xa7, 0xe8, 0xe8, 0xb1, 0xc8, 0x6c, 0x72, 0x7f];
export const FEE_CONFIG_DISCRIMINATOR = [0x8f, 0x34, 0x92, 0xbb, 0xdb, 0x7b, 0x4c, 0x9b];
export const USER_VOLUME_ACCUMULATOR_DISCRIMINATOR = [0x56, 0xff, 0x70, 0x0e, 0x66, 0x35, 0x9a, 0xfa];
export const PUMP_GLOBAL_LEN = 1_087;

export interface BondingCurve {
    readonly vt: bigint;
    readonly vq: bigint;
    readonly rt: bigint;
    readonly rq: bigint;
    readonly tokenTotalSupply: bigint;
    readonly complete: boolean;
    readonly creator: Address;
    readonly isMayhemMode: boolean;
    readonly isCashbackCoin: boolean;
    /** `DEFAULT_KEY` for a SOL curve. */
    readonly quoteMint: Address;
    readonly creatorFeeBps: bigint;
    readonly canEditCreatorFee: boolean;
    readonly isHolderReward: boolean;
    /** `creator_fee` @125: the creator-fee bucket (v3 trades; outside `rQ`). */
    readonly creatorFeeBucket: bigint;
    /** `protocol_fees` @133: the protocol-fee bucket (v3 trades; outside `rQ`). */
    readonly protocolFeesBucket: bigint;
    /** `depth` @141: 0 for a SOL / USDC curve (a plain `u8`, not a bool). */
    readonly depth: number;
    /** `initial_virtual_quote_reserves` @142 (`ivqr`): `vQ − rQ` of a `create_v2` curve, 0 on a legacy one. */
    readonly initialVirtualQuoteReserves: bigint;
    /** `post_complete_base_out` @150 (`pcB`): what the completing buy took from the would-be pool. */
    readonly postCompleteBaseOut: bigint;
    /** `post_complete_quote_in` @158 (`pcQ`): what it paid into it; `migrate_v2` adds it to the pool. */
    readonly postCompleteQuoteIn: bigint;
}

/**
 * pump's invariant `vQ − rQ == initial_virtual_quote_reserves` (`BondingCurve::
 * quote_offset_consistent`): vacuous when `ivqr` is 0 (a legacy curve) or `vT` is 0 (migrated).
 */
export function quoteOffsetConsistent(c: BondingCurve): boolean {
    return (
        c.initialVirtualQuoteReserves === 0n ||
        c.vt === 0n ||
        (c.vq >= c.rq && c.vq - c.rq === c.initialVirtualQuoteReserves)
    );
}

/**
 * pump `BondingCurve` (`pump::read_curve`): owner pump, discriminator, every bool 0 / 1; the fields
 * of the 166-byte layout (2026-10-08: @125–@158) read 0 past the end of a shorter curve.
 */
export function readCurve(owner: Address, d: Uint8Array): Read<BondingCurve> {
    if (owner !== PUMP_PROGRAM) return err('WrongOwner');
    if (!hasDiscriminator(d, BONDING_CURVE_DISCRIMINATOR)) return err('WrongTag');
    const bools = [48, 81, 82, 123, 124].map((o) => boolAt(d, o));
    if (bools.some((b) => b === null)) return err('Malformed');
    const [complete, mayhem, cashback, canEdit, holder] = bools as boolean[];
    return ok({
        vt: u64At(d, 8),
        vq: u64At(d, 16),
        rt: u64At(d, 24),
        rq: u64At(d, 32),
        tokenTotalSupply: u64At(d, 40),
        complete: complete as boolean,
        creator: keyAt(d, 49),
        isMayhemMode: mayhem as boolean,
        isCashbackCoin: cashback as boolean,
        quoteMint: keyAt(d, 83),
        creatorFeeBps: u64At(d, 115),
        canEditCreatorFee: canEdit as boolean,
        isHolderReward: holder as boolean,
        creatorFeeBucket: u64At(d, 125),
        protocolFeesBucket: u64At(d, 133),
        depth: u8At(d, 141),
        initialVirtualQuoteReserves: u64At(d, 142),
        postCompleteBaseOut: u64At(d, 150),
        postCompleteQuoteIn: u64At(d, 158),
    });
}

const keysAt = (d: Uint8Array, off: number): Address[] => Array.from({ length: 8 }, (_, i) => keyAt(d, off + 32 * i));
const headAndRest = (d: Uint8Array, head: number, rest: number): Address[] => [
    keyAt(d, head),
    ...Array.from({ length: 7 }, (_, j) => keyAt(d, rest + 32 * j)),
];

export interface PumpGlobal {
    /** The 8 normal fee recipients: `fee_recipient` @41 then `fee_recipients[7]` @162. */
    readonly feeRecipients: readonly Address[];
    /** `buyback_fee_recipients[8]` @741. */
    readonly buybackFeeRecipients: readonly Address[];
    /** `pool_migration_fee` @146. */
    readonly poolMigrationFee: bigint;
    /** `creator_fee_configurable` @1045. */
    readonly creatorFeeConfigurable: boolean;
}

/** pump `Global` (`pump::read_global`): owner, discriminator, ≥ 1,087 bytes, every bool 0 / 1. */
export function readPumpGlobal(owner: Address, d: Uint8Array): Read<PumpGlobal> {
    if (owner !== PUMP_PROGRAM) return err('WrongOwner');
    if (!hasDiscriminator(d, PUMP_GLOBAL_DISCRIMINATOR)) return err('WrongTag');
    if (d.length < PUMP_GLOBAL_LEN) return err('TooShort');
    for (const o of [8, 145, 450, 515, 740, 1045, 1086]) if (boolAt(d, o) === null) return err('Malformed');
    return ok({
        feeRecipients: headAndRest(d, 41, 162),
        buybackFeeRecipients: keysAt(d, 741),
        poolMigrationFee: u64At(d, 146),
        creatorFeeConfigurable: u8At(d, 1045) === 1,
    });
}

export interface FeeTier {
    readonly threshold: bigint;
    readonly lp: number;
    readonly protocol: number;
    readonly creator: number;
}

export interface FeeConfig {
    /** SOL-like quotes: the curve's schedule and a canonical SOL PumpSwap pool's. */
    readonly feeTiers: readonly FeeTier[];
}

/** pump-fees `FeeConfig` (`pump::read_fee_config`, pump's or PumpSwap's): both tier vectors inside the account. */
export function readFeeConfig(owner: Address, d: Uint8Array): Read<FeeConfig> {
    if (owner !== PUMP_FEES_PROGRAM) return err('WrongOwner');
    if (!hasDiscriminator(d, FEE_CONFIG_DISCRIMINATOR)) return err('WrongTag');
    const tiersAt = (off: number): Read<{ tiers: FeeTier[]; next: number }> => {
        const len = u32At(d, off);
        const start = off + 4;
        const end = start + len * 40;
        if (end > d.length) return err('TooShort');
        const tiers: FeeTier[] = [];
        for (let i = 0; i < len; i++) {
            const t = start + 40 * i;
            tiers.push({
                threshold: u128At(d, t),
                lp: Number(u64At(d, t + 16)),
                protocol: Number(u64At(d, t + 24)),
                creator: Number(u64At(d, t + 32)),
            });
        }
        return ok({ tiers, next: end });
    };
    const normal = tiersAt(65);
    if (!normal.ok) return err(normal.error);
    const stable = tiersAt(normal.value.next);
    if (!stable.ok) return err(stable.error);
    if (d.length < stable.value.next + 24) return err('TooShort');
    return ok({ feeTiers: normal.value.tiers });
}

export interface UserVolumeAccumulator {
    readonly user: Address;
    readonly needsClaim: boolean;
    readonly cashbackEarned: bigint;
}

/** A pump or PumpSwap `UserVolumeAccumulator` (`program` its owner). */
export function readUserVolumeAccumulator(
    owner: Address,
    d: Uint8Array,
    program: Address = PUMP_PROGRAM,
): Read<UserVolumeAccumulator> {
    if (owner !== program) return err('WrongOwner');
    if (!hasDiscriminator(d, USER_VOLUME_ACCUMULATOR_DISCRIMINATOR)) return err('WrongTag');
    const needsClaim = boolAt(d, 40);
    if (needsClaim === null) return err('Malformed');
    return ok({ user: keyAt(d, 8), needsClaim, cashbackEarned: u64At(d, 74) });
}

// ---- PumpSwap (pump_amm.rs) ----------------------------------------------------------------------

export const PAMM_POOL_DISCRIMINATOR = [0xf1, 0x9a, 0x6d, 0x04, 0x11, 0xb1, 0x6d, 0xbc];
export const PAMM_GLOBAL_CONFIG_DISCRIMINATOR = [0x95, 0x08, 0x9c, 0xca, 0xa0, 0xfc, 0xb0, 0xd9];
export const PAMM_GLOBAL_CONFIG_LEN = 949;

export interface PammPool {
    readonly index: number;
    readonly creator: Address;
    readonly baseMint: Address;
    readonly quoteMint: Address;
    readonly lpMint: Address;
    readonly poolBaseTokenAccount: Address;
    readonly poolQuoteTokenAccount: Address;
    readonly lpSupply: bigint;
    readonly coinCreator: Address;
    readonly isMayhemMode: boolean;
    readonly isCashbackCoin: boolean;
    /** Signed (negative from 2026-09-30). */
    readonly virtualQuoteReserves: bigint;
    readonly creatorFeeBps: bigint;
    /**
     * The fee buckets of pump's 2026-10-02 redeploy (`protocol_fees` @271, `creator_fees` @279; 0 on
     * a pool shorter than 287 bytes): v2 trades' fees kept in the quote vault and off the VQR, never
     * paid out by a sale (PUMP-2026-10-1).
     */
    readonly protocolFees: bigint;
    readonly creatorFees: bigint;
}

/** PumpSwap `Pool` (`pump_amm::read_pool`): owner, discriminator, every bool 0 / 1; zero-padded. */
export function readPammPool(owner: Address, d: Uint8Array): Read<PammPool> {
    if (owner !== PUMP_AMM_PROGRAM) return err('WrongOwner');
    if (!hasDiscriminator(d, PAMM_POOL_DISCRIMINATOR)) return err('WrongTag');
    const bools = [243, 244, 269, 270].map((o) => boolAt(d, o));
    if (bools.some((b) => b === null)) return err('Malformed');
    return ok({
        index: u16At(d, 9),
        creator: keyAt(d, 11),
        baseMint: keyAt(d, 43),
        quoteMint: keyAt(d, 75),
        lpMint: keyAt(d, 107),
        poolBaseTokenAccount: keyAt(d, 139),
        poolQuoteTokenAccount: keyAt(d, 171),
        lpSupply: u64At(d, 203),
        coinCreator: keyAt(d, 211),
        isMayhemMode: bools[0] as boolean,
        isCashbackCoin: bools[1] as boolean,
        virtualQuoteReserves: i128At(d, 245),
        creatorFeeBps: u64At(d, 261),
        protocolFees: u64At(d, PAMM_POOL_OFF_PROTOCOL_FEES),
        creatorFees: u64At(d, PAMM_POOL_OFF_CREATOR_FEES),
    });
}

export interface PammReserves {
    /** `X`: the base vault. */
    readonly base: bigint;
    /** `Y`: the WSOL vault + `virtual_quote_reserves`. */
    readonly effectiveQuote: bigint;
    /** The WSOL vault. */
    readonly realQuote: bigint;
    /** `R`'s vault part: the WSOL vault less the fee buckets, what a sale can be paid out of. */
    readonly payableQuote: bigint;
}

const U64_MAX = (1n << 64n) - 1n;

/** `Pool.protocol_fees` (u64; pump's 2026-10-02 redeploy). */
export const PAMM_POOL_OFF_PROTOCOL_FEES = 271;
/** `Pool.creator_fees` (u64). */
export const PAMM_POOL_OFF_CREATOR_FEES = 279;

/**
 * `Pool::reserves`: `None` when `quote vault + VQR` is negative or above `u64`, or when the fee
 * buckets exceed the quote vault (PUMP-2026-10-1: pump's invariant broken, not executable).
 */
export function pammReserves(pool: PammPool, baseAmount: bigint, quoteAmount: bigint): PammReserves | null {
    const effective = quoteAmount + pool.virtualQuoteReserves;
    if (effective < 0n || effective > U64_MAX) return null;
    const buckets = pool.protocolFees + pool.creatorFees;
    if (buckets > U64_MAX || buckets > quoteAmount) return null;
    return { base: baseAmount, effectiveQuote: effective, realQuote: quoteAmount, payableQuote: quoteAmount - buckets };
}

/**
 * `Pool::virtual_part`: the virtual part of the effective reserve the tier subtracts, the boost
 * `max(VQR + buckets, 0)` (the buckets come off the VQR; PUMP-2026-10-1); `null` past `u64`.
 */
export function pammVirtualPart(pool: PammPool): bigint | null {
    const buckets = pool.protocolFees + pool.creatorFees;
    if (buckets > U64_MAX) return null;
    const v = pool.virtualQuoteReserves + buckets;
    if (v <= 0n) return 0n;
    return v > U64_MAX ? null : v;
}

export interface PammGlobalConfig {
    readonly disableFlags: number;
    readonly protocolFeeRecipients: readonly Address[];
    readonly buybackFeeRecipients: readonly Address[];
    readonly creatorFeeConfigurable: boolean;
}

/** PumpSwap `GlobalConfig` (`pump_amm::read_global_config`): owner, discriminator, ≥ 949 bytes, bools. */
export function readPammGlobalConfig(owner: Address, d: Uint8Array): Read<PammGlobalConfig> {
    if (owner !== PUMP_AMM_PROGRAM) return err('WrongOwner');
    if (!hasDiscriminator(d, PAMM_GLOBAL_CONFIG_DISCRIMINATOR)) return err('WrongTag');
    if (d.length < PAMM_GLOBAL_CONFIG_LEN) return err('TooShort');
    for (const o of [417, 642, 939, 940]) if (boolAt(d, o) === null) return err('Malformed');
    return ok({
        disableFlags: u8At(d, 56),
        protocolFeeRecipients: keysAt(d, 57),
        buybackFeeRecipients: keysAt(d, 643),
        creatorFeeConfigurable: u8At(d, 940) === 1,
    });
}

/**
 * The boost vault ATA's WSOL (`pump_amm::boost_vault_amount`): 0 for a missing account, else an
 * SPL Token account of WSOL owned by `boostVault`.
 */
export function boostVaultAmount(acc: RawAccount | null, boostVault: Address): Read<bigint> {
    if (!acc || acc.data.length === 0) return ok(0n);
    if (acc.owner !== TOKEN_PROGRAM) return err('WrongOwner');
    const t = readTokenAccount(acc.owner, acc.data);
    if (!t.ok) return err(t.error);
    if (t.value.mint !== WSOL_MINT || t.value.owner !== boostVault) return err('Malformed');
    return ok(t.value.amount);
}

// ---- Raydium AMM v4 (raydium_v4.rs) --------------------------------------------------------------

export const AMM_V4_LEN = 752;

export interface AmmInfo {
    readonly status: bigint;
    readonly nonce: bigint;
    readonly swapFeeNumerator: bigint;
    readonly swapFeeDenominator: bigint;
    readonly needTakePnlCoin: bigint;
    readonly needTakePnlPc: bigint;
    readonly poolOpenTime: bigint;
    readonly coinVault: Address;
    readonly pcVault: Address;
    readonly coinVaultMint: Address;
    readonly pcVaultMint: Address;
    readonly lpMint: Address;
    readonly lpAmount: bigint;
}

/** Raydium AMM v4 `AmmInfo` (`raydium_v4::read`): owner and exactly 752 bytes. */
export function readAmmInfo(owner: Address, d: Uint8Array): Read<AmmInfo> {
    if (owner !== RAYDIUM_V4_PROGRAM) return err('WrongOwner');
    if (d.length < AMM_V4_LEN) return err('TooShort');
    if (d.length > AMM_V4_LEN) return err('WrongTag');
    return ok({
        status: u64At(d, 0),
        nonce: u64At(d, 8),
        swapFeeNumerator: u64At(d, 176),
        swapFeeDenominator: u64At(d, 184),
        needTakePnlCoin: u64At(d, 192),
        needTakePnlPc: u64At(d, 200),
        poolOpenTime: u64At(d, 224),
        coinVault: keyAt(d, 336),
        pcVault: keyAt(d, 368),
        coinVaultMint: keyAt(d, 400),
        pcVaultMint: keyAt(d, 432),
        lpMint: keyAt(d, 464),
        lpAmount: u64At(d, 720),
    });
}

/** `AmmInfo::sol_is_coin`: `true` if the coin is WSOL, `false` if the pc is, `null` otherwise. */
export function v4SolIsCoin(i: AmmInfo): boolean | null {
    const c = i.coinVaultMint === WSOL_MINT;
    const p = i.pcVaultMint === WSOL_MINT;
    return c && !p ? true : p && !c ? false : null;
}

const reached = (now: bigint, t: bigint): boolean => now >= 0n && now >= t;

/** `AmmInfo::swap_enabled`: status 1 / 6, or 7 from `pool_open_time`. */
export function v4SwapEnabled(i: AmmInfo, now: bigint): boolean {
    if (i.status === 1n || i.status === 6n) return true;
    return i.status === 7n && reached(now, i.poolOpenTime);
}

/** `AmmInfo::reserves`: `(coin, pc)` = vaults − `need_take_pnl`; `null` if negative. */
export function v4Reserves(i: AmmInfo, coin: bigint, pc: bigint): readonly [bigint, bigint] | null {
    if (coin < i.needTakePnlCoin || pc < i.needTakePnlPc) return null;
    return [coin - i.needTakePnlCoin, pc - i.needTakePnlPc];
}

/** `AmmInfo::venue_executable`. */
export function v4Executable(i: AmmInfo, now: bigint, coin: bigint, pc: bigint): boolean {
    const r = v4Reserves(i, coin, pc);
    return v4SwapEnabled(i, now) && r !== null && r[0] > 0n && r[1] > 0n;
}

// ---- Raydium CPMM (raydium_cpmm.rs) --------------------------------------------------------------

export const CPMM_POOL_DISCRIMINATOR = [0xf7, 0xed, 0xe3, 0xf5, 0xd7, 0xc3, 0xde, 0x46];
export const CPMM_CONFIG_DISCRIMINATOR = [0xda, 0xf4, 0x21, 0x68, 0xcb, 0xcb, 0x2b, 0x6f];
export const CPMM_POOL_LEN = 637;
export const CPMM_CONFIG_LEN = 236;
export const T_CLAMP = 1_717_200_000n;

export interface CpmmPool {
    readonly ammConfig: Address;
    readonly token0Vault: Address;
    readonly token1Vault: Address;
    readonly lpMint: Address;
    readonly token0Mint: Address;
    readonly token1Mint: Address;
    readonly token0Program: Address;
    readonly token1Program: Address;
    readonly observationKey: Address;
    readonly status: number;
    readonly lpSupply: bigint;
    readonly protocolFeesToken0: bigint;
    readonly protocolFeesToken1: bigint;
    readonly fundFeesToken0: bigint;
    readonly fundFeesToken1: bigint;
    readonly openTime: bigint;
    readonly creatorFeeOn: number;
    readonly enableCreatorFee: boolean;
    readonly creatorFeesToken0: bigint;
    readonly creatorFeesToken1: bigint;
}

/** CPMM `PoolState` (`raydium_cpmm::read_pool`): owner, discriminator, ≥ 637 bytes, `enable_creator_fee` 0 / 1. */
export function readCpmmPool(owner: Address, d: Uint8Array): Read<CpmmPool> {
    if (owner !== RAYDIUM_CPMM_PROGRAM) return err('WrongOwner');
    if (!hasDiscriminator(d, CPMM_POOL_DISCRIMINATOR)) return err('WrongTag');
    if (d.length < CPMM_POOL_LEN) return err('TooShort');
    const enable = boolAt(d, 390);
    if (enable === null) return err('Malformed');
    return ok({
        ammConfig: keyAt(d, 8),
        token0Vault: keyAt(d, 72),
        token1Vault: keyAt(d, 104),
        lpMint: keyAt(d, 136),
        token0Mint: keyAt(d, 168),
        token1Mint: keyAt(d, 200),
        token0Program: keyAt(d, 232),
        token1Program: keyAt(d, 264),
        observationKey: keyAt(d, 296),
        status: u8At(d, 329),
        lpSupply: u64At(d, 333),
        protocolFeesToken0: u64At(d, 341),
        protocolFeesToken1: u64At(d, 349),
        fundFeesToken0: u64At(d, 357),
        fundFeesToken1: u64At(d, 365),
        openTime: u64At(d, 373),
        creatorFeeOn: u8At(d, 389),
        enableCreatorFee: enable,
        creatorFeesToken0: u64At(d, 397),
        creatorFeesToken1: u64At(d, 405),
    });
}

export interface CpmmConfig {
    readonly tradeFeeRate: bigint;
    readonly protocolFeeRate: bigint;
    readonly fundFeeRate: bigint;
    readonly creatorFeeRate: bigint;
}

/** CPMM `AmmConfig` (`raydium_cpmm::read_config`): owner, discriminator, ≥ 236 bytes, `disable_create_pool` 0 / 1. */
export function readCpmmConfig(owner: Address, d: Uint8Array): Read<CpmmConfig> {
    if (owner !== RAYDIUM_CPMM_PROGRAM) return err('WrongOwner');
    if (!hasDiscriminator(d, CPMM_CONFIG_DISCRIMINATOR)) return err('WrongTag');
    if (d.length < CPMM_CONFIG_LEN) return err('TooShort');
    if (boolAt(d, 9) === null) return err('Malformed');
    return ok({
        tradeFeeRate: u64At(d, 12),
        protocolFeeRate: u64At(d, 20),
        fundFeeRate: u64At(d, 28),
        creatorFeeRate: u64At(d, 108),
    });
}

/** `PoolState::sol_is_token0`. */
export function cpmmSolIsToken0(p: CpmmPool): boolean | null {
    const a = p.token0Mint === WSOL_MINT;
    const b = p.token1Mint === WSOL_MINT;
    return a && !b ? true : b && !a ? false : null;
}

/** `PoolState::reserves`: `vault_i − (protocol_i + fund_i + creator_i)`; `null` if negative. */
export function cpmmReserves(p: CpmmPool, v0: bigint, v1: bigint): readonly [bigint, bigint] | null {
    const c0 = p.protocolFeesToken0 + p.fundFeesToken0 + p.creatorFeesToken0;
    const c1 = p.protocolFeesToken1 + p.fundFeesToken1 + p.creatorFeesToken1;
    if (c0 > U64_MAX || c1 > U64_MAX || v0 < c0 || v1 < c1) return null;
    return [v0 - c0, v1 - c1];
}

/** `PoolState::creator_fee_rate`: the config's rate if the pool enabled it, else 0. */
export function cpmmCreatorFeeRate(p: CpmmPool, c: CpmmConfig): bigint {
    return p.enableCreatorFee ? c.creatorFeeRate : 0n;
}

/** `PoolState::creator_fee_on_input`; `null` for `creator_fee_on > 2`. */
export function cpmmCreatorFeeOnInput(p: CpmmPool, inputIsToken0: boolean): boolean | null {
    return p.creatorFeeOn === 0
        ? true
        : p.creatorFeeOn === 1
          ? inputIsToken0
          : p.creatorFeeOn === 2
            ? !inputIsToken0
            : null;
}

/** `PoolState::fee_ppm`: `t + c` exactly, `null` unless `< 10⁶`. */
export function cpmmFeePpm(p: CpmmPool, c: CpmmConfig): number | null {
    const phi = c.tradeFeeRate + cpmmCreatorFeeRate(p, c);
    return phi < 1_000_000n ? Number(phi) : null;
}

/** `PoolState::venue_executable` (incl. M3-SEC-1's fee split of a buy). */
export function cpmmExecutable(p: CpmmPool, c: CpmmConfig, now: bigint, v0: bigint, v1: bigint): boolean {
    const swap = (p.status & (1 << 2)) === 0 && reached(now, p.openTime);
    if (!swap || p.creatorFeeOn > 2) return false;
    const sol0 = cpmmSolIsToken0(p);
    if (sol0 === null) return false;
    // A buy's input is WSOL: token 0 iff SOL is token 0.
    const onInput = cpmmCreatorFeeOnInput(p, sol0);
    const splits = onInput === false || c.tradeFeeRate !== 0n || cpmmCreatorFeeRate(p, c) !== 0n;
    const r = cpmmReserves(p, v0, v1);
    return splits && r !== null && r[0] > 0n && r[1] > 0n;
}

/** `age_start`: `min(now, open_time)` when `open_time ≥ T_CLAMP`, else `now`. */
export function cpmmAgeStart(p: CpmmPool, now: bigint): bigint {
    return p.openTime < T_CLAMP ? now : p.openTime < now ? p.openTime : now;
}

// ---- mock (mock_cpamm.rs, TEST-ONLY) -------------------------------------------------------------

export const MOCK_POOL_DISCRIMINATOR = [0x3a, 0xc4, 0xd0, 0xf1, 0x1f, 0xd8, 0xf3, 0x30];
export const MOCK_POOL_MIN_LEN = 328 + 32;

export interface MockPoolState {
    readonly disabled: boolean;
    readonly feePpm: number;
    readonly protocolFeeBase: bigint;
    readonly protocolFeeQuote: bigint;
    readonly baseVault: Address;
    readonly quoteVault: Address;
    readonly baseMint: Address;
    readonly quoteMint: Address;
    readonly baseTokenProgram: Address;
}

/** The TEST-ONLY mock pool (`mock_cpamm::read`): owner, discriminator, length, `layout_version == 0`. */
export function readMockPool(owner: Address, d: Uint8Array): Read<MockPoolState> {
    if (owner !== MOCK_CPAMM_PROGRAM) return err('WrongOwner');
    if (!hasDiscriminator(d, MOCK_POOL_DISCRIMINATOR)) return err('WrongTag');
    if (d.length < MOCK_POOL_MIN_LEN) return err('TooShort');
    if (u8At(d, 8) !== 0) return err('Malformed');
    const disabled = boolAt(d, 9);
    if (disabled === null) return err('Malformed');
    return ok({
        disabled,
        feePpm: u32At(d, 10),
        protocolFeeBase: u64At(d, 22),
        protocolFeeQuote: u64At(d, 30),
        baseVault: keyAt(d, 167),
        quoteVault: keyAt(d, 199),
        baseMint: keyAt(d, 264),
        quoteMint: keyAt(d, 296),
        baseTokenProgram: keyAt(d, 328),
    });
}

/** `mock_cpamm::reserves`: `null` when disabled, a vault under its counter, or an empty side. */
export function mockReserves(p: MockPoolState, base: bigint, quote: bigint): readonly [bigint, bigint] | null {
    if (p.disabled || base < p.protocolFeeBase || quote < p.protocolFeeQuote) return null;
    const x = base - p.protocolFeeBase;
    const y = quote - p.protocolFeeQuote;
    return x > 0n && y > 0n ? [x, y] : null;
}

export { DEFAULT_KEY };
