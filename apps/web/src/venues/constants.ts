/**
 * The venues' static accounts and the protocol's venue constants (the program's
 * `constants.rs`, ARCH §2.3.2, §3.12). Program ids come from `config/programs.ts` (pinned);
 * `constants.test.ts` holds every literal here to `programs/shortwin/src/constants.rs` and the
 * `market.rs` / `venue_extras.rs` codes.
 */
import { type Address, address } from '@solana/kit';

export {
    ASSOCIATED_TOKEN_PROGRAM,
    MOCK_CPAMM_PROGRAM,
    PUMP_AMM_PROGRAM,
    PUMP_FEES_PROGRAM,
    PUMP_PROGRAM,
    RAYDIUM_CPMM_PROGRAM,
    RAYDIUM_V4_PROGRAM,
    SHORTWIN_PROGRAM,
    SYSTEM_PROGRAM,
    TOKEN_2022_PROGRAM,
    TOKEN_PROGRAM,
    WSOL_MINT,
} from '../config/programs.ts';

/** pump `Global` = PDA(pump, ["global"]). */
export const PUMP_GLOBAL: Address = address('4wTV1YmiEkRvAtNtsSGPtUrqRYQMe5SKy2uB4Jjaxnjf');
/** pump `event_authority`. */
export const PUMP_EVENT_AUTHORITY: Address = address('Ce6TQqeHC9p8KetsN6JsjHK7UTZk7nasjjnr7XxXp9F1');
/** pump global volume accumulator. */
export const PUMP_GLOBAL_VOLUME_ACCUMULATOR: Address = address('Hq2wp8uJ9jCPsYgNHex8RtqdvMPfVGoYwjvF1ATiwn2Y');
/** pump-fees `FeeConfig` of pump. */
export const PUMP_FEE_CONFIG: Address = address('8Wf5TiAheLUqBrKXeYg2JtAFFMWtKdG2BSFgqUcPVwTt');
/** PumpSwap `GlobalConfig`. */
export const PUMP_AMM_GLOBAL_CONFIG: Address = address('ADyA8hdefvWN2dbGGWFotbzWxrAvLW83WG6QCVXvJKqw');
/** PumpSwap `event_authority`. */
export const PUMP_AMM_EVENT_AUTHORITY: Address = address('GS4CU59F31iL7aR2Q8zVS8DRrcRnXX1yjQ66TqNVQnaR');
/** PumpSwap global volume accumulator. */
export const PUMP_AMM_GLOBAL_VOLUME_ACCUMULATOR: Address = address('C2aFPdENg4A2HQsmrd5rTw5TaYBX5Ku887cWjbFKtZpw');
/** pump-fees `FeeConfig` of PumpSwap. */
export const PUMP_AMM_FEE_CONFIG: Address = address('5PHirr8joyTMp9JMm6nW7hNDVyEYdkzDqazxPD7RaTjx');
/** Raydium AMM v4 vault / LP authority (nonce 254). */
export const RAYDIUM_V4_AUTHORITY: Address = address('5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1');
/** Raydium CPMM vault / LP authority. */
export const RAYDIUM_CPMM_AUTHORITY: Address = address('GpMZbSM2GgvTKHJirzeGfMFoaZ8UR2X7F4v8vHTvxFbL');
/** TEST-ONLY mock venue's `pool_authority` = PDA(mock, ["authority"]). */
export const MOCK_CPAMM_AUTHORITY: Address = address('FchvTASEWCcGETHi6QMLW7v64Jzn66W8Z3yqZjY9iqLA');

/** `VenueKind` codes (`state/market.rs`). */
export const KIND = {
    none: 0,
    pump_curve: 1,
    pump_migrating: 2,
    pump_swap: 3,
    raydium_v4: 4,
    raydium_cpmm: 5,
    mock: 6,
} as const;
export type KindCode = (typeof KIND)[keyof typeof KIND];
export type KindName = keyof typeof KIND;

export function kindName(code: number): KindName | null {
    for (const [k, v] of Object.entries(KIND)) if (v === code) return k as KindName;
    return null;
}

/** `Market.flags` bits (`state/market.rs`). */
export const MARKET_FLAG = {
    pausedOpens: 1 << 0,
    mintUnsafe: 1 << 1,
    routeBroken: 1 << 2,
    solIsToken0OrCoin: 1 << 3,
    graduatedElsewhere: 1 << 4,
    wipeLive: 1 << 5,
    pausedCoverage: 1 << 6,
} as const;

/** `GlobalConfig.flags` bit 0. */
export const CONFIG_FLAG_PAUSED_OPENS = 1;

/** `GlobalConfig.venue_enabled` / `venue_route_broken` bits (`VENUE_BIT_*`). */
export const VENUE_BIT: Readonly<Partial<Record<KindName, number>>> = {
    pump_curve: 1 << 0,
    pump_swap: 1 << 1,
    raydium_v4: 1 << 2,
    raydium_cpmm: 1 << 3,
    mock: 1 << 5,
};

/** `VenueGuard.upgraded` bit indices (`GUARD_*`) and the masks that block opens per venue. */
const GUARD = { pump: 0, pumpAmm: 1, pumpFees: 2, raydiumV4: 3, raydiumCpmm: 4 } as const;
export const GUARD_MASK: Readonly<Partial<Record<KindName, number>>> = {
    pump_curve: (1 << GUARD.pump) | (1 << GUARD.pumpFees) | (1 << GUARD.pumpAmm),
    pump_swap: (1 << GUARD.pumpAmm) | (1 << GUARD.pumpFees),
    raydium_v4: (1 << GUARD.raydiumV4) | (1 << GUARD.raydiumCpmm),
    raydium_cpmm: (1 << GUARD.raydiumV4) | (1 << GUARD.raydiumCpmm),
    mock: (1 << GUARD.raydiumV4) | (1 << GUARD.raydiumCpmm),
};

/** `Market.pool_cache` indices while the venue is Raydium (and the mock, its stand-in). */
export const RAY_CACHE = {
    pool: 0,
    vaultSol: 1,
    vaultTok: 2,
    ammConfig: 3,
    observation: 4,
    lpMint: 5,
    tokProgram: 6,
} as const;

/** `Market.pool_cache` indices while the venue is PumpSwap. */
export const PAMM_CACHE = {
    poolBaseVault: 0,
    poolQuoteVault: 1,
    poolV2Marker: 2,
    boostVaultAta: 3,
    uva: 4,
    uvaQuoteAta: 5,
    cachedCoinCreator: 6,
    cachedCoinCreatorVaultAuth: 7,
    cachedCoinCreatorVaultAta: 8,
    boostVault: 9,
} as const;

/** `VenueExtras` instruction kinds (`EXTRAS_IX_*`). */
export const EXTRAS_IX = { sell: 0, buy: 1, claim: 2 } as const;
/** `ExtrasSel.set`. */
export const EXTRAS_SET = { none: 0, current: 1, previous: 2, stable: 3 } as const;
/** `ExtraEntry.kind`, `SeedPart.kind`, placeholders (`state/venue_extras.rs`). */
export const ENTRY = { none: 0, fixed: 1, template: 2 } as const;
export const SEED = { none: 0, const: 1, placeholder: 2 } as const;
export const PLACEHOLDER = { mint: 0, pool: 1, trader: 2, bondingCurve: 3, coinCreator: 4 } as const;
export const EXTRAS_MAX_SEEDS = 4;

/** `COVERAGE_CLEAR_DELAY_S`: opens wait this long after a coverage pause is cleared (M7-SEC-3). */
export const COVERAGE_CLEAR_DELAY_S = 5n;
/** `STANDARD_POOL_TOKENS`: `migrate_v2`'s token seed on a standard curve (the EMA leg's `X_p`). */
export const STANDARD_POOL_TOKENS = 206_900_000_000_000n;
/**
 * What pump's 2026-10-02 redeploys grow a shorter account to on its first trade, charging that trade
 * the rent (PUMP-2026-10-2): a PumpSwap pool to 287 bytes (through the fee buckets), a bonding curve
 * to 141.
 */
export const PAMM_POOL_LEN_2026_10 = 287;
export const PUMP_CURVE_LEN_2026_10 = 141;
/** `fees::PUMP_CURVE_MCAP_SUPPLY` (10¹⁵): the curve's fee-tier supply and the new pool's base supply. */
export const PUMP_CURVE_MCAP_SUPPLY = 1_000_000_000_000_000n;
/** The mock venue's protocol share of the trade fee, ppm (`mock_cpamm::PROTOCOL_FEE_RATE`). */
export const MOCK_PROTOCOL_FEE_RATE = 120_000;
/** PumpSwap `disable_flags` bits (T-DISABLE): bit 0 halts `create_pool`, bit 3 every buy. */
export const PAMM_CREATE_POOL_DISABLED_BIT = 1 << 0;
export const PAMM_BUY_DISABLED_BIT = 1 << 3;
/** A curve shorter than this must be extended before it lists (`CurveNeedsExtend`). */
export const CURVE_MIN_LEN = 115;
/** pump `extend_account` discriminator (R-PUMP §3.5): `[account W, user S W, system, event_authority, program]`. */
export const PUMP_IX_EXTEND_ACCOUNT = [0xea, 0x66, 0xc2, 0xcb, 0x96, 0x48, 0x3e, 0xe5] as const;
/** `sha256("global:claim_cashback")[..8]`, the same on pump and PumpSwap (`venues/pump_amm.rs`; the post-wipe sweep). */
export const IX_CLAIM_CASHBACK = new Uint8Array([0x25, 0x3a, 0x23, 0x7e, 0xbe, 0x35, 0xe4, 0xc5]);
/** `sha256("global:init_user_volume_accumulator")[..8]` (`venues/pump_amm.rs`; the post-wipe sweep). */
export const IX_INIT_USER_VOLUME_ACCUMULATOR = new Uint8Array([0x5e, 0x06, 0xca, 0x73, 0xff, 0x60, 0xe8, 0xb7]);
