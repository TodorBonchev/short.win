/**
 * Human messages for every short.win program error (FRONTEND.md §5.9). Codes are stable
 * (ARCH §3.11: numbered from 6000 in declaration order, appended only). `errors.test.ts` checks
 * this table against the program's `ShortwinError` enum (`programs/shortwin/src/errors.rs`) and
 * `@shortwin/client`'s `SHORTWIN_ERROR__*` constants, so a new program error fails CI until it has
 * a message here.
 *
 * `action` tells the UI what to offer: `retry` (transient, or fixed by a new plan: the review's Retry
 * re-simulates a transient failure and rebuilds the plan from the chain — a new quote, new bounds,
 * the steps it now needs — after a refusal of the plan itself, `useTxFlow` `retryMode`),
 * `adjust` (change the input), `wait` (a market state that ends by itself), `blocked` (not
 * possible on this market now), `report` (should not happen from this app: show the code and a
 * report link), `admin` (an instruction this app never sends).
 */
export type ErrorAction = 'retry' | 'adjust' | 'wait' | 'blocked' | 'report' | 'admin';

export interface ProgramErrorInfo {
    readonly code: number;
    readonly name: string;
    readonly message: string;
    readonly action: ErrorAction;
}

type Row = readonly [code: number, name: string, action: ErrorAction, message: string];

const ROWS: readonly Row[] = [
    [6000, 'MathOverflow', 'adjust', 'The amounts are outside what the program can compute. Try a smaller amount.'],
    [6001, 'AccountMismatch', 'report', 'An account in the transaction does not match what the program expects.'],
    [
        6002,
        'VenueAccountMismatch',
        'retry',
        'The venue’s accounts changed (for example the coin creator). A new review re-reads them.',
    ],
    [
        6003,
        'DeniedAccount',
        'report',
        'The transaction passed an account the program refuses here (a protocol account, or a venue extra naming the pool or the borrower).',
    ],
    [
        6004,
        'BadRemainingAccounts',
        'retry',
        'The venue accounts are missing, extra or out of order. A new review re-reads them.',
    ],
    [6005, 'BadExtras', 'retry', 'The venue’s extra accounts changed. A new review re-reads them.'],
    [
        6006,
        'ConfigVersion',
        'report',
        'The protocol configuration has a version this app does not know. Reload the app.',
    ],
    [
        6007,
        'EpochRecordMismatch',
        'retry',
        'A share-epoch record does not match this market. A new review re-reads the records.',
    ],
    [6008, 'Unauthorized', 'blocked', 'This wallet is not allowed to do this.'],
    [6009, 'TimelockNotElapsed', 'admin', 'The pending configuration change is still timelocked.'],
    [6010, 'NoPendingChange', 'admin', 'There is no pending configuration change.'],
    [6011, 'InvalidParams', 'adjust', 'A parameter is outside its allowed bounds.'],
    [6012, 'TimelockTooShort', 'admin', 'The timelock is below the compiled minimum.'],
    [6013, 'VenueNotUpgraded', 'admin', 'The venue program has not been upgraded.'],
    [6014, 'VenueSlotMismatch', 'admin', 'The venue program’s deploy slot is not the expected one.'],
    [6015, 'CoverageAuthorityDisabled', 'admin', 'The coverage authority is disabled.'],
    [
        6016,
        'MintUnsupportedProgram',
        'blocked',
        'This token is not an SPL Token or Token-2022 mint, so it cannot be listed.',
    ],
    [6017, 'MintNative', 'blocked', 'Wrapped SOL cannot be listed.'],
    [
        6018,
        'MintFreezeAuthority',
        'blocked',
        'This token has an active freeze authority (its issuer could freeze the pool), so it cannot be listed.',
    ],
    [
        6019,
        'MintDisallowedExtension',
        'blocked',
        'This token uses a Token-2022 extension that is unsafe for lending (for example a permanent delegate, a transfer hook or a transfer fee), so it cannot be listed.',
    ],
    [6020, 'MintMalformed', 'blocked', 'This token’s mint account could not be read.'],
    [6021, 'MintSafe', 'blocked', 'The token passes the safety filter; there is nothing to flag.'],
    [
        6022,
        'MintUnsafeFlagged',
        'blocked',
        'This token was flagged unsafe. Deposits and new shorts are blocked; withdrawals, claims and exits continue.',
    ],
    [
        6023,
        'NoSupportedVenue',
        'blocked',
        'This token has no venue short.win can trade yet. It can be lent, not shorted.',
    ],
    [
        6024,
        'PumpVenuePrecedence',
        'blocked',
        'This is a Pump.fun token: its market must use the Pump.fun curve or its PumpSwap pool.',
    ],
    [6025, 'CurveMayhemUnsupported', 'blocked', 'This Pump.fun coin runs in a mode short.win does not support.'],
    [6026, 'CurveQuoteNotSol', 'blocked', 'This Pump.fun curve is not quoted in SOL.'],
    [
        6027,
        'CurveNeedsExtend',
        'retry',
        'The Pump.fun curve account must be extended first. A new review adds that step.',
    ],
    [
        6028,
        'VenueMigrating',
        'wait',
        'This token is migrating from the Pump.fun curve to its pool. Exits pause during the migration and resume after it.',
    ],
    [6029, 'NoRoute', 'wait', 'The venue cannot trade right now (halted, drained or disabled). Try again later.'],
    [
        6030,
        'RouteBroken',
        'blocked',
        'This market’s route is marked broken. New shorts are refused; exits are not paused.',
    ],
    [
        6031,
        'CurveStraddle',
        'wait',
        'Buying back this position would complete the Pump.fun curve, which one transaction cannot do. It is exited on the pool right after graduation; you can add margin meanwhile.',
    ],
    [6032, 'VenueDisabled', 'blocked', 'New shorts are disabled for this venue.'],
    [
        6033,
        'VenueUpgraded',
        'wait',
        'A venue program this market uses was upgraded and is under review. New shorts wait; exits are not affected.',
    ],
    [6034, 'VenueFlagged', 'blocked', 'The venue has disabled trading on this pool. New shorts are refused.'],
    [6035, 'VenueFeeTooHigh', 'blocked', 'The venue’s fee is above short.win’s cap, so new shorts are refused.'],
    [
        6036,
        'BoostInProgress',
        'wait',
        'This token just graduated and its post-graduation buyback is still running. New shorts wait until it ends.',
    ],
    [6037, 'VenueResultMismatch', 'retry', 'The venue did not deliver what it quoted. Try again.'],
    [6038, 'VenueRentOnSell', 'retry', 'The venue charged unexpected rent on the sale. Try again.'],
    [6039, 'TraderStateInvalid', 'retry', 'The market’s trading account is in an unexpected state. Try again shortly.'],
    [6040, 'DelegateNotCleared', 'report', 'A token delegate was not cleared after the trade.'],
    [6041, 'AuthorityTampered', 'report', 'A vault authority was changed.'],
    [
        6042,
        'PoolIneligible',
        'blocked',
        'This pool is not eligible (not tradeable, fee too high or too little locked liquidity).',
    ],
    [6043, 'PoolNotDominant', 'blocked', 'This pool is not deeper than the current one.'],
    [6044, 'PositionsOpen', 'blocked', 'The pool cannot change while positions are open.'],
    [6045, 'PoolSwitchNotDue', 'wait', 'The pool-switch window has not ended yet.'],
    [6046, 'NoPendingSwitch', 'blocked', 'There is no pending pool switch.'],
    [6047, 'PoolListWindow', 'wait', 'This market was listed moments ago. New shorts open after the listing window.'],
    [6048, 'PoolLockTooLow', 'blocked', 'The pool’s locked liquidity is too low.'],
    [6049, 'PoolTooThin', 'blocked', 'The pool’s locked liquidity is below the minimum.'],
    [6050, 'OpensPaused', 'wait', 'New shorts are paused (on this market or globally). Exits continue.'],
    [
        6051,
        'CoveragePaused',
        'wait',
        'New shorts are paused on this market because keeper coverage lapsed. Exits continue; new shorts resume when coverage returns.',
    ],
    [6052, 'MarketWarmingUp', 'wait', 'This market is new. New shorts open after its warm-up.'],
    [
        6053,
        'PriceDeviatesFromEma',
        'retry',
        'The price is moving too fast (the live price is too far from the smoothed price). Wait a few seconds and try again.',
    ],
    [6054, 'PriceAboveOpenMax', 'blocked', 'The token’s price is above the highest price the program can open at.'],
    [
        6055,
        'CapPositionImpact',
        'adjust',
        'This size is above the per-position cap: buying it back would move the price too much. Reduce the size.',
    ],
    [6056, 'CapMarketOi', 'adjust', 'This size would exceed the market’s open-interest cap. Reduce the size.'],
    [
        6057,
        'CapUtilization',
        'adjust',
        'This would borrow more than the pool’s utilisation cap allows. Reduce the size.',
    ],
    [
        6058,
        'CapCurveGamma',
        'adjust',
        'This size would leave too little of the Pump.fun curve for a safe graduation. Reduce the size.',
    ],
    [
        6059,
        'CapWallet',
        'adjust',
        'This would exceed the per-wallet share of the market’s open interest. Reduce the size.',
    ],
    [6060, 'InsufficientInventory', 'adjust', 'Not enough tokens are available to borrow.'],
    [6061, 'BelowMinNotional', 'adjust', 'This position is below the minimum size.'],
    [6062, 'MarketNotionalCap', 'blocked', 'The market’s total notional cap is reached.'],
    [
        6063,
        'ProceedsBelowMin',
        'retry',
        'The sale would return less than your minimum (slippage): the price moved. Review again with a new quote, or raise the slippage.',
    ],
    [
        6064,
        'MarginAboveMax',
        'retry',
        'The required margin rose above your maximum because the price moved. Review again with a new quote.',
    ],
    [6065, 'OpenFeeAboveMax', 'retry', 'The open fee rose above your maximum. Review again with a new quote.'],
    [
        6066,
        'InsufficientVault',
        'blocked',
        'The vault no longer covers the buyback plus rent. Add margin, or the position will be liquidated.',
    ],
    [
        6067,
        'BuybackAboveMax',
        'retry',
        'The buyback would cost more than your maximum (slippage): the price moved. Review again with a new quote, or raise the slippage.',
    ],
    [6068, 'NotLiquidatable', 'admin', 'The position is not liquidatable.'],
    [6069, 'UseSolSettle', 'admin', 'This position must settle in SOL.'],
    [6070, 'UseExpire', 'admin', 'This position must be expired, not liquidated.'],
    [6071, 'NotExpired', 'admin', 'The position has not reached its deadline.'],
    [6072, 'ReferenceStale', 'admin', 'The reference price is stale.'],
    [6073, 'ExitBoundNotMet', 'admin', 'The exit bound is not met.'],
    [6074, 'SettlementNotAllowed', 'admin', 'A SOL settlement is not allowed now.'],
    [6075, 'S3TimerRunning', 'admin', 'The no-route timer is still running.'],
    [6076, 'ZeroAmount', 'adjust', 'Enter an amount above zero.'],
    [
        6077,
        'SharesBelowMin',
        'retry',
        'You would receive fewer pool shares than quoted (the pool changed). Review again with a new quote.',
    ],
    [
        6078,
        'TokensBelowMin',
        'retry',
        'You would receive fewer tokens than quoted (the pool changed). Review again with a new quote.',
    ],
    [
        6079,
        'InsufficientShares',
        'adjust',
        'You do not have that many shares. A position behind a share rebase can end one share lower than quoted if another lender synced first: review again.',
    ],
    [6080, 'InsufficientFreeInventory', 'adjust', 'Not enough unborrowed tokens to withdraw that much now.'],
    [
        6081,
        'AboveProRataAllowance',
        'adjust',
        'That is more than your pro-rata share of the unborrowed tokens. Withdraw less, or wait for positions to close.',
    ],
    [
        6082,
        'NegativePending',
        'adjust',
        'Your SOL balance in this pool is in deficit. Cover the deficit before withdrawing.',
    ],
    [
        6083,
        'LenderBehindEpochs',
        'retry',
        'Your lender position must be synced first. A new review adds that step, or asks you to sync when it is too far behind.',
    ],
    [
        6084,
        'WipeSweepPending',
        'retry',
        'This pool was fully settled and must be swept before new deposits. A new review adds that step.',
    ],
    [6085, 'OracleOwner', 'admin', 'The SOL/USD price account has the wrong owner.'],
    [6086, 'OracleFeed', 'admin', 'The SOL/USD price update is for the wrong feed.'],
    [6087, 'OracleStale', 'admin', 'The SOL/USD price update is too old.'],
    [6088, 'OracleConfidence', 'admin', 'The SOL/USD price update is too uncertain.'],
    [6089, 'OracleStep', 'admin', 'The SOL/USD price moved too far in one update.'],
    [6090, 'OracleVerification', 'admin', 'The SOL/USD price update is not fully verified.'],
    [
        6091,
        'SolBelowMin',
        'retry',
        'You would receive less SOL than quoted (the pool changed). Review again with a new quote.',
    ],
    [
        6092,
        'LastSharesBorrowed',
        'adjust',
        'The last shares cannot leave while tokens are borrowed. You can withdraw all but your last share.',
    ],
    [6093, 'TimelockTooLong', 'admin', 'The timelock is above the compiled maximum.'],
    [6094, 'PendingChangeExpired', 'admin', 'The pending configuration change has expired.'],
    [
        6095,
        'MemoProgramRequired',
        'retry',
        'Your token account requires a memo on incoming transfers. A new review adds it.',
    ],
    [
        6096,
        'CreationFeeAboveMax',
        'retry',
        'The market-creation fee rose above your maximum. Review again with the new fee.',
    ],
    [6097, 'OracleFutureDated', 'admin', 'The SOL/USD price update is dated in the future.'],
    [6098, 'EmergencyCannotCancel', 'admin', 'The emergency multisig cannot cancel this change.'],
    [6099, 'UseLiquidate', 'admin', 'This position must be liquidated, not expired.'],
    [6100, 'PoolWindowClosed', 'wait', 'The pool-switch window has closed; it must be resolved first.'],
    [
        6101,
        'EmaWarmingUp',
        'wait',
        'Prices on this market are being confirmed after a venue change or listing (8 seconds of observations). Try again shortly.',
    ],
    [6102, 'CarriedPositionsTooLarge', 'blocked', 'An open position would not fit the new pool’s opening limits.'],
    [6103, 'ReservedScanOutOfOrder', 'report', 'Reserved error code (never returned).'],
    [
        6104,
        'GraduationHalted',
        'blocked',
        'PumpSwap pool creation is disabled, so this curve cannot graduate. New curve positions are refused until it is re-enabled; open positions are unaffected.',
    ],
    [
        6105,
        'AuthoritiesNotDistinct',
        'admin',
        'Governance, the Emergency key, the fee recipient and the coverage key must be four different keys.',
    ],
    [
        6106,
        'EpochRecordReadOnly',
        'retry',
        'A share-epoch record your position converts through was passed read-only. A new review passes it writable.',
    ],
    [
        6107,
        'RebaseNeedsOwner',
        'blocked',
        'Only the position’s owner can sync it through a share rebase. Connect the wallet that owns the position.',
    ],
    [
        6108,
        'PoolDrained',
        'wait',
        'This pool is drained: at most 1/1,000 of its tokens are unborrowed. Deposits wait until repayments or settlements free more.',
    ],
];

export const PROGRAM_ERRORS: ReadonlyMap<number, ProgramErrorInfo> = new Map(
    ROWS.map(([code, name, action, message]) => [code, { code, name, action, message }]),
);

/** The info of a short.win custom error code, or `undefined` for a code outside the program's range. */
export function programError(code: number): ProgramErrorInfo | undefined {
    return PROGRAM_ERRORS.get(code);
}
