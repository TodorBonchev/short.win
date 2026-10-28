use anchor_lang::prelude::*;

#[error_code]
pub enum SimpleError {
    #[msg("Checked arithmetic overflow")]
    Arithmetic,
    #[msg("The signer is not authorized")]
    Unauthorized,
    #[msg("Account identity, owner, size or generation does not match")]
    AccountMismatch,
    #[msg("New borrowing is paused")]
    Paused,
    #[msg("Price-dependent actions are paused")]
    PricePaused,
    #[msg("Configuration or immutable policy is invalid")]
    InvalidPolicy,
    #[msg("This token program or mint layout is not supported")]
    MintUnsupported,
    #[msg("This mint behavior is not supported for entry")]
    MintUnsafe,
    #[msg("Amount must be positive")]
    ZeroAmount,
    #[msg("Output is below a signed minimum")]
    MinimumNotMet,
    #[msg("The pool has insufficient available principal")]
    InsufficientLiquidity,
    #[msg("The last shares cannot leave while loans remain")]
    LastSharesBorrowed,
    #[msg("This generation cannot be retired or replaced")]
    GenerationNotEmpty,
    #[msg("The reference does not bind the required mint, quantity and policy")]
    BadReference,
    #[msg("The reference is stale")]
    StaleReference,
    #[msg("The reference sequence must progress")]
    WrongSequence,
    #[msg("The reference is not finite and ready")]
    UnknownReference,
    #[msg("The loan nonce is not the next market nonce")]
    InvalidNonce,
    #[msg("The position notional cap would be exceeded")]
    PositionLimit,
    #[msg("The market notional cap would be exceeded")]
    MarketLimit,
    #[msg("The inventory utilization cap would be exceeded")]
    UtilizationLimit,
    #[msg("The wallet notional cap would be exceeded")]
    WalletLimit,
    #[msg("The position is below the minimum notional")]
    BelowMinNotional,
    #[msg("Required margin exceeds the signed maximum")]
    MarginAboveMax,
    #[msg("Upfront fee exceeds the signed maximum")]
    FeeAboveMax,
    #[msg("Actual sale proceeds are below the signed minimum")]
    ProceedsBelowMin,
    #[msg("The route is not compatible with the required action")]
    BadRoute,
    #[msg("Buyback input exceeds the signed or policy maximum")]
    BuybackAboveMax,
    #[msg("This loan is not eligible for liquidation")]
    NotLiquidatable,
    #[msg("The delayed liquidation confirmation is not complete")]
    ConfirmationPending,
    #[msg("The loan has not reached maturity")]
    NotMatured,
    #[msg("The loan has not reached its locked overdue recovery time")]
    NotOverdue,
    #[msg("Temporary execution funding exceeds the signed maximum")]
    FundingAboveMax,
    #[msg("Creation fee exceeds the signed maximum")]
    SetupFeeAboveMax,
    #[msg("Required setup rent exceeds the signed maximum")]
    SetupRentAboveMax,
    #[msg("Actual token or SOL balances do not match the agreed flow")]
    BalanceMismatch,
    #[msg("This pool generation is retired for new borrowing and deposits")]
    RetiredPool,
    #[msg("Pool or lender accounting refused the transition")]
    CoreAccounting,
    #[msg("Route instruction is not supported")]
    RouteInvalid,
    #[msg("Route accounts do not match the isolated execution")]
    RouteAccounts,
    #[msg("This mint cannot use the current routed execution")]
    RouteMintUnsupported,
    #[msg("Actual route output is insufficient")]
    RouteOutputShort,
    #[msg("Actual execution balances do not match the agreed flow")]
    RouteBalance,
    #[msg("Insufficient collateral or transaction funding")]
    InsufficientFunds,
    #[msg("Checked route arithmetic overflow")]
    MathOverflow,
    #[msg("Actual buyback cost leaves a non-expired loan healthy")]
    HealthyFill,
    #[msg("Source or runtime clock is invalid")]
    BadClock,
    #[msg("The loan has already been resolved")]
    LoanClosed,
    #[msg("The grace period has ended; only deterministic default is available")]
    DefaultRequired,
}

pub trait CheckedExt<T> {
    fn checked(self) -> Result<T>;
}
impl<T> CheckedExt<T> for Option<T> {
    fn checked(self) -> Result<T> {
        self.ok_or_else(|| error!(SimpleError::Arithmetic))
    }
}
