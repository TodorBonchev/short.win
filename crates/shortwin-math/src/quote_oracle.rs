//! Pure, versioned size-cost evidence for the proposed quote-oracle risk backend.
//!
//! This module does not authenticate an oracle, attest liquidity, choose financial terms,
//! authorize liquidation or select S1/S2/S3. Slot values and bands are untrusted mathematical
//! inputs until a separately reviewed verifier binds them to the market, units and policy.
//! A finite band is an accepted reference interval, not a proof of globally optimal routing.
//! Unknown, stale and attested-no-route observations never become the legacy `None = +infinity`.
//! No reserve synthesis, interpolation, fee-free impact or historical persistence is supplied.

use crate::{MathError, risk, settle};

/// Version of this mathematical representation; not an on-chain account/decoder ABI.
pub const COST_EVIDENCE_VERSION: u16 = 1;
/// Bounded prototype storage/work capacity, not an adopted admission or economic default.
pub const PROTOTYPE_MAX_GRID_POINTS: usize = 8;

/// Refusals specific to quote evidence. No stable program error codes are assigned here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvidenceError {
    InvalidBand,
    InvalidGrid,
    InvalidQuantity,
    QuantityMismatch,
    UncoveredQuantity,
    Unknown,
    AttestedNoRoute,
    FutureObservation,
    StaleObservation,
    Arithmetic(MathError),
}

impl From<MathError> for EvidenceError {
    fn from(value: MathError) -> Self {
        Self::Arithmetic(value)
    }
}

/// A finite, positive, ordered interval of total buy cost in integer quote units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CostBand {
    low: u64,
    high: u64,
}

impl CostBand {
    pub fn new(low: u64, high: u64) -> Result<Self, EvidenceError> {
        if low == 0 || low > high {
            return Err(EvidenceError::InvalidBand);
        }
        Ok(Self { low, high })
    }

    /// Checked conversion for wider decoded calculations; truncation is never permitted.
    pub fn from_u128(low: u128, high: u128) -> Result<Self, EvidenceError> {
        Self::new(
            u64::try_from(low).map_err(|_| MathError::Overflow)?,
            u64::try_from(high).map_err(|_| MathError::Overflow)?,
        )
    }

    pub const fn low(self) -> u64 {
        self.low
    }

    pub const fn high(self) -> u64 {
        self.high
    }
}

/// A no-route attestation is a separate claim, never numerical infinity or forced-action proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CostEvidence {
    Finite(CostBand),
    Unknown,
    AttestedNoRoute,
}

impl CostEvidence {
    fn finite(self) -> Result<CostBand, EvidenceError> {
        match self {
            Self::Finite(band) => Ok(band),
            Self::Unknown => Err(EvidenceError::Unknown),
            Self::AttestedNoRoute => Err(EvidenceError::AttestedNoRoute),
        }
    }
}

/// One exact raw-token quantity and its cost/status. Grid construction validates ordering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridPoint {
    quantity: u64,
    evidence: CostEvidence,
}

impl GridPoint {
    pub fn new(quantity: u64, evidence: CostEvidence) -> Result<Self, EvidenceError> {
        if quantity == 0 {
            return Err(EvidenceError::InvalidQuantity);
        }
        Ok(Self { quantity, evidence })
    }

    pub const fn quantity(self) -> u64 {
        self.quantity
    }

    pub const fn evidence(self) -> CostEvidence {
        self.evidence
    }
}

/// Caller-supplied current slot and maximum permitted age. There is no default age policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotWindow {
    pub now: u64,
    pub max_age: u64,
}

impl SlotWindow {
    fn check(self, observed_slot: u64) -> Result<(), EvidenceError> {
        let age = self
            .now
            .checked_sub(observed_slot)
            .ok_or(EvidenceError::FutureObservation)?;
        if age > self.max_age {
            return Err(EvidenceError::StaleObservation);
        }
        Ok(())
    }
}

/// Validated borrowed V1 grid. One slot is attributed to the whole vector by the caller;
/// authenticated same-epoch assembly and clock provenance remain verifier responsibilities.
///
/// Quantities increase strictly; costs need not be convex or monotone across quantities.
/// This deliberately makes no inference between independently observed nodes.
#[derive(Clone, Copy, Debug)]
pub struct CostGridV1<'a> {
    observed_slot: u64,
    points: &'a [GridPoint],
}

impl<'a> CostGridV1<'a> {
    pub fn new(observed_slot: u64, points: &'a [GridPoint]) -> Result<Self, EvidenceError> {
        if points.is_empty() || points.len() > PROTOTYPE_MAX_GRID_POINTS {
            return Err(EvidenceError::InvalidGrid);
        }
        let mut previous = 0;
        for point in points {
            if point.quantity <= previous {
                return Err(EvidenceError::InvalidGrid);
            }
            previous = point.quantity;
        }
        Ok(Self {
            observed_slot,
            points,
        })
    }

    /// Exact quantities only. In-between quantities, stale/future data and nonfinite status fail.
    pub fn exact(self, quantity: u64, window: SlotWindow) -> Result<ExactCostV1, EvidenceError> {
        if quantity == 0 {
            return Err(EvidenceError::InvalidQuantity);
        }
        window.check(self.observed_slot)?;
        let point = self
            .points
            .iter()
            .find(|point| point.quantity == quantity)
            .ok_or(EvidenceError::UncoveredQuantity)?;
        Ok(ExactCostV1 {
            quantity,
            observed_slot: self.observed_slot,
            band: point.evidence.finite()?,
        })
    }

    /// Select the first quantity >= existing debt + proposed debt. Addition is checked.
    ///
    /// Only that node's upper reference mark is exposed; its low edge is NOT an exact-Q
    /// forced-action input. This is a ceiling-node capacity lookup, not interpolation or
    /// a certified executable bound for smaller Q. Its economic interpretation still needs
    /// the separately specified oracle/capacity policy and actual route validation.
    pub fn aggregate_upper(
        self,
        existing_debt: u64,
        proposed_debt: u64,
        window: SlotWindow,
    ) -> Result<AggregateUpperV1, EvidenceError> {
        if proposed_debt == 0 {
            return Err(EvidenceError::InvalidQuantity);
        }
        let requested_quantity = existing_debt
            .checked_add(proposed_debt)
            .ok_or(MathError::Overflow)?;
        window.check(self.observed_slot)?;
        let point = self
            .points
            .iter()
            .find(|point| point.quantity >= requested_quantity)
            .ok_or(EvidenceError::UncoveredQuantity)?;
        Ok(AggregateUpperV1 {
            requested_quantity,
            covered_quantity: point.quantity,
            observed_slot: self.observed_slot,
            upper_cost: point.evidence.finite()?.high,
        })
    }
}

/// Finite exact-Q evidence. Freshness is checked at lookup AND again by each risk helper.
/// This type proves neither oracle authenticity nor temporal persistence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExactCostV1 {
    quantity: u64,
    observed_slot: u64,
    band: CostBand,
}

impl ExactCostV1 {
    pub const fn quantity(self) -> u64 {
        self.quantity
    }

    pub fn band_at(self, window: SlotWindow) -> Result<CostBand, EvidenceError> {
        window.check(self.observed_slot)?;
        Ok(self.band)
    }

    /// Numeric liquidation leg only: even the cheap edge must be below CR_liq.
    /// No missing data, route status or expensive-only mark can satisfy this predicate.
    pub fn cr_below_all(
        self,
        v_eff: u64,
        cr_liq_bps: u32,
        window: SlotWindow,
    ) -> Result<bool, EvidenceError> {
        Ok(risk::cr_below(
            v_eff,
            Some(self.band_at(window)?.low),
            cr_liq_bps,
        )?)
    }

    /// Numeric S1 budget leg only; persistence and settlement authorization are absent.
    pub fn budget_exceeded_all(
        self,
        budget: risk::Budget,
        window: SlotWindow,
    ) -> Result<bool, EvidenceError> {
        Ok(budget.exceeded_by(Some(self.band_at(window)?.low))?)
    }
}

/// An aggregate capacity mark; deliberately not interchangeable with exact-Q risk evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AggregateUpperV1 {
    requested_quantity: u64,
    covered_quantity: u64,
    observed_slot: u64,
    upper_cost: u64,
}

impl AggregateUpperV1 {
    pub const fn requested_quantity(self) -> u64 {
        self.requested_quantity
    }

    pub const fn covered_quantity(self) -> u64 {
        self.covered_quantity
    }

    /// Recheck freshness at capacity use, including marks cached after a valid lookup.
    pub fn upper_at(self, window: SlotWindow) -> Result<u64, EvidenceError> {
        window.check(self.observed_slot)?;
        Ok(self.upper_cost)
    }
}

/// D4 algebra on two exact-Q upper reference marks, with no optional/undefined EMA fallback.
/// These are oracle pre-open snapshot marks, NOT the old post-sale live reserve quote.
/// S2 fee-free impact, safe admission, fee views and financial policy are not supplied here.
pub fn snapshot_margin_d4(
    live: ExactCostV1,
    history: ExactCostV1,
    sale_proceeds: u64,
    cr_open_bps: u32,
    window: SlotWindow,
) -> Result<u64, EvidenceError> {
    if live.quantity != history.quantity {
        return Err(EvidenceError::QuantityMismatch);
    }
    Ok(risk::margin_d4(
        sale_proceeds,
        live.band_at(window)?.high,
        Some(history.band_at(window)?.high),
        cr_open_bps,
    )?)
}

/// Explicit scalar inputs for exploring S2/S3 amount consistency, with no adopted defaults.
/// `take` must be established separately from V-K_s. This supplies no settlement trigger,
/// price-authentication proof, chosen payout mark or no-route recovery policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AmountInputs {
    pub take: u64,
    pub expired: bool,
    pub v_eff: u64,
    pub r_due: u64,
    pub cr_liq_bps: u32,
    pub penalty_bps: u32,
    pub expiry_fee_bps: u32,
}

impl AmountInputs {
    fn amount_at(self, buy_cost: u64) -> Result<u64, EvidenceError> {
        let rate = settle::pprime(
            self.expired,
            self.v_eff,
            buy_cost,
            self.cr_liq_bps,
            self.penalty_bps,
            self.expiry_fee_bps,
        );
        let fair_value =
            risk::fair_value(Some(buy_cost), rate, self.r_due).ok_or(MathError::Invariant)?;
        Ok(self.take.min(fair_value))
    }
}

/// Exact extrema of the proposed scalar payout expression over an integer cost interval.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AmountInterval {
    minimum: u64,
    maximum: u64,
}

impl AmountInterval {
    pub const fn minimum(self) -> u64 {
        self.minimum
    }

    pub const fn maximum(self) -> u64 {
        self.maximum
    }

    /// Exact integer stability inequality; true is NOT permission to cash-settle.
    pub fn consistent(self, fv_dev_bps: u32) -> bool {
        settle::amount_ok(self.minimum, self.maximum, fv_dev_bps)
    }
}

/// Compute extrema of min(take, B + ceil(pprime(B)*B/10000) + R_due).
///
/// Proof: on either constant-rate branch, multiplication by a nonnegative u32,
/// ceiling division, addition, saturation and min(take, _) are nondecreasing in B.
/// The only possible branch change is between t=floor(V_eff*10000/CR_liq) and t+1
/// for expired positions. Evaluate both interval endpoints and both adjacent branch
/// points that lie inside it. This captures even a downward expiry/penalty jump;
/// two endpoints alone do not generally suffice. CR_liq=0 or nonexpired has no branch.
/// u128 intermediates fit for all u64/u32 inputs; existing FV saturation is exact
/// for the final min(take, _) because take fits u64. No fake reserve surface is used.
pub fn amount_interval(
    cost: ExactCostV1,
    inputs: AmountInputs,
    window: SlotWindow,
) -> Result<AmountInterval, EvidenceError> {
    let band = cost.band_at(window)?;
    let a = inputs.amount_at(band.low)?;
    let b = inputs.amount_at(band.high)?;
    let mut result = AmountInterval {
        minimum: a.min(b),
        maximum: a.max(b),
    };
    if inputs.expired && inputs.cr_liq_bps != 0 {
        let numerator = u128::from(inputs.v_eff)
            .checked_mul(u128::from(crate::num::BPS))
            .ok_or(MathError::Overflow)?;
        let threshold = numerator
            .checked_div(u128::from(inputs.cr_liq_bps))
            .ok_or(MathError::DivByZero)?;
        if let Ok(threshold) = u64::try_from(threshold) {
            for candidate in [Some(threshold), threshold.checked_add(1)]
                .into_iter()
                .flatten()
            {
                if candidate >= band.low && candidate <= band.high {
                    let amount = inputs.amount_at(candidate)?;
                    result.minimum = result.minimum.min(amount);
                    result.maximum = result.maximum.max(amount);
                }
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::arithmetic_side_effects)]
mod tests {
    use super::*;

    const NOW: SlotWindow = SlotWindow {
        now: 100,
        max_age: 10,
    };

    fn finite(quantity: u64, low: u64, high: u64) -> GridPoint {
        GridPoint::new(
            quantity,
            CostEvidence::Finite(CostBand::new(low, high).unwrap()),
        )
        .unwrap()
    }

    fn exact(quantity: u64, low: u64, high: u64) -> ExactCostV1 {
        CostGridV1::new(100, &[finite(quantity, low, high)])
            .unwrap()
            .exact(quantity, NOW)
            .unwrap()
    }

    #[test]
    fn rejects_zero_reversed_wide_bands_and_invalid_grids() {
        assert_eq!(CostBand::new(0, 1), Err(EvidenceError::InvalidBand));
        assert_eq!(CostBand::new(2, 1), Err(EvidenceError::InvalidBand));
        assert_eq!(
            CostBand::from_u128(1, u128::from(u64::MAX) + 1),
            Err(EvidenceError::Arithmetic(MathError::Overflow))
        );
        assert_eq!(
            GridPoint::new(0, CostEvidence::Unknown),
            Err(EvidenceError::InvalidQuantity)
        );
        assert!(matches!(
            CostGridV1::new(100, &[]),
            Err(EvidenceError::InvalidGrid)
        ));
        for points in [
            [finite(2, 1, 1), finite(2, 1, 1)],
            [finite(2, 1, 1), finite(1, 1, 1)],
        ] {
            assert!(matches!(
                CostGridV1::new(100, &points),
                Err(EvidenceError::InvalidGrid)
            ));
        }
        let too_many: [GridPoint; PROTOTYPE_MAX_GRID_POINTS + 1] =
            core::array::from_fn(|i| finite(u64::try_from(i).unwrap() + 1, 1, 1));
        assert!(CostGridV1::new(100, &too_many[..PROTOTYPE_MAX_GRID_POINTS]).is_ok());
        assert!(matches!(
            CostGridV1::new(100, &too_many),
            Err(EvidenceError::InvalidGrid)
        ));
    }

    #[test]
    fn exact_positions_do_not_interpolate_and_aggregate_returns_only_ceiling_upper() {
        // Independent route marks need not define a convex or monotone curve.
        let points = [finite(10, 10, 90), finite(20, 8, 70), finite(40, 50, 100)];
        let grid = CostGridV1::new(100, &points).unwrap();
        assert_eq!(grid.exact(15, NOW), Err(EvidenceError::UncoveredQuantity));
        assert_eq!(grid.exact(0, NOW), Err(EvidenceError::InvalidQuantity));
        assert_eq!(grid.exact(20, NOW).unwrap().quantity(), 20);
        assert_eq!(
            grid.aggregate_upper(8, 7, NOW).unwrap(),
            AggregateUpperV1 {
                requested_quantity: 15,
                covered_quantity: 20,
                observed_slot: 100,
                upper_cost: 70
            }
        );
        assert_eq!(
            grid.aggregate_upper(30, 11, NOW),
            Err(EvidenceError::UncoveredQuantity)
        );
        assert_eq!(
            grid.aggregate_upper(u64::MAX, 1, NOW),
            Err(EvidenceError::Arithmetic(MathError::Overflow))
        );
        assert_eq!(
            grid.aggregate_upper(1, 0, NOW),
            Err(EvidenceError::InvalidQuantity)
        );
    }

    #[test]
    fn unknown_and_no_route_are_not_infinity_or_cash_liquidation_evidence() {
        for (status, error) in [
            (CostEvidence::Unknown, EvidenceError::Unknown),
            (
                CostEvidence::AttestedNoRoute,
                EvidenceError::AttestedNoRoute,
            ),
        ] {
            let points = [GridPoint::new(10, status).unwrap(), finite(20, 100, 200)];
            let grid = CostGridV1::new(100, &points).unwrap();
            assert_eq!(grid.exact(10, NOW), Err(error));
            // Do not silently skip an unavailable ceiling node for a later finite one.
            assert_eq!(grid.aggregate_upper(0, 1, NOW), Err(error));
            let liquidation = grid
                .exact(10, NOW)
                .and_then(|cost| cost.cr_below_all(0, 17_500, NOW));
            assert_eq!(liquidation, Err(error));
            let s1 = grid.exact(10, NOW).and_then(|cost| {
                cost.budget_exceeded_all(
                    risk::Budget {
                        v: 0,
                        k_s: 0,
                        ert: 0,
                    },
                    NOW,
                )
            });
            assert_eq!(s1, Err(error));
        }
    }

    #[test]
    fn stale_future_and_cached_evidence_fail_closed_at_actual_use() {
        let points = [finite(10, 10, 20)];
        let stale = CostGridV1::new(89, &points).unwrap();
        assert_eq!(stale.exact(10, NOW), Err(EvidenceError::StaleObservation));
        assert_eq!(
            stale.aggregate_upper(0, 10, NOW),
            Err(EvidenceError::StaleObservation)
        );
        let future = CostGridV1::new(101, &points).unwrap();
        assert_eq!(future.exact(10, NOW), Err(EvidenceError::FutureObservation));
        assert!(CostGridV1::new(90, &points).unwrap().exact(10, NOW).is_ok());
        let upper = CostGridV1::new(100, &points)
            .unwrap()
            .aggregate_upper(0, 10, NOW)
            .unwrap();
        assert_eq!(upper.requested_quantity(), 10);
        assert_eq!(upper.covered_quantity(), 10);
        assert_eq!(
            upper.upper_at(SlotWindow {
                now: 111,
                max_age: 10
            }),
            Err(EvidenceError::StaleObservation)
        );
        let cached = exact(10, 10, 20);
        let later = SlotWindow {
            now: 111,
            max_age: 10,
        };
        assert_eq!(
            cached.cr_below_all(0, 17_500, later),
            Err(EvidenceError::StaleObservation)
        );
        assert_eq!(
            cached.budget_exceeded_all(
                risk::Budget {
                    v: 0,
                    k_s: 0,
                    ert: 0
                },
                later
            ),
            Err(EvidenceError::StaleObservation)
        );
    }

    #[test]
    fn expensive_edge_alone_cannot_force_cheap_edge_legs() {
        let cost = exact(10, 10, 100);
        assert!(!cost.cr_below_all(30, 17_500, NOW).unwrap());
        assert!(risk::cr_below(30, Some(100), 17_500).unwrap());
        let budget = risk::Budget {
            v: 50,
            k_s: 0,
            ert: 0,
        };
        assert!(!cost.budget_exceeded_all(budget, NOW).unwrap());
        assert!(budget.exceeded_by(Some(100)).unwrap());
    }

    #[test]
    fn snapshot_d4_matches_scalar_algebra_without_changing_quote_semantics() {
        let live = exact(10, 20, 30);
        let history = exact(10, 25, 40);
        assert_eq!(
            snapshot_margin_d4(live, history, 7, 17_501, NOW).unwrap(),
            risk::margin_d4(7, 30, Some(40), 17_501).unwrap()
        );
        assert_eq!(
            snapshot_margin_d4(live, exact(20, 25, 40), 7, 17_501, NOW),
            Err(EvidenceError::QuantityMismatch)
        );
        assert_eq!(
            snapshot_margin_d4(exact(10, u64::MAX, u64::MAX), history, 0, u32::MAX, NOW),
            Err(EvidenceError::Arithmetic(MathError::Overflow))
        );
    }

    #[test]
    fn interval_checks_both_sides_of_a_downward_fee_branch_jump() {
        let inputs = AmountInputs {
            take: 100,
            expired: true,
            v_eff: 10,
            r_due: 0,
            cr_liq_bps: 10_000,
            penalty_bps: 0,
            expiry_fee_bps: 10_000,
        };
        let range = amount_interval(exact(1, 9, 12), inputs, NOW).unwrap();
        assert_eq!(
            range,
            AmountInterval {
                minimum: 11,
                maximum: 20
            }
        );
        // Endpoints 18 and 12 would falsely accept this 50% consistency limit.
        assert!(settle::amount_ok(12, 18, 5_000));
        assert!(!range.consistent(5_000));
    }

    #[test]
    fn normalized_vault_counterexample_cannot_pass_zero_deviation_on_endpoints() {
        // V=20, K_s=0, R_due=0 gives both take=20 and V_eff=20. The prototype
        // does not inherit an unexpressed penalty>=expiry-fee policy restriction.
        let inputs = AmountInputs {
            take: 20,
            expired: true,
            v_eff: 20,
            r_due: 0,
            cr_liq_bps: 20_000,
            penalty_bps: 0,
            expiry_fee_bps: 2_000,
        };
        assert_eq!(inputs.amount_at(9).unwrap(), 11);
        assert_eq!(inputs.amount_at(11).unwrap(), 11);
        assert!(settle::amount_ok(11, 11, 0));
        let actual = amount_interval(exact(1, 9, 11), inputs, NOW).unwrap();
        assert_eq!(
            actual,
            AmountInterval {
                minimum: 11,
                maximum: 12
            }
        );
        assert!(!actual.consistent(0));
    }

    #[test]
    fn interval_extrema_equal_exhaustive_integer_evaluation() {
        for expired in [false, true] {
            for v_eff in 0..=6 {
                for cr_liq_bps in [0, 1, 5_000, 10_000, 20_000] {
                    for penalty_bps in [0, 1, 10_000, 20_000] {
                        for expiry_fee_bps in [0, 1, 10_000, 30_000] {
                            for take in [0, 5, 100, u64::MAX] {
                                for r_due in [0, 3] {
                                    let inputs = AmountInputs {
                                        take,
                                        expired,
                                        v_eff,
                                        r_due,
                                        cr_liq_bps,
                                        penalty_bps,
                                        expiry_fee_bps,
                                    };
                                    for low in 1..=8 {
                                        for high in low..=8 {
                                            let got =
                                                amount_interval(exact(1, low, high), inputs, NOW)
                                                    .unwrap();
                                            let amounts =
                                                (low..=high).map(|b| inputs.amount_at(b).unwrap());
                                            assert_eq!(
                                                got.minimum(),
                                                amounts.clone().min().unwrap(),
                                                "{inputs:?} {low} {high}"
                                            );
                                            assert_eq!(
                                                got.maximum(),
                                                amounts.max().unwrap(),
                                                "{inputs:?} {low} {high}"
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn interval_handles_u64_edges_fv_saturation_and_threshold_outside_u64() {
        for (low, high) in [(1, 1), (1, u64::MAX), (u64::MAX - 1, u64::MAX)] {
            let cost = exact(u64::MAX, low, high);
            for cr_liq_bps in [0, 1, u32::MAX] {
                let inputs = AmountInputs {
                    take: u64::MAX,
                    expired: true,
                    v_eff: u64::MAX,
                    r_due: u64::MAX,
                    cr_liq_bps,
                    penalty_bps: u32::MAX,
                    expiry_fee_bps: 0,
                };
                assert_eq!(
                    amount_interval(cost, inputs, NOW).unwrap(),
                    AmountInterval {
                        minimum: u64::MAX,
                        maximum: u64::MAX
                    }
                );
            }
        }
        let cost = exact(1, 1, 2);
        assert_eq!(
            amount_interval(
                cost,
                AmountInputs {
                    take: 1,
                    expired: false,
                    v_eff: 0,
                    r_due: 0,
                    cr_liq_bps: 0,
                    penalty_bps: 0,
                    expiry_fee_bps: 0
                },
                SlotWindow {
                    now: 111,
                    max_age: 10
                }
            ),
            Err(EvidenceError::StaleObservation)
        );
    }
}
