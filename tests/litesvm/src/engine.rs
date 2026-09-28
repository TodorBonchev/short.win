//! The position engine on the mock venue (M2): a ready-to-trade environment, mock markets,
//! borrowers and the instruction builders of every M2 instruction.
//!
//! - [`Env::test_venue`]: short.win's **test-venue** build (`VenueKind::Mock`; A49) with the mock
//!   venue deployed, the config initialized with the mock's venue bit enabled, and the mock's
//!   `VenueExtras` account (`["venue_extras", 6]`, which `initialize_config` creates only for the
//!   four real venues) installed empty.
//! - [`Env::mock_market`]: a mint, its mock pool with liquidity, `create_market(venue_hint =
//!   Mock)`, a lender deposit and the warm-up — through real instructions ([`MarketSpec`]).
//! - [`MockMarket`]: the market's keys and its remaining-account lists (ReadSet, TradeExtras).
//! - builders: [`open_short_ix`], [`close_short_ix`], [`crank_price_ix`],
//!   [`set_route_broken_ix`], [`set_venue_extras_ix`], [`sweep_trader_ix`] (M2-SEC-4), and the
//!   account lists of the pending tracks ([`add_margin_ix`], [`liquidate_ix`], [`expire_ix`],
//!   [`sol_settle_ix`]).
//! - [`Borrower`]: a funded borrower with its nonces; [`Env::open`] / [`Env::close`] run the real
//!   instructions with generous bounds.
//!
//! Every instruction goes through [`Env::send`], so the invariant hooks run after each one.

use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_lang::{InstructionData, ToAccountMetas};
use shortwin::constants::{
    ASSOCIATED_TOKEN_PROGRAM_ID, MOCK_CPAMM_AUTHORITY, MOCK_CPAMM_PROGRAM_ID, TOKEN_PROGRAM_ID,
    VENUE_BIT_MOCK, VENUE_EXTRAS_VERSION, WSOL_MINT,
};
use shortwin::state::{ShortPosition, VenueExtras, VenueKind};
use shortwin::venues::ExtrasSel;
use shortwin::{ExtraEntryArg, PauseScope};
use solana_keypair::Keypair;
use solana_signer::Signer;

use crate::fixtures::{self, MarketKeys};
use crate::ix::{self, LenderKeys};
use crate::mock::{self, MockPool, PoolFixture};
use crate::tokens::{self, TokenProgram};
use crate::{Env, TxResult, errors, pda, read};

/// `create_market`'s mock hint (`VenueKind::Mock as u8`).
pub const VENUE_HINT_MOCK: u8 = VenueKind::Mock as u8;

fn system_program() -> Pubkey {
    anchor_lang::system_program::ID
}

fn build(accounts: impl ToAccountMetas, data: impl InstructionData) -> Instruction {
    Instruction {
        program_id: shortwin::ID,
        accounts: accounts.to_account_metas(None),
        data: data.data(),
    }
}

impl Env {
    /// The ready-to-trade engine environment (module docs).
    pub fn test_venue() -> Self {
        let mut env = Self::new_test_venue_program();
        env.load_mock_cpamm();
        let mut args = env.default_init_args();
        args.venue_enabled |= VENUE_BIT_MOCK;
        errors::ok(env.initialize_config(args));
        env.install_mock_extras();
        env
    }

    /// Writes the mock's empty `VenueExtras` account (`["venue_extras", 6]`), as
    /// `initialize_config` writes the four real venues' (ARCH §2.3.2).
    pub fn install_mock_extras(&mut self) {
        let (key, bump) =
            Pubkey::find_program_address(&[VenueExtras::SEED, &[VENUE_HINT_MOCK]], &shortwin::ID);
        let mut ve: VenueExtras = bytemuck::Zeroable::zeroed();
        ve.version = VENUE_EXTRAS_VERSION;
        ve.bump = bump;
        ve.venue = VENUE_HINT_MOCK;
        fixtures::set_zc(&mut self.svm, &key, &ve, 0);
    }
}

/// The mock's `VenueExtras` account.
pub fn mock_extras_key() -> Pubkey {
    pda::venue_extras_seed(VENUE_HINT_MOCK)
}

// ---- markets ------------------------------------------------------------------------------

/// A mock market's parameters.
#[derive(Clone, Copy, Debug)]
pub struct MarketSpec {
    pub program: TokenProgram,
    pub decimals: u8,
    /// The mock's fee on the input, ppm (φ).
    pub fee_ppm: u32,
    /// Pool reserves: base tokens (raw) and SOL (lamports).
    pub base: u64,
    pub quote: u64,
    /// Tokens the lender deposits.
    pub lend: u64,
    /// Seconds to warp after listing (≥ `market_warmup_s` = 50 by default).
    pub warmup: i64,
}

impl Default for MarketSpec {
    /// 10⁹ tokens (6 decimals) against 1,000 SOL (`P = 10⁻³` lamport per raw unit), a 25 bps fee,
    /// 10 % of the pool lent, 60 s of warm-up.
    fn default() -> Self {
        Self {
            program: TokenProgram::Spl,
            decimals: 6,
            fee_ppm: 2_500,
            base: 1_000_000_000_000_000,
            quote: 1_000_000_000_000,
            lend: 100_000_000_000_000,
            warmup: 60,
        }
    }
}

/// A mock market: the pool, the market's keys and its lender.
pub struct MockMarket {
    pub pool: PoolFixture,
    pub keys: MarketKeys,
    pub spec: MarketSpec,
    pub trader_wsol: Pubkey,
    /// The lender that seeded the pool (and its token account).
    pub lender: Keypair,
    pub lender_account: Pubkey,
}

impl MockMarket {
    /// The mint.
    pub fn mint(&self) -> Pubkey {
        self.keys.mint
    }

    /// The market.
    pub fn market(&self) -> Pubkey {
        self.keys.market
    }

    /// The mock pool's keys.
    pub fn mock(&self) -> &MockPool {
        &self.pool.keys
    }

    /// The ReadSet `[pool, base_vault, quote_vault]` (writable in trades).
    pub fn read_set(&self, writable: bool) -> Vec<AccountMeta> {
        let m = |k: Pubkey| {
            if writable {
                AccountMeta::new(k, false)
            } else {
                AccountMeta::new_readonly(k, false)
            }
        };
        vec![
            m(self.pool.keys.pool),
            m(self.pool.keys.base_vault),
            m(self.pool.keys.quote_vault),
        ]
    }

    /// The TradeExtras `[mock program, pool authority, WSOL mint, SPL Token, trader_wsol (W)]`.
    pub fn trade_extras(&self) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new_readonly(MOCK_CPAMM_PROGRAM_ID, false),
            AccountMeta::new_readonly(MOCK_CPAMM_AUTHORITY, false),
            AccountMeta::new_readonly(WSOL_MINT, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
            AccountMeta::new(self.trader_wsol, false),
        ]
    }

    /// The trading remaining accounts: `ReadSet ++ TradeExtras ++ extras`.
    pub fn trade_remaining(&self, extras: &[AccountMeta]) -> Vec<AccountMeta> {
        let mut v = self.read_set(true);
        v.extend(self.trade_extras());
        v.extend_from_slice(extras);
        v
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
}

/// A market the instruction builders and [`Env::open`] / [`Env::close`] can trade: its keys and
/// its venue's remaining-account lists (ARCH §3.0: `ReadSet ++ TradeExtras ++ Extras`). The mock
/// markets of this module and the Raydium markets of [`crate::raydium`] implement it.
pub trait VenueMarket {
    /// The market's keys.
    fn keys(&self) -> &MarketKeys;
    /// The venue's ReadSet (writable in trades).
    fn read_set(&self, writable: bool) -> Vec<AccountMeta>;
    /// The venue's TradeExtras.
    fn trade_extras(&self) -> Vec<AccountMeta>;
    /// The market.
    fn market(&self) -> Pubkey {
        self.keys().market
    }
    /// The mint.
    fn mint(&self) -> Pubkey {
        self.keys().mint
    }
    /// The trading remaining accounts: `ReadSet ++ TradeExtras ++ extras`.
    fn trade_remaining(&self, extras: &[AccountMeta]) -> Vec<AccountMeta> {
        let mut v = self.read_set(true);
        v.extend(self.trade_extras());
        v.extend_from_slice(extras);
        v
    }
}

impl<T: VenueMarket + ?Sized> VenueMarket for &T {
    fn keys(&self) -> &MarketKeys {
        (**self).keys()
    }
    fn read_set(&self, writable: bool) -> Vec<AccountMeta> {
        (**self).read_set(writable)
    }
    fn trade_extras(&self) -> Vec<AccountMeta> {
        (**self).trade_extras()
    }
}

impl VenueMarket for MockMarket {
    fn keys(&self) -> &MarketKeys {
        &self.keys
    }
    fn read_set(&self, writable: bool) -> Vec<AccountMeta> {
        MockMarket::read_set(self, writable)
    }
    fn trade_extras(&self) -> Vec<AccountMeta> {
        MockMarket::trade_extras(self)
    }
}

/// `create_market(Mock, u64::MAX)` for `pool`'s base mint: the detection set, then the mock
/// ReadSet.
pub fn create_mock_market_ix(creator: &Pubkey, pool: &MockPool) -> Instruction {
    let ix = ix::create_market(
        creator,
        &pool.base_mint,
        &pool.base_program.id(),
        VENUE_HINT_MOCK,
        &pda::canonical_pool(&pool.base_mint),
    );
    ix::with_remaining(
        ix,
        &[
            AccountMeta::new_readonly(pool.pool, false),
            AccountMeta::new_readonly(pool.base_vault, false),
            AccountMeta::new_readonly(pool.quote_vault, false),
        ],
    )
}

impl Env {
    /// A mock market per `spec` (module docs), through real instructions: the pool, its
    /// liquidity, `create_market(Mock)`, the lender's deposit, the warm-up.
    pub fn mock_market(&mut self, spec: MarketSpec) -> MockMarket {
        let pool = mock::new_pool(self, spec.program, spec.decimals, spec.fee_ppm);
        mock::seed(self, &pool, spec.base, spec.quote);
        let creator = self.funded_keypair();
        errors::ok(self.send(
            &[create_mock_market_ix(&creator.pubkey(), &pool.keys)],
            &[&creator],
        ));
        let market = pda::market(&pool.keys.base_mint).0;
        let keys = MarketKeys {
            mint: pool.keys.base_mint,
            token_program: spec.program.id(),
            market,
            market_vault: pda::market_vault(&market).0,
            pool_vault: pda::pool_vault(&market).0,
            trader: pda::trader(&market).0,
        };
        let lender = self.funded_keypair();
        let payer = self.payer.insecure_clone();
        let lender_account = tokens::create_token_account(
            &mut self.svm,
            &payer,
            &keys.mint,
            &lender.pubkey(),
            spec.program,
        );
        let mm = MockMarket {
            trader_wsol: pda::trader_wsol(&market).0,
            pool,
            keys,
            spec,
            lender,
            lender_account,
        };
        if spec.lend > 0 {
            tokens::mint_to(
                &mut self.svm,
                &payer,
                &mm.keys.mint,
                &mm.lender_account,
                &mm.pool.mint_authority,
                spec.lend,
                spec.program,
            );
            let lender = mm.lender.insecure_clone();
            errors::ok(self.send(&[ix::deposit(&mm.lender_keys(), spec.lend, 0)], &[&lender]));
        }
        self.warp(spec.warmup);
        mm
    }

    /// `crank_price` on `mm` (must succeed).
    pub fn crank(&mut self, mm: &impl VenueMarket) {
        errors::ok(self.send(&[crank_price_ix(mm)], &[]));
    }
}

// ---- builders -----------------------------------------------------------------------------

/// `open_short(nonce, q, min_proceeds, max_margin, max_open_fee, sel)` with
/// `ReadSet ++ TradeExtras ++ extras`.
#[allow(clippy::too_many_arguments)]
pub fn open_short_ix(
    mm: &impl VenueMarket,
    borrower: &Pubkey,
    nonce: u64,
    q: u64,
    min_proceeds: u64,
    max_margin: u64,
    max_open_fee: u64,
    sel: ExtrasSel,
    extras: &[AccountMeta],
) -> Instruction {
    let market = mm.market();
    let ix = build(
        shortwin::accounts::OpenShort {
            borrower: *borrower,
            config: pda::config(),
            venue_guard: pda::venue_guard(),
            oracle_cache: pda::oracle_cache(),
            market,
            position: pda::position(&market, borrower, nonce).0,
            exposure: pda::exposure(&market, borrower).0,
            mint: mm.mint(),
            pool_vault: mm.keys().pool_vault,
            market_vault: mm.keys().market_vault,
            trader: mm.keys().trader,
            token_program: mm.keys().token_program,
            system_program: system_program(),
            event_authority: pda::event_authority(),
            program: shortwin::ID,
        },
        shortwin::instruction::OpenShort {
            nonce,
            q,
            min_proceeds,
            max_margin,
            max_open_fee,
            extras: sel,
        },
    );
    ix::with_remaining(ix, &mm.trade_remaining(extras))
}

/// `close_short(max_buyback_cost, sel)` of `borrower`'s position `nonce`.
pub fn close_short_ix(
    mm: &impl VenueMarket,
    borrower: &Pubkey,
    nonce: u64,
    max_buyback_cost: u64,
    sel: ExtrasSel,
    extras: &[AccountMeta],
) -> Instruction {
    let market = mm.market();
    let ix = build(
        shortwin::accounts::CloseShort {
            borrower: *borrower,
            config: pda::config(),
            market,
            position: pda::position(&market, borrower, nonce).0,
            exposure: pda::exposure(&market, borrower).0,
            mint: mm.mint(),
            pool_vault: mm.keys().pool_vault,
            market_vault: mm.keys().market_vault,
            trader: mm.keys().trader,
            token_program: mm.keys().token_program,
            system_program: system_program(),
            event_authority: pda::event_authority(),
            program: shortwin::ID,
        },
        shortwin::instruction::CloseShort {
            max_buyback_cost,
            extras: sel,
        },
    );
    ix::with_remaining(ix, &mm.trade_remaining(extras))
}

/// `crank_price()` with the market's venue ReadSet (read-only).
pub fn crank_price_ix(mm: &impl VenueMarket) -> Instruction {
    let ix = build(
        shortwin::accounts::CrankPrice {
            config: pda::config(),
            market: mm.market(),
            mint: mm.mint(),
            oracle_cache: pda::oracle_cache(),
            event_authority: pda::event_authority(),
            program: shortwin::ID,
        },
        shortwin::instruction::CrankPrice {},
    );
    ix::with_remaining(ix, &mm.read_set(false))
}

/// `set_route_broken(scope, venue_bit, broken)`; `market` only for `PauseScope::Market`.
pub fn set_route_broken_ix(
    authority: &Pubkey,
    scope: PauseScope,
    venue_bit: u8,
    broken: bool,
    market: Option<Pubkey>,
) -> Instruction {
    build(
        shortwin::accounts::SetRouteBroken {
            authority: *authority,
            config: pda::config(),
            market,
            event_authority: pda::event_authority(),
            program: shortwin::ID,
        },
        shortwin::instruction::SetRouteBroken {
            scope,
            venue_bit,
            broken,
        },
    )
}

/// `set_venue_extras(venue, ix, entries)`.
pub fn set_venue_extras_ix(
    authority: &Pubkey,
    venue: u8,
    ix_kind: u8,
    entries: Vec<ExtraEntryArg>,
) -> Instruction {
    build(
        shortwin::accounts::SetVenueExtras {
            authority: *authority,
            config: pda::config(),
            venue_extras: pda::venue_extras_seed(venue),
            event_authority: pda::event_authority(),
            program: shortwin::ID,
        },
        shortwin::instruction::SetVenueExtras {
            venue,
            ix: ix_kind,
            entries,
        },
    )
}

/// `add_margin(amount)` (the account list; the body is its track's).
pub fn add_margin_ix(
    mm: &impl VenueMarket,
    borrower: &Pubkey,
    nonce: u64,
    amount: u64,
) -> Instruction {
    let market = mm.market();
    build(
        shortwin::accounts::AddMargin {
            borrower: *borrower,
            market,
            position: pda::position(&market, borrower, nonce).0,
            system_program: system_program(),
            event_authority: pda::event_authority(),
            program: shortwin::ID,
        },
        shortwin::instruction::AddMargin { amount },
    )
}

/// `liquidate(sel)` by `keeper` (the account list; the body is its track's).
pub fn liquidate_ix(
    mm: &impl VenueMarket,
    keeper: &Pubkey,
    borrower: &Pubkey,
    nonce: u64,
) -> Instruction {
    let market = mm.market();
    let ix = build(
        shortwin::accounts::Liquidate {
            keeper: *keeper,
            borrower: *borrower,
            config: pda::config(),
            market,
            position: pda::position(&market, borrower, nonce).0,
            exposure: pda::exposure(&market, borrower).0,
            mint: mm.mint(),
            pool_vault: mm.keys().pool_vault,
            market_vault: mm.keys().market_vault,
            trader: mm.keys().trader,
            token_program: mm.keys().token_program,
            system_program: system_program(),
            event_authority: pda::event_authority(),
            program: shortwin::ID,
        },
        shortwin::instruction::Liquidate {
            extras: ExtrasSel::NONE,
        },
    );
    ix::with_remaining(ix, &mm.trade_remaining(&[]))
}

/// `expire(sel)` by `keeper` (the account list; the body is its track's).
pub fn expire_ix(
    mm: &impl VenueMarket,
    keeper: &Pubkey,
    borrower: &Pubkey,
    nonce: u64,
) -> Instruction {
    let market = mm.market();
    let ix = build(
        shortwin::accounts::Expire {
            keeper: *keeper,
            borrower: *borrower,
            config: pda::config(),
            market,
            position: pda::position(&market, borrower, nonce).0,
            exposure: pda::exposure(&market, borrower).0,
            mint: mm.mint(),
            pool_vault: mm.keys().pool_vault,
            market_vault: mm.keys().market_vault,
            trader: mm.keys().trader,
            token_program: mm.keys().token_program,
            system_program: system_program(),
            event_authority: pda::event_authority(),
            program: shortwin::ID,
        },
        shortwin::instruction::Expire {
            extras: ExtrasSel::NONE,
        },
    );
    ix::with_remaining(ix, &mm.trade_remaining(&[]))
}

/// `sol_settle()` by `keeper` (the account list; the body is its track's).
pub fn sol_settle_ix(
    env: &Env,
    mm: &impl VenueMarket,
    keeper: &Pubkey,
    borrower: &Pubkey,
    nonce: u64,
) -> Instruction {
    let market = mm.market();
    let epoch = env.market(&market).share_epoch;
    let ix = build(
        shortwin::accounts::SolSettle {
            keeper: *keeper,
            config: pda::config(),
            market,
            position: pda::position(&market, borrower, nonce).0,
            borrower: *borrower,
            exposure: pda::exposure(&market, borrower).0,
            market_vault: mm.keys().market_vault,
            mint: mm.mint(),
            share_epoch_rec: pda::share_epoch(&market, epoch).0,
            system_program: system_program(),
            event_authority: pda::event_authority(),
            program: shortwin::ID,
        },
        shortwin::instruction::SolSettle {},
    );
    ix::with_remaining(ix, &mm.read_set(false))
}

/// `ATA(trader, WSOL)` of a market (SPL Token): the account `sweep_trader` closes.
pub fn trader_wsol_ata(market: &Pubkey) -> Pubkey {
    let trader = pda::trader(market).0;
    Pubkey::find_program_address(
        &[
            trader.as_ref(),
            TOKEN_PROGRAM_ID.as_ref(),
            WSOL_MINT.as_ref(),
        ],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    )
    .0
}

/// `sweep_trader()` (ARCH §3.9; M2-SEC-4) of `market`; `wipe_record`: the latest `ShareEpoch`
/// record (writable), which the instruction takes exactly while a full-wipe record is live.
pub fn sweep_trader_ix(market: &Pubkey, wipe_record: Option<Pubkey>) -> Instruction {
    let ix = build(
        shortwin::accounts::SweepTrader {
            config: pda::config(),
            market: *market,
            market_vault: pda::market_vault(market).0,
            trader: pda::trader(market).0,
            trader_wsol: pda::trader_wsol(market).0,
            trader_wsol_ata: trader_wsol_ata(market),
            token_program: TOKEN_PROGRAM_ID,
            system_program: system_program(),
            event_authority: pda::event_authority(),
            program: shortwin::ID,
        },
        shortwin::instruction::SweepTrader {},
    );
    let extra: Vec<AccountMeta> = wipe_record
        .map(|k| AccountMeta::new(k, false))
        .into_iter()
        .collect();
    ix::with_remaining(ix, &extra)
}

/// [`sweep_trader_ix`] with the record the market's state asks for: the latest `ShareEpoch`
/// record while a full-wipe record is live, none otherwise.
pub fn sweep_trader_ix_for(env: &Env, market: &Pubkey) -> Instruction {
    let m = env.market(market);
    let live = m.total_shares.get() == 0 && m.has_flag(shortwin::state::MARKET_FLAG_WIPE_LIVE);
    let rec = live.then(|| pda::share_epoch(market, m.share_epoch.saturating_sub(1)).0);
    sweep_trader_ix(market, rec)
}

// ---- borrowers ----------------------------------------------------------------------------

/// A funded borrower and the nonce of its next position.
pub struct Borrower {
    pub kp: Keypair,
    pub next_nonce: u64,
}

impl Borrower {
    /// The borrower's key.
    pub fn key(&self) -> Pubkey {
        self.kp.pubkey()
    }
}

impl Env {
    /// A new borrower with [`crate::env::FUNDING`] lamports.
    pub fn borrower(&mut self) -> Borrower {
        Borrower {
            kp: self.funded_keypair(),
            next_nonce: 0,
        }
    }

    /// `open_short` of `q` with no bounds of its own (min proceeds 0, max margin and fee
    /// `u64::MAX`), extras `none`. Returns the result and the nonce used (the nonce advances
    /// only on success).
    pub fn open(&mut self, mm: &impl VenueMarket, b: &mut Borrower, q: u64) -> (TxResult, u64) {
        let nonce = b.next_nonce;
        let ix = open_short_ix(
            mm,
            &b.key(),
            nonce,
            q,
            0,
            u64::MAX,
            u64::MAX,
            ExtrasSel::NONE,
            &[],
        );
        let kp = b.kp.insecure_clone();
        let res = self.send(&[ix], &[&kp]);
        if res.is_ok() {
            b.next_nonce += 1;
        }
        (res, nonce)
    }

    /// `close_short` of `b`'s position `nonce` with `max_buyback_cost = u64::MAX`, extras
    /// `none`.
    pub fn close(&mut self, mm: &impl VenueMarket, b: &Borrower, nonce: u64) -> TxResult {
        let ix = close_short_ix(mm, &b.key(), nonce, u64::MAX, ExtrasSel::NONE, &[]);
        let kp = b.kp.insecure_clone();
        self.send(&[ix], &[&kp])
    }

    /// A position (panics if missing).
    pub fn position(&self, mm: &impl VenueMarket, borrower: &Pubkey, nonce: u64) -> ShortPosition {
        read::borsh(&self.svm, &pda::position(&mm.market(), borrower, nonce).0)
            .unwrap_or_else(|| panic!("no position {borrower}/{nonce}"))
    }
}
