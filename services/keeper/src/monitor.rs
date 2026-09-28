//! The venue drift monitor (ARCHITECTURE §7.1 duty 10, §3.1 "Venue-upgrade latch", §5.5 venue
//! admin powers): what the keeper watches besides prices, as pure functions the keeper loop
//! ([`crate::keeper`]) feeds each tick.
//!
//! **Program upgrades → `flag_venue_upgraded`.** The keeper reads the header (the first 45
//! bytes: tag, deploy slot, upgrade authority) of the six venue ProgramData accounts the
//! `VenueGuard` latch indexes — pump, pAMM, pfee, AMM v4, CPMM, Jupiter
//! ([`shortwin::constants::VENUE_PROGRAM_DATA`]) — and of Token-2022's (alert only), and
//! compares them with the latch. [`latch_action`] is the program's own rule
//! (`flag_venue_upgraded`): with the venue's bit clear, a deploy slot other than `guard_slot`
//! is an upgrade to flag; with the bit set, a deploy slot other than `flagged_slot` is a second
//! upgrade during review, whose flag refreshes the record. Anything else is not an upgrade (the
//! instruction would fail with `VenueNotUpgraded`, writing nothing). The keeper sends the flag
//! in the slot after it sees the change, at the exit priority ceiling, alerts, and pages the
//! Emergency multisig to run the fork suite against the new binary and `revalidate_venue` —
//! both Raydium programs share one latch mask, so an upgrade of either blocks opens on every
//! Raydium market (ARCH §3.1, R4B-7).
//!
//! **Pool admin actions.** The Raydium pool admin (`GThUX1…`, a 2-of-3 Squads v3 vault with no
//! timelock, R-RAY §5) can, per AMM v4 pool, disable swaps (`SetParams` status 2–5, or 7 with a
//! future open time) or set its swap fee to anything below 100 % (`SetParams` fees); per CPMM
//! pool, disable swaps (`update_pool_status` bit 2); and per CPMM **config**, raise the trade and
//! creator fees of every pool on it at once (`update_amm_config`) — or lower them to 0, which
//! stops every buy of a pool whose creator fee is on the buy's input (`ZeroTradingTokens`; the
//! program's reader calls it not executable, and the alert's lost route says why, M3-SEC-1). A polling keeper watches the
//! accounts those instructions write — each market's adopted pool and, for CPMM, its
//! `AmmConfig` (both are in the ReadSet it reads every tick) — and reports every change of the
//! swap route, the status field, the open time and the one-way fee ([`PoolSnapshot`],
//! [`diff`]), with the instruction that writes that field and whether the new fee crosses the
//! opens cap (`venue_fee_cap_bps`: opens refused) or the hard cap (`venue_fee_hard_cap_bps`:
//! `s3_no_route`, the S3 chain). Every field is read with the program's own readers
//! (`shortwin_venue_state::raydium_v4` / `raydium_cpmm`), so a halted pool still reports its
//! fee and a pool that stops parsing (a layout change, "not executable" per A63) is reported as
//! such. The duties themselves need no extra action: the program's route predicates already
//! put a halted market on duty 5's no-route cadence and its S3 chain ([`crate::decide`]).

use anchor_lang::prelude::Pubkey;
use shortwin::constants::{
    TOKEN_2022_PROGRAM_ID, VENUE_PROGRAM_DATA, VENUE_PROGRAM_IDS, VENUE_PROGRAMS,
};
use shortwin::state::{VenueGuard, VenueKind};
use shortwin_services_common::AccountData;
use shortwin_venue_state::{program_data, raydium_cpmm, raydium_v4};

use crate::view::MarketView;

/// Bytes of a ProgramData account the watcher reads (tag, deploy slot, upgrade authority).
pub const HEADER_LEN: usize = program_data::HEADER_LEN;

/// An upgradeable program whose ProgramData the keeper watches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WatchedProgram {
    pub program: Pubkey,
    pub program_data: Pubkey,
    /// Its `VenueGuard` index (`flag_venue_upgraded(venue)`), or `None`: alert only
    /// (Token-2022).
    pub guard: Option<u8>,
}

/// The upgradeable loader's ProgramData address of `program`.
pub fn program_data_address(program: &Pubkey) -> Pubkey {
    let loader = Pubkey::new_from_array(program_data::LOADER_UPGRADEABLE);
    Pubkey::find_program_address(&[program.as_ref()], &loader).0
}

/// The six venue programs of the latch, in `VenueGuard` order, then Token-2022 (alert only;
/// ARCH §7.1 duty 10).
pub fn watched_programs() -> Vec<WatchedProgram> {
    let mut v: Vec<WatchedProgram> = VENUE_PROGRAM_IDS
        .iter()
        .zip(VENUE_PROGRAM_DATA.iter())
        .enumerate()
        .map(|(i, (program, pd))| WatchedProgram {
            program: *program,
            program_data: *pd,
            guard: u8::try_from(i).ok(),
        })
        .collect();
    v.push(WatchedProgram {
        program: TOKEN_2022_PROGRAM_ID,
        program_data: program_data_address(&TOKEN_2022_PROGRAM_ID),
        guard: None,
    });
    v
}

/// The deploy slot in a ProgramData header (`None` if missing, not owned by the upgradeable
/// loader, not tag 3 or too short: the same check as `flag_venue_upgraded`).
pub fn deploy_slot(acc: Option<&AccountData>) -> Option<u64> {
    let acc = acc?;
    program_data::deploy_slot(&acc.owner.to_bytes(), &acc.data).ok()
}

/// What the latch needs for one venue program.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LatchAction {
    /// Nothing: the deploy slot is the validated one (bit clear) or the flagged one (bit set).
    Nothing,
    /// The bit is clear and the deploy slot is not `guard_slot`: an upgrade to flag.
    Flag,
    /// The bit is set and the deploy slot is not `flagged_slot`: a second upgrade during
    /// review, whose flag refreshes `flagged_slot` / `flagged_at`.
    Refresh,
}

/// The program's `flag_venue_upgraded` rule for venue `venue` at ProgramData `deploy_slot`
/// ([module docs](self)).
pub fn latch_action(guard: &VenueGuard, venue: u8, deploy_slot: u64) -> LatchAction {
    let i = usize::from(venue);
    if i >= VENUE_PROGRAMS {
        return LatchAction::Nothing;
    }
    if guard.is_flagged(venue) {
        if guard.flagged_slot.get(i) == Some(&deploy_slot) {
            LatchAction::Nothing
        } else {
            LatchAction::Refresh
        }
    } else if guard.guard_slot.get(i) == Some(&deploy_slot) {
        LatchAction::Nothing
    } else {
        LatchAction::Flag
    }
}

/// A pool's admin-controlled state as the keeper saw it ([module docs](self)).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PoolSnapshot {
    /// The program's `venue_executable` (the reader's predicate on the whole ReadSet).
    pub executable: bool,
    /// The pool account parses (a Raydium venue; the mock's state is taken from its reader).
    pub parses: bool,
    /// AMM v4 `status` @0, CPMM `status` @329.
    pub status: Option<u64>,
    /// AMM v4 `pool_open_time` @224, CPMM `open_time` @373.
    pub open_time: Option<u64>,
    /// The live one-way fee φ in ppm, read even while the pool is halted (AMM v4
    /// `⌈num·10⁶/den⌉`, CPMM `t + c` of its config).
    pub fee_ppm: Option<u32>,
    /// The CPMM config the pool names.
    pub config: Option<Pubkey>,
    /// CPMM: whether a buy (WSOL in) can split its fee on the pool's config
    /// (`PoolState::fee_splits`); `Some(false)`: the trade and effective creator fee are 0 with
    /// the creator fee on the buy's input, so every buy reverts with `ZeroTradingTokens` and the
    /// route is lost although the status is clear (M3-SEC-1). `None` elsewhere.
    pub buy_fee_splits: Option<bool>,
}

fn parse_v4(acc: Option<&AccountData>) -> Option<raydium_v4::AmmInfo> {
    let a = acc?;
    raydium_v4::read(&a.owner.to_bytes(), &a.data).ok()
}

fn parse_cpmm(
    pool: Option<&AccountData>,
    config: Option<&AccountData>,
) -> Option<(raydium_cpmm::PoolState, Option<raydium_cpmm::AmmConfig>)> {
    let p = pool?;
    let state = raydium_cpmm::read_pool(&p.owner.to_bytes(), &p.data).ok()?;
    let cfg = config.and_then(|c| raydium_cpmm::read_config(&c.owner.to_bytes(), &c.data).ok());
    Some((state, cfg))
}

/// The snapshot of `mv`'s pool from this tick's ReadSet accounts.
pub fn snapshot(mv: &MarketView) -> PoolSnapshot {
    let acc = |i: usize| mv.read_accounts.get(i).and_then(Option::as_ref);
    let executable = mv.state.executable;
    match mv.kind {
        VenueKind::RaydiumV4 => match parse_v4(acc(0)) {
            Some(info) => PoolSnapshot {
                executable,
                parses: true,
                status: Some(info.status),
                open_time: Some(info.pool_open_time),
                fee_ppm: info.fee_ppm(),
                config: None,
                buy_fee_splits: None,
            },
            None => PoolSnapshot::unreadable(executable),
        },
        VenueKind::RaydiumCpmm => match parse_cpmm(acc(0), acc(1)) {
            Some((state, cfg)) => PoolSnapshot {
                executable,
                parses: true,
                status: Some(u64::from(state.status)),
                open_time: Some(state.open_time),
                fee_ppm: cfg.as_ref().and_then(|c| state.fee_ppm(c)),
                config: Some(Pubkey::new_from_array(state.amm_config)),
                buy_fee_splits: cfg.as_ref().and_then(|c| {
                    state
                        .sol_is_token0()
                        .and_then(|sol0| state.fee_splits(c, sol0))
                }),
            },
            None => PoolSnapshot::unreadable(executable),
        },
        _ => PoolSnapshot {
            executable,
            parses: true,
            status: None,
            open_time: None,
            fee_ppm: executable.then_some(mv.state.fee_ppm),
            config: None,
            buy_fee_splits: None,
        },
    }
}

impl PoolSnapshot {
    fn unreadable(executable: bool) -> Self {
        Self {
            executable,
            parses: false,
            status: None,
            open_time: None,
            fee_ppm: None,
            config: None,
            buy_fee_splits: None,
        }
    }
}

/// The fee caps a fee change is measured against, in ppm (the config's `venue_fee_cap_bps` and
/// `venue_fee_hard_cap_bps` × 100; ARCH §4.2 "Units").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FeeCaps {
    pub open_ppm: u64,
    pub hard_ppm: u64,
}

impl FeeCaps {
    /// From the global config's parameters.
    pub fn of(config: &shortwin::state::GlobalConfig) -> Self {
        Self {
            open_ppm: u64::from(config.params.venue_fee_cap_bps).saturating_mul(100),
            hard_ppm: u64::from(config.params.venue_fee_hard_cap_bps).saturating_mul(100),
        }
    }
}

/// One observed change of a pool's admin-controlled state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdminChange {
    /// The venue can no longer execute (`venue_executable` turned false): swaps disabled by the
    /// admin, an open time in the future, a vault below its fee counters, or the pool no longer
    /// parses. Positions freeze; liquidatable and expired ones reach S3 through the chain.
    RouteLost { detail: String },
    /// The venue executes again.
    RouteRestored { detail: String },
    /// The status field changed (`via`: the admin instruction that writes it).
    StatusChanged {
        from: Option<u64>,
        to: Option<u64>,
        via: &'static str,
    },
    /// The open time changed (AMM v4 `SetParams` param 3; CPMM cannot change it).
    OpenTimeChanged {
        from: Option<u64>,
        to: Option<u64>,
        via: &'static str,
    },
    /// The one-way fee changed; `from` is `None` at first sight of a pool whose fee is already
    /// above the opens cap.
    FeeChanged {
        from_ppm: Option<u32>,
        to_ppm: u32,
        /// Above `venue_fee_cap_bps`: opens are refused.
        above_open_cap: bool,
        /// Above `venue_fee_hard_cap_bps`: `s3_no_route` (the S3 chain; risk quotes stay at the
        /// locked cap).
        above_hard_cap: bool,
        via: &'static str,
    },
    /// The pool account no longer parses under the reader (a layout change): not executable
    /// (A63).
    Unreadable,
}

impl AdminChange {
    /// A change that pages (a lost route, a fee above a cap, an unreadable pool).
    pub fn is_severe(&self) -> bool {
        match self {
            Self::RouteLost { .. } | Self::Unreadable => true,
            Self::FeeChanged {
                above_open_cap,
                above_hard_cap,
                ..
            } => *above_open_cap || *above_hard_cap,
            _ => false,
        }
    }
}

fn status_via(kind: VenueKind) -> &'static str {
    match kind {
        VenueKind::RaydiumV4 => "AMM v4 SetParams (param 0, status)",
        VenueKind::RaydiumCpmm => "CPMM update_pool_status",
        _ => "venue admin",
    }
}

fn fee_via(kind: VenueKind) -> &'static str {
    match kind {
        VenueKind::RaydiumV4 => "AMM v4 SetParams (param 2, fees)",
        VenueKind::RaydiumCpmm => "CPMM update_amm_config (every pool on the config)",
        _ => "venue admin",
    }
}

/// Why a pool is (not) executable, for an alert.
fn route_detail(kind: VenueKind, s: &PoolSnapshot) -> String {
    if !s.parses {
        return "the pool account no longer parses (layout change, A63)".into();
    }
    match (kind, s.status) {
        (VenueKind::RaydiumV4, Some(st)) => {
            let name = match st {
                raydium_v4::STATUS_INITIALIZED => "1 Initialized",
                raydium_v4::STATUS_DISABLED => "2 Disabled",
                raydium_v4::STATUS_WITHDRAW_ONLY => "3 WithdrawOnly",
                raydium_v4::STATUS_LIQUIDITY_ONLY => "4 LiquidityOnly",
                raydium_v4::STATUS_ORDERBOOK_ONLY => "5 OrderBookOnly",
                raydium_v4::STATUS_SWAP_ONLY => "6 SwapOnly",
                raydium_v4::STATUS_WAITING_TRADE => "7 WaitingTrade",
                _ => "unknown (reverts in the venue)",
            };
            format!(
                "AMM v4 status {name}, open time {}",
                s.open_time.unwrap_or(0)
            )
        }
        (VenueKind::RaydiumCpmm, Some(st)) => format!(
            "CPMM status {st:#05b} (swap bit {}), open time {}{}",
            if st & u64::from(raydium_cpmm::STATUS_BIT_SWAP) != 0 {
                "set: swaps disabled"
            } else {
                "clear"
            },
            s.open_time.unwrap_or(0),
            if s.buy_fee_splits == Some(false) {
                ", buys revert: trade + creator fee 0 with the creator fee on the buy's input \
                 (ZeroTradingTokens; CPMM update_amm_config)"
            } else {
                ""
            }
        ),
        _ => format!("venue_executable = {}", s.executable),
    }
}

fn fee_change(kind: VenueKind, from_ppm: Option<u32>, to_ppm: u32, caps: FeeCaps) -> AdminChange {
    AdminChange::FeeChanged {
        from_ppm,
        to_ppm,
        above_open_cap: u64::from(to_ppm) > caps.open_ppm,
        above_hard_cap: u64::from(to_ppm) > caps.hard_ppm,
        via: fee_via(kind),
    }
}

/// What to report at the first sight of a pool: only what is already abnormal (no route, a fee
/// above the opens cap).
pub fn first_sight(kind: VenueKind, new: &PoolSnapshot, caps: FeeCaps) -> Vec<AdminChange> {
    let mut out = Vec::new();
    if !new.parses && matches!(kind, VenueKind::RaydiumV4 | VenueKind::RaydiumCpmm) {
        out.push(AdminChange::Unreadable);
    }
    if !new.executable {
        out.push(AdminChange::RouteLost {
            detail: route_detail(kind, new),
        });
    }
    if let Some(f) = new.fee_ppm
        && u64::from(f) > caps.open_ppm
    {
        out.push(fee_change(kind, None, f, caps));
    }
    out
}

/// The changes from `old` to `new` ([module docs](self)). `last_fee` is the last readable fee
/// (a halted mock or an unreadable pool has none; its return does not count as a change).
pub fn diff(
    kind: VenueKind,
    old: &PoolSnapshot,
    new: &PoolSnapshot,
    last_fee: Option<u32>,
    caps: FeeCaps,
) -> Vec<AdminChange> {
    let mut out = Vec::new();
    let raydium = matches!(kind, VenueKind::RaydiumV4 | VenueKind::RaydiumCpmm);
    if raydium && old.parses && !new.parses {
        out.push(AdminChange::Unreadable);
    }
    if old.executable && !new.executable {
        out.push(AdminChange::RouteLost {
            detail: route_detail(kind, new),
        });
    } else if !old.executable && new.executable {
        out.push(AdminChange::RouteRestored {
            detail: route_detail(kind, new),
        });
    }
    if new.parses && old.parses && old.status != new.status {
        out.push(AdminChange::StatusChanged {
            from: old.status,
            to: new.status,
            via: status_via(kind),
        });
    }
    if new.parses && old.parses && old.open_time != new.open_time {
        out.push(AdminChange::OpenTimeChanged {
            from: old.open_time,
            to: new.open_time,
            via: "AMM v4 SetParams (param 3, open time)",
        });
    }
    if let Some(f) = new.fee_ppm
        && last_fee.is_some_and(|l| l != f)
    {
        out.push(fee_change(kind, last_fee, f, caps));
    }
    out
}

/// Per-market monitor state.
#[derive(Clone, Debug, Default)]
pub struct PoolWatch {
    pub last: Option<PoolSnapshot>,
    /// The last readable fee.
    pub last_fee: Option<u32>,
}

impl PoolWatch {
    /// Records `new` and returns what changed since the last tick ([`first_sight`] the first
    /// time).
    pub fn observe(
        &mut self,
        kind: VenueKind,
        new: PoolSnapshot,
        caps: FeeCaps,
    ) -> Vec<AdminChange> {
        let out = match &self.last {
            None => first_sight(kind, &new, caps),
            Some(old) => diff(kind, old, &new, self.last_fee, caps),
        };
        if new.fee_ppm.is_some() {
            self.last_fee = new.fee_ppm;
        }
        self.last = Some(new);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shortwin::constants::{
        GUARD_RAYDIUM_CPMM, GUARD_RAYDIUM_V4, RAYDIUM_V4_PROGRAM_ID, VENUE_GUARD_SLOTS_2026_09_27,
    };

    fn guard() -> VenueGuard {
        let mut g: VenueGuard = bytemuck::Zeroable::zeroed();
        g.guard_slot = VENUE_GUARD_SLOTS_2026_09_27;
        g
    }

    #[test]
    fn the_watched_programs_are_the_latch_order_then_token_2022() {
        let w = watched_programs();
        assert_eq!(w.len(), VENUE_PROGRAMS + 1);
        for (i, p) in w.iter().take(VENUE_PROGRAMS).enumerate() {
            assert_eq!(p.guard, Some(i as u8));
            assert_eq!(p.program_data, VENUE_PROGRAM_DATA[i]);
            assert_eq!(program_data_address(&p.program), p.program_data);
        }
        assert_eq!(
            w[usize::from(GUARD_RAYDIUM_V4)].program,
            RAYDIUM_V4_PROGRAM_ID
        );
        let t22 = w.last().unwrap();
        assert_eq!((t22.program, t22.guard), (TOKEN_2022_PROGRAM_ID, None));
    }

    /// The program's rule, all four cases (ARCH §3.1; T-LATCH's host twin).
    #[test]
    fn latch_actions_follow_flag_venue_upgraded() {
        let mut g = guard();
        let v = GUARD_RAYDIUM_CPMM;
        let slot = g.guard_slot[usize::from(v)];
        assert_eq!(latch_action(&g, v, slot), LatchAction::Nothing);
        assert_eq!(latch_action(&g, v, slot + 777), LatchAction::Flag);
        // Flagged at slot + 777.
        g.upgraded |= 1 << v;
        g.flagged_slot[usize::from(v)] = slot + 777;
        assert_eq!(latch_action(&g, v, slot + 777), LatchAction::Nothing);
        assert_eq!(latch_action(&g, v, slot + 999), LatchAction::Refresh);
        // Even a return to the validated slot is a change while flagged (the program refreshes).
        assert_eq!(latch_action(&g, v, slot), LatchAction::Refresh);
        // Other venues are independent; an out-of-range index is never acted on.
        let o = GUARD_RAYDIUM_V4;
        assert_eq!(
            latch_action(&g, o, g.guard_slot[usize::from(o)]),
            LatchAction::Nothing
        );
        assert_eq!(latch_action(&g, 6, 1), LatchAction::Nothing);
    }

    #[test]
    fn deploy_slots_parse_only_real_program_data_headers() {
        let loader = Pubkey::new_from_array(program_data::LOADER_UPGRADEABLE);
        let acc = |owner, data: Vec<u8>| AccountData {
            lamports: 1,
            owner,
            executable: false,
            data,
        };
        let header = program_data::encode_header(445_763_204, None).to_vec();
        assert_eq!(
            deploy_slot(Some(&acc(loader, header.clone()))),
            Some(445_763_204)
        );
        assert_eq!(
            deploy_slot(Some(&acc(Pubkey::new_unique(), header.clone()))),
            None
        );
        let mut bad = header;
        bad[0] = 2; // tag Program, not ProgramData
        assert_eq!(deploy_slot(Some(&acc(loader, bad))), None);
        assert_eq!(deploy_slot(None), None);
    }

    fn snap(executable: bool, status: u64, fee: u32) -> PoolSnapshot {
        PoolSnapshot {
            executable,
            parses: true,
            status: Some(status),
            open_time: Some(0),
            fee_ppm: Some(fee),
            config: None,
            buy_fee_splits: None,
        }
    }

    const CAPS: FeeCaps = FeeCaps {
        open_ppm: 15_000,
        hard_ppm: 50_000,
    };

    #[test]
    fn a_halt_a_fee_hike_and_a_resume_are_reported_once_each() {
        let k = VenueKind::RaydiumV4;
        let mut w = PoolWatch::default();
        assert!(w.observe(k, snap(true, 1, 2_500), CAPS).is_empty());
        assert!(w.observe(k, snap(true, 1, 2_500), CAPS).is_empty());
        // SetParams status 2.
        let c = w.observe(k, snap(false, 2, 2_500), CAPS);
        assert_eq!(c.len(), 2, "{c:?}");
        assert!(
            matches!(&c[0], AdminChange::RouteLost { detail } if detail.contains("2 Disabled"))
        );
        assert!(c[0].is_severe());
        assert!(matches!(
            c[1],
            AdminChange::StatusChanged {
                from: Some(1),
                to: Some(2),
                ..
            }
        ));
        // SetParams fees while halted: 3 % (above the 1.5 % opens cap, below the 5 % hard cap).
        let c = w.observe(k, snap(false, 2, 30_000), CAPS);
        assert_eq!(
            c,
            vec![AdminChange::FeeChanged {
                from_ppm: Some(2_500),
                to_ppm: 30_000,
                above_open_cap: true,
                above_hard_cap: false,
                via: "AMM v4 SetParams (param 2, fees)",
            }]
        );
        // Resume.
        let c = w.observe(k, snap(true, 1, 30_000), CAPS);
        assert!(matches!(c[0], AdminChange::RouteRestored { .. }), "{c:?}");
        assert!(!c[0].is_severe());
    }

    #[test]
    fn first_sight_reports_only_what_is_already_abnormal() {
        let k = VenueKind::RaydiumCpmm;
        assert!(first_sight(k, &snap(true, 0, 2_500), CAPS).is_empty());
        let c = first_sight(k, &snap(false, 4, 60_000), CAPS);
        assert!(
            matches!(&c[0], AdminChange::RouteLost { detail } if detail.contains("swaps disabled"))
        );
        assert!(matches!(
            c[1],
            AdminChange::FeeChanged {
                from_ppm: None,
                to_ppm: 60_000,
                above_open_cap: true,
                above_hard_cap: true,
                ..
            }
        ));
    }

    /// M3-SEC-1: `update_amm_config(0, 0)` on a pool whose creator fee is on the buy's input
    /// leaves the status clear but every buy reverts (`ZeroTradingTokens`): the program's reader
    /// calls it not executable, and the alert is a lost route that says why, besides the fee
    /// change to 0.
    #[test]
    fn a_zero_fee_route_loss_says_why() {
        let k = VenueKind::RaydiumCpmm;
        let mut w = PoolWatch::default();
        assert!(w.observe(k, snap(true, 0, 2_500), CAPS).is_empty());
        let zero = PoolSnapshot {
            buy_fee_splits: Some(false),
            ..snap(false, 0, 0)
        };
        let c = w.observe(k, zero, CAPS);
        assert_eq!(c.len(), 2, "{c:?}");
        assert!(
            matches!(&c[0], AdminChange::RouteLost { detail }
                if detail.contains("swap bit clear") && detail.contains("ZeroTradingTokens")),
            "{c:?}"
        );
        assert!(c[0].is_severe());
        assert_eq!(
            c[1],
            AdminChange::FeeChanged {
                from_ppm: Some(2_500),
                to_ppm: 0,
                above_open_cap: false,
                above_hard_cap: false,
                via: "CPMM update_amm_config (every pool on the config)",
            }
        );
        // A halt's detail does not blame the fee.
        let halted = first_sight(k, &snap(false, 4, 2_500), CAPS);
        assert!(
            matches!(&halted[0], AdminChange::RouteLost { detail } if !detail.contains("ZeroTradingTokens")),
            "{halted:?}"
        );
    }

    #[test]
    fn a_pool_that_stops_parsing_is_unreadable_and_not_a_fee_change() {
        let k = VenueKind::RaydiumCpmm;
        let mut w = PoolWatch::default();
        w.observe(k, snap(true, 0, 2_500), CAPS);
        let c = w.observe(k, PoolSnapshot::unreadable(false), CAPS);
        assert_eq!(c[0], AdminChange::Unreadable);
        assert!(
            matches!(&c[1], AdminChange::RouteLost { detail } if detail.contains("no longer parses"))
        );
        assert_eq!(c.len(), 2, "{c:?}");
        // It parses again at the same fee: the route is back, the fee did not change.
        let c = w.observe(k, snap(true, 0, 2_500), CAPS);
        assert_eq!(c.len(), 1, "{c:?}");
        assert!(matches!(c[0], AdminChange::RouteRestored { .. }));
    }
}
