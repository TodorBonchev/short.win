//! Actual token-program instruction helpers. Each mint/account identity can be
//! deterministic; helper calls use the environment's ordinary transaction path.
use crate::{assert_ok, Env};
use anchor_lang::prelude::Pubkey;
use litesvm_token::spl_token::instruction;
use solana_keypair::Keypair;
use solana_signer::Signer;
use solana_system_interface::instruction as system;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenProgram {
    Spl,
    Token2022,
}
impl TokenProgram {
    pub const BOTH: [Self; 2] = [Self::Spl, Self::Token2022];
    pub fn id(self) -> Pubkey {
        match self {
            Self::Spl => {
                anchor_lang::prelude::pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA")
            }
            Self::Token2022 => {
                anchor_lang::prelude::pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb")
            }
        }
    }
}
pub fn create_mint(
    env: &mut Env,
    mint: &Keypair,
    authority: &Pubkey,
    freeze: Option<&Pubkey>,
    decimals: u8,
    program: TokenProgram,
) -> Pubkey {
    let key = mint.pubkey();
    let rent = env.svm.minimum_balance_for_rent_exemption(82);
    let create = system::create_account(&env.payer.pubkey(), &key, rent, 82, &program.id());
    let init = instruction::initialize_mint2(&program.id(), &key, authority, freeze, decimals)
        .expect("mint instruction");
    assert_ok(env.send(&[create, init], &[mint]));
    key
}
pub fn create_account(
    env: &mut Env,
    account: &Keypair,
    mint: &Pubkey,
    owner: &Pubkey,
    program: TokenProgram,
) -> Pubkey {
    let key = account.pubkey();
    let rent = env.svm.minimum_balance_for_rent_exemption(165);
    let create = system::create_account(&env.payer.pubkey(), &key, rent, 165, &program.id());
    let init = instruction::initialize_account3(&program.id(), &key, mint, owner)
        .expect("account instruction");
    assert_ok(env.send(&[create, init], &[account]));
    key
}
pub fn mint_to(
    env: &mut Env,
    mint: &Pubkey,
    destination: &Pubkey,
    authority: &Keypair,
    amount: u64,
    program: TokenProgram,
) {
    let ix = instruction::mint_to(
        &program.id(),
        mint,
        destination,
        &authority.pubkey(),
        &[],
        amount,
    )
    .expect("mint_to instruction");
    assert_ok(env.send(&[ix], &[authority]));
}
pub fn wrap_sol(env: &mut Env, destination: &Pubkey, amount: u64) {
    let transfer = system::transfer(&env.payer.pubkey(), destination, amount);
    let sync = instruction::sync_native(&TokenProgram::Spl.id(), destination).expect("sync native");
    assert_ok(env.send(&[transfer, sync], &[]));
}
