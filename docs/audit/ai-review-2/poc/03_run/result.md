# PoC 03_run

- Key: `shortwin|run|start-sample-pins-warmup-seed`
- Label: **CONFIRMED**
- Note: The PumpSwap price start records the pushed-down spot as a warm-up bucket. At the gate, P_ema is that spot, 11 % of the pool spot.
- Test: `poc_03_run.rs`. It is a LiteSVM integration test of the `shortwin-litesvm-tests` package. Put it in `tests/litesvm/tests/` of a short.win 1.0.1 checkout, build the programs as that package's README says, and run the command below from the checkout's root.
- Command: `cargo test -p shortwin-litesvm-tests --test poc_03_run -- --nocapture`

## Result of the PoC run

```text
running 1 test
POC-03_run: honest pool spot (×WAD)      = 48416447748031702
POC-03_run: pushed start spot (×WAD)     = 5401123726570019
POC-03_run: P_ema at the gate (×WAD)     = 5401123726570019  (8 warm-up cranks)
POC-03_run: gate P_ema / honest spot     = 11 %
POC-03_run: a short opened here is under-margined by ~8x

thread '...' panicked at tests/litesvm/tests/poc_03_run.rs:142:5:
HARM: the price start pinned P_ema at the attacker's pushed-down spot. P_ema at the gate = 5401123726570019 (×WAD) equals the pushed start 5401123726570019, not the pool's honest spot 48416447748031702; a short opened against this P_ema posts collateral priced ~8x too low, and the lenders carry the shortfall. The fix (do not record the start's own sample) lets the warm-up's honest buckets lift P_ema off the pushed start.
test the_pumpswap_price_start_does_not_pin_p_ema_at_a_pushed_down_spot ... FAILED

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.27s
```

## Faithfulness (the PoC agent's account)

Each step uses only instructions and accounts an attacker can really use; the pool is the dumped mainnet PumpSwap `mature` pool.

- Env::fork_pamm_coin(Mature) + env.list_pamm: a real create_market(Auto) on the coin's canonical PumpSwap pool -> a PumpSwap market with no price. Matches the finding's listing path (init set, price starts only at the first PumpSwap observation).
- env.pamm_market_listed(mint, lend=b0/20, warmup=0): a real lender deposit (so lenders have funds at risk) and the venue ATAs; warmup=0 means NO crank, so the price is still unstarted. This is the finding's precondition ("lender deposits already present ... nobody cranks between the gate and the open"). Verified by ring_count==0, total_shares>0.
- Attacker = PammUser (an ordinary funded trader with its own base ATA, WSOL, pAMM UVA). The ONE direct state write is fork::set_token_amount on the attacker's OWN base ATA, planting 3*X0 base tokens. This stands for the attacker having bought/held the token, which any user can do; the swaps that move the pool are real PumpSwap trades on the dumped ELF. No short.win account and no venue (pool) account is written directly.
- The attack is ONE atomic transaction [cu_limit, sell 2*X0 base, crank_price, buy 2*X0 base]: the sell pushes the pool spot ~9x down (real swap), the permissionless crank_price runs prologue::run which calls start_price (P_ema = pushed spot, ema_warm=8, one ring entry lo=pushed) and then observe::record (the same pushed spot into this second's bucket), and the buy restores the pool to its honest spot. This is exactly the finding's "pushes the start down ... crank_price -> start_price then record ... buys back", and bundling guarantees nobody else cranks between the sell and the start.
- The keeper then cranks once per second through the whole warm-up (clock advanced 1 s per crank, ordinary passage of time) with the pool at its restored honest spot, modelling "the keeper observes this market every second during the warm-up and stops after it".
- Final assertion reads the program's own stored market.ema_price at ema_warm==0 (the gate) and requires it to have risen above the pushed start (the SAFE behaviour the Fix restores). On 1.0.1 it equals the pushed start exactly (the warm_step running minimum keeps the start's own lo in range for all 8 integrations), so the assertion fails and prints the harm: P_ema is ~8x below the price the pool actually trades at, so a short opened against it is under-margined at the lenders' cost.

## Adversarial review

Verdict: kept. My re-run fails at the final assertion (poc_03_run.rs:142). The setup is a real one-transaction [sell 2·X0, crank_price, buy back] price start on the dumped mature pool, then eight honest warm-up cranks, and at ema_warm == 0 P_ema equals the pushed start exactly (11 % of the pool spot): prologue.rs:177-180 records the start sample, and warm_step's running minimum keeps that bucket's lo for all eight integrations. Every step is a real permissionless instruction, and the only direct write is the attacker's own base balance (3·X0, about 11 % of supply, a large but possible holding); open_short reads this P_ema in its PriceDeviatesFromEma gate and in its D4 margin, so the assertion measures the finding's defect. The test stops at the gate and opens no short, so the lender loss is inferred: on this listing path an open must wait for created_at + 50 s (MarketWarmingUp), the pin lasts that long only if nobody cranks after the gate (the keeper's per-second duty ends at ema_warm == 0 unless its burst rule fires), and with Tier C's 300 % cr_open the position covers about a third of its debt at the pool spot.

```text
POC-03_run: honest pool spot (×WAD)      = 48416447748031702
POC-03_run: pushed start spot (×WAD)     = 5401123726570019
POC-03_run: P_ema at the gate (×WAD)     = 5401123726570019  (8 warm-up cranks)
POC-03_run: gate P_ema / honest spot     = 11 %
thread 'the_pumpswap_price_start_does_not_pin_p_ema_at_a_pushed_down_spot' panicked at tests/litesvm/tests/poc_03_run.rs:142:5:
HARM: the price start pinned P_ema at the attacker's pushed-down spot. P_ema at the gate = 5401123726570019 (×WAD) equals the pushed start 5401123726570019, not the pool's honest spot 48416447748031702; ...
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.24s (exit=101)
```
