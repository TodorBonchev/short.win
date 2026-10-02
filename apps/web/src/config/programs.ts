/**
 * Pinned program and account addresses (FRONTEND.md §9.1). Every address the app puts in a
 * transaction as a program comes from this file; `tx/integrity.ts` refuses any other top-level
 * program, and `programs.test.ts` holds these literals to the generated client and to
 * `programs/shortwin/src/constants.rs`.
 */
import { type Address, address } from '@solana/kit';

/** short.win (`declare_id!`; `@shortwin/client`'s `SHORTWIN_PROGRAM_ADDRESS`). */
export const SHORTWIN_PROGRAM: Address = address('5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK');
/** The stateless straddle helper (ARCH §5.4, A66). */
export const COMPLETE_CURVE_PROGRAM: Address = address('4B5mGykX9EWwaYdbTvsp8qXCUTbc7caQourz16Gat6LJ');

export const SYSTEM_PROGRAM: Address = address('11111111111111111111111111111111');
export const COMPUTE_BUDGET_PROGRAM: Address = address('ComputeBudget111111111111111111111111111111');
export const TOKEN_PROGRAM: Address = address('TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA');
export const TOKEN_2022_PROGRAM: Address = address('TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb');
export const ASSOCIATED_TOKEN_PROGRAM: Address = address('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');
/** spl-memo v3: `withdraw`'s `memo_program` for a memo-required destination (ARCH §3.3, M1-SEC-7). */
export const MEMO_PROGRAM: Address = address('MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr');
export const WSOL_MINT: Address = address('So11111111111111111111111111111111111111112');
export const WSOL_2022_MINT: Address = address('9pan9bMn5HatX4EJdBwg9VgCa7Uz5HL8N1m5D3NdXejP');

/** Venue programs (compile-time constants of the program; ARCH §3.12). */
export const PUMP_PROGRAM: Address = address('6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P');
export const PUMP_AMM_PROGRAM: Address = address('pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA');
export const PUMP_FEES_PROGRAM: Address = address('pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ');
export const RAYDIUM_V4_PROGRAM: Address = address('675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8');
export const RAYDIUM_CPMM_PROGRAM: Address = address('CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C');
/** TEST-ONLY mock venue (`test-venue` builds; enabled by VITE_ENABLE_TEST_VENUE on localnet). */
export const MOCK_CPAMM_PROGRAM: Address = address('BPAZFPkmVtYEWEZHCzFUns7oybh1VWfAvmu3ofM2mQi7');

/**
 * Programs a transaction built by this app may invoke at the top level (FRONTEND.md §5.8): only
 * those with at least one allowed instruction. The gate itself is per instruction —
 * `tx/allowlist.ts` `ALLOWED_INSTRUCTIONS` (ATA `CreateIdempotent`; Token-2022 `Enable` /
 * `DisableCpiGuard` for the CPI Guard bracket, FE-1; pump `extend_account` and `claim_cashback`;
 * PumpSwap `init_user_volume_accumulator` and `claim_cashback` for the post-wipe sweep bundle) —
 * and the compute-budget instructions are the pipeline's own (`tx/message.ts`), never a plan's.
 * System, SPL Token and `complete_curve` are not here: no plan invokes them at the top level.
 */
export const TOP_LEVEL_ALLOWLIST: readonly Address[] = [
    SHORTWIN_PROGRAM,
    ASSOCIATED_TOKEN_PROGRAM,
    TOKEN_2022_PROGRAM,
    PUMP_PROGRAM,
    PUMP_AMM_PROGRAM,
];
