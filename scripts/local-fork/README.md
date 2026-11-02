# Local Jupiter lending demo

Open <http://127.0.0.1:5173>. Choose **Local demo wallet**, connect it, and load the prefilled token pool. The wallet uses a public deterministic test identity; it needs no extension, recovery phrase or keypair file.

The demo executes the lending program, the hash-pinned genuine Jupiter Router V2 program, and historical Raydium CPMM program/account snapshots on an isolated local validator. Quotes execute that Jupiter route in local simulations. Submitted opening and closing transactions execute the same programs locally. The SOL, holder allocations, lending liquidity and price history are synthetic demo inputs. The captured accounts come from several historical slots, as recorded in the fixture manifest; this is a historical snapshot demonstration, not a synchronized live mainnet fork or a live mainnet price feed.

All application endpoints use loopback:

| Component | Address |
| --- | --- |
| Web app | `http://127.0.0.1:5173` |
| Local quote/history bridge | `http://127.0.0.1:8787` |
| Validator RPC | `http://127.0.0.1:8899` |
| Validator WebSocket | `ws://127.0.0.1:8900` |

## Try a 100-token short

1. Choose **Local demo wallet** and select **Connect wallet**. The page should display **Local fork** and **Local test wallet · no real funds**.
2. Select **Load pool** with mint `Dz9mQ9NzkBcCsuGPFJ3r1bS4wgqKMHBPiVuniW8Mbonk`. Bootstrap supplies 1,000,000 tokens to this lending pool.
3. Open the short tab. Keep the prefilled **100** tokens and demo spending limits, then select **Prepare short**.
4. Review the prepared transaction and submit it through the page's signing action. Its blockhash and genesis are checked against the local validator before the demo wallet signs.
5. Open positions, select the new loan and choose **Prepare full close** to buy back the full token debt through Jupiter. Review and submit that local transaction. **Repay in tokens** provides the alternative local repayment flow.

The bridge publishes clearly synthetic completed price-history epochs and refreshes its reference values. It uses the validator's Clock, which can be ahead of host wallclock after the initial slot warp. The wallet's **Devnet / local fork** signing-chain label is wallet metadata; submission remains pinned to `127.0.0.1:8899` and the verified local genesis.

## Start or resume

Run from the repository root with the installed project dependencies, Node.js with TypeScript support, Python 3, a C compiler, systemd user services, and `solana-test-validator` 4.1.2 available. The current `target/deploy/shortwin_simple.so` must already be built.

```bash
bash scripts/local-fork/start-demo.sh
```

This starts/resumes the validator, bootstraps a fresh demo when needed, and launches the bridge and web app as scoped systemd user services. Existing initialized state receives a read-only bootstrap check. Verified foreground bridge/web processes are reused without restarting them or creating duplicate services. A process occupying either app port with different content or non-loopback configuration is left untouched and reported as a conflict.

To start/reuse only the bridge and web app after bootstrap:

```bash
bash scripts/local-fork/start-demo.sh --apps-only
```

The validator helper resumes an existing ledger by default. On first startup it materializes checked snapshot accounts, creates the local ledger, verifies the account bytes and executable identities through RPC, and writes `target/local-fork/runtime-verification.json` with that genesis. Bootstrap reads this receipt, verifies live executable identities again, and simulates each required setup transaction before sending it locally. Individual setup and verification commands are also available:

```bash
bash scripts/local-fork/start-validator.sh
node scripts/local-fork/bootstrap.mts
```

```bash
node scripts/local-fork/bootstrap.mts --verify-only
```

For foreground development, run the bridge and web app in separate terminals instead of starting duplicate services:

```bash
node scripts/local-fork/server.mts
```

```bash
VITE_ENABLE_LOCAL_DEMO=true \
VITE_CLUSTER=localnet \
VITE_RPC_URL=http://127.0.0.1:8899 \
VITE_SERVICE_URL=http://127.0.0.1:8787 \
VITE_DEMO_MINT=Dz9mQ9NzkBcCsuGPFJ3r1bS4wgqKMHBPiVuniW8Mbonk \
pnpm --dir web dev --port 5173 --strictPort
```

The validator runs as `shortwin-local-fork-validator.service`, with four Rayon threads and a 6 GiB memory limit. Its process-local `LD_PRELOAD` shim changes wildcard socket binds to loopback because this Agave CLI otherwise binds RPC and faucet to all interfaces. The helper adds the slot warp only when creating a ledger and never stores `--reset` in its service command. Snapshot inputs are read from repository fixtures; startup makes no mainnet RPC requests.

New bridge and web instances run as `shortwin-local-fork-bridge.service` and `shortwin-local-fork-web.service`, each with a 1 GiB memory limit. They bind explicitly to `127.0.0.1`; Vite uses a strict port so a conflict cannot silently move the demo to another address. Reused foreground instances retain their original terminal/session ownership.

## Stop and inspect

```bash
bash scripts/local-fork/stop.sh
systemctl --user status 'shortwin-local-fork-*.service'
```

`stop.sh` stops only `shortwin-local-fork-web.service`, `shortwin-local-fork-bridge.service` and `shortwin-local-fork-validator.service`. It preserves local balances, deposits and loans. Stop any bridge/web processes launched directly in terminals with Ctrl-C. Unrelated fuzz runners and services are untouched.

Runtime output stays under `target/local-fork/`: `fixtures.receipt.json` records the historical inputs and labelled synthetic allocations; `runtime-verification.json` pins the verified local genesis; `bootstrap.json` records application addresses and setup transaction signatures; `reference-publications.json` records synthetic-history publications; `service.log`, `server.log` and `web.log` contain process output when launched with the demo service configuration.

## Social videos

The editable video project is in [`tools/social-video`](../../tools/social-video/README.md). Keep the local demo running, then capture the actual lend, short and manage/close screens listed in its capture manifest. Phone captures use a consistent 390 × 844 viewport and are saved under `tools/social-video/public/captures/`. Preserve real balances, review screens and transaction results when taking these screenshots.

Install the standalone video dependencies and open its Remotion Studio:

```bash
npm --prefix tools/social-video install
npm --prefix tools/social-video run dev -- --port=3001
```

Use the Studio URL printed by that command. Once all required real captures are present:

```bash
npm --prefix tools/social-video run previews
npm --prefix tools/social-video run export
```

The exporter checks the capture manifest and blocks incomplete exports. It writes three vertical workflow MP4s (`01-lend.mp4`, `02-short.mp4`, `03-manage-close.mp4`), a landscape `04-overview.mp4`, and matching poster JPGs to `/home/todor_dev/Videos/shortwin-2026-11-02` by default. Set `SHORTWIN_VIDEO_OUTPUT` to choose another directory. Keep the persistent local historical fork, test-funds and synthetic-history disclosures in published cuts. Suggested social captions and full media details live in the video project's README.

## Explicitly start over

Resetting discards the active demo's local state and requires bootstrap again. Stop foreground bridge/web sessions with Ctrl-C, stop the demo services, then pass the explicit reset flag:

```bash
bash scripts/local-fork/stop.sh
bash scripts/local-fork/start-validator.sh --reset-local-state
bash scripts/local-fork/start-demo.sh
```

The reset helper archives the old ledger and session receipts under `target/local-fork/resets/`, creates and verifies a new genesis, and leaves the historical source fixtures unchanged. `start-demo.sh` then bootstraps and starts the apps against that new genesis. Their old bootstrap/genesis bindings cannot be reused across a reset.
