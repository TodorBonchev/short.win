use anchor_lang::prelude::*;

#[event]
pub struct MarketRegistered {
    pub market: Pubkey,
    pub pool: Pubkey,
    pub mint: Pubkey,
    pub creator: Pubkey,
    pub token_program: Pubkey,
    pub generation: u64,
    pub slot: u64,
    pub ts: i64,
}
#[event]
pub struct Deposit {
    pub pool: Pubkey,
    pub owner: Pubkey,
    pub tokens: u64,
    pub shares: u128,
    pub total_shares: u128,
    pub cash: u64,
    pub principal_out: u64,
}
#[event]
pub struct Withdraw {
    pub pool: Pubkey,
    pub owner: Pubkey,
    pub tokens: u64,
    pub sol: u64,
    pub shares: u128,
    pub total_shares: u128,
    pub cash: u64,
    pub principal_out: u64,
}
#[event]
pub struct SolClaimed {
    pub pool: Pubkey,
    pub owner: Pubkey,
    pub sol: u64,
}
#[event]
pub struct PoolRolled {
    pub market: Pubkey,
    pub old_pool: Pubkey,
    pub new_pool: Pubkey,
    pub generation: u64,
}
#[event]
pub struct PolicyCreated {
    pub policy: Pubkey,
    pub version: u64,
}
#[event]
pub struct ConfigChanged {
    pub kind: u8,
}

#[event]
pub struct Opened {
    pub market: Pubkey,
    pub pool: Pubkey,
    pub loan: Pubkey,
    pub borrower: Pubkey,
    pub policy: Pubkey,
    pub nonce: u64,
    pub quantity: u64,
    pub sale_proceeds: u64,
    pub margin: u64,
    pub collateral: u64,
    pub opening_notional: u64,
    pub term_fee: u64,
    pub lender_fee: u64,
    pub protocol_fee: u64,
    pub deadline: i64,
    pub default_at: i64,
    pub slot: u64,
    pub ts: i64,
}
#[event]
pub struct Repaid {
    pub pool: Pubkey,
    pub loan: Pubkey,
    pub borrower: Pubkey,
    pub quantity: u64,
    pub collateral_refund: u64,
    pub slot: u64,
    pub ts: i64,
}
#[event]
pub struct Closed {
    pub pool: Pubkey,
    pub loan: Pubkey,
    pub borrower: Pubkey,
    pub quantity: u64,
    pub buyback_cost: u64,
    pub token_surplus: u64,
    pub collateral_refund: u64,
    pub slot: u64,
    pub ts: i64,
}
#[event]
pub struct ForcedExit {
    pub pool: Pubkey,
    pub loan: Pubkey,
    pub borrower: Pubkey,
    pub keeper: Pubkey,
    pub quantity: u64,
    pub kind: u8,
    pub buyback_cost: u64,
    pub token_surplus: u64,
    pub keeper_reward: u64,
    pub collateral_refund: u64,
    pub slot: u64,
    pub ts: i64,
}
#[event]
pub struct Defaulted {
    pub pool: Pubkey,
    pub loan: Pubkey,
    pub borrower: Pubkey,
    pub keeper: Pubkey,
    pub quantity_lost: u64,
    pub lender_sol: u64,
    pub keeper_reward: u64,
    pub borrower_collateral_refund: u64,
    pub slot: u64,
    pub ts: i64,
}
#[event]
pub struct MarginAdded {
    pub loan: Pubkey,
    pub borrower: Pubkey,
    pub amount: u64,
    pub collateral: u64,
}
#[event]
pub struct ReferencePublished {
    pub reference: Pubkey,
    pub market: Pubkey,
    pub policy: Pubkey,
    pub quantity: u64,
    pub sequence: u64,
    pub source_slot: u64,
    pub source_ts: i64,
}

#[event]
pub struct LiquidationArmed {
    pub loan: Pubkey,
    pub sequence: u64,
    pub reporter_epoch: u64,
    pub timestamp: i64,
    pub slot: u64,
    pub armed: bool,
}
