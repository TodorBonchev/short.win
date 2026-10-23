# Private devnet RPC relay for local operator smoke

On 2026-10-23 the operator supplied Helius **devnet** HTTP and WebSocket endpoints in `/home/todor_dev/.helius`. That file is outside Git, owned by the operator and mode `0600`. The API key and full URLs must not be copied to a unit, script argument, browser bundle, deployment document or log.

The local `shortwin-devnet-helius-relay.service` runs `/home/todor_dev/.local/state/shortwin-devnet-1.0.3/bin/helius-rpc-relay.py`. It reads the private file itself and checks that both endpoints use Helius's devnet host; startup calls `getGenesisHash` and refuses anything other than `EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG`. It listens **only** on `127.0.0.1:18899`. Its upstream URL appears neither in the systemd unit nor in process arguments or logs. It accepts single JSON-RPC `POST /` requests of at most 1 MiB, rejects batch requests, and forwards only to the fixed devnet endpoint. The current governor allows at most **8 upstream requests/second**, **1 `sendTransaction`/second**, and **10,000 upstream requests per UTC day**. The daily count resets at UTC midnight without a restart. This is a bounded smoke relay, not public RPC infrastructure. A process restart resets the in-memory daily count, so do not restart repeatedly to bypass the credit budget.

The [free Helius plan](https://www.helius.dev/pricing) presently lists 10 RPC requests/second, 1 `sendTransaction`/second and 1 million credits/month. The local limits leave throughput headroom; they do **not** prove that a keeper can meet ShortWin's per-second observation duty or that a month of continuous operation fits the credit allowance. The indexer and public browser RPC remain on the shared devnet endpoint to conserve Helius credits. Its WebSocket URL is validated but is not forwarded by this HTTP-only relay.

The keeper's base user unit and its reversible `50-zero-position-throttle.conf` both name only `http://127.0.0.1:18899/`. The active **paused, zero-position** override polls every **300 seconds**; the base unit's 400 ms cadence is for a supervised open-position smoke. A private, ready-to-install `keeper-smoke-helius-400ms.conf` sits in `/home/todor_dev/.local/state/shortwin-devnet-1.0.3/config/` and contains only the loopback URL. The keeper allowlist contains only the deeper devnet market `tmRs5cmSphuftivEwfKr3QMRVdGynZMHqWwki2qnD3H`. At relay cutover, that market had `paused_opens=true` and zero open positions. Do not clear the pause or open until the fast-cadence test has sufficient throughput, price coverage is monitored, and the exit command is prepared. After the smoke closes, remove the fast override to return to 300-second idle cadence, or stop the keeper to conserve credits. A market with no open positions does not require continuous price observations.

At cutover, read-only `getGenesisHash` and `getBalance` through the relay succeeded, while a local GET and JSON-RPC batch returned 405 and 400. A roughly one-minute **zero-position** 400-ms trial showed about 17 completed keeper ticks and 135 forwarded RPC requests (about 2.3 requests/second on average), with no upstream or local 429, no failed keeper transactions, and no required observation misses. It had zero price observations because there was no position; this does not establish the sustained one-observation-per-second requirement after an open. The 30-second idle cadence used roughly 19–21 RPC requests per tick, projecting above the relay's 10,000-call daily cap. The 300-second paused-market cadence projects roughly 5,500–6,100 requests/day at that observed idle cost, before any other use of this key. Projections are inferences from a short sample; monitor the actual counters and stop before the daily cap.

Read-only and local checks:

```sh
systemctl --user is-active shortwin-devnet-helius-relay.service shortwin-devnet-keeper-active.service
systemctl --user show -p ExecStart shortwin-devnet-keeper-active.service
journalctl --user -u shortwin-devnet-helius-relay.service -n 30 --no-pager
journalctl --user -u shortwin-devnet-keeper-active.service -n 30 --no-pager
curl -fsS -H 'Content-Type: application/json' \
  --data '{"jsonrpc":"2.0","id":1,"method":"shortwin_relayStats"}' \
  http://127.0.0.1:18899/
curl -fsS http://127.0.0.1:9464/metrics
```

The local `shortwin_relayStats` method returns only UTC date, forwarded and send counts, sanitized per-method counts, local/upstream 429 counts, and the daily cap; it does not consume an upstream request. It exposes no key, URL, account, parameter or request body. The relay logs only sanitized startup, daily request milestones, and upstream 429 counts. A 429 from the relay or an observation miss during the fast-cadence test is a gate failure: leave the market paused, close any test position promptly using its rehearsed exit, and return to low-rate setup. Do not use the presence of an active systemd unit as evidence of continuous coverage.

To activate the prepared fast keeper only for a supervised smoke after the pause decision and exit plan are ready:

```sh
install -m 644 /home/todor_dev/.local/state/shortwin-devnet-1.0.3/config/keeper-smoke-helius-400ms.conf \
  /home/todor_dev/.config/systemd/user/shortwin-devnet-keeper-active.service.d/60-supervised-smoke.conf
systemctl --user daemon-reload
systemctl --user restart shortwin-devnet-keeper-active.service
```

To return to the paused, empty-market cadence after the position is closed:

```sh
rm /home/todor_dev/.config/systemd/user/shortwin-devnet-keeper-active.service.d/60-supervised-smoke.conf
systemctl --user daemon-reload
systemctl --user restart shortwin-devnet-keeper-active.service
```
