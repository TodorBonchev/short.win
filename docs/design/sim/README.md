# Design simulations

These standard-library Python models use integer arithmetic and fixed seeds. Run commands
from the repository root. The current models describe the uncommitted v1.0.2 candidate;
passing simulations do not constitute a release or an audit sign-off.

`shortwin_sim.py` models venues, quotes, observations and position economics. Its current
locked depth is `floor(locked_lp * isqrt(X * Y) / lp_counter)`, with effective PumpSwap quote
reserves including VQR. `lock_bps()` remains a display helper, not an input to that product.

`pool_model.py` models lender accounting. Current defaults round nonzero converted locks up
and cap them at converted shares, bound withdrawal value by unlocked shares, preserve
calendar expiry across a backwards clock step, and require pending sweeps after dust flushes.
Pending sweeps credit existing shares immediately; ordinary income still vests. Its
`deposit_hold_until` integration models the program's `-1` initialized / `0` legacy / positive
settlement-hold sentinel and waits for actual remaining vest, including extensions.
The first mutable accrual of a legacy marker clears both source bits and sets a positive
hold even when its vest is zero. A required venue-reward source still pending with surviving
shares also arms the hold: cashback claimed after an empty trader sweep can re-vest through
the completed Trader source, but newcomers must wait for that vest to release. Set
`rewards_required=False` for venues that need only the trader source.

```sh
python3 -m unittest discover -s docs/design/sim -p 'test_*.py' -v
python3 docs/design/sim/pool_model.py > docs/design/sim/out/pool_model.txt
python3 docs/design/sim/shortwin_sim.py --mc 400 > docs/design/sim/out/full.txt
python3 docs/design/sim/shortwin_sim.py --only ema > docs/design/sim/out/ema.txt
python3 docs/design/sim/config_check.py --jobs 1
python3 docs/design/arch/layout.py
```

The pool output starts with three current 60,000-operation invariant/liveness runs. Its
subsequent comparison tables explicitly use `AUDIT2_MODEL = False`: they reproduce the
dated ECONOMICS analysis. `--historical` also runs the initial fuzz on that older model.
Historical attack drivers likewise opt out explicitly: `attacks/common.py` retains
`RAW_LOCKED_DEPTH = False`; `pool_r3.py` and `pool_edge.py` retain the old pool arithmetic.
Their checked-in `out/attacks/` figures and the frozen `legacy/v0_3/` models are preserved.
An old table is not evidence that a current fix has been validated.

The focused `test_audit2_models.py` checks the new formulas and their boundaries. The full
position simulation is an economic model, not a byte-for-byte model of program accounts,
wallet refresh transactions, freshness gates, or settlement-arm migration. Those paths
are covered by program and frontend regressions.

Math-crate fixtures are a separate current-arithmetic artifact:

```sh
python3 crates/shortwin-math/tests/vectors/quotes/gen_sim.py
python3 crates/shortwin-math/tests/vectors/quotes/gen_caps.py
python3 crates/shortwin-math/tests/vectors/pool/gen_pool.py
python3 crates/shortwin-math/tests/vectors/settle/gen_settle.py
python3 crates/shortwin-math/tests/vectors/risk/gen_exits.py
cargo test -p shortwin-math
```

The pool recorder pins current math regardless of a historical driver's compatibility
switch. It disables `settlement_deposit_gate` (including legacy/source-hold preparation)
because the hold marker belongs to the
program's Market account, outside `shortwin_math::pool::deposit`; `pool::settle` reports
whether a hold is required. Consequently its fuzz counts differ from the whole-pool
simulation's. It encodes the trader sweep bit and uses `rewards_required = false`, matching
its Rust harness; the focused Python tests exercise both required sweep sources.
