#[cfg(test)]
mod tests {
    use shortwin_math::{MathError, settle::*, risk::{Budget, SettleKind}};
    struct Quotes { cost: u64, sigma: u64 }
    impl BoxQuotes for Quotes {
        fn cost(&mut self, _: u128, _: u128) -> Result<Option<u64>, MathError> { Ok(Some(self.cost)) }
        fn sigma(&mut self, _: u128, _: u128) -> Result<u64, MathError> { Ok(self.sigma) }
    }
    fn terms() -> SettleTerms { SettleTerms {
        cr_liq_bps: 12_500, sigma_max_bps: 2_000, penalty_bps: 500,
        expiry_fee_bps: 200, fv_dev_bps: 100, persist_n: 40, arm_gap_s: 10,
        s3: S3Terms::DEFAULT,
    }}
    fn arm(count: u64) -> ArmState { ArmState { count, since: 60, last: 100, armed: 1, _pad: [0; 7] } }
    fn input(s2: bool) -> SettleInputs { SettleInputs {
        now: 109, deadline: 1_000, ema_count: 108, restart_ema_count: 101,
        p_ema: 10, l_ema: 2, bx: SettleBox::point(10, 2),
        budget: Budget { v: 1_000, k_s: 20, ert: 0 }, r_due: 0,
        cr_liquidatable: true, ema_leg: true, executable: true, warmed: true,
        p_rs: Some(10), l_live: 2, b_live: if s2 { Some(900) } else { None },
        sigma_live: 3_000,
        s3: S3Chain { no_route_now: false, no_route_since: None, graduated_at: None,
            migrating: false, migration_seen_at: None },
    }}
    fn quotes(s2: bool) -> Quotes { Quotes { cost: if s2 { 900 } else { 5_000 }, sigma: 3_000 } }
    #[test]
    fn both_s1_and_s2_discard_older_and_same_count_arms_then_wait_forty_new_updates() {
        for s2 in [false, true] { for old_count in [60, 100] {
            let mut inp = input(s2); let mut state = arm(old_count);
            let first = evaluate(&mut state, &inp, &terms(), &mut quotes(s2)).unwrap();
            assert_eq!(first.arm, ArmChange::Armed); assert!(first.settlement.is_none());
            assert_eq!((state.count, state.since), (108, 109));
            for n in 1..=40 {
                inp.now += 1; inp.ema_count += 1;
                let out = evaluate(&mut state, &inp, &terms(), &mut quotes(s2)).unwrap();
                assert_eq!(out.settlement.is_some(), n == 40, "s2={s2}, old={old_count}, n={n}");
                if let Some(settled) = out.settlement { assert_eq!(settled.kind, if s2 { SettleKind::S2 } else { SettleKind::S1 }); }
            }
            // Negative control: without restart identity, exactly the same initial arm is kept.
            let mut old_input = input(s2); old_input.restart_ema_count = 0;
            let old = evaluate(&mut arm(old_count), &old_input, &terms(), &mut quotes(s2)).unwrap();
            assert_eq!(old.arm, ArmChange::Kept);
            assert_eq!(old.settlement.is_some(), old_count == 60);
        }}
    }
    #[test]
    fn nonliquidatable_and_no_fresh_spot_branches_do_not_revive_pre_restart_clock() {
        for step_two in [false, true] {
            let mut inp = input(false);
            if step_two { inp.cr_liquidatable = false; } else { inp.p_rs = None; }
            let mut state = arm(100);
            let out = evaluate(&mut state, &inp, &terms(), &mut quotes(false)).unwrap();
            assert_eq!(state.last, 100); assert_eq!(out.arm, ArmChange::None); assert!(out.settlement.is_none());
            inp.restart_ema_count = 0;
            let mut no_marker = arm(100);
            let control = evaluate(&mut no_marker, &inp, &terms(), &mut quotes(false)).unwrap();
            assert_eq!(control.arm, ArmChange::Kept); assert_eq!(no_marker.last, 109);
        }
    }
}