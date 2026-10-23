# Devnet web and local service publication — 2026-10-23

## Current state

The 1.0.3 program is deployed on devnet, and the devnet client is published at `https://short.win` and `https://www.short.win`. Its `GlobalConfig` is initialized, two Pump curve markets and lender deposits exist, and a bounded short open/close round trip has landed on the deeper market. The site shows a visible DEVNET badge and both markets. The first controlled coin is too shallow for a legal short under the deployed limits; new opens on the deeper market are currently **paused for supervised operation** while keeper coverage and RPC capacity are checked. See [the landed trading receipt](devnet-trading-bootstrap-2026-10-23.md).

On the operator workstation, PostgreSQL, the indexer `live` writer, read-only `serve` API, rollups, active keeper, and bounded local RPC relay run as enabled systemd **user** services with linger. The database, API, keeper metrics and relay listen only on loopback. The API is read-only and its CORS origins include `https://devnet.short.win`, `https://short.win` and `https://www.short.win`. The active keeper is limited to the deeper market, uses 300-second ticks while the market is paused and empty, and is not yet a proven unattended one-second coverage service. The public API depends on this workstation staying awake, online and connected to Cloudflare Tunnel.

Verify the local services without printing credentials:

```sh
systemctl --user show shortwin-devnet-db.service shortwin-devnet-indexer-live.service shortwin-devnet-indexer-api.service shortwin-devnet-indexer-rollups.service shortwin-devnet-keeper-active.service shortwin-devnet-helius-relay.service -p Id -p ActiveState -p SubState -p MainPID -p NRestarts --no-pager
curl -fsS http://127.0.0.1:8080/v1/health
curl -fsS http://127.0.0.1:8080/v1/status
curl -fsS http://127.0.0.1:9464/metrics
```

The private database URLs and passwords are in mode-`0600` files under `/home/todor_dev/.local/state/shortwin-devnet-1.0.3/config/`, outside Git. The Helius RPC credential is also outside Git and mode `0600`. The service units are under `~/.config/systemd/user/`. Do not copy private configuration into a deployment artifact.

## Publication topology

The owner selected `https://short.win` for the devnet client, replacing the former landing page; `https://www.short.win` serves the same app. `https://api-devnet.short.win` (including `wss://api-devnet.short.win/v1/ws`) routes through a supervised Cloudflare Tunnel to the local read-only API. PostgreSQL, the writer, rollups, observer metrics and the active keeper are not exposed. The public client does not embed `127.0.0.1` URLs.

The working client build requires these public values:

```sh
VITE_CLUSTER=devnet
VITE_RPC_URL=https://api.devnet.solana.com
VITE_RPC_WS_URL=wss://api.devnet.solana.com
VITE_INDEXER_URL=https://api-devnet.short.win
VITE_INDEXER_WS_URL=wss://api-devnet.short.win/v1/ws
VITE_PROTOCOL_ALT=F9VQX1wvHjzthd3BaQQQ2d22WLJxogAbPRKMZEnCuNna
VITE_ENABLE_TEST_VENUE=false
VITE_EXPLORER_TX_URL='https://explorer.solana.com/tx/{sig}?cluster=devnet'
```

The 2026-10-23 local `pnpm --filter web build` with these values passed. Its `dist/` directory includes the generated Cloudflare Pages `_headers`, `/.well-known/security.txt`, and no published source maps. The protocol ALT was added after the initial publish and is present in the current production bundle. The initial production deployment was main merge commit `591db1a988aa1da247ca96addb941f82bb27fe75`: `https://34e66540.shortwin-devnet.pages.dev` (project `shortwin-devnet`, default host `https://shortwin-devnet.pages.dev`).

Cloudflare Pages serves the static `apps/web/dist` bundle. The `shortwin-devnet` Direct Upload project and remote-managed `shortwin-devnet-indexer` Tunnel belong to the account that owns the zone. The Tunnel's ingress points only `api-devnet.short.win` to `http://127.0.0.1:8080` and otherwise returns 404. Its connector is running and reported healthy. The reader's direct tunnel peer is trusted as `127.0.0.1`; verify that public `X-Forwarded-For` produces distinct client rate-limit buckets during operations. The zone's sole Free-plan rate-limit rule blocks an IP for 10 seconds after more than 100 requests in 10 seconds to `/v1/health`, `/v1/status` or the `/v1/ws` handshake. Keep the origin allowlist exact. Recheck HTTPS, WSS, deep links, CORS and security headers after future deployments:

```sh
pnpm --filter web check:headers https://short.win
```

The local `cloudflared` binary is installed at `~/.local/bin/cloudflared` from Cloudflare release `2026.10.3`; its downloaded SHA-256 matched the release asset digest `053b80b0a23031a038fdbf3c073f59f82b8f1f152aa67971d7383ffafc0dbab0`. The enabled `shortwin-devnet-api-tunnel.service` user unit reads its token from a mode-`0600` file outside Git and passes no token on the command line. The Cloudflare API credential file was changed from mode `0644` to `0600`. The previous `shortwin-landing` Worker script, custom-domain configuration and zone DNS have private rollback copies under the state directory's `backup/` folder.

## Publication receipt and rollback

The owner supplied a local Cloudflare credential path; its account-scoped verification succeeded. The proxied `api-devnet.short.win` CNAME points to the Tunnel, and public HTTPS `/v1/health` and `/v1/status` return 200. The former `shortwin-landing` Worker custom domains and their `AAAA 100::` placeholder records were removed **one hostname at a time**, after the Pages production build was verified. The replacement proxied CNAME records point `short.win` (`8c596883e8a6dc9e2063be56865d3933`) and `www.short.win` (`bc44cb617d93618562cdec0f230ac4c6`) to `shortwin-devnet.pages.dev`. The Pages domain IDs are `7f531f9e-2315-41bf-8163-4c62d91cfd82` and `922c6ee9-5667-4b52-a5ff-f2807b6b1f81`, respectively. Cloudflare reports both domains `active`, with verification and HTTP validation `active`.

The production and both custom-domain pages returned 200 for `/` and `/markets`; `pnpm --filter web check:headers` passed for all three, including the security.txt and source-map checks. Headless Chromium loaded both custom domains with the `DEVNET` badge and `Indexer awaiting activity` state; `/v1/markets`, `/v1/status` and `/v1/stats` each returned 200, a WebSocket opened at `wss://api-devnet.short.win/v1/ws`, and no page errors occurred. The public API accepted exact CORS origins for apex and www. The local database, writer, API, rollups, observer and Tunnel units were active after cutover. The changed-code `web`, `web-e2e`, `services` and `indexer` jobs passed in [CI run 39870779850](https://github.com/TodorBonchev/short.win/actions/runs/39870779850); see that run for the remaining protocol-test result.

Private rollback copies of the old Worker script, custom-domain configuration and zone DNS remain under `/home/todor_dev/.local/state/shortwin-devnet-1.0.3/backup/`. To restore the landing page, remove the affected Pages custom domain and its new CNAME, recreate its proxied `AAAA 100::` record, and reattach that hostname to the saved `shortwin-landing` Worker in zone `7142026904839ad9395bcf68bcc7c65a`; verify HTTPS before changing the other hostname. The new manual [GitHub workflow](../../.github/workflows/deploy-devnet-web.yml) prepares repeatable deployments; its `devnet-web` environment exists, but future unattended runs need a separately scoped `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` stored as environment secrets.

## Lookup-table redeploy and market index check

After the controlled Pump curve market was confirmed, the client was rebuilt from branch source commit `e48ddd4989e90ad38b2432ee566d6dd75ad7d232` with `VITE_PROTOCOL_ALT=F9VQX1wvHjzthd3BaQQQ2d22WLJxogAbPRKMZEnCuNna` and the public devnet RPC/API settings above. The bundle checks confirmed the ALT and API URL, generated `_headers` and security.txt, and no published source maps. Cloudflare Pages production deployment `73eb2ebb-9d84-4bfa-822d-5d2545b64c4d` is at `https://73eb2ebb.shortwin-devnet.pages.dev`. This was a direct upload from the feature branch, identified by that source commit; refresh it from `main` after the branch merge. Header checks passed on the deployment URL, `https://short.win`, and `https://www.short.win`. Headless Chromium loaded the apex and `www` market page with the `DEVNET` badge, HTTP 200, and no page errors.

The indexer's default `/v1/markets` list intentionally hides markets with no open position, lender, or shares. Each newly created market was already available by `/v1/markets/{mint}` and an explicit mint search while absent from the default list. After its lender deposited, each appeared in the default list and browser; the list now contains both controlled markets. The indexer's Pump CreateV2 token-metadata path currently records decimals but no symbol/name for the first Token-2022 mint. A display-only browser fallback shows `SWTEST` / `ShortWin Devnet Test` for that exact mint on devnet when API metadata is missing; the mint remains visible, real API metadata takes precedence, and mainnet behavior is unchanged. The second mint has no such fallback and displays its shortened address. The local Postgres ingest and API role passwords were rotated after an accidental local config output, both private service configs updated, and live, rollups, and API user services restarted successfully. Local and public market reads and `/v1/health` passed after the rotation; no password is recorded here.

The updated display build from branch commit `13786fe02d603998930573597ba01be47ea02fc6` is Cloudflare Pages production deployment `97a8ef5b-92ce-4c5d-afd3-cec2347acb0b` (`https://97a8ef5b.shortwin-devnet.pages.dev`). The Pages metadata marks the working tree dirty because separate operator notes were untracked during upload; all frontend source changes in the bundle were committed in that source commit. Its bundle contained the verified ALT and `SWTEST` fallback, with no published source maps. Header checks passed for that deployment URL, the apex, and `www`. Headless Chromium loaded `/markets` and both `/m/{mint}` pages with HTTP 200, the `DEVNET` badge, both markets, `SWTEST` on the first mint, and no page errors. The public Solana devnet RPC intermittently returned HTTP 429 during these checks; the indexer-backed pages still rendered. A later production upload from merged `main` should replace this branch-attributed build.

Trading readiness is tracked separately in [the devnet trading bootstrap](devnet-trading-bootstrap-2026-10-23.md), [operator plan](devnet-operator-plan-2026-10-22.md), and [market feasibility assessment](devnet-market-feasibility-2026-10-22.md).

## Merged-main publication after the signed smoke

[PR #4](https://github.com/TodorBonchev/short.win/pull/4) merged into `main` as `491725201cfc12cefa4a1d09986d10bfa00245d1` after [CI run 39922560548](https://github.com/TodorBonchev/short.win/actions/runs/39922560548) completed successfully on its exact head `636807d81e1b0d6361ef9ee5aee6aa4e3dc07ed3`. The devnet client was rebuilt from the clean merged-main worktree with the public configuration above. The 77-file upload contained the verified protocol ALT and public API URL, and no published source maps or private Cloudflare credential values.

Cloudflare Pages production deployment `f7e33af7-f6be-4c6f-8854-69189830d917` serves `https://f7e33af7.shortwin-devnet.pages.dev`; its metadata records the exact merge commit above. Both `https://short.win` and `https://www.short.win` passed the security-header, security.txt and source-map checks. Headless Chromium loaded the deeper market and `/status` on both hosts: the supervised-trading pause notice appeared, the indexer state was `live`, and there were no page errors. The public API `/v1/health` returned `ok`, and the deeper market API showed `paused_opens=true` with zero open positions. The prior production deployment `97a8ef5b-92ce-4c5d-afd3-cec2347acb0b` remains available for rollback.

This publication provides the working devnet information, lending market and a proven bounded short round trip. New shorts remain paused for unattended use because sustained price-coverage throughput and alert delivery have not been established on the current free RPC plan; see [the trading receipt](devnet-trading-bootstrap-2026-10-23.md).
