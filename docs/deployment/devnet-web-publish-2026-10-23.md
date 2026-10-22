# Devnet web and local service publication — 2026-10-23

## Current state

The 1.0.3 program is deployed on devnet, but its `GlobalConfig`, `VenueGuard` and `OracleCache` accounts have not been initialized and no market exists. A public website can show the devnet client and protocol status now; lending and shorting cannot work until a separately reviewed initialization and market smoke succeeds. The devnet build retains its cluster badge and now shows a prominent notice while `GlobalConfig` is absent, with market-creation prompts hidden.

On the operator workstation, PostgreSQL, the indexer `live` writer, read-only `serve` API, rollups, and the keeper's new read-only `--observe` mode run as enabled systemd **user** services with linger. The database, API, and observer metrics listen only on `127.0.0.1:5432`, `127.0.0.1:8080`, and `127.0.0.1:9464`. The API is read-only and its CORS origins include `https://devnet.short.win` and `https://short.win`. The keeper's existing `dry_run` mode signs a transaction before suppressing the send; the new observer does not load any keypair or enter the signing path. Observation is not active price coverage or liquidation service.

Verify the local services without printing credentials:

```sh
systemctl --user show shortwin-devnet-db.service shortwin-devnet-indexer-live.service shortwin-devnet-indexer-api.service shortwin-devnet-indexer-rollups.service shortwin-devnet-keeper-observe.service -p Id -p ActiveState -p SubState -p MainPID -p NRestarts --no-pager
curl -fsS http://127.0.0.1:8080/v1/health
curl -fsS http://127.0.0.1:8080/v1/status
curl -fsS http://127.0.0.1:9464/metrics
```

The private database URLs and passwords are in mode-`0600` files under `/home/todor_dev/.local/state/shortwin-devnet-1.0.3/config/`, outside Git. The service units are under `~/.config/systemd/user/`. Do not copy either into a deployment artifact.

## Publication topology

The owner selected `https://short.win` for the devnet client, replacing the current landing page. Keep `https://www.short.win` consistent with the apex. Route `https://api-devnet.short.win` (including `wss://api-devnet.short.win/v1/ws`) through a supervised Cloudflare Tunnel to the local read-only API. Do not expose PostgreSQL, the writer, rollups, observer metrics, or the active keeper. The public client must never embed `127.0.0.1` URLs.

The working client build requires these public values:

```sh
VITE_CLUSTER=devnet
VITE_RPC_URL=https://api.devnet.solana.com
VITE_RPC_WS_URL=wss://api.devnet.solana.com
VITE_INDEXER_URL=https://api-devnet.short.win
VITE_INDEXER_WS_URL=wss://api-devnet.short.win/v1/ws
VITE_PROTOCOL_ALT=
VITE_ENABLE_TEST_VENUE=false
VITE_EXPLORER_TX_URL='https://explorer.solana.com/tx/{sig}?cluster=devnet'
```

The 2026-10-23 local `pnpm --filter web build` with these values passed. Its `dist/` directory includes the generated Cloudflare Pages `_headers`, `/.well-known/security.txt`, and no published source maps. The website remains a prepared artifact until the Pages deployment is reachable from a browser.

Cloudflare Pages can serve the static `apps/web/dist` bundle. The `shortwin-devnet` Direct Upload project and remote-managed `shortwin-devnet-indexer` Tunnel have been created in the account that owns the zone. The Tunnel's ingress points only `api-devnet.short.win` to `http://127.0.0.1:8080` and otherwise returns 404. Its connector is running and reported healthy. The reader's direct tunnel peer is trusted as `127.0.0.1`; verify that public `X-Forwarded-For` produces distinct client rate-limit buckets. The zone's sole Free-plan rate-limit rule blocks an IP for 10 seconds after more than 100 requests in 10 seconds to `/v1/health`, `/v1/status` or the `/v1/ws` handshake. Keep the origin allowlist exact. Validate HTTPS, WSS, deep links, CORS and security headers after publication:

```sh
pnpm --filter web check:headers https://short.win
```

The local `cloudflared` binary is installed at `~/.local/bin/cloudflared` from Cloudflare release `2026.10.3`; its downloaded SHA-256 matched the release asset digest `053b80b0a23031a038fdbf3c073f59f82b8f1f152aa67971d7383ffafc0dbab0`. The enabled `shortwin-devnet-api-tunnel.service` user unit reads its token from a mode-`0600` file outside Git and passes no token on the command line. The Cloudflare API credential file was changed from mode `0644` to `0600`. The previous `shortwin-landing` Worker script, custom-domain configuration and zone DNS have private rollback copies under the state directory's `backup/` folder.

## Inputs before the public route

The owner supplied a local Cloudflare credential path; its account-scoped verification succeeded. The proxied `api-devnet.short.win` CNAME now points to the Tunnel, and public HTTPS `/v1/health` and `/v1/status` returned 200 with the expected `https://short.win` CORS origin. The current landing page is a Worker custom domain on both `short.win` and `www.short.win`, backed by proxied `AAAA 100::` records. Deploy and inspect the Pages build at its `pages.dev` URL before detaching those Worker domains. Preserve the private rollback copies. Then attach apex and www to Pages, check DNS/TLS activation and run the deployed-header check. The new manual [GitHub workflow](../../.github/workflows/deploy-devnet-web.yml) prepares repeatable deployments; its `devnet-web` environment exists, but future unattended runs need a separately scoped `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` stored as environment secrets.

A market-capable website additionally needs approved, simulated devnet transactions to initialize the program and later create one supported market, plus a funded active keeper and an operator alert route. The local indexer and a public status page do not close those separate readiness gates; see [the devnet operator plan](devnet-operator-plan-2026-10-22.md) and [market feasibility assessment](devnet-market-feasibility-2026-10-22.md).
