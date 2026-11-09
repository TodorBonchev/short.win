# Pooled wallet interface

This Vite + vanilla TypeScript interface imports only `@shortwin/simple-client` and Solana Kit. It is a local development client, not a qualified mainnet release.

From the repository root, install the frozen workspace lock and run `pnpm --filter @shortwin/simple-web dev`. The default RPC is `http://127.0.0.1:8899`. Initialize the program with a real matching validator genesis hash before preparing transactions. A Wallet Standard wallet with `solana:signTransaction` is required for signing. The optional **Local demo wallet** uses a public deterministic test identity and submits signed test transactions only to its verified loopback validator. It requires `VITE_ENABLE_LOCAL_DEMO=true`, a loopback page, and loopback RPC/service URLs. Follow the [local demo guide](../scripts/local-fork/README.md) to start this isolated environment with test funds and synthetic price history.

The interface reads the actual config, immutable policy, mint, current or selected historical pool generation, wallet receipt and wallet loans. It prepares public registration, deposits, signed-minimum withdrawals, SOL claims, opening, whole-token repayment, collateral additions and full Jupiter close. The SDK simulates a bounded v0 transaction, then the user reviews it and separately approves the exact message in the wallet. Account rent caps and refunded scratch advances are visible in local development settings. Token accounts must already exist; a holder's associated token account is filled in by default.

The optional in-house HTTP bridge supplies fresh Jupiter V2 ExactIn routes and accepts exact-quantity history warm-up requests. Configure its service URL in Connection settings. A funded operator publishes a reference after the required completed history; new quantities can remain Warming or unavailable at capacity. A real raw build response may still be pasted in advanced settings for local testing. The SDK binds mints, whole-position amount and actual loan scratch endpoints, and loads lookup-table contents from RPC. Provider credentials stay on the server; wallet secrets are never requested. The program enforces finite-reference freshness and all economic limits. No hosted/funded bridge deployment is claimed.

Initialized plain SPL and plain/metadata-only Token-2022 mints without freeze authority qualify, subject to the program's 4096-byte mint limit and extension checks. A registered market can still be Unknown/warming for borrowing. The UI discloses trusted in-house pricing, uncapped deposits/market count, upfront fee with no early refund, available-cash withdrawal constraints and deterministic collateral forfeiture after deadline plus 24-hour grace. Archived generation claims remain accessible by selecting that generation.

`pnpm --filter @shortwin/simple-web check` verifies type consistency and exact precision/rounding behavior; `build` creates the static local bundle. These checks do not replace actual program qualification, a funded operator deployment, wallet integration checks or the planned later external review. The owner’s release policy remains borrowing-capped mainnet before external audit and bug bounty; deployment and funded operating qualification remain separate from the software checks.


## Public workflow showcase

The public website is a separate, read-only showcase of the four recorded workflows. `src/entry.ts` selects it when `VITE_PUBLIC_PREVIEW=true`; the wallet workspace remains the default local build. The public bundle contains no transaction controls, Solana SDK, wallet signer, or local RPC/service code. The only local-app link is an explicit navigation link for visitors already running the demo on their own device.

Render the films with `tools/social-video/scripts/export.mjs`, then build from the repository root:

```sh
SHORTWIN_VIDEO_OUTPUT="$PWD/tools/social-video/exports" \
  node scripts/build-public-web.mjs
```

Set `SHORTWIN_VIDEO_OUTPUT` to another export directory as needed; its default is `tools/social-video/exports`. The helper requires all four MP4s and their matching `*-poster.jpg` files, runs the TypeScript check and public Vite build, and copies the eight media files into `web/dist/videos`. It checks the built page's four native video players, disclosure text, and documentation links; it also rejects operational app/SDK chunks, local RPC/service code, and source maps. MP4 exports are build inputs, not tracked repository files.

The public `dist/` also ships `robots.txt`, `sitemap.xml`, and `404.html` so crawlers see a canonical home URL, a real sitemap, and HTTP 404 for unknown paths (Cloudflare Pages serves `404.html` instead of falling back to the SPA shell).

`VITE_SOURCE_REF` pins GitHub source and documentation links to a branch or commit. It defaults to `main`; deployment CI sets it to the exact source commit being published. To check the local app's unit tests separately, run:

```sh
pnpm --filter @shortwin/simple-web check
```
