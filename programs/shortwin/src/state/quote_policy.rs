//! Immutable governed QuotePilot contract. Initialization never enables financial admission.
//! Producer commitments are separately pinned per canonical exact-q feed. They are excluded
//! from the shared financial hash because each producer commitment includes that global hash.
use super::{Market, PriceSnapshot, QUOTE_DEFAULT_GRACE_SECONDS};
use crate::constants::{MAX_RELEASE_SPAN_S, TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID, WSOL_MINT};
use crate::errors::ShortwinError;
use anchor_lang::prelude::*;
use shortwin_math::{
    params::{Rule, MIN_NOTIONAL_FLOOR},
    quote_native, quote_rent, risk,
};
use shortwin_price_report as report;

pub const QUOTE_POLICY_VERSION: u16 = 1;
/// Operational price-action pause; independent of the backend selector and repayment/default.
pub const MARKET_FLAG_QUOTE_PRICE_PAUSED: u16 = 1 << 10;
pub const QUOTE_MAX_LOTS: usize = 8;
pub const QUOTE_MAX_FEEDS: usize = QUOTE_MAX_LOTS + 2;
pub const QUOTE_MARKET_NOTIONAL_CEILING: u64 = 25_000_000_000;

#[derive(AnchorSerialize, AnchorDeserialize, InitSpace, Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuotePolicyDomain {
    pub genesis_hash: [u8; 32],
    pub program: Pubkey,
    pub config: Pubkey,
    pub market: Pubkey,
    pub base_mint: Pubkey,
    pub base_token_program: Pubkey,
    pub quote_mint: Pubkey,
    pub quote_token_program: Pubkey,
    pub base_decimals: u8,
    pub quote_decimals: u8,
}
impl QuotePolicyDomain {
    pub fn for_market(
        genesis_hash: [u8; 32],
        config: Pubkey,
        market_key: Pubkey,
        market: &Market,
    ) -> Self {
        Self {
            genesis_hash,
            program: crate::ID,
            config,
            market: market_key,
            base_mint: market.mint,
            base_token_program: market.token_program,
            base_decimals: market.decimals,
            quote_mint: WSOL_MINT,
            quote_token_program: TOKEN_PROGRAM_ID,
            quote_decimals: 9,
        }
    }
    pub fn core(&self) -> report::Domain {
        report::Domain {
            genesis_hash: self.genesis_hash,
            program: self.program.to_bytes(),
            config: self.config.to_bytes(),
            market: self.market.to_bytes(),
            base_mint: self.base_mint.to_bytes(),
            base_token_program: self.base_token_program.to_bytes(),
            quote_mint: self.quote_mint.to_bytes(),
            quote_token_program: self.quote_token_program.to_bytes(),
            base_decimals: self.base_decimals,
            quote_decimals: self.quote_decimals,
        }
    }
    fn write(&self, out: &mut CanonicalWriter) {
        for key in [
            self.genesis_hash,
            self.program.to_bytes(),
            self.config.to_bytes(),
            self.market.to_bytes(),
            self.base_mint.to_bytes(),
            self.base_token_program.to_bytes(),
            self.quote_mint.to_bytes(),
            self.quote_token_program.to_bytes(),
        ] {
            out.bytes(&key);
        }
        out.u8(self.base_decimals);
        out.u8(self.quote_decimals);
    }
    pub fn digest(&self) -> [u8; 32] {
        let mut out = CanonicalWriter::new();
        out.bytes(b"shortwin-quote-domain-v1");
        self.write(&mut out);
        out.hash()
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, InitSpace, Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuoteHistoryPolicy {
    pub max_age_slots: u64,
    pub max_age_seconds: u64,
    pub max_window_slots: u64,
    pub max_window_seconds: u64,
    pub max_update_gap_slots: u64,
    pub max_update_gap_seconds: u64,
    pub epoch_seconds: u64,
    pub minimum_completed_epochs: u8,
}
impl QuoteHistoryPolicy {
    pub fn core(&self) -> report::HistoryPolicy {
        report::HistoryPolicy {
            max_age_slots: self.max_age_slots,
            max_age_seconds: self.max_age_seconds,
            max_window_slots: self.max_window_slots,
            max_window_seconds: self.max_window_seconds,
            max_update_gap_slots: self.max_update_gap_slots,
            max_update_gap_seconds: self.max_update_gap_seconds,
            epoch_seconds: self.epoch_seconds,
            minimum_completed_epochs: self.minimum_completed_epochs,
        }
    }
    pub fn validate(&self) -> Result<()> {
        require!(
            self.max_age_slots > 0
                && self.max_age_slots <= 512
                && self.max_age_seconds > 0
                && self.max_age_seconds <= 60
                && self.max_window_slots <= self.max_age_slots
                && self.max_window_seconds <= self.max_age_seconds
                && self.max_update_gap_slots > 0
                && self.max_update_gap_slots <= 512
                && self.max_update_gap_seconds > 0
                && self.max_update_gap_seconds <= self.epoch_seconds
                && self.epoch_seconds > 0
                && self.epoch_seconds <= 3600
                && self.minimum_completed_epochs > 0
                && usize::from(self.minimum_completed_epochs) <= report::HISTORY_CAPACITY,
            ShortwinError::InvalidParams
        );
        Ok(())
    }
    fn write(&self, out: &mut CanonicalWriter) {
        for n in [
            self.max_age_slots,
            self.max_age_seconds,
            self.max_window_slots,
            self.max_window_seconds,
            self.max_update_gap_slots,
            self.max_update_gap_seconds,
            self.epoch_seconds,
        ] {
            out.u64(n);
        }
        out.u8(self.minimum_completed_epochs);
    }
    pub fn evidence_policy(&self) -> quote_native::EvidencePolicy {
        quote_native::EvidencePolicy {
            epoch_seconds: self.epoch_seconds,
            history_length: usize::from(self.minimum_completed_epochs),
            max_age_seconds: self.max_age_seconds,
            max_age_slots: self.max_age_slots,
            max_source_window_seconds: self.max_window_seconds,
            max_source_window_slots: self.max_window_slots,
        }
    }
}

/// All fields are explicit governance inputs. No selected production values are supplied.
/// One QuotePilot profile, with conservative legacy-D CR floors, never automatic A/B tiering.
#[derive(AnchorSerialize, AnchorDeserialize, InitSpace, Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuoteFinancialTerms {
    pub position_quantity_max: u64,
    pub market_quantity_max: u64,
    pub min_notional: u64,
    pub position_notional_max: u64,
    pub market_notional_max: u64,
    pub utilization_cap_bps: u16,
    pub wallet_cap_bps: u16,
    pub position_premium_bps: u16,
    pub capacity_premium_bps: u16,
    pub cr_open_bps: u32,
    pub cr_liq_bps: u32,
    pub open_fee_bps: u16,
    pub rent_protocol_bps: u16,
    pub util_kink_bps: u16,
    pub rate_base_bps: u32,
    pub rate_slope1_bps: u32,
    pub rate_slope2_bps: u32,
    pub min_rent_seconds: u32,
    pub max_duration_seconds: u32,
    pub penalty_bps: u16,
    pub penalty_lender_bps: u16,
    pub penalty_protocol_bps: u16,
    pub penalty_keeper_bps: u16,
    pub expiry_fee_bps: u16,
    pub settle_keeper_fixed: u64,
    pub settle_keeper_bps: u16,
    pub settle_keeper_cap: u64,
    pub expiry_keeper_fixed: u64,
    pub expiry_keeper_bps: u16,
    pub expiry_keeper_cap: u64,
    pub liquidation_delta_bps: u16,
    pub expiry_delta_bps: u16,
    pub completed_liquidation_epochs: u8,
    pub cr_arm_delay_seconds: u32,
    pub cr_arm_delay_slots: u64,
    pub coherence_max_seconds: u64,
    pub coherence_max_slots: u64,
    pub lump_vest_seconds: u32,
    /// Fixed contractual maturity default: borrower zero, lenders V-K even if healthy.
    pub default_grace_seconds: u32,
}
fn in_bounds(rule: Rule, value: u64) -> bool {
    let (lo, hi) = rule.bounds();
    lo <= value && value <= hi
}
impl QuoteFinancialTerms {
    pub fn validate(&self, history: &QuoteHistoryPolicy) -> Result<()> {
        history.validate()?;
        require!(
            self.position_quantity_max > 0
                && self.position_quantity_max <= self.market_quantity_max
                && self.min_notional >= MIN_NOTIONAL_FLOOR
                && in_bounds(Rule::MinNotional, self.min_notional)
                && self.min_notional <= self.position_notional_max
                && self.position_notional_max <= self.market_notional_max
                && self.market_notional_max <= QUOTE_MARKET_NOTIONAL_CEILING
                && self.position_premium_bps <= 10_000
                && self.capacity_premium_bps <= 10_000
                && self.cr_open_bps >= 40_000
                && self.cr_open_bps <= 100_000
                && self.cr_liq_bps >= 17_500
                && self
                    .cr_liq_bps
                    .checked_add(2_000)
                    .is_some_and(|min| self.cr_open_bps >= min)
                && self.util_kink_bps <= self.utilization_cap_bps
                && u32::from(self.penalty_lender_bps)
                    .checked_add(u32::from(self.penalty_protocol_bps))
                    .and_then(|n| n.checked_add(u32::from(self.penalty_keeper_bps)))
                    == Some(10_000)
                && self.settle_keeper_fixed <= self.settle_keeper_cap
                && self.expiry_keeper_fixed <= self.expiry_keeper_cap
                && self.liquidation_delta_bps >= self.expiry_delta_bps
                && self.completed_liquidation_epochs > 0
                && self.completed_liquidation_epochs <= history.minimum_completed_epochs
                && (2..=3600).contains(&self.cr_arm_delay_seconds)
                && self.cr_arm_delay_slots > 0
                && self.cr_arm_delay_slots <= 9000
                && self.coherence_max_seconds > 0
                && self.coherence_max_seconds <= history.max_age_seconds
                && self.coherence_max_slots > 0
                && self.coherence_max_slots <= history.max_age_slots
                && self.max_duration_seconds <= 259_200
                && self.min_rent_seconds <= self.max_duration_seconds
                && i64::from(self.default_grace_seconds) == QUOTE_DEFAULT_GRACE_SECONDS
                && i64::from(self.max_duration_seconds)
                    .checked_add(QUOTE_DEFAULT_GRACE_SECONDS)
                    .is_some_and(|h| h <= MAX_RELEASE_SPAN_S),
            ShortwinError::InvalidParams
        );
        for (rule, value) in [
            (Rule::UtilCapBps, u64::from(self.utilization_cap_bps)),
            (Rule::WalletCapBps, u64::from(self.wallet_cap_bps)),
            (Rule::OpenFeeBps, u64::from(self.open_fee_bps)),
            (Rule::RentProtocolBps, u64::from(self.rent_protocol_bps)),
            (Rule::UtilKinkBps, u64::from(self.util_kink_bps)),
            (Rule::TierRateBaseBps, u64::from(self.rate_base_bps)),
            (Rule::TierRateSlope1Bps, u64::from(self.rate_slope1_bps)),
            (Rule::TierRateSlope2Bps, u64::from(self.rate_slope2_bps)),
            (Rule::TierMinRentS, u64::from(self.min_rent_seconds)),
            (Rule::TierMaxDurationS, u64::from(self.max_duration_seconds)),
            (Rule::TierPenaltyBps, u64::from(self.penalty_bps)),
            (
                Rule::PenaltyProtocolBps,
                u64::from(self.penalty_protocol_bps),
            ),
            (Rule::PenaltyKeeperBps, u64::from(self.penalty_keeper_bps)),
            (Rule::ExpiryFeeBps, u64::from(self.expiry_fee_bps)),
            (Rule::SettleKeeperFixed, self.settle_keeper_fixed),
            (Rule::SettleKeeperBps, u64::from(self.settle_keeper_bps)),
            (Rule::SettleKeeperCap, self.settle_keeper_cap),
            (Rule::ExpiryKeeperFixed, self.expiry_keeper_fixed),
            (Rule::ExpiryKeeperBps, u64::from(self.expiry_keeper_bps)),
            (Rule::ExpiryKeeperCap, self.expiry_keeper_cap),
            (Rule::DeltaLiqBps, u64::from(self.liquidation_delta_bps)),
            (Rule::DeltaExpBps, u64::from(self.expiry_delta_bps)),
            (Rule::LumpVestS, u64::from(self.lump_vest_seconds)),
        ] {
            require!(in_bounds(rule, value), ShortwinError::InvalidParams);
        }
        Ok(())
    }
    pub fn rates(&self) -> quote_rent::LockedRates {
        quote_rent::LockedRates::new(
            self.rate_base_bps,
            self.rate_slope1_bps,
            self.rate_slope2_bps,
        )
    }
    pub fn default_keeper_terms(&self) -> risk::SettleKeeperTerms {
        risk::SettleKeeperTerms {
            fixed: self.settle_keeper_fixed,
            bps: u32::from(self.settle_keeper_bps),
            cap: self.settle_keeper_cap,
        }
    }
    pub fn expiry_keeper_terms(&self) -> risk::ExpiryKeeperTerms {
        risk::ExpiryKeeperTerms {
            fixed: self.expiry_keeper_fixed,
            bps: u32::from(self.expiry_keeper_bps),
            cap: self.expiry_keeper_cap,
        }
    }
    pub fn coherence_limits(&self) -> quote_native::CoherenceLimits {
        quote_native::CoherenceLimits {
            max_seconds: self.coherence_max_seconds,
            max_slots: self.coherence_max_slots,
        }
    }
    fn write(&self, out: &mut CanonicalWriter) {
        for n in [
            self.position_quantity_max,
            self.market_quantity_max,
            self.min_notional,
            self.position_notional_max,
            self.market_notional_max,
        ] {
            out.u64(n);
        }
        for n in [
            self.utilization_cap_bps,
            self.wallet_cap_bps,
            self.position_premium_bps,
            self.capacity_premium_bps,
        ] {
            out.u16(n);
        }
        out.u32(self.cr_open_bps);
        out.u32(self.cr_liq_bps);
        for n in [
            self.open_fee_bps,
            self.rent_protocol_bps,
            self.util_kink_bps,
        ] {
            out.u16(n);
        }
        for n in [
            self.rate_base_bps,
            self.rate_slope1_bps,
            self.rate_slope2_bps,
            self.min_rent_seconds,
            self.max_duration_seconds,
        ] {
            out.u32(n);
        }
        for n in [
            self.penalty_bps,
            self.penalty_lender_bps,
            self.penalty_protocol_bps,
            self.penalty_keeper_bps,
            self.expiry_fee_bps,
        ] {
            out.u16(n);
        }
        out.u64(self.settle_keeper_fixed);
        out.u16(self.settle_keeper_bps);
        out.u64(self.settle_keeper_cap);
        out.u64(self.expiry_keeper_fixed);
        out.u16(self.expiry_keeper_bps);
        out.u64(self.expiry_keeper_cap);
        out.u16(self.liquidation_delta_bps);
        out.u16(self.expiry_delta_bps);
        out.u8(self.completed_liquidation_epochs);
        out.u32(self.cr_arm_delay_seconds);
        out.u64(self.cr_arm_delay_slots);
        out.u64(self.coherence_max_seconds);
        out.u64(self.coherence_max_slots);
        out.u32(self.lump_vest_seconds);
        out.u32(self.default_grace_seconds);
    }
}

#[derive(
    AnchorSerialize, AnchorDeserialize, InitSpace, Clone, Copy, Debug, PartialEq, Eq, Default,
)]
pub struct QuoteFeedBinding {
    pub quantity: u64,
    pub snapshot: Pubkey,
    pub producer_commitment: [u8; 32],
}

#[account]
#[derive(InitSpace, Debug, PartialEq, Eq)]
pub struct QuotePolicy {
    pub version: u16,
    pub bump: u8,
    pub domain: QuotePolicyDomain,
    pub model_version: u32,
    pub model_epoch: u64,
    pub policy_hash: [u8; 32],
    pub history: QuoteHistoryPolicy,
    pub lot_count: u8,
    pub lots: [u64; 8],
    pub small_lot_quantity: u64,
    pub capacity_quantity: u64,
    pub feed_count: u8,
    pub feeds: [QuoteFeedBinding; 10],
    pub terms: QuoteFinancialTerms,
}
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub struct InitializeQuotePolicyArgs {
    pub genesis_hash: [u8; 32],
    pub model_version: u32,
    pub model_epoch: u64,
    pub expected_policy_hash: [u8; 32],
    pub history: QuoteHistoryPolicy,
    pub lot_count: u8,
    pub lots: [u64; 8],
    pub small_lot_quantity: u64,
    pub capacity_quantity: u64,
    /// Canonical sorted union order; unused suffix must be all zero.
    pub producer_commitments: [[u8; 32]; 10],
    pub terms: QuoteFinancialTerms,
}
impl InitializeQuotePolicyArgs {
    /// Off-chain preparation helper; the supplied domain is independently derived/checked
    /// against actual market/config accounts by initialization. No producer hash recursion.
    pub fn financial_hash_for_domain(&self, domain: &QuotePolicyDomain) -> [u8; 32] {
        let bytes = canonical_financial_preimage(
            domain,
            self.model_version,
            self.model_epoch,
            &self.history,
            self.lot_count,
            &self.lots,
            self.small_lot_quantity,
            self.capacity_quantity,
            &self.terms,
        );
        solana_sha256_hasher::hashv(&[&bytes]).to_bytes()
    }
}
impl QuotePolicy {
    pub const SEED: &'static [u8] = b"quote_policy";
    pub const SPACE: usize = 8 + Self::INIT_SPACE;
    #[inline(never)]
    pub fn from_args(
        config: Pubkey,
        market_key: Pubkey,
        market: &Market,
        args: &InitializeQuotePolicyArgs,
        bump: u8,
    ) -> Result<Box<Self>> {
        let quantities = quantity_registry(
            &args.lots,
            args.lot_count,
            args.small_lot_quantity,
            args.capacity_quantity,
            &args.terms,
        )?;
        let mut feeds = [QuoteFeedBinding::default(); QUOTE_MAX_FEEDS];
        let mut count = 0u8;
        for (index, quantity) in quantities.iter().enumerate() {
            if *quantity == 0 {
                require!(
                    args.producer_commitments[index] == [0; 32],
                    ShortwinError::InvalidParams
                );
            } else {
                require!(
                    args.producer_commitments[index] != [0; 32],
                    ShortwinError::InvalidParams
                );
                feeds[index] = QuoteFeedBinding {
                    quantity: *quantity,
                    snapshot: Pubkey::find_program_address(
                        &[
                            PriceSnapshot::SEED,
                            market_key.as_ref(),
                            &quantity.to_le_bytes(),
                            &args.model_epoch.to_le_bytes(),
                        ],
                        &crate::ID,
                    )
                    .0,
                    producer_commitment: args.producer_commitments[index],
                };
                count = count.checked_add(1).ok_or(ShortwinError::MathOverflow)?;
            }
        }
        let policy = Box::new(Self {
            version: QUOTE_POLICY_VERSION,
            bump,
            domain: QuotePolicyDomain {
                genesis_hash: args.genesis_hash,
                program: crate::ID,
                config,
                market: market_key,
                base_mint: market.mint,
                base_token_program: market.token_program,
                base_decimals: market.decimals,
                quote_mint: WSOL_MINT,
                quote_token_program: TOKEN_PROGRAM_ID,
                quote_decimals: 9,
            },
            model_version: args.model_version,
            model_epoch: args.model_epoch,
            policy_hash: args.expected_policy_hash,
            history: args.history,
            lot_count: args.lot_count,
            lots: args.lots,
            small_lot_quantity: args.small_lot_quantity,
            capacity_quantity: args.capacity_quantity,
            feed_count: count,
            feeds,
            terms: args.terms,
        });
        policy.validate_binding(&config, &market_key, market)?;
        Ok(policy)
    }
    #[inline(never)]
    pub fn validate_binding(
        &self,
        config: &Pubkey,
        market_key: &Pubkey,
        market: &Market,
    ) -> Result<()> {
        require_eq!(
            self.version,
            QUOTE_POLICY_VERSION,
            ShortwinError::ConfigVersion
        );
        let d = &self.domain;
        require!(
            d.genesis_hash != [0; 32]
                && d.program == crate::ID
                && d.config == *config
                && d.market == *market_key
                && d.base_mint == market.mint
                && d.base_mint != WSOL_MINT
                && d.base_token_program == market.token_program
                && [TOKEN_PROGRAM_ID, TOKEN_2022_PROGRAM_ID].contains(&d.base_token_program)
                && d.base_decimals == market.decimals
                && d.quote_mint == WSOL_MINT
                && d.quote_token_program == TOKEN_PROGRAM_ID
                && d.quote_decimals == 9
                && self.model_version > 0
                && self.model_epoch > 0
                && self.policy_hash != [0; 32],
            ShortwinError::AccountMismatch
        );
        self.terms.validate(&self.history)?;
        let registry = quantity_registry(
            &self.lots,
            self.lot_count,
            self.small_lot_quantity,
            self.capacity_quantity,
            &self.terms,
        )?;
        let count = registry.iter().filter(|q| **q != 0).count();
        require!(
            usize::from(self.feed_count) == count,
            ShortwinError::AccountMismatch
        );
        for (index, q) in registry.iter().enumerate() {
            let f = &self.feeds[index];
            if *q == 0 {
                require!(
                    *f == QuoteFeedBinding::default(),
                    ShortwinError::AccountMismatch
                );
            } else {
                let expected = Pubkey::find_program_address(
                    &[
                        PriceSnapshot::SEED,
                        market_key.as_ref(),
                        &q.to_le_bytes(),
                        &self.model_epoch.to_le_bytes(),
                    ],
                    &crate::ID,
                )
                .0;
                require!(
                    f.quantity == *q && f.snapshot == expected && f.producer_commitment != [0; 32],
                    ShortwinError::AccountMismatch
                );
            }
        }
        require!(
            self.financial_hash() == self.policy_hash,
            ShortwinError::AccountMismatch
        );
        Ok(())
    }
    /// Manual little-endian canonical hash preimage; excludes storage bump, expected hash,
    /// and per-feed producer commitments/derived addresses. No serde, Borsh or SDK layout.
    #[inline(never)]
    pub fn financial_preimage(&self) -> Vec<u8> {
        canonical_financial_preimage(
            &self.domain,
            self.model_version,
            self.model_epoch,
            &self.history,
            self.lot_count,
            &self.lots,
            self.small_lot_quantity,
            self.capacity_quantity,
            &self.terms,
        )
    }
    pub fn financial_hash(&self) -> [u8; 32] {
        solana_sha256_hasher::hashv(&[&self.financial_preimage()]).to_bytes()
    }
    pub fn feed(&self, quantity: u64) -> Result<&QuoteFeedBinding> {
        self.feeds
            .get(..usize::from(self.feed_count))
            .and_then(|f| f.iter().find(|f| f.quantity == quantity))
            .ok_or_else(|| error!(ShortwinError::AccountMismatch))
    }
    pub fn admission_terms(&self) -> Result<quote_native::AdmissionTerms<'_>> {
        let lots = self
            .lots
            .get(..usize::from(self.lot_count))
            .ok_or(ShortwinError::AccountMismatch)?;
        Ok(quote_native::AdmissionTerms {
            supported_position_lots: lots,
            small_lot_quantity: self.small_lot_quantity,
            capacity_quantity: self.capacity_quantity,
            position_quantity_max: self.terms.position_quantity_max,
            market_quantity_max: self.terms.market_quantity_max,
            utilization_bps: u32::from(self.terms.utilization_cap_bps),
            wallet_bps: u32::from(self.terms.wallet_cap_bps),
            position_premium_bps: u32::from(self.terms.position_premium_bps),
            capacity_premium_bps: u32::from(self.terms.capacity_premium_bps),
            min_notional: self.terms.min_notional,
            position_notional_max: self.terms.position_notional_max,
            market_notional_max: self.terms.market_notional_max,
            cr_open_bps: self.terms.cr_open_bps,
            open_fee_bps: u32::from(self.terms.open_fee_bps),
        })
    }
}

/// Sorted unique union of position lots plus small/capacity quantities. Zero suffix is canonical.
pub fn quantity_registry(
    lots: &[u64; 8],
    count: u8,
    small: u64,
    capacity: u64,
    terms: &QuoteFinancialTerms,
) -> Result<[u64; 10]> {
    let count = usize::from(count);
    require!(
        count > 0
            && count <= QUOTE_MAX_LOTS
            && small > 0
            && capacity >= terms.market_quantity_max
            && capacity > 0,
        ShortwinError::InvalidParams
    );
    let mut previous = 0u64;
    for (index, q) in lots.iter().enumerate() {
        if index < count {
            require!(
                *q > previous && *q >= small && *q <= terms.position_quantity_max,
                ShortwinError::InvalidParams
            );
            previous = *q;
        } else {
            require!(*q == 0, ShortwinError::InvalidParams);
        }
    }
    let mut all = [0u64; 10];
    all[..count].copy_from_slice(&lots[..count]);
    all[count] = small;
    all[count + 1] = capacity;
    all.sort_unstable();
    let mut result = [0; 10];
    let mut used = 0usize;
    let mut last = 0u64;
    for q in all {
        if q != 0 && q != last {
            *result.get_mut(used).ok_or(ShortwinError::InvalidParams)? = q;
            used = used.checked_add(1).ok_or(ShortwinError::MathOverflow)?;
            last = q;
        }
    }
    Ok(result)
}
struct CanonicalWriter {
    data: Vec<u8>,
}
impl CanonicalWriter {
    fn new() -> Self {
        Self {
            data: Vec::with_capacity(768),
        }
    }
    fn bytes(&mut self, v: &[u8]) {
        self.data.extend_from_slice(v);
    }
    fn u8(&mut self, v: u8) {
        self.data.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.bytes(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.bytes(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.bytes(&v.to_le_bytes());
    }
    fn hash(self) -> [u8; 32] {
        solana_sha256_hasher::hashv(&[&self.data]).to_bytes()
    }
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn canonical_financial_preimage(
    domain: &QuotePolicyDomain,
    model_version: u32,
    model_epoch: u64,
    history: &QuoteHistoryPolicy,
    lot_count: u8,
    lots: &[u64; 8],
    small: u64,
    capacity: u64,
    terms: &QuoteFinancialTerms,
) -> Vec<u8> {
    let mut out = CanonicalWriter::new();
    out.bytes(b"shortwin-quote-pilot-financial-policy-v1");
    out.u16(QUOTE_POLICY_VERSION);
    domain.write(&mut out);
    out.u32(model_version);
    out.u64(model_epoch);
    history.write(&mut out);
    out.u8(lot_count);
    for q in lots {
        out.u64(*q);
    }
    out.u64(small);
    out.u64(capacity);
    terms.write(&mut out);
    out.data
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::arithmetic_side_effects)]
pub(crate) mod tests {
    use super::*;
    use bytemuck::Zeroable;
    pub(crate) fn terms() -> QuoteFinancialTerms {
        QuoteFinancialTerms {
            position_quantity_max: 2000,
            market_quantity_max: 10000,
            min_notional: 100_000_000,
            position_notional_max: 5_000_000_000,
            market_notional_max: 25_000_000_000,
            utilization_cap_bps: 9000,
            wallet_cap_bps: 5000,
            position_premium_bps: 1000,
            capacity_premium_bps: 1500,
            cr_open_bps: 40000,
            cr_liq_bps: 17500,
            open_fee_bps: 25,
            rent_protocol_bps: 1500,
            util_kink_bps: 8000,
            rate_base_bps: 10000,
            rate_slope1_bps: 37500,
            rate_slope2_bps: 550000,
            min_rent_seconds: 3600,
            max_duration_seconds: 86400,
            penalty_bps: 1000,
            penalty_lender_bps: 7500,
            penalty_protocol_bps: 1250,
            penalty_keeper_bps: 1250,
            expiry_fee_bps: 100,
            settle_keeper_fixed: 1_000_000,
            settle_keeper_bps: 50,
            settle_keeper_cap: 10_000_000,
            expiry_keeper_fixed: 100_000,
            expiry_keeper_bps: 10,
            expiry_keeper_cap: 5_000_000,
            liquidation_delta_bps: 300,
            expiry_delta_bps: 100,
            completed_liquidation_epochs: 2,
            cr_arm_delay_seconds: 2,
            cr_arm_delay_slots: 2,
            coherence_max_seconds: 4,
            coherence_max_slots: 16,
            lump_vest_seconds: 86400,
            default_grace_seconds: 86400,
        }
    }
    pub(crate) fn history() -> QuoteHistoryPolicy {
        QuoteHistoryPolicy {
            max_age_slots: 16,
            max_age_seconds: 4,
            max_window_slots: 8,
            max_window_seconds: 2,
            max_update_gap_slots: 8,
            max_update_gap_seconds: 2,
            epoch_seconds: 10,
            minimum_completed_epochs: 2,
        }
    }
    pub(crate) fn fixture() -> (Box<Market>, Pubkey, Pubkey, InitializeQuotePolicyArgs) {
        let mut market = Box::new(Market::zeroed());
        market.mint = Pubkey::new_from_array([5; 32]);
        market.token_program = TOKEN_PROGRAM_ID;
        market.decimals = 6;
        let config = Pubkey::new_from_array([3; 32]);
        let market_key = Pubkey::new_from_array([4; 32]);
        let mut commitments = [[0; 32]; 10];
        for (i, c) in commitments.iter_mut().take(4).enumerate() {
            *c = [u8::try_from(i + 1).unwrap(); 32];
        }
        let mut args = InitializeQuotePolicyArgs {
            genesis_hash: [7; 32],
            model_version: 1,
            model_epoch: 1,
            expected_policy_hash: [0; 32],
            history: history(),
            lot_count: 2,
            lots: [1000, 2000, 0, 0, 0, 0, 0, 0],
            small_lot_quantity: 100,
            capacity_quantity: 10000,
            producer_commitments: commitments,
            terms: terms(),
        };
        args.expected_policy_hash = args.financial_hash_for_domain(&QuotePolicyDomain::for_market(
            args.genesis_hash,
            config,
            market_key,
            &market,
        ));
        (market, config, market_key, args)
    }
    pub(crate) fn policy() -> Box<QuotePolicy> {
        let (market, config, key, args) = fixture();
        let bump = Pubkey::find_program_address(
            &[
                QuotePolicy::SEED,
                key.as_ref(),
                &args.model_epoch.to_le_bytes(),
            ],
            &crate::ID,
        )
        .1;
        QuotePolicy::from_args(config, key, &market, &args, bump).unwrap()
    }
    #[test]
    fn canonical_registry_deduplicates_only_required_small_and_capacity_quantities() {
        let p = policy();
        assert_eq!(p.feed_count, 4);
        assert_eq!(
            p.feeds[..4].iter().map(|f| f.quantity).collect::<Vec<_>>(),
            vec![100, 1000, 2000, 10000]
        );
        assert!(p.feeds[4..]
            .iter()
            .all(|f| *f == QuoteFeedBinding::default()));
        assert_eq!(
            p.admission_terms().unwrap().supported_position_lots,
            &[1000, 2000]
        );
        assert_eq!(p.terms.default_keeper_terms().bps, 50);
    }
    #[test]
    fn policy_hash_is_shared_across_quantities_and_producer_hashes_are_nonrecursive() {
        let p = policy();
        let expected = p.financial_hash();
        let mut changed = p.clone();
        changed.feeds[0].producer_commitment = [99; 32];
        assert_eq!(changed.financial_hash(), expected);
        changed.bump ^= 1;
        assert_eq!(changed.financial_hash(), expected);
        changed.terms.market_notional_max -= 1;
        assert_ne!(changed.financial_hash(), expected);
        let p = policy();
        let mut changed = p.clone();
        changed.lots[0] += 1;
        assert_ne!(changed.financial_hash(), expected);
        let mut changed = p.clone();
        changed.history.max_age_slots += 1;
        assert_ne!(changed.financial_hash(), expected);
        let mut changed = p.clone();
        changed.domain.genesis_hash[0] ^= 1;
        assert_ne!(changed.financial_hash(), expected);
    }
    #[test]
    fn all_noncanonical_grids_feed_bindings_and_wrong_hashes_are_refused() {
        let (market, config, key, args) = fixture();
        for variant in 0..9 {
            let mut bad = args;
            match variant {
                0 => bad.lot_count = 0,
                1 => bad.lot_count = 9,
                2 => bad.lots[1] = 1000,
                3 => bad.lots[7] = 1,
                4 => bad.capacity_quantity = 9999,
                5 => bad.small_lot_quantity = 1001,
                6 => bad.producer_commitments[9] = [9; 32],
                7 => bad.producer_commitments[0] = [0; 32],
                _ => bad.expected_policy_hash = [88; 32],
            };
            assert!(
                QuotePolicy::from_args(config, key, &market, &bad, 254).is_err(),
                "variant {variant}"
            );
        }
        let p = policy();
        for variant in 0..5 {
            let mut bad = p.clone();
            match variant {
                0 => bad.feeds[0].quantity += 1,
                1 => bad.feeds[0].snapshot = Pubkey::new_unique(),
                2 => bad.feeds[0].producer_commitment = [0; 32],
                3 => bad.feed_count += 1,
                _ => bad.feeds[9].quantity = 1,
            };
            assert!(bad.validate_binding(&config, &key, &market).is_err());
        }
    }
    #[test]
    fn economic_resource_and_owner_selected_default_bounds_are_checked() {
        let h = history();
        let mutations: &[fn(&mut QuoteFinancialTerms)] = &[
            |t| t.market_notional_max = 25_000_000_001,
            |t| t.min_notional = 52_999_999,
            |t| t.cr_open_bps = 39999,
            |t| t.cr_liq_bps = 17499,
            |t| t.default_grace_seconds = 86399,
            |t| t.max_duration_seconds = 259201,
            |t| t.completed_liquidation_epochs = 3,
            |t| t.penalty_keeper_bps += 1,
            |t| t.rate_slope2_bps = 1000001,
            |t| t.settle_keeper_bps = 101,
            |t| t.util_kink_bps = 9501,
            |t| t.coherence_max_seconds = 5,
            |t| t.cr_arm_delay_seconds = 0,
            |t| t.position_quantity_max = 10001,
        ];
        for mutate in mutations {
            let mut t = terms();
            mutate(&mut t);
            assert!(t.validate(&h).is_err());
        }
        assert!(terms().validate(&h).is_ok());
        let mut bad = h;
        bad.max_age_seconds = 61;
        assert!(bad.validate().is_err());
        let mut bad = h;
        bad.minimum_completed_epochs = 9;
        assert!(bad.validate().is_err());
    }
    #[test]
    fn canonical_hash_matches_independent_python_little_endian_vector() {
        assert_eq!(policy().financial_preimage().len(), 619);
        assert_eq!(
            policy().financial_hash(),
            [
                216, 69, 40, 233, 245, 177, 48, 199, 11, 46, 31, 131, 94, 128, 166, 58, 31, 53,
                204, 0, 166, 159, 222, 18, 65, 72, 22, 174, 58, 173, 7, 56
            ]
        );
    }
    #[test]
    fn initialization_args_fit_a_plain_packet_and_policy_storage_is_under_anchor_limit() {
        let (_, _, _, args) = fixture();
        let mut data = Vec::new();
        args.serialize(&mut data).unwrap();
        assert!(data.len() + 8 < 900, "args {}", data.len());
        const {
            assert!(QuotePolicy::SPACE < 10_240);
        }
        eprintln!(
            "QuotePolicy initargs {} account {} preimage {} hash {:?}",
            data.len(),
            QuotePolicy::SPACE,
            policy().financial_preimage().len(),
            policy().financial_hash()
        );
    }
}
