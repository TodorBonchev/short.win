# Devnet active keeper: controlled Pump market

On 2026-10-23 the read-only `shortwin-devnet-keeper-observe.service` was disabled and stopped after the first controlled Pump market `GhLuvCJEJWpCK2rzQvCo4EB5BnvKEEhykCC4idkKtmB7` was created. The `shortwin-devnet-keeper-active.service` user unit was then enabled and started. Both units bind keeper metrics to loopback port `9464`, so only one may run at a time. The first curve was too shallow to admit a short under the unchanged minimum-notional and price-impact checks. A deeper market, `tmRs5cmSphuftivEwfKr3QMRVdGynZMHqWwki2qnD3H`, was subsequently created.

The active keeper uses the release binary copied to `~/.local/state/shortwin-devnet-1.0.3/bin/shortwin-keeper-active` (current SHA-256 `dbce270c7a86ee1b49c4fcbc06d9cf58473d37df93dff6b9fd4d50ded476b6fd`). It was rebuilt with `cargo build --release -p shortwin-keeper --locked` after the clock and explicit-market RPC reductions and replaced while the market was paused and empty; the previous binary (`b9142b9e034aaf5ffeeb62a18ecf03a6ed6d9783000ac003f79810c9b0afb106`) is retained beside it for local rollback. Its private local TOML config now allows **only the deeper market** `tmRs5cmSphuftivEwfKr3QMRVdGynZMHqWwki2qnD3H`, has `dry_run=false`, `tx_format="auto"`, and uses protocol lookup table `F9VQX1wvHjzthd3BaQQQ2d22WLJxogAbPRKMZEnCuNna`. The observation budget is 1 SOL per UTC day with zero value-at-risk floor; the priority ceiling is 100,000 microlamports/CU. Automatic coverage pause is enabled with the separate designated authority `52oLAneAy9CUhtAnSjfjxMX7fqYnbCuG4Nd9hKvmKQpS`. Its fee payer is `35DeTJZZKgHzbLfWe9yiM1EwFfVUTcPCFSizLd8crDbc`, funded with 2 devnet SOL. Both signing files remain outside Git with mode `0600`.

The first active ticks found the market and landed a `crank_sol_usd` transaction. After seven ticks, metrics showed one landed transaction, zero failed transactions, zero failed simulations, zero missed required observation seconds, zero coverage-pause failures, and market degraded/paused gauges both zero. This is a startup check, not evidence of sustained coverage under open positions. Raydium AMM v4, CPMM, and Jupiter ProgramData were unreadable on devnet; their warnings are expected for this Pump-only market. The selected Pump venue produced no ProgramData warning.

During zero-position operator setup, the shared public devnet RPC returned HTTP 429 to trade simulations. At 10:16 EEST a reversible systemd drop-in changed the keeper tick from 400 ms to 30 s. At 10:24 EEST the allowlist moved to the deeper market and the base 400-ms cadence was restored for a 60-second check. Metrics after 29 completed ticks showed four landed oracle cranks, four full market reads and 25 cold-market skips; failed transactions, failed simulations, missed required observations, and degraded/paused coverage were all zero. The keeper journal had no 429, but the operator's unsigned open preflight received HTTP 429 during this check. No short was signed or opened. A Pump curve without positions does not require automatic `crank_price`, so the zero price observations over this minute are expected and **do not prove per-second coverage after an open**.

At 10:30 EEST the operator ran a second bounded check with the 400-ms keeper and a fresh, landed manual price crank. The unsigned open simulation again received HTTP 429. No short was signed during either public-RPC attempt. The shared public endpoint had insufficient capacity for the keeper and operator preflight together.

Later the keeper moved to the authenticated, [bounded local Helius relay](devnet-helius-relay-2026-10-23.md). The private URL stays in a mode-`0600` file outside Git, and only the loopback relay address appears in the unit arguments. A fresh bounded open/close test then landed; see [the trading receipt](devnet-trading-bootstrap-2026-10-23.md). During the relay run, 856 calls and 20 sends were forwarded without a local or upstream 429. The indexer observed the live position and its close. The keeper had no reported transaction failures, required-observation misses or degraded market gauge during the short window, but its effective tick rate was roughly 0.45–0.5/s at the requested 400 ms cadence. This does not establish sustained per-second coverage under volatile market conditions.

After the test position closed, a market-scoped pause for new opens was confirmed on-chain. At 10:51 EEST the temporary 400-ms supervised-smoke override was removed and the keeper restarted with the `50-zero-position-throttle.conf` drop-in set to **300,000 ms**. The active service remains restricted to the deeper market, with zero open positions. The pause permits exits, deposits and price cranks; reopening for unattended trading requires throughput, RPC credit budget, and alert-route verification. The current Helius free plan and relay cap are for bounded testing, not a sustained 1-Hz trading service.

After the optimized release binary was installed at 10:57 EEST, one clean paused/empty 300-second tick forwarded **16 RPC calls**: two `getAccountInfo`, six `getMultipleAccounts`, one `getProgramAccounts`, and one each of `getLatestBlockhash`, `getRecentPrioritizationFees`, `getTransaction`, `sendTransaction`, `simulateTransaction`, plus two `getSignatureStatuses`. It made zero `getSlot` calls. Keeper metrics stayed at zero misses, degraded markets, failed simulations and failed transactions; the relay reported zero local or upstream 429s. This is 4,608 calls and 288 sends per day if each idle tick has the same shape, under the local 10,000-call daily relay cap. Under [Helius's detailed credit table](https://www.helius.dev/docs/billing/credits), the measured mix is 25 credits/tick, or about **216,000 credits per 30 days** before other clients or operations. The one-tick projection does not establish sustained active-position coverage.

At 17:21 EEST, an on-chain market read and the indexer both confirmed `paused_opens=true` and zero positions. The operator copied the unchanged active config to `/home/todor_dev/.local/state/shortwin-devnet-1.0.3/config/keeper-paused-idle.toml` (mode `0600`) and set only `[oracle] enabled=false` in that copy. The 300-second systemd drop-in now selects this idle config; the original drop-in is saved at `/home/todor_dev/.local/state/shortwin-devnet-1.0.3/backup/50-zero-position-throttle.pre-idle-oracle-2026-10-23.conf.bak` (mode `0600`). The restarted service was active and its first tick made zero oracle reads, zero sends and zero failed transactions. This reduces idle oracle traffic while the market remains paused and empty; it does **not** pass the unattended coverage or alert gates.

Restore the active config before preparing any unpause:

```sh
install -m 644 /home/todor_dev/.local/state/shortwin-devnet-1.0.3/backup/50-zero-position-throttle.pre-idle-oracle-2026-10-23.conf.bak \
  /home/todor_dev/.config/systemd/user/shortwin-devnet-keeper-active.service.d/50-zero-position-throttle.conf
systemctl --user daemon-reload
systemctl --user restart shortwin-devnet-keeper-active.service
```

Verify its `ExecStart` selects `keeper-active.toml` with the default `oracle.enabled=true`, then restore fast coverage and prove fresh oracle and price observations before clearing `paused_opens`.

Operator checks:

```sh
systemctl --user status shortwin-devnet-keeper-active.service --no-pager
curl -fsS http://127.0.0.1:9464/metrics
journalctl --user -u shortwin-devnet-keeper-active.service -n 100 --no-pager
```

Monitor `shortwin_keeper_market_degraded`, `shortwin_keeper_market_paused_coverage`, `shortwin_keeper_market_misses`, `shortwin_keeper_tx_failed_total`, and `shortwin_keeper_coverage_pause_failures_total` when testing with a position. A running systemd unit alone does not establish a working price-coverage service. No independent alert-delivery route has yet been verified.
