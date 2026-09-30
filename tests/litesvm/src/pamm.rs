//! PumpSwap **markets** on the mainnet fork (M6; ARCH §2.4.1 PumpSwap row, §2.5 "On → PumpSwap",
//! §3.2, §3.12 PumpSwap, §5.4): the release short.win trading against the **dumped** PumpSwap
//! (with pump and pump-fees) programs and canonical pools of [`crate::fork`] — groups `pamm_state`
//! and `pamm_synthetic` (R-VEN Update 2026-09-30 H10: five pool roles at slot 451,784,607 and the
//! synthetic negative-VQR pool), and the graduated coins of `pump_state` — or a pool a real
//! `migrate_v2` just created.
//!
//! - [`Env::fork_pamm_coin`] loads a [`PammCoin`] (release short.win, the pump programs and
//!   statics, PumpSwap's statics, the role's accounts, mainnet's Rent, the `pamm_state` clock);
//!   [`Env::plant_negative_vqr`] replaces the `mature` pool with its synthetic negative-VQR copy.
//! - [`PammMarket`]: a market's keys and its venue's account lists in ARCH §3.12's order (ReadSet
//!   P0–P5, TradeExtras T0–T18), built from the fixture's or the live pool's keys and the
//!   harness's own derivations, never from the program's key functions; it implements
//!   [`VenueMarket`], so every builder of [`crate::engine`] works on it. [`PammMarket::graduated`]
//!   is a curve market's after its `→ PumpSwap` transition.
//! - Listing: [`create_pamm_market_ix`] (`create_market(Auto)` with the pAMM init set) and
//!   [`Env::pamm_market`]; the existence requirement's accounts ([`venue_account_ixs`]).
//! - PumpSwap's own instructions for the tests: [`pamm_trade_ix`] (`buy` / `sell` with R-VEN §2.2's
//!   exact account order), [`PammUser`] (a normal trader), [`pamm_init_uva_ix`],
//!   [`pamm_claim_cashback_ix`] (no signature of `user`, T-CASHBACK), [`pamm_liquidity_ix`]
//!   (`deposit` / `withdraw`, H11), [`set_disable_flags`] (T-DISABLE), [`sweep_trader_pamm_ix`].
//!
//! Every instruction goes through [`Env::send`] (the invariant hooks, which see PumpSwap markets
//! through `invariants::venue_view`).

use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use litesvm::LiteSVM;
use shortwin::constants::{
    ASSOCIATED_TOKEN_PROGRAM_ID, PUMP_AMM_EVENT_AUTHORITY, PUMP_AMM_FEE_CONFIG,
    PUMP_AMM_GLOBAL_CONFIG, PUMP_AMM_GLOBAL_VOLUME_ACCUMULATOR, PUMP_AMM_PROGRAM_ID,
    PUMP_EVENT_AUTHORITY, PUMP_FEES_PROGRAM_ID, PUMP_PROGRAM_ID, TOKEN_2022_PROGRAM_ID,
    TOKEN_PROGRAM_ID, WSOL_MINT,
};
use shortwin::instructions::create_market::VENUE_HINT_AUTO;
use shortwin_venue_state::{pump_amm, token_account};
use solana_keypair::Keypair;
use solana_signer::Signer;

use crate::engine::VenueMarket;
use crate::fixtures::MarketKeys;
use crate::fork;
use crate::ix::{self, LenderKeys};
use crate::pump::{PumpMarket, wsol_ata};
use crate::tokens::TokenProgram;
use crate::{Env, TxResult, errors, pda, read};

/// PumpSwap `buy`'s discriminator (R-VEN §2.1).
pub const PAMM_BUY: [u8; 8] = [0x66, 0x06, 0x3d, 0x12, 0x01, 0xda, 0xeb, 0xea];
/// PumpSwap `sell`'s discriminator.
pub const PAMM_SELL: [u8; 8] = [0x33, 0xe6, 0x85, 0xa4, 0x01, 0x7f, 0x83, 0xad];
/// `init_user_volume_accumulator` (the same Anchor name on pump and PumpSwap).
pub const PAMM_INIT_UVA: [u8; 8] = [0x5e, 0x06, 0xca, 0x73, 0xff, 0x60, 0xe8, 0xb7];
/// `claim_cashback` (the same Anchor name on pump and PumpSwap).
pub const PAMM_CLAIM_CASHBACK: [u8; 8] = [0x25, 0x3a, 0x23, 0x7e, 0xbe, 0x35, 0xe4, 0xc5];
/// `extend_account` (the same Anchor name on pump and PumpSwap).
pub const EXTEND_ACCOUNT: [u8; 8] = [0xea, 0x66, 0xc2, 0xcb, 0x96, 0x48, 0x3e, 0xe5];
/// `GlobalConfig.disable_flags` (u8 @56).
pub const GC_DISABLE_FLAGS: usize = 56;
/// PumpSwap's `DisabledBuy` (T-DISABLE, bit 3).
pub const PAMM_ERR_DISABLED_BUY: u32 = 6020;
/// PumpSwap's `DisabledSell` (bit 4).
pub const PAMM_ERR_DISABLED_SELL: u32 = 6021;
/// The compute-unit limit the harness requests for PumpSwap opens and closes (a PumpSwap trade is
/// 79–92 k CU on the ELF, R-VEN Update H5, on top of short.win's own).
pub const PAMM_TRADE_CU: u32 = 300_000;

fn m(key: Pubkey, writable: bool) -> AccountMeta {
    if writable {
        AccountMeta::new(key, false)
    } else {
        AccountMeta::new_readonly(key, false)
    }
}

fn pamm_pda(seeds: &[&[u8]]) -> Pubkey {
    Pubkey::find_program_address(seeds, &PUMP_AMM_PROGRAM_ID).0
}

/// `PDA(pAMM, ["user_volume_accumulator", user])`.
pub fn pamm_uva(user: &Pubkey) -> Pubkey {
    pamm_pda(&[b"user_volume_accumulator", user.as_ref()])
}

/// `PDA(pAMM, ["creator_vault", coin_creator])` (underscore).
pub fn coin_creator_vault_authority(coin_creator: &Pubkey) -> Pubkey {
    pamm_pda(&[b"creator_vault", coin_creator.as_ref()])
}

/// `PDA(pAMM, ["pool-v2", mint])`.
pub fn pool_v2(mint: &Pubkey) -> Pubkey {
    pamm_pda(&[b"pool-v2", mint.as_ref()])
}

/// `PDA(pAMM, ["boost_vault", pool])`.
pub fn boost_vault(pool: &Pubkey) -> Pubkey {
    pamm_pda(&[b"boost_vault", pool.as_ref()])
}

fn global_config(svm: &LiteSVM) -> Vec<u8> {
    svm.get_account(&PUMP_AMM_GLOBAL_CONFIG)
        .expect("PumpSwap GlobalConfig")
        .data
}

/// Protocol-fee recipient `i` (0–7) of the live `GlobalConfig`.
pub fn protocol_fee_recipient(svm: &LiteSVM, i: usize) -> Pubkey {
    let d = global_config(svm);
    let g = pump_amm::read_global_config(&pump_amm::PROGRAM, &d).expect("GlobalConfig");
    Pubkey::new_from_array(g.protocol_fee_recipients()[i])
}

/// Buyback-fee recipient `i` (0–7) of the live `GlobalConfig`.
pub fn buyback_fee_recipient(svm: &LiteSVM, i: usize) -> Pubkey {
    let d = global_config(svm);
    let g = pump_amm::read_global_config(&pump_amm::PROGRAM, &d).expect("GlobalConfig");
    Pubkey::new_from_array(g.buyback_fee_recipients()[i])
}

/// Writes `GlobalConfig.disable_flags` (what the admin's `disable` writes, T-DISABLE; a byte-level
/// plant of the venue's account, as the Raydium admin builders).
pub fn set_disable_flags(svm: &mut LiteSVM, flags: u8) {
    fork::patch(svm, &PUMP_AMM_GLOBAL_CONFIG, |d| {
        d[GC_DISABLE_FLAGS] = flags
    });
}

// ---- the coin creator (M6-SEC-4) ----------------------------------------------------------------

/// Metaplex Token Metadata (`metaqbxx…`), whose metadata account pAMM's `set_coin_creator` reads.
pub const METAPLEX_PROGRAM_ID: Pubkey =
    anchor_lang::pubkey!("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s");

/// The Metaplex metadata account of the `legacy211` mint (`G8ZmGTba…pump`), as mainnet holds it
/// (`getAccountInfo` at slot 451,858,756; owner Metaplex, 607 bytes, 3,733,800 lamports; key 4
/// `MetadataV1`, update authority pump's `TSLvdd1p…`, one creator `4E9s4kuE…` with a 100 % share).
/// It is not part of `tests/fixtures/` (no fixture group dumps metadata); the M6-SEC-4 tests plant
/// it, as mainnet state, for `set_coin_creator` only.
pub const LEGACY211_METADATA_B64: &str = "BAbFwc5jjSVn0mRosF65UdGijcxuEjSCtcZ1FJdw5ivy4M9qQGdUj+3cqpXSV8syj2tINABGyXwAPny5ZOdUtO8gAAAAVGhlIFRyZW5jaGVzIFNhdmlvcgAAAAAAAAAAAAAAAAAKAAAAM1E2cG9kAAAAAMgAAABodHRwczovL2lwZnMuaW8vaXBmcy9RbWFHTFY5RFNVUkZ4QjFXa0hTUFdoUEM3YWVVOEFnQnc5OU02WmdoWGtZVXNkAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAQEAAAAv8aIH571lqZ/pNJ+7Y9KeoYWdow3mZ/ZguXP35RifxgBkAAAB/wECAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==";

/// The one creator in [`LEGACY211_METADATA_B64`]: what `set_coin_creator` gives the `legacy211`
/// pool.
pub const LEGACY211_METADATA_CREATOR: Pubkey =
    anchor_lang::pubkey!("4E9s4kuEXzppwNY6DRqLUJ8wPoZfbrdUhYXYjHVx5fJ5");

/// `PDA(Metaplex, ["metadata", Metaplex, mint])`.
pub fn metaplex_metadata(mint: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"metadata", METAPLEX_PROGRAM_ID.as_ref(), mint.as_ref()],
        &METAPLEX_PROGRAM_ID,
    )
    .0
}

/// Plants [`LEGACY211_METADATA_B64`] at the `legacy211` mint's metadata address.
pub fn plant_legacy211_metadata(env: &mut Env) {
    let mint = PammCoin::Legacy211.mint();
    let data = fork::base64_decode(LEGACY211_METADATA_B64).expect("metadata base64");
    env.svm
        .set_account(
            metaplex_metadata(&mint),
            solana_account::Account {
                lamports: 3_733_800,
                data,
                owner: METAPLEX_PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        )
        .expect("metadata");
}

/// pAMM `set_coin_creator` of `p` (PPD IDL: `[pool W, metadata, bonding_curve, event_authority,
/// program]`, **no signer**; "Sets Pool::coin_creator from Metaplex metadata creator or
/// BondingCurve::creator").
pub fn set_coin_creator_ix(p: &PammPool) -> Instruction {
    Instruction {
        program_id: PUMP_AMM_PROGRAM_ID,
        accounts: vec![
            m(p.pool, true),
            m(metaplex_metadata(&p.mint), false),
            m(pda::bonding_curve(&p.mint), false),
            m(PUMP_AMM_EVENT_AUTHORITY, false),
            m(PUMP_AMM_PROGRAM_ID, false),
        ],
        data: fork::sha256(b"global:set_coin_creator")[..8].to_vec(),
    }
}

// ---- pools -------------------------------------------------------------------------------------

/// The PumpSwap pool roles of the `pamm_state` dump (R-VEN Update 2026-09-30 H10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PammCoin {
    /// Token-2022, 301 B, 65 s after its migration: the boost vault still holds 13.98 SOL.
    Fresh,
    /// e/acc (Token-2022, 301 B): boost spent, 7.02 % third-party LP (H11).
    Mature,
    /// A cashback coin (Token-2022, 301 B).
    Cashback,
    /// SPL Token, a 211-byte pool (no coin creator, VQR absent) and a 49-byte curve.
    Legacy211,
    /// SPL Token, a 261-byte pool never extended, a 256-byte curve.
    Legacy261,
}

impl PammCoin {
    pub const ALL: [Self; 5] = [
        Self::Fresh,
        Self::Mature,
        Self::Cashback,
        Self::Legacy211,
        Self::Legacy261,
    ];

    /// The manifest role.
    pub fn role(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::Mature => "mature",
            Self::Cashback => "cashback",
            Self::Legacy211 => "legacy211",
            Self::Legacy261 => "legacy261",
        }
    }

    /// The role's mint.
    pub fn mint(self) -> Pubkey {
        fork::pamm_role_mint(self.role())
    }
}

/// A canonical PumpSwap pool's keys (read from the pool account).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PammPool {
    pub mint: Pubkey,
    /// The base mint's token program.
    pub base_program: Pubkey,
    pub pool: Pubkey,
    pub base_vault: Pubkey,
    pub quote_vault: Pubkey,
    pub lp_mint: Pubkey,
    pub boost_vault: Pubkey,
    pub boost_vault_ata: Pubkey,
}

impl PammPool {
    /// The canonical pool of `mint` as the VM holds it (panics if there is none).
    pub fn of(svm: &LiteSVM, mint: &Pubkey) -> Self {
        let pool = pda::canonical_pool(mint);
        let acc = svm.get_account(&pool).expect("canonical pool");
        let p = pump_amm::read_pool(&acc.owner.to_bytes(), &acc.data).expect("Pool");
        let base_program = svm.get_account(mint).expect("mint").owner;
        let bv = boost_vault(&pool);
        Self {
            mint: *mint,
            base_program,
            pool,
            base_vault: Pubkey::new_from_array(p.pool_base_token_account),
            quote_vault: Pubkey::new_from_array(p.pool_quote_token_account),
            lp_mint: Pubkey::new_from_array(p.lp_mint),
            boost_vault: bv,
            boost_vault_ata: wsol_ata(&bv),
        }
    }

    /// The pool as the reader sees it now.
    pub fn state(&self, svm: &LiteSVM) -> pump_amm::Pool {
        let acc = svm.get_account(&self.pool).expect("pool");
        pump_amm::read_pool(&acc.owner.to_bytes(), &acc.data).expect("Pool")
    }

    /// PumpSwap's pricing reserves now (vaults + VQR).
    pub fn reserves(&self, svm: &LiteSVM) -> Option<pump_amm::PoolReserves> {
        self.state(svm).reserves(
            fork::vault_amount(svm, &self.base_vault),
            fork::vault_amount(svm, &self.quote_vault),
        )
    }

    /// The boost vault's WSOL as the boost gate reads it (0 if the ATA is missing).
    pub fn boost_amount(&self, svm: &LiteSVM) -> u64 {
        let (owner, data) = svm
            .get_account(&self.boost_vault_ata)
            .map_or(([0u8; 32], Vec::new()), |a| (a.owner.to_bytes(), a.data));
        pump_amm::boost_vault_amount(&owner, &data, &self.boost_vault.to_bytes())
            .expect("boost vault ATA")
    }

    /// The pool's live coin creator.
    pub fn coin_creator(&self, svm: &LiteSVM) -> Pubkey {
        Pubkey::new_from_array(self.state(svm).coin_creator)
    }

    /// The base token program as the harness's enum.
    pub fn token_program(&self) -> TokenProgram {
        if self.base_program == TOKEN_2022_PROGRAM_ID {
            TokenProgram::Token2022
        } else {
            TokenProgram::Spl
        }
    }

    /// Spot `⌊(quote + VQR)·WAD/base⌋`.
    pub fn spot_wad(&self, svm: &LiteSVM) -> u128 {
        let r = self.reserves(svm).expect("reserves");
        r.effective_quote * shortwin_math::num::WAD / u128::from(r.base)
    }
}

// ---- PumpSwap's own instructions ---------------------------------------------------------------

/// PumpSwap's `init_user_volume_accumulator(payer, user)` (`user` need not sign; R-VEN Update H6).
pub fn pamm_init_uva_ix(payer: &Pubkey, user: &Pubkey) -> Instruction {
    Instruction {
        program_id: PUMP_AMM_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(*payer, true),
            m(*user, false),
            m(pamm_uva(user), true),
            m(anchor_lang::system_program::ID, false),
            m(PUMP_AMM_EVENT_AUTHORITY, false),
            m(PUMP_AMM_PROGRAM_ID, false),
        ],
        data: PAMM_INIT_UVA.to_vec(),
    }
}

/// PumpSwap's `claim_cashback` for `user` into its WSOL account `user_wsol` (IDL: 9 accounts,
/// `user` writable and **not** a signer — T-CASHBACK: anyone may send it; the whole balance of
/// `ATA(UVA(user), WSOL)` moves to `user_wsol`, R-VEN Update H7).
pub fn pamm_claim_cashback_ix(user: &Pubkey, user_wsol: &Pubkey) -> Instruction {
    let u = pamm_uva(user);
    Instruction {
        program_id: PUMP_AMM_PROGRAM_ID,
        accounts: vec![
            m(*user, true),
            m(u, true),
            m(WSOL_MINT, false),
            m(TOKEN_PROGRAM_ID, false),
            m(wsol_ata(&u), true),
            m(*user_wsol, true),
            m(anchor_lang::system_program::ID, false),
            m(PUMP_AMM_EVENT_AUTHORITY, false),
            m(PUMP_AMM_PROGRAM_ID, false),
        ],
        data: PAMM_CLAIM_CASHBACK.to_vec(),
    }
}

/// pump's `extend_account` of `account` (`[account W, user S W, system, event_authority,
/// program]`): a curve under 115 bytes must be extended before a listing (`CurveNeedsExtend`).
pub fn pump_extend_account_ix(user: &Pubkey, account: &Pubkey) -> Instruction {
    Instruction {
        program_id: PUMP_PROGRAM_ID,
        accounts: vec![
            m(*account, true),
            AccountMeta::new(*user, true),
            m(anchor_lang::system_program::ID, false),
            m(PUMP_EVENT_AUTHORITY, false),
            m(PUMP_PROGRAM_ID, false),
        ],
        data: EXTEND_ACCOUNT.to_vec(),
    }
}

/// A PumpSwap trade direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PammTrade {
    /// `buy(base_amount_out, max_quote_amount_in, [false])`.
    Buy,
    /// `sell(base_amount_in, min_quote_amount_out)`.
    Sell,
}

/// PumpSwap `buy` / `sell` by `user` (R-VEN §2.2: 23 / 21 fixed accounts, then the cashback ATA
/// (buy) or the cashback ATA and the UVA (sell) on a cashback coin, the `pool-v2` marker when the
/// coin creator is set, the buyback recipient and its WSOL ATA); the first protocol and buyback
/// recipients of the live `GlobalConfig`.
#[allow(clippy::too_many_arguments)]
pub fn pamm_trade_ix(
    svm: &LiteSVM,
    p: &PammPool,
    kind: PammTrade,
    user: &Pubkey,
    user_base: &Pubkey,
    user_quote: &Pubkey,
    a: u64,
    b: u64,
) -> Instruction {
    let pool = p.state(svm);
    let protocol = protocol_fee_recipient(svm, 0);
    let buyback = buyback_fee_recipient(svm, 0);
    let coin_creator = Pubkey::new_from_array(pool.coin_creator);
    let va = coin_creator_vault_authority(&coin_creator);
    let u = pamm_uva(user);
    let mut acc = vec![
        m(p.pool, true),
        AccountMeta::new(*user, true),
        m(PUMP_AMM_GLOBAL_CONFIG, false),
        m(p.mint, false),
        m(WSOL_MINT, false),
        m(*user_base, true),
        m(*user_quote, true),
        m(p.base_vault, true),
        m(p.quote_vault, true),
        m(protocol, false),
        m(wsol_ata(&protocol), true),
        m(p.base_program, false),
        m(TOKEN_PROGRAM_ID, false),
        m(anchor_lang::system_program::ID, false),
        m(ASSOCIATED_TOKEN_PROGRAM_ID, false),
        m(PUMP_AMM_EVENT_AUTHORITY, false),
        m(PUMP_AMM_PROGRAM_ID, false),
        m(wsol_ata(&va), true),
        m(va, false),
    ];
    if kind == PammTrade::Buy {
        acc.push(m(PUMP_AMM_GLOBAL_VOLUME_ACCUMULATOR, false));
        acc.push(m(u, true));
    }
    acc.push(m(PUMP_AMM_FEE_CONFIG, false));
    acc.push(m(PUMP_FEES_PROGRAM_ID, false));
    if pool.is_cashback_coin {
        acc.push(m(wsol_ata(&u), true));
        if kind == PammTrade::Sell {
            acc.push(m(u, true));
        }
    }
    if coin_creator != Pubkey::default() {
        acc.push(m(pool_v2(&p.mint), false));
    }
    acc.push(m(buyback, false));
    acc.push(m(wsol_ata(&buyback), true));
    let mut data = match kind {
        PammTrade::Buy => PAMM_BUY.to_vec(),
        PammTrade::Sell => PAMM_SELL.to_vec(),
    };
    data.extend_from_slice(&a.to_le_bytes());
    data.extend_from_slice(&b.to_le_bytes());
    if kind == PammTrade::Buy {
        data.push(0);
    }
    Instruction {
        program_id: PUMP_AMM_PROGRAM_ID,
        accounts: acc,
        data,
    }
}

/// PumpSwap `deposit(lp_out, max_base_in, max_quote_in)` or `withdraw(lp_in, min_base_out,
/// min_quote_out)` by `user` (IDL: 15 accounts; the LP mint and account under Token-2022; R-VEN
/// Update H11: anyone can add liquidity to a canonical pool and take it out again).
#[allow(clippy::too_many_arguments)]
pub fn pamm_liquidity_ix(
    p: &PammPool,
    deposit: bool,
    user: &Pubkey,
    user_base: &Pubkey,
    user_quote: &Pubkey,
    user_lp: &Pubkey,
    a: u64,
    b: u64,
    c: u64,
) -> Instruction {
    let name: &[u8] = if deposit {
        b"global:deposit"
    } else {
        b"global:withdraw"
    };
    let mut data = fork::sha256(name)[..8].to_vec();
    for x in [a, b, c] {
        data.extend_from_slice(&x.to_le_bytes());
    }
    Instruction {
        program_id: PUMP_AMM_PROGRAM_ID,
        accounts: vec![
            m(p.pool, true),
            m(PUMP_AMM_GLOBAL_CONFIG, false),
            AccountMeta::new_readonly(*user, true),
            m(p.mint, false),
            m(WSOL_MINT, false),
            m(p.lp_mint, true),
            m(*user_base, true),
            m(*user_quote, true),
            m(*user_lp, true),
            m(p.base_vault, true),
            m(p.quote_vault, true),
            m(TOKEN_PROGRAM_ID, false),
            m(TOKEN_2022_PROGRAM_ID, false),
            m(PUMP_AMM_EVENT_AUTHORITY, false),
            m(PUMP_AMM_PROGRAM_ID, false),
        ],
        data,
    }
}

/// A normal PumpSwap trader: a funded keypair with its base ATA, a WSOL account and its pAMM UVA
/// (created through `Env::send` before any trade).
pub struct PammUser {
    pub kp: Keypair,
    pub base: Pubkey,
    pub wsol: Pubkey,
}

impl PammUser {
    /// A new trader of `p` holding `wsol` wrapped lamports.
    pub fn new(env: &mut Env, p: &PammPool, wsol: u64) -> Self {
        let kp = env.funded_keypair();
        let payer = env.payer.pubkey();
        let ixs = [
            fork::create_ata_idempotent_ix(&payer, &kp.pubkey(), &p.mint, &p.base_program),
            pamm_init_uva_ix(&payer, &kp.pubkey()),
        ];
        errors::ok(env.send(&ixs, &[]));
        let w = fork::fund_wsol(env, &kp.pubkey(), wsol);
        Self {
            base: pda::ata(&kp.pubkey(), &p.mint, &p.base_program),
            wsol: w,
            kp,
        }
    }

    /// `buy(base_out, max_in)`.
    pub fn buy(&self, env: &mut Env, p: &PammPool, base_out: u64, max_in: u64) -> TxResult {
        let ix = pamm_trade_ix(
            &env.svm,
            p,
            PammTrade::Buy,
            &self.kp.pubkey(),
            &self.base,
            &self.wsol,
            base_out,
            max_in,
        );
        let kp = self.kp.insecure_clone();
        env.send(&[ix], &[&kp])
    }

    /// `sell(base_in, min_out)` (plants the base balance first if short).
    pub fn sell(&self, env: &mut Env, p: &PammPool, base_in: u64, min_out: u64) -> TxResult {
        let have = read::token_amount(&env.svm, &self.base).unwrap_or(0);
        if have < base_in {
            fork::set_token_amount(&mut env.svm, &self.base, base_in);
        }
        let ix = pamm_trade_ix(
            &env.svm,
            p,
            PammTrade::Sell,
            &self.kp.pubkey(),
            &self.base,
            &self.wsol,
            base_in,
            min_out,
        );
        let kp = self.kp.insecure_clone();
        env.send(&[ix], &[&kp])
    }

    /// Buys until the pool's spot is `×num/den` its current one (constant product: the base
    /// reserve falls by `√(den/num)`).
    pub fn push_price(&self, env: &mut Env, p: &PammPool, num: u128, den: u128) -> TxResult {
        let r = p.reserves(&env.svm).expect("reserves");
        let base = u128::from(r.base);
        let target = (base * base * den / num).isqrt();
        let out = u64::try_from(base - target).expect("u64");
        self.buy(env, p, out, u64::MAX)
    }
}

// ---- markets -----------------------------------------------------------------------------------

/// A PumpSwap market on the fork: its keys, its pool, its lender and the recipients its trades
/// name.
pub struct PammMarket {
    pub keys: MarketKeys,
    pub pool: PammPool,
    /// `ATA(trader, mint)`.
    pub trader_base_ata: Pubkey,
    /// The protocol-fee and buyback recipients the trades name (from the live `GlobalConfig`).
    pub protocol_recipient: Pubkey,
    pub buyback_recipient: Pubkey,
    /// The coin creator the trades name (re-read with [`PammMarket::refresh_creator`]).
    pub coin_creator: Pubkey,
    /// The lender that seeded the market (and its token account).
    pub lender: Keypair,
    pub lender_account: Pubkey,
}

impl PammMarket {
    /// The PumpSwap market of `mint` on the VM's canonical pool.
    pub fn new(svm: &LiteSVM, mint: &Pubkey, lender: Keypair, lender_account: Pubkey) -> Self {
        let pool = PammPool::of(svm, mint);
        let market = pda::market(mint).0;
        let keys = MarketKeys {
            mint: *mint,
            token_program: pool.base_program,
            market,
            market_vault: pda::market_vault(&market).0,
            pool_vault: pda::pool_vault(&market).0,
            trader: pda::trader(&market).0,
        };
        Self {
            trader_base_ata: pda::ata(&keys.trader, mint, &pool.base_program),
            protocol_recipient: protocol_fee_recipient(svm, 0),
            buyback_recipient: buyback_fee_recipient(svm, 0),
            coin_creator: pool.coin_creator(svm),
            pool,
            keys,
            lender,
            lender_account,
        }
    }

    /// A curve market's after its `→ PumpSwap` transition: the same market, lender and keys on
    /// the canonical pool `migrate_v2` created.
    pub fn graduated(svm: &LiteSVM, pm: &PumpMarket) -> Self {
        Self::new(
            svm,
            &pm.keys.mint,
            pm.lender.insecure_clone(),
            pm.lender_account,
        )
    }

    /// The market.
    pub fn market(&self) -> Pubkey {
        self.keys.market
    }

    /// The trader PDA.
    pub fn trader(&self) -> Pubkey {
        self.keys.trader
    }

    /// The trader's pAMM UVA (T14).
    pub fn trader_uva(&self) -> Pubkey {
        pamm_uva(&self.keys.trader)
    }

    /// `ATA(trader's pAMM UVA, WSOL)` (T18): the trader's cashback.
    pub fn cashback_ata(&self) -> Pubkey {
        wsol_ata(&self.trader_uva())
    }

    /// The coin-creator vault authority and its WSOL ATA the trades name (T12, T11).
    pub fn creator_vault(&self) -> (Pubkey, Pubkey) {
        let a = coin_creator_vault_authority(&self.coin_creator);
        (a, wsol_ata(&a))
    }

    /// Re-reads the pool's coin creator (after an admin change).
    pub fn refresh_creator(&mut self, svm: &LiteSVM) {
        self.coin_creator = self.pool.coin_creator(svm);
    }

    /// Chooses other recipients (protocol recipient `i`, buyback recipient `j`).
    pub fn use_recipients(&mut self, svm: &LiteSVM, i: usize, j: usize) {
        self.protocol_recipient = protocol_fee_recipient(svm, i);
        self.buyback_recipient = buyback_fee_recipient(svm, j);
    }

    /// The lender's keys for the lender builders.
    pub fn lender_keys(&self) -> LenderKeys {
        LenderKeys {
            owner: self.lender.pubkey(),
            mint: self.keys.mint,
            token_program: self.keys.token_program,
            owner_token_account: self.lender_account,
        }
    }

    /// The TradeExtras T0–T18 as a list of keys (ARCH §3.12 order).
    pub fn trade_keys(&self) -> Vec<Pubkey> {
        self.trade_extras().iter().map(|a| a.pubkey).collect()
    }
}

impl VenueMarket for PammMarket {
    fn keys(&self) -> &MarketKeys {
        &self.keys
    }

    /// P0 the pool (W), P1 its base vault (W), P2 its WSOL vault (W), P3 `GlobalConfig`, P4
    /// `FeeConfig`, P5 the boost vault's WSOL ATA.
    fn read_set(&self, writable: bool) -> Vec<AccountMeta> {
        vec![
            m(self.pool.pool, writable),
            m(self.pool.base_vault, writable),
            m(self.pool.quote_vault, writable),
            m(PUMP_AMM_GLOBAL_CONFIG, false),
            m(PUMP_AMM_FEE_CONFIG, false),
            m(self.pool.boost_vault_ata, false),
        ]
    }

    /// T0–T18 (ARCH §3.12).
    fn trade_extras(&self) -> Vec<AccountMeta> {
        let (auth, auth_ata) = self.creator_vault();
        let uva = self.trader_uva();
        vec![
            m(PUMP_AMM_PROGRAM_ID, false),
            m(PUMP_AMM_EVENT_AUTHORITY, false),
            m(PUMP_FEES_PROGRAM_ID, false),
            m(WSOL_MINT, false),
            m(TOKEN_PROGRAM_ID, false),
            m(ASSOCIATED_TOKEN_PROGRAM_ID, false),
            m(anchor_lang::system_program::ID, false),
            m(self.trader_base_ata, true),
            m(pda::trader_wsol(&self.keys.market).0, true),
            m(self.protocol_recipient, false),
            m(wsol_ata(&self.protocol_recipient), true),
            m(auth_ata, true),
            m(auth, false),
            m(PUMP_AMM_GLOBAL_VOLUME_ACCUMULATOR, false),
            m(uva, true),
            m(pool_v2(&self.keys.mint), false),
            m(self.buyback_recipient, false),
            m(wsol_ata(&self.buyback_recipient), true),
            m(wsol_ata(&uva), true),
        ]
    }

    fn cu_limit(&self) -> Option<u32> {
        Some(PAMM_TRADE_CU)
    }
}

/// The pAMM init set of a PumpSwap listing (after the detection set): `[pAMM program, the
/// trader's pAMM UVA (W), pAMM event authority]`.
pub fn pamm_init_set(market: &Pubkey) -> Vec<AccountMeta> {
    let trader = pda::trader(market).0;
    vec![
        m(PUMP_AMM_PROGRAM_ID, false),
        m(pamm_uva(&trader), true),
        m(PUMP_AMM_EVENT_AUTHORITY, false),
    ]
}

/// `create_market(Auto, u64::MAX)` of `mint` with the pAMM init set.
pub fn create_pamm_market_ix(
    creator: &Pubkey,
    mint: &Pubkey,
    token_program: &Pubkey,
) -> Instruction {
    let ix = ix::create_market(
        creator,
        mint,
        token_program,
        VENUE_HINT_AUTO,
        &pda::canonical_pool(mint),
    );
    let market = pda::market(mint).0;
    ix::with_remaining(ix, &pamm_init_set(&market))
}

/// The accounts ARCH §3.12's existence requirement names that a market's trades need and that may
/// not exist yet, created by `payer` (idempotent, any payer): the trader's pAMM UVA (T14), the
/// coin-creator vault's WSOL ATA (T11), the trader's cashback ATA (T18, cashback coins) and the
/// two recipients' WSOL ATAs (T10, T17).
pub fn venue_account_ixs(svm: &LiteSVM, payer: &Pubkey, pm: &PammMarket) -> Vec<Instruction> {
    let spl = TOKEN_PROGRAM_ID;
    let (auth, _) = pm.creator_vault();
    let mut ixs = Vec::new();
    if svm
        .get_account(&pm.trader_uva())
        .is_none_or(|a| a.data.is_empty())
    {
        ixs.push(pamm_init_uva_ix(payer, &pm.trader()));
    }
    ixs.push(fork::create_ata_idempotent_ix(
        payer, &auth, &WSOL_MINT, &spl,
    ));
    ixs.push(fork::create_ata_idempotent_ix(
        payer,
        &pm.protocol_recipient,
        &WSOL_MINT,
        &spl,
    ));
    ixs.push(fork::create_ata_idempotent_ix(
        payer,
        &pm.buyback_recipient,
        &WSOL_MINT,
        &spl,
    ));
    if pm.pool.state(svm).is_cashback_coin {
        ixs.push(fork::create_ata_idempotent_ix(
            payer,
            &pm.trader_uva(),
            &WSOL_MINT,
            &spl,
        ));
    }
    ixs
}

/// `sweep_trader()` of a PumpSwap market: the token leg `[trader_base_ata, pool_vault, mint, token
/// program, the trader's pump UVA, ATA(the trader's pAMM UVA, WSOL)]` (M6), then the latest
/// `ShareEpoch` record while a full-wipe record is live.
pub fn sweep_trader_pamm_ix(env: &Env, pm: &PammMarket) -> Instruction {
    let base = crate::engine::sweep_trader_ix_for(env, &pm.market());
    let mk = env.market(&pm.market());
    let live = mk.total_shares.get() == 0 && mk.has_flag(shortwin::state::MARKET_FLAG_WIPE_LIVE);
    let (head, rest) = base
        .accounts
        .split_at(base.accounts.len() - usize::from(live));
    let mut accounts = head.to_vec();
    accounts.extend([
        m(pm.trader_base_ata, true),
        m(pm.keys.pool_vault, true),
        m(pm.keys.mint, false),
        m(pm.keys.token_program, false),
        m(fork::pump_user_volume_accumulator(&pm.trader()), false),
        m(pm.cashback_ata(), false),
    ]);
    accounts.extend_from_slice(rest);
    Instruction {
        program_id: base.program_id,
        accounts,
        data: base.data,
    }
}

impl Env {
    /// Loads PumpSwap's statics of the `pamm_state` dump (`GlobalConfig`, `FeeConfig`, the global
    /// volume accumulator, WSOL, the 16 recipient WSOL ATAs; its Rent sysvar is
    /// [`Env::set_pump_rent`]'s, byte-identical).
    pub fn load_pamm_statics(&mut self) {
        let names: Vec<String> = fork::all_pamm_fixtures()
            .filter(|f| f.group == "pamm_state" && f.role.is_none())
            .filter(|f| !f.name.ends_with("sysvar_rent"))
            .map(|f| f.name.clone())
            .collect();
        for name in names {
            fork::load(&mut self.svm, &name);
        }
    }

    /// Loads every `pamm_state` account of `coin`'s role (pool, mint, vaults, LP mint, coin-creator
    /// vault and ATA, boost vault ATA, pool authority, the migrated curve). Returns the pool.
    pub fn load_pamm_coin(&mut self, coin: PammCoin) -> PammPool {
        let names: Vec<String> = fork::all_pamm_fixtures()
            .filter(|f| f.group == "pamm_state" && f.role.as_deref() == Some(coin.role()))
            .map(|f| f.name.clone())
            .collect();
        for name in names {
            fork::load(&mut self.svm, &name);
        }
        PammPool::of(&self.svm, &coin.mint())
    }

    /// The **synthetic** negative-VQR pool (`pamm_mature_pool_negative_vqr`: the `mature` pool with
    /// VQR = −⌊vault/2⌋) in place of the `mature` pool (same address).
    pub fn plant_negative_vqr(&mut self) {
        fork::load(&mut self.svm, "pamm_mature_pool_negative_vqr");
    }

    /// Sets the clock to the `pamm_state` snapshot's slot and block time.
    pub fn set_pamm_clock(&mut self) {
        let s = fork::pamm_snapshot();
        self.set_clock(s.block_time, s.slot);
    }

    /// [`Env::initialized`], the pump programs and statics, PumpSwap's statics, `coin`'s accounts,
    /// mainnet's Rent and the `pamm_state` clock.
    pub fn fork_pamm_coin(coin: PammCoin) -> (Self, PammPool) {
        let mut env = Self::initialized();
        env.load_pump_programs();
        env.load_pump_statics();
        env.load_pamm_statics();
        let p = env.load_pamm_coin(coin);
        env.set_pump_rent();
        env.set_pamm_clock();
        (env, p)
    }

    /// Lists `mint` with `create_market(Auto)` and the pAMM init set, as a new funded creator.
    pub fn list_pamm(&mut self, mint: &Pubkey, token_program: &Pubkey) -> TxResult {
        let creator = self.funded_keypair();
        let ix = create_pamm_market_ix(&creator.pubkey(), mint, token_program);
        self.send(&[ix], &[&creator])
    }

    /// The [`PammMarket`] of an already listed `mint`: a lender deposits `lend` (planted in its
    /// account first), the existence requirement's accounts are created, and `warmup` seconds pass
    /// with a crank each second (the price start and its warm-up, M6).
    pub fn pamm_market_listed(&mut self, mint: &Pubkey, lend: u64, warmup: i64) -> PammMarket {
        let program = if self.svm.get_account(mint).expect("mint").owner == TOKEN_2022_PROGRAM_ID {
            TokenProgram::Token2022
        } else {
            TokenProgram::Spl
        };
        let lender = self.funded_keypair();
        let lender_account = fork::fund_token(self, &lender.pubkey(), mint, program, lend);
        let pm = PammMarket::new(&self.svm, mint, lender, lender_account);
        if lend > 0 {
            let lender = pm.lender.insecure_clone();
            errors::ok(self.send(&[ix::deposit(&pm.lender_keys(), lend, 0)], &[&lender]));
        }
        let payer = self.payer.pubkey();
        let ixs = venue_account_ixs(&self.svm, &payer, &pm);
        errors::ok(self.send(&ixs, &[]));
        for _ in 0..warmup {
            self.warp(1);
            errors::ok(self.send(&[crate::engine::crank_price_ix(&pm)], &[]));
        }
        pm
    }

    /// A PumpSwap market on `coin` on a fresh fork: listed, `lend` tokens lent, 60 s of cranks
    /// (the listing warm-up `market_warmup_s` and the price's post-change warm-up).
    pub fn pamm_market(coin: PammCoin, lend: u64) -> (Self, PammMarket) {
        let (mut env, p) = Self::fork_pamm_coin(coin);
        let payer = env.payer.pubkey();
        if env
            .svm
            .get_account(&pda::bonding_curve(&p.mint))
            .is_some_and(|a| a.data.len() < shortwin_venue_state::pump::CURVE_MIN_LEN)
        {
            // A 2024-layout curve: the client prepends pump's `extend_account` (ARCH §3.2 check 3).
            let kp = env.payer.insecure_clone();
            errors::ok(env.send(
                &[pump_extend_account_ix(&payer, &pda::bonding_curve(&p.mint))],
                &[&kp],
            ));
        }
        errors::ok(env.list_pamm(&p.mint, &p.base_program));
        let pm = env.pamm_market_listed(&p.mint, lend, 60);
        (env, pm)
    }
}

/// The token-account state of `key` (`None` if it is not one).
pub fn token_state(svm: &LiteSVM, key: &Pubkey) -> Option<token_account::TokenAccount> {
    let acc = svm.get_account(key)?;
    token_account::read(&acc.owner.to_bytes(), &acc.data).ok()
}
