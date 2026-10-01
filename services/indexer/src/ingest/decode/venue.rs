//! Venue trades and lifecycle events (INDEXER.md §7.2). Owner: INGEST.
//!
//! - pump `TradeEvent` (an `emit_cpi!` of the pump program): mint, sol_amount, token_amount,
//!   is_buy, user, the post-trade virtual / real reserves, fee, creator, creator_fee (the later
//!   fields are decoded tolerantly: known prefix, trailing bytes ignored);
//! - PumpSwap `BuyEvent` / `SellEvent` (`emit_cpi!` of pAMM): base / quote amounts, lp_fee,
//!   protocol_fee, pool, user, coin_creator, coin_creator_fee, pool reserves;
//! - Raydium CPMM `SwapEvent` (`emit!`, discriminator `40c6cde8260871e2`, **only from the CPMM
//!   program**: CLMM and others emit an event of the same name and discriminator);
//! - Raydium AMM v4 `ray_log` (base64 bincode `SwapBaseInLog` log_type 3 / `SwapBaseOutLog`
//!   log_type 4), oriented with `venue_pools.sol_is_first` — or, for short.win's own legs, by
//!   the leg's direction (an open sells tokens, an exit buys them);
//! - lifecycle: pump `CompleteEvent` and the other pump / pAMM events → `venue_events`; BPF
//!   upgradeable loader `Upgrade` of a venue program → `venue_program_upgrades`.
//!
//! Normalised row (`venue_trades`): `side` from the trader's view (buy = tokens out of the pool),
//! `token_amount`, `sol_amount` = lamports the trader paid (buy) or received (sell) with the
//! venue's fees, the fees, `fee_creator_lamports` (a creator fee charged in tokens is valued at
//! the trade's price `⌊fee·sol_amount/token_amount⌋`), `creator`, `user_key`, the post-trade
//! reserves and `price_wad = ⌊sol_amount·WAD/token_amount⌋`.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use solana_pubkey::Pubkey;

use super::idl::key_json;
use crate::numeric::{WAD, hex};

/// Programs (mainnet ids; the fixtures run the dumped programs at the same addresses).
pub const PUMP: Pubkey = shortwin::constants::PUMP_PROGRAM_ID;
pub const PUMP_AMM: Pubkey = shortwin::constants::PUMP_AMM_PROGRAM_ID;
pub const RAYDIUM_V4: Pubkey = shortwin::constants::RAYDIUM_V4_PROGRAM_ID;
pub const RAYDIUM_CPMM: Pubkey = shortwin::constants::RAYDIUM_CPMM_PROGRAM_ID;
pub const WSOL: Pubkey = shortwin::constants::WSOL_MINT;
/// The BPF upgradeable loader.
pub const BPF_UPGRADEABLE_LOADER: Pubkey =
    solana_pubkey::pubkey!("BPFLoaderUpgradeab1e11111111111111111111111");

/// Anchor's event discriminator `sha256("event:<name>")[..8]`.
pub fn anchor_event_discriminator(name: &str) -> [u8; 8] {
    let h = Sha256::digest(format!("event:{name}").as_bytes());
    let mut d = [0u8; 8];
    d.copy_from_slice(&h[..8]);
    d
}

/// Known pump and pAMM event names (their discriminators are derived, not copied).
const PUMP_EVENTS: &[&str] = &[
    "TradeEvent",
    "CompleteEvent",
    "CreateEvent",
    "SetParamsEvent",
    "CompletePumpAmmMigrationEvent",
    "CollectCreatorFeeEvent",
    "ExtendAccountEvent",
    "SetCreatorEvent",
    "SetMetaplexCreatorEvent",
    "UpdateGlobalAuthorityEvent",
    "AdminSetCreatorEvent",
    "AdminSetIdlAuthorityEvent",
    "AdminUpdateTokenIncentivesEvent",
    "ClaimTokenIncentivesEvent",
    "InitUserVolumeAccumulatorEvent",
    "CloseUserVolumeAccumulatorEvent",
    "SyncUserVolumeAccumulatorEvent",
];
const PUMP_AMM_EVENTS: &[&str] = &[
    "BuyEvent",
    "SellEvent",
    "CreatePoolEvent",
    "DepositEvent",
    "WithdrawEvent",
    "DisableEvent",
    "CreateConfigEvent",
    "UpdateAdminEvent",
    "UpdateFeeConfigEvent",
    "ExtendAccountEvent",
    "CollectCoinCreatorFeeEvent",
    "SetBondingCurveCoinCreatorEvent",
    "SetMetaplexCoinCreatorEvent",
    "AdminSetCoinCreatorEvent",
    "AdminUpdateTokenIncentivesEvent",
    "ClaimTokenIncentivesEvent",
    "InitUserVolumeAccumulatorEvent",
    "CloseUserVolumeAccumulatorEvent",
    "SyncUserVolumeAccumulatorEvent",
];

fn table(names: &'static [&'static str]) -> Vec<([u8; 8], &'static str)> {
    names
        .iter()
        .map(|n| (anchor_event_discriminator(n), *n))
        .collect()
}

fn pump_names() -> &'static [([u8; 8], &'static str)] {
    static T: OnceLock<Vec<([u8; 8], &'static str)>> = OnceLock::new();
    T.get_or_init(|| table(PUMP_EVENTS))
}

fn pump_amm_names() -> &'static [([u8; 8], &'static str)] {
    static T: OnceLock<Vec<([u8; 8], &'static str)>> = OnceLock::new();
    T.get_or_init(|| table(PUMP_AMM_EVENTS))
}

/// The pump (`program == PUMP`) or pAMM event name of `payload` (discriminator ‖ Borsh).
pub fn venue_event_name(program: &Pubkey, payload: &[u8]) -> Option<&'static str> {
    let t = if *program == PUMP {
        pump_names()
    } else if *program == PUMP_AMM {
        pump_amm_names()
    } else {
        return None;
    };
    t.iter()
        .find(|(d, _)| payload.starts_with(d))
        .map(|(_, n)| *n)
}

/// CPMM `SwapEvent`'s discriminator (`sha256("event:SwapEvent")[..8]` = `40c6cde8260871e2`).
pub const CPMM_SWAP_EVENT: [u8; 8] = [0x40, 0xc6, 0xcd, 0xe8, 0x26, 0x08, 0x71, 0xe2];

/// The trader's direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// Tokens out of the pool.
    Buy,
    /// Tokens into the pool.
    Sell,
}

impl Side {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Buy => "buy",
            Self::Sell => "sell",
        }
    }
}

/// A normalised venue trade (`venue_trades`), before the writer adds the chain position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trade {
    pub venue: &'static str,
    pub program: Pubkey,
    pub pool: Pubkey,
    pub mint: Option<Pubkey>,
    pub side: Side,
    pub token_amount: u64,
    pub sol_amount: u64,
    pub fee_lp: Option<u64>,
    pub fee_protocol: Option<u64>,
    pub fee_creator: Option<u64>,
    pub fee_creator_lamports: Option<u64>,
    pub creator: Option<Pubkey>,
    pub user: Pubkey,
    pub pool_token_reserve: Option<u64>,
    pub pool_sol_reserve: Option<u64>,
}

impl Trade {
    /// `⌊sol_amount·WAD / token_amount⌋` (`None` for a zero token amount).
    pub fn price_wad(&self) -> Option<u128> {
        (self.token_amount > 0)
            .then(|| u128::from(self.sol_amount) * WAD / u128::from(self.token_amount))
    }
}

/// `⌊fee·sol / tokens⌋`: a token-side fee valued at the trade's price.
fn at_price(fee: u64, sol: u64, tokens: u64) -> u64 {
    if tokens == 0 {
        return 0;
    }
    u64::try_from(u128::from(fee) * u128::from(sol) / u128::from(tokens)).unwrap_or(u64::MAX)
}

/// A little-endian Borsh reader that reports running out of bytes as `None`.
struct Rd<'a>(&'a [u8]);

impl Rd<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        if self.0.len() < n {
            return None;
        }
        let (h, t) = self.0.split_at(n);
        self.0 = t;
        Some(h)
    }
    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
    fn i64(&mut self) -> Option<i64> {
        Some(i64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
    fn key(&mut self) -> Option<Pubkey> {
        Some(Pubkey::new_from_array(self.take(32)?.try_into().ok()?))
    }
    fn bool(&mut self) -> Option<bool> {
        Some(self.take(1)?[0] != 0)
    }
}

/// The pump curve's bonding-curve PDA of `mint` (`["bonding-curve", mint]`).
pub fn bonding_curve_of(mint: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"bonding-curve", mint.as_ref()], &PUMP).0
}

/// pump `TradeEvent` (body after the discriminator; field order per pump's IDL).
pub fn pump_trade(body: &[u8]) -> Option<Trade> {
    let mut r = Rd(body);
    let mint = r.key()?;
    let sol_amount = r.u64()?;
    let token_amount = r.u64()?;
    let is_buy = r.bool()?;
    let user = r.key()?;
    let _timestamp = r.i64()?;
    let virtual_sol = r.u64()?;
    let virtual_token = r.u64()?;
    let _real_sol = r.u64()?;
    let _real_token = r.u64()?;
    let _fee_recipient = r.key()?;
    let _fee_bps = r.u64()?;
    let fee = r.u64()?;
    let creator = r.key()?;
    let _creator_fee_bps = r.u64()?;
    let creator_fee = r.u64()?;
    let fees = fee.saturating_add(creator_fee);
    let (side, trader_sol) = if is_buy {
        (Side::Buy, sol_amount.saturating_add(fees))
    } else {
        (Side::Sell, sol_amount.saturating_sub(fees))
    };
    Some(Trade {
        venue: "pump_curve",
        program: PUMP,
        pool: bonding_curve_of(&mint),
        mint: Some(mint),
        side,
        token_amount,
        sol_amount: trader_sol,
        fee_lp: None,
        fee_protocol: Some(fee),
        fee_creator: Some(creator_fee),
        fee_creator_lamports: Some(creator_fee),
        creator: (creator != Pubkey::default()).then_some(creator),
        user,
        pool_token_reserve: Some(virtual_token),
        pool_sol_reserve: Some(virtual_sol),
    })
}

/// pAMM `BuyEvent` / `SellEvent` (`buy` = BuyEvent). `base_mint` / `quote_mint`: the pool's
/// mints from the emitting instruction's accounts (3 and 4). `None` unless one side is WSOL: a
/// pool pairing a token with anything else is not a SOL market, and its quote amounts are not
/// lamports (M8-SEC-6; the CPMM decoder checks the same).
pub fn pump_amm_trade(
    buy: bool,
    body: &[u8],
    base_mint: Option<Pubkey>,
    quote_mint: Option<Pubkey>,
) -> Option<Trade> {
    if base_mint != Some(WSOL) && quote_mint != Some(WSOL) {
        return None;
    }
    let mut r = Rd(body);
    let _timestamp = r.i64()?;
    let base_amount = r.u64()?; // base_amount_out (buy) / base_amount_in (sell)
    let _bound = r.u64()?; // max_quote_amount_in / min_quote_amount_out
    let _user_base_reserves = r.u64()?;
    let _user_quote_reserves = r.u64()?;
    let pool_base = r.u64()?;
    let pool_quote = r.u64()?;
    let _quote_amount = r.u64()?; // quote_amount_in / quote_amount_out (before fees)
    let _lp_fee_bps = r.u64()?;
    let lp_fee = r.u64()?;
    let _protocol_fee_bps = r.u64()?;
    let protocol_fee = r.u64()?;
    let quote_net_lp = r.u64()?; // quote_amount_in_with_lp_fee / quote_amount_out_without_lp_fee
    let user_quote = r.u64()?; // user_quote_amount_in / user_quote_amount_out
    let pool = r.key()?;
    let user = r.key()?;
    let _user_base_ata = r.key()?;
    let _user_quote_ata = r.key()?;
    let _protocol_fee_recipient = r.key()?;
    let _protocol_fee_recipient_ata = r.key()?;
    let coin_creator = r.key()?;
    let _coin_creator_fee_bps = r.u64()?;
    let coin_creator_fee = r.u64()?;
    // Post-trade reserves: the lp fee stays in the pool, the other fees leave it.
    let (base_after, quote_after) = if buy {
        (
            pool_base.saturating_sub(base_amount),
            pool_quote.saturating_add(quote_net_lp),
        )
    } else {
        (
            pool_base.saturating_add(base_amount),
            pool_quote.saturating_sub(quote_net_lp),
        )
    };
    // Canonical pools: base = the token, quote = WSOL. A reversed pool (base = WSOL) swaps the
    // trader's view.
    let base_is_sol = base_mint == Some(WSOL);
    let (side, token_amount, sol_amount, mint, tok_res, sol_res) = if base_is_sol {
        (
            if buy { Side::Sell } else { Side::Buy },
            user_quote,
            base_amount,
            quote_mint,
            quote_after,
            base_after,
        )
    } else {
        (
            if buy { Side::Buy } else { Side::Sell },
            base_amount,
            user_quote,
            base_mint.filter(|m| *m != WSOL),
            base_after,
            quote_after,
        )
    };
    let creator_lamports = if base_is_sol {
        at_price(coin_creator_fee, sol_amount, token_amount)
    } else {
        coin_creator_fee
    };
    Some(Trade {
        venue: "pump_swap",
        program: PUMP_AMM,
        pool,
        mint,
        side,
        token_amount,
        sol_amount,
        fee_lp: Some(lp_fee),
        fee_protocol: Some(protocol_fee),
        fee_creator: Some(coin_creator_fee),
        fee_creator_lamports: Some(creator_lamports),
        creator: (coin_creator != Pubkey::default()).then_some(coin_creator),
        user,
        pool_token_reserve: Some(tok_res),
        pool_sol_reserve: Some(sol_res),
    })
}

/// Raydium CPMM `SwapEvent` (the whole "Program data:" payload, discriminator included).
/// `user`: the swap instruction's payer (its account 0).
pub fn cpmm_swap(payload: &[u8], user: Pubkey) -> Option<Trade> {
    let body = payload.strip_prefix(&CPMM_SWAP_EVENT)?;
    let mut r = Rd(body);
    let pool = r.key()?;
    let input_before = r.u64()?;
    let output_before = r.u64()?;
    let input_amount = r.u64()?;
    let output_amount = r.u64()?;
    let _input_transfer_fee = r.u64()?;
    let _output_transfer_fee = r.u64()?;
    let _base_input = r.bool()?;
    let input_mint = r.key()?;
    let output_mint = r.key()?;
    let trade_fee = r.u64()?;
    let creator_fee = r.u64()?;
    let creator_fee_on_input = r.bool()?;
    let buy = input_mint == WSOL;
    if !buy && output_mint != WSOL {
        return None; // not a SOL pool
    }
    let (side, mint, token_amount, sol_amount) = if buy {
        (Side::Buy, output_mint, output_amount, input_amount)
    } else {
        (Side::Sell, input_mint, input_amount, output_amount)
    };
    // The trade fee is charged on the input; the creator fee on the input or the output.
    let trade_fee_lamports = if buy {
        trade_fee
    } else {
        at_price(trade_fee, sol_amount, token_amount)
    };
    let creator_on_sol = creator_fee_on_input == buy;
    let creator_lamports = if creator_on_sol {
        creator_fee
    } else {
        at_price(creator_fee, sol_amount, token_amount)
    };
    let (tok_res, sol_res) = if buy {
        (
            output_before.saturating_sub(output_amount),
            input_before.saturating_add(input_amount),
        )
    } else {
        (
            input_before.saturating_add(input_amount),
            output_before.saturating_sub(output_amount),
        )
    };
    Some(Trade {
        venue: "raydium_cpmm",
        program: RAYDIUM_CPMM,
        pool,
        mint: Some(mint),
        side,
        token_amount,
        sol_amount,
        fee_lp: Some(trade_fee_lamports),
        fee_protocol: None,
        fee_creator: Some(creator_fee),
        fee_creator_lamports: Some(creator_lamports),
        creator: None,
        user,
        pool_token_reserve: Some(tok_res),
        pool_sol_reserve: Some(sol_res),
    })
}

/// A decoded AMM v4 `ray_log` swap record (R-RAY §3.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RayLog {
    /// 3 = SwapBaseIn, 4 = SwapBaseOut.
    pub log_type: u8,
    /// Amount in (3) / max in (4).
    pub amount: u64,
    /// Minimum out (3) / amount out (4).
    pub bound: u64,
    /// 1 = PC2Coin (pc in), 2 = Coin2PC (coin in).
    pub direction: u64,
    pub pool_coin: u64,
    pub pool_pc: u64,
    /// Amount out (3) / the input deducted (4).
    pub result: u64,
}

impl RayLog {
    /// `(amount in, amount out)`.
    pub fn amounts(&self) -> (u64, u64) {
        if self.log_type == 3 {
            (self.amount, self.result)
        } else {
            (self.result, self.bound)
        }
    }

    /// `true` if the coin side is the input.
    pub fn coin_in(&self) -> bool {
        self.direction == 2
    }
}

/// Decodes a `ray_log` payload (57 bytes: log type + 7 u64); other log types are not swaps.
pub fn ray_log(b: &[u8]) -> Option<RayLog> {
    if b.len() != 57 || !matches!(b[0], 3 | 4) {
        return None;
    }
    let u = |i: usize| u64::from_le_bytes(b[1 + 8 * i..9 + 8 * i].try_into().unwrap_or([0; 8]));
    Some(RayLog {
        log_type: b[0],
        amount: u(0),
        bound: u(1),
        direction: u(2),
        pool_coin: u(4),
        pool_pc: u(5),
        result: u(6),
    })
}

/// An AMM v4 trade from its `ray_log`, with SOL on the coin side (`sol_is_coin`) or not.
pub fn ray_v4_trade(log: &RayLog, pool: Pubkey, user: Pubkey, sol_is_coin: bool) -> Trade {
    let (amount_in, amount_out) = log.amounts();
    let sol_in = log.coin_in() == sol_is_coin;
    let (side, token_amount, sol_amount) = if sol_in {
        (Side::Buy, amount_out, amount_in)
    } else {
        (Side::Sell, amount_in, amount_out)
    };
    let (coin_after, pc_after) = if log.coin_in() {
        (
            log.pool_coin.saturating_add(amount_in),
            log.pool_pc.saturating_sub(amount_out),
        )
    } else {
        (
            log.pool_coin.saturating_sub(amount_out),
            log.pool_pc.saturating_add(amount_in),
        )
    };
    let (sol_res, tok_res) = if sol_is_coin {
        (coin_after, pc_after)
    } else {
        (pc_after, coin_after)
    };
    Trade {
        venue: "raydium_v4",
        program: RAYDIUM_V4,
        pool,
        mint: None,
        side,
        token_amount,
        sol_amount,
        fee_lp: None,
        fee_protocol: None,
        fee_creator: None,
        fee_creator_lamports: None,
        creator: None,
        user,
        pool_token_reserve: Some(tok_res),
        pool_sol_reserve: Some(sol_res),
    }
}

/// `sol_is_coin` of an AMM v4 pool from a short.win leg: the leg's direction is known (an open
/// sells tokens, an exit buys them), so the log's direction tells which side SOL is on.
pub fn ray_v4_orientation_from_leg(log: &RayLog, shortwin_ix: &str) -> Option<bool> {
    let trader_sells = match shortwin_ix {
        "open_short" => true,
        "close_short" | "liquidate" | "expire" => false,
        _ => return None,
    };
    // Selling tokens: the input is the token side, so SOL is the coin iff pc is the input.
    Some(if trader_sells {
        !log.coin_in()
    } else {
        log.coin_in()
    })
}

/// A lifecycle (non-trade) venue event (`venue_events`).
#[derive(Clone, Debug, PartialEq)]
pub struct LifecycleEvent {
    pub venue: &'static str,
    pub program: Pubkey,
    pub kind: &'static str,
    pub pool: Option<Pubkey>,
    pub mint: Option<Pubkey>,
    pub data: Value,
}

/// A pump / pAMM lifecycle event: the decoded prefix where the layout is known (pump
/// `CompleteEvent`: user, mint, bonding_curve, timestamp; pAMM `CreatePoolEvent`: through
/// `pool`), always the raw body as hex.
pub fn lifecycle(program: &Pubkey, kind: &'static str, body: &[u8]) -> LifecycleEvent {
    let venue = if *program == PUMP {
        "pump_curve"
    } else {
        "pump_swap"
    };
    let mut data = json!({ "raw": hex(body) });
    let (mut pool, mut mint) = (None, None);
    let mut r = Rd(body);
    match (venue, kind) {
        ("pump_curve", "CompleteEvent") => {
            if let (Some(user), Some(m), Some(curve), Some(ts)) =
                (r.key(), r.key(), r.key(), r.i64())
            {
                data["user"] = key_json(&user);
                data["mint"] = key_json(&m);
                data["bonding_curve"] = key_json(&curve);
                data["timestamp"] = Value::String(ts.to_string());
                pool = Some(curve);
                mint = Some(m);
            }
        }
        ("pump_swap", "CreatePoolEvent") => {
            let decoded = (|| {
                let ts = r.i64()?;
                let _index = r.take(2)?;
                let creator = r.key()?;
                let base_mint = r.key()?;
                let quote_mint = r.key()?;
                let _decimals = r.take(2)?;
                for _ in 0..7 {
                    r.u64()?;
                }
                let _bump = r.take(1)?;
                let pool = r.key()?;
                let lp_mint = r.key()?;
                Some((ts, creator, base_mint, quote_mint, pool, lp_mint))
            })();
            if let Some((ts, creator, base_mint, quote_mint, p, lp_mint)) = decoded {
                data["timestamp"] = Value::String(ts.to_string());
                data["creator"] = key_json(&creator);
                data["base_mint"] = key_json(&base_mint);
                data["quote_mint"] = key_json(&quote_mint);
                data["pool"] = key_json(&p);
                data["lp_mint"] = key_json(&lp_mint);
                pool = Some(p);
                mint = Some(if base_mint == WSOL {
                    quote_mint
                } else {
                    base_mint
                });
            }
        }
        _ => {}
    }
    LifecycleEvent {
        venue,
        program: *program,
        kind,
        pool,
        mint,
        data,
    }
}

/// A BPF upgradeable loader `Upgrade` of a venue program (`venue_program_upgrades`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProgramUpgrade {
    /// `VenueGuard` index.
    pub venue: u8,
    pub program: Pubkey,
    pub programdata: Pubkey,
}

/// `Upgrade` (instruction 3: `[programdata, program, buffer, spill, rent, clock, authority]`)
/// of one of the six venue programs.
pub fn venue_upgrade(
    program_id: &Pubkey,
    data: &[u8],
    accounts: &[Pubkey],
) -> Option<ProgramUpgrade> {
    if *program_id != BPF_UPGRADEABLE_LOADER || data.get(..4) != Some(&3u32.to_le_bytes()[..]) {
        return None;
    }
    let programdata = *accounts.first()?;
    let program = *accounts.get(1)?;
    let venue = shortwin::constants::VENUE_PROGRAM_IDS
        .iter()
        .position(|p| *p == program)?;
    Some(ProgramUpgrade {
        venue: u8::try_from(venue).ok()?,
        program,
        programdata,
    })
}

/// What the decoder knows about pools (from `venue_pools`): AMM v4 orientation, mints and
/// markets of pools whose events do not carry them.
#[derive(Clone, Debug, Default)]
pub struct PoolCtx {
    pub pools: HashMap<Pubkey, PoolInfo>,
}

/// One `venue_pools` row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoolInfo {
    pub mint: Pubkey,
    pub market: Option<Pubkey>,
    pub sol_is_first: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discriminators_derive_from_the_event_names() {
        assert_eq!(
            anchor_event_discriminator("TradeEvent"),
            [0xbd, 0xdb, 0x7f, 0xd3, 0x4e, 0xe6, 0x61, 0xee]
        );
        assert_eq!(anchor_event_discriminator("SwapEvent"), CPMM_SWAP_EVENT);
        let d = anchor_event_discriminator("CompleteEvent");
        assert_eq!(venue_event_name(&PUMP, &d), Some("CompleteEvent"));
        assert_eq!(venue_event_name(&RAYDIUM_CPMM, &d), None);
    }

    #[test]
    fn ray_logs_orient_by_the_pool_or_the_leg() {
        // SwapBaseIn, coin in (direction 2), 100 in, 7 out; coin = token (SOL is pc).
        let mut b = vec![3u8];
        for v in [100u64, 5, 2, 0, 1_000, 70, 7] {
            b.extend(v.to_le_bytes());
        }
        let log = ray_log(&b).expect("a swap");
        assert_eq!(log.amounts(), (100, 7));
        let t = ray_v4_trade(&log, Pubkey::default(), Pubkey::default(), false);
        assert_eq!((t.side, t.token_amount, t.sol_amount), (Side::Sell, 100, 7));
        assert_eq!(t.pool_token_reserve, Some(1_100));
        assert_eq!(t.pool_sol_reserve, Some(63));
        assert_eq!(ray_v4_orientation_from_leg(&log, "open_short"), Some(false));
        assert_eq!(ray_v4_orientation_from_leg(&log, "close_short"), Some(true));
        // SOL as the coin: coin in = SOL in = a buy.
        let t = ray_v4_trade(&log, Pubkey::default(), Pubkey::default(), true);
        assert_eq!((t.side, t.token_amount, t.sol_amount), (Side::Buy, 7, 100));
        assert!(ray_log(&b[..56]).is_none());
    }

    #[test]
    fn cpmm_swaps_value_a_token_side_creator_fee_at_the_trade_price() {
        let mut p = CPMM_SWAP_EVENT.to_vec();
        let tok = Pubkey::new_from_array([5; 32]);
        p.extend([9u8; 32]); // pool
        for v in [1_000u64, 500, 100, 40, 0, 0] {
            p.extend(v.to_le_bytes());
        }
        p.push(1); // base_input
        p.extend(tok.to_bytes()); // input: the token (a sell)
        p.extend(WSOL.to_bytes());
        p.extend(1u64.to_le_bytes()); // trade fee (tokens)
        p.extend(10u64.to_le_bytes()); // creator fee
        p.push(1); // on the input (tokens)
        let t = cpmm_swap(&p, Pubkey::default()).expect("a SOL swap");
        assert_eq!(
            (t.side, t.token_amount, t.sol_amount),
            (Side::Sell, 100, 40)
        );
        assert_eq!(t.fee_creator_lamports, Some(4));
        assert_eq!(t.mint, Some(tok));
        assert_eq!(t.price_wad(), Some(40 * WAD / 100));
    }

    #[test]
    fn upgrades_of_venue_programs_only() {
        let mut data = 3u32.to_le_bytes().to_vec();
        data.extend([0u8; 4]);
        let pd = Pubkey::new_from_array([1; 32]);
        let accounts = vec![pd, RAYDIUM_CPMM];
        let u = venue_upgrade(&BPF_UPGRADEABLE_LOADER, &data, &accounts).expect("an upgrade");
        assert_eq!(u.venue, shortwin::constants::GUARD_RAYDIUM_CPMM);
        assert_eq!(u.programdata, pd);
        assert!(venue_upgrade(&BPF_UPGRADEABLE_LOADER, &data, &[pd, Pubkey::default()]).is_none());
        assert!(venue_upgrade(&PUMP, &data, &accounts).is_none());
    }
}
