"""Focused current-model regressions; historical comparison drivers opt out explicitly."""
import unittest

import pool_model as pm
import shortwin_sim as sim


class LockedDepthTests(unittest.TestCase):
    def pools(self):
        return (sim.CPPool(10**12, 10**12, sim.PUMPSWAP_CANONICAL_FEES),
                sim.RaydiumPool(10**12, 10**12))

    def test_liquidity_deposit_cannot_cross_a_bps_rounding_cliff(self):
        for pool in self.pools():
            with self.subTest(pool=pool.kind):
                pool.lp_units, pool.lp_locked = 10_000, 1
                self.assertEqual(pool.lock_bps(), 1)
                before = pool.locked_depth()
                pool.add_liquidity(10_000)
                self.assertEqual(pool.lock_bps(), 0)
                self.assertEqual(pool.locked_depth(), before)
                self.assertGreater(before, 0)

    def test_zero_counter_and_invalid_supply_fail_closed(self):
        for pool in self.pools():
            for counter, locked in ((0, 0), (10, -1), (10, 0)):
                with self.subTest(pool=pool.kind, counter=counter, locked=locked):
                    pool.lp_units, pool.lp_locked = counter, locked
                    self.assertEqual(pool.locked_depth(), 0)

    def test_historical_switch_preserves_the_old_double_floor(self):
        old = sim.RAW_LOCKED_DEPTH
        try:
            sim.RAW_LOCKED_DEPTH = False
            for pool in self.pools():
                pool.lp_units, pool.lp_locked = 20_000, 1
                self.assertEqual(pool.locked_depth(), 0)
        finally:
            sim.RAW_LOCKED_DEPTH = old


class PoolTests(unittest.TestCase):
    def converted_lender(self, lock, *, first=False, audit2=True):
        pool = pm.Pool(audit2=audit2)
        pool.deposit("locked", 1)
        lender = pool.lenders["locked"]
        other = pool._touch("other")
        lender.shares, other.shares = 600, 400
        lender.lock, lender.lock_pi = lock, pm.WAD
        lender.lock_epoch, lender.lock_until = pool.pi_epoch, pm.LOCK_BUCKET_S
        pool._roll(1, 1000)
        if not first:
            pool._touch("other")  # contributes the remainder that mints this lender's share
        return pool._touch("locked")

    def test_nonzero_rebase_lock_rounds_up_with_the_converted_share(self):
        lender = self.converted_lender(600)
        self.assertEqual((lender.shares, lender.lock), (1, 1))
        old = self.converted_lender(600, audit2=False)
        self.assertEqual((old.shares, old.lock), (1, 0))

    def test_rebase_does_not_create_a_lock_or_lock_nonexistent_shares(self):
        unlocked = self.converted_lender(0)
        self.assertEqual((unlocked.shares, unlocked.lock), (1, 0))
        floored = self.converted_lender(600, first=True)
        self.assertEqual((floored.shares, floored.lock), (0, 0))

    def test_dust_flush_gates_each_source_and_credits_the_old_shares(self):
        pool = pm.Pool(util_cap_bps=10_000, audit2=True)
        pool.deposit("old", 1_000_000)
        pool.borrow("short", 999_900)
        pool.settle("short", 1_000, lump=1_000)
        self.assertGreater(pool.S, 0)
        self.assertEqual((pool.T, pool.vest_bal, pool.wipe_swept), (100, 0, 0))
        for source, amount in ((pm.WIPE_TRADER_SWEPT, 101), (pm.WIPE_REWARDS_CLAIMED, 103)):
            with self.assertRaisesRegex(ValueError, "WipeSweepPending"):
                pool.deposit("new", 1_000_000)
            pool.sweep_sol(amount, source)
            self.assertEqual(pool.vest_bal, 0)
            self.assertEqual(pool.wipe_swept & source, source)
        self.assertEqual(pool.claim_sol("old"), 1_204)
        pool.deposit("new", 1_000_000)
        self.assertEqual(pool.claim_sol("new"), 0)
        pool.sweep_sol(107, pm.WIPE_TRADER_SWEPT)
        self.assertEqual(pool.vest_bal, 107)  # ordinary later income still vests
        pool.check()

    def test_even_a_small_settlement_holds_until_the_actual_extended_vest_ends(self):
        pool = pm.Pool(lump_vest_s=100, audit2=True)
        pool.deposit("old", 1_000_000)
        pool.borrow("short", 1)
        pool.settle("short", 1_000, lump=1_000)
        marker = pool.deposit_hold_until
        self.assertEqual(marker, 100)
        with self.assertRaisesRegex(ValueError, "PoolDrained"):
            pool.deposit("new", 1_000_000)
        pool.advance(50)
        pool.sweep_sol(1_000)
        self.assertGreater(pool.vest_end, marker)
        self.assertEqual(pool.deposit_hold_until, marker)
        pool.advance(marker - pool.now)
        with self.assertRaisesRegex(ValueError, "PoolDrained"):
            pool.deposit("new", 1_000_000)
        pool.advance(pool.vest_end - pool.now)
        pool.deposit("new", 1_000_000)
        self.assertEqual(pool.deposit_hold_until, -1)
        pool.check()

    def test_legacy_live_vest_waits_for_release_before_initialization(self):
        pool = pm.Pool(lump_vest_s=100, audit2=True)
        pool.deposit("old", 100)
        pool.sweep_sol(100)
        pool.deposit_hold_until = 0
        with self.assertRaisesRegex(ValueError, "PoolDrained"):
            pool.deposit("new", 100)
        pool.advance(100)
        with self.assertRaisesRegex(ValueError, "WipeSweepPending"):
            pool.deposit("new", 100)
        pool.sweep_sol(0)
        pool.deposit("new", 100)
        self.assertEqual(pool.deposit_hold_until, -1)

    def test_legacy_zero_vest_rechecks_old_sources_once(self):
        pool = pm.Pool(util_cap_bps=10_000, audit2=True)
        pool.deposit("old", 1_000_000)
        pool.borrow("short", 999_900)
        pool.settle("short", 1_000, lump=1_000)
        # A pre-upgrade dust flush retained the completed source bits and no vest.
        pool.deposit_hold_until, pool.wipe_swept = 0, pm.WIPE_BOTH
        with self.assertRaisesRegex(ValueError, "WipeSweepPending"):
            pool.deposit("new", 1_000_000)
        self.assertGreater(pool.deposit_hold_until, 0)
        self.assertEqual(pool.wipe_swept, 0)
        pool.sweep_sol(1_000, pm.WIPE_TRADER_SWEPT)
        pool.sweep_sol(0, pm.WIPE_REWARDS_CLAIMED)
        pool.advance(0)
        self.assertEqual(pool.wipe_swept, pm.WIPE_BOTH)  # migration is one-time
        pool.deposit("new", 1_000_000)
        self.assertEqual(pool.claim_sol("new"), 0)
        self.assertEqual(pool.claim_sol("old"), 2_000)

    def test_empty_trader_sweep_then_cashback_cannot_dilute_revested_income(self):
        pool = pm.Pool(util_cap_bps=10_000, lump_vest_s=100, audit2=True)
        pool.deposit("old", 1_000_000)
        pool.borrow("short", 999_900)
        pool.settle("short", 1_000, lump=1_000)
        pool.sweep_sol(0, pm.WIPE_TRADER_SWEPT)
        # Real cashback reaches the trader after its empty sweep. The pure math source
        # routing re-vests it; the program hold protects that vest until actual release.
        pool.sweep_sol(1_000, pm.WIPE_TRADER_SWEPT)
        pool.sweep_sol(0, pm.WIPE_REWARDS_CLAIMED)
        self.assertEqual((pool.vest_bal, pool.wipe_swept), (1_000, pm.WIPE_BOTH))
        with self.assertRaisesRegex(ValueError, "PoolDrained"):
            pool.deposit("new", 1_000_000)
        pool.advance(50)
        pool.sweep_sol(1_000, pm.WIPE_TRADER_SWEPT)
        pool.advance(50)
        with self.assertRaisesRegex(ValueError, "PoolDrained"):
            pool.deposit("new", 1_000_000)
        pool.advance(pool.vest_end - pool.now)
        pool.deposit("new", 1_000_000)
        self.assertEqual(pool.claim_sol("new"), 0)
        self.assertEqual(pool.claim_sol("old"), 3_000)

    def test_pure_math_source_bookkeeping_has_no_program_migration_or_hold(self):
        pool = pm.Pool(util_cap_bps=10_000, audit2=True, settlement_deposit_gate=False)
        pool.deposit("old", 1_000_000)
        pool.borrow("short", 999_900)
        pool.settle("short", 1_000, lump=1_000)
        pool.sweep_sol(0, pm.WIPE_TRADER_SWEPT)
        pool.sweep_sol(1_000, pm.WIPE_TRADER_SWEPT)
        pool.sweep_sol(0, pm.WIPE_REWARDS_CLAIMED)
        pool.deposit_hold_until = 0  # absent from the math ABI; must not alter math state
        pool.deposit("new", 1_000_000)
        self.assertEqual(pool.vest_bal, 1_000)
        self.assertEqual(pool.wipe_swept, pm.WIPE_BOTH)

    def test_no_reward_venue_does_not_hold_ordinary_income_for_an_unused_bit(self):
        pool = pm.Pool(util_cap_bps=10_000, audit2=True, rewards_required=False)
        pool.deposit("old", 1_000_000)
        pool.borrow("short", 999_900)
        pool.settle("short", 1_000, lump=1_000)
        pool.sweep_sol(0, pm.WIPE_TRADER_SWEPT)
        self.assertEqual(pool.wipe_swept, pm.WIPE_TRADER_SWEPT)
        pool.sweep_sol(1_000, pm.WIPE_TRADER_SWEPT)
        self.assertEqual(pool.deposit_hold_until, -1)
        self.assertEqual(pool.vest_bal, 1_000)
        pool.deposit("new", 1_000_000)

    def test_initialized_market_can_accept_deposits_during_ordinary_sweep_vesting(self):
        pool = pm.Pool(audit2=True)
        pool.deposit("old", 100)
        pool.sweep_sol(100)
        pool.deposit("new", 100)
        self.assertEqual(pool.deposit_hold_until, -1)
        self.assertEqual(pool.vest_bal, 100)

    def test_calendar_expiry_is_not_reversed_by_clock_regression(self):
        pool = pm.Pool(audit2=True)
        pool.deposit("old", 1)
        lender = pool.lenders["old"]
        lender.lock, lender.lock_pi = 500, pm.WAD
        lender.lock_epoch, lender.lock_until = pool.pi_epoch, pm.LOCK_BUCKET_S
        pool._cal_add(lender.lock_until, 500)
        pool.now = pm.LOCK_BUCKET_S
        pool._cal_advance()  # expire aggregate without touching the lender
        pool.now -= 1
        self.assertEqual(pool._lock_of(lender), 0)
        self.assertEqual(pool.locked_total(), 0)

    def test_tiny_deposit_cannot_unlock_the_value_of_preexisting_locked_shares(self):
        pool = pm.Pool(audit2=True)
        pool.deposit("locked", 1_000_000)
        pool.deposit("other", 1_000_000)
        lender = pool.lenders["locked"]
        lender.lock, lender.lock_pi = lender.shares, pm.WAD
        lender.lock_epoch, lender.lock_until = pool.pi_epoch, pm.LOCK_BUCKET_S
        pool._cal_add(lender.lock_until, lender.lock)
        pool.borrow("short", 1_000_000)
        self.assertEqual(pool.free_allowance("locked"), 0)
        minted = pool.deposit("locked", 1)
        value_of_new_shares = minted * (pool.T + pool.B + pm.VA) // (pool.S + pm.VS)
        self.assertLessEqual(pool.free_allowance("locked"), value_of_new_shares)
        self.assertEqual(pool.free_allowance("locked"), 1)

    def test_allowance_view_does_not_commit_calendar_expiry(self):
        pool = pm.Pool(audit2=True)
        pool.deposit("old", 1)
        lender = pool.lenders["old"]
        lender.lock, lender.lock_pi = 500, pm.WAD
        lender.lock_epoch, lender.lock_until = pool.pi_epoch, pm.LOCK_BUCKET_S
        pool._cal_add(lender.lock_until, 500)
        before = pool.cal_day, pool.lock_norm, dict(pool.lock_cal)
        pool.now = pm.LOCK_BUCKET_S
        pool.allowance_view("old")
        self.assertEqual((pool.cal_day, pool.lock_norm, pool.lock_cal), before)


if __name__ == "__main__":
    unittest.main()
