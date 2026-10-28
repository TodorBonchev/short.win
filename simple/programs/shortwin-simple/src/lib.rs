#![allow(unexpected_cfgs, ambiguous_glob_reexports)]
use anchor_lang::prelude::*;
declare_id!("kXYTnbz7MJ3g1d1cKX5wfZLy8KmN7Cctx4oePy5ndB2");
pub mod errors;
pub mod events;
pub mod initialization;
pub mod route;
pub mod state;
pub use initialization::*;
pub mod lender;
pub use lender::*;
pub mod exits;
pub mod finance;
pub use exits::*;
pub mod open;
pub use open::*;
pub mod reference;
pub use reference::*;
#[program]
pub mod shortwin_simple {
    use super::*;
    pub fn repay(ctx: Context<Repay>) -> Result<()> {
        exits::repay(ctx)
    }
    pub fn close_position<'info>(
        ctx: Context<'info, ClosePosition<'info>>,
        args: CloseArgs,
    ) -> Result<()> {
        exits::close_position(ctx, args)
    }
    pub fn add_margin(ctx: Context<AddMargin>, amount: u64) -> Result<()> {
        exits::add_margin(ctx, amount)
    }
    pub fn arm_liquidation(
        ctx: Context<ArmLiquidation>,
        minimum_reference_sequence: u64,
    ) -> Result<()> {
        exits::arm_liquidation(ctx, minimum_reference_sequence)
    }
    pub fn force_exit<'info>(
        ctx: Context<'info, ForceExit<'info>>,
        args: ForcedExitArgs,
    ) -> Result<()> {
        exits::force_exit(ctx, args)
    }
    pub fn default_loan(ctx: Context<DefaultLoan>) -> Result<()> {
        exits::default_loan(ctx)
    }

    pub fn open_position<'info>(
        ctx: Context<'info, OpenPosition<'info>>,
        args: OpenArgs,
    ) -> Result<()> {
        open::open_position(ctx, args)
    }

    pub fn publish_reference(
        ctx: Context<PublishReference>,
        args: PublishReferenceArgs,
    ) -> Result<()> {
        reference::publish_reference(ctx, args)
    }
    pub fn deposit(
        ctx: Context<DepositTokens>,
        amount: u64,
        min_shares: u128,
        max_receipt_rent: u64,
    ) -> Result<()> {
        lender::deposit(ctx, amount, min_shares, max_receipt_rent)
    }
    pub fn withdraw(
        ctx: Context<WithdrawTokens>,
        shares: u128,
        min_tokens: u64,
        min_sol: u64,
    ) -> Result<()> {
        lender::withdraw(ctx, shares, min_tokens, min_sol)
    }
    pub fn claim_sol(ctx: Context<ClaimSol>, min_sol: u64) -> Result<()> {
        lender::claim_sol(ctx, min_sol)
    }

    pub fn initialize(ctx: Context<Initialize>, args: InitializeArgs) -> Result<()> {
        initialization::initialize(ctx, args)
    }
    pub fn register_market(
        ctx: Context<RegisterMarket>,
        max_creation_fee: u64,
        max_setup_rent: u64,
    ) -> Result<()> {
        initialization::register_market(ctx, max_creation_fee, max_setup_rent)
    }
    pub fn new_policy(ctx: Context<NewPolicy>, args: NewPolicyArgs) -> Result<()> {
        initialization::new_policy(ctx, args)
    }
    pub fn set_pause(ctx: Context<RoleAuthority>, prices: bool, paused: bool) -> Result<()> {
        initialization::set_pause(ctx, prices, paused)
    }
    pub fn set_reporter(ctx: Context<RoleAuthority>, reporter: Pubkey) -> Result<()> {
        initialization::set_reporter(ctx, reporter)
    }
    pub fn review_jupiter(
        ctx: Context<ReviewJupiter>,
        deploy_slot: u64,
        programdata_len: u32,
        elf_sha256: [u8; 32],
    ) -> Result<()> {
        initialization::review_jupiter(ctx, deploy_slot, programdata_len, elf_sha256)
    }
    pub fn roll_pool(ctx: Context<RollPool>, max_setup_rent: u64) -> Result<()> {
        initialization::roll_pool(ctx, max_setup_rent)
    }
}
