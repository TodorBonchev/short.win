# short.win — Web frontend (M9 contract)

> Current redesign decision (owner, 2026-10-27): new Jupiter markets use trusted
> off-chain VPS price history and on-demand authenticated snapshots, not continuous
> on-chain publication. Private samples are operator assertions. See the
> [active financial contract](jupiter/QUOTE_NATIVE_FINANCIAL_CONTRACT.md) and
> [owner decision](../audit/owner-jupiter-redesign-decision-2026-10-27.md).
> Legacy reference behavior below remains baseline evidence until the distinct
> backend is fully integrated and qualified; this is not a mainnet-ready claim.


*v1.4 • 2026-10-09 • **as built** (M9 integrated, then the M9 security and UX review fixed — §13
"Review fixes"; v1.3 folds in M10 — §13 "M10 changes"; v1.4 makes the app responsive — phones,
tablets and desktops, the owner's request of 2026-10-09, §7.6–§7.7 and §13 "M10-RESP changes"; v1.5 adds the
owner's thirteenth-round UX items — newcomer help, the deposit order, one coverage notice, the Claim
wording and the 12 px floor, §13 "Thirteenth round"; 2026-10-13 adds the Mobile Wallet Adapter,
O-6 lifted by the owner — §5.4 (6), §9.3, §10.2 and §13 "Mobile wallets", its E2E runs and the
responsive gate passed the same day (§10.2, §10.5), the owner's device test still pending;
2026-10-15 follows pump's v3 adapter (design pump-final §8, the owner's decisions of 2026-10-13) —
a close past a live curve's `rT` completes it in one instruction (FE-3 closed), a live cashback
curve is refused, the two D4 strings, §5.9, §5.11 and §13 "pump v3"; v1.0 of
2026-10-01 was the contract the four
parallel tracks — MARKETS, LEND, SHORT, E2E — built against, §12). Owner decision for the visual
style: **dark trading terminal** — dense, dark, data-first, as memecoin traders know it from
Photon / BullX / Axiom, and still accessible (§7, §8).*

This document turns ARCHITECTURE §7.3 ("Frontend") and the M9 row of ARCH §9 ("All §7.3 flows with
simulation-before-sign and disclosures"; exit: "Playwright E2E; CPI Guard and memo UX tests") into a
buildable specification: the information architecture and routes, every user-facing disclosure with
its exact copy, the transaction lifecycle, the data layer, the design system, accessibility,
security, the test plan, the `data-testid` contract and the tracks' file ownership.

The machine-readable halves of the contract live in `apps/web` and are held to this document by
tests:

| Artifact | Path | What it fixes | Held by |
|---|---|---|---|
| Disclosure copy | `src/config/disclosures.ts` | every disclosure's id, title, body, acknowledgement text (§4) | `disclosures.test.ts` (SPEC §5.3 verbatim; every paragraph present in §4 of this file) |
| `data-testid` contract | `src/config/testids.ts` | every test id and prefix (§11) | `testids.test.ts` (unique; every id present in §11) |
| Program error messages | `src/config/errors.ts` | a human message and an action for each code 6000–6109 (§5.9) | `errors.test.ts` (against `@shortwin/client`'s constants) |
| Pinned program ids | `src/config/programs.ts` | the programs a transaction may invoke (§9.1) | `programs.test.ts` (against `constants.rs` and the client) |
| Route tree | `src/router.tsx` | paths, typed search parameters, page components and their props (§3.2) | `router.test.tsx` (every route renders in the shell) |
| Transaction pipeline | `src/tx/types.ts`, `useTxFlow.ts`, `TxReviewModal.tsx` | `TxPlan`, the stages, the review (§5) | `tx/*.test.ts` |
| Venue layer interface | `src/venues/types.ts` | what MARKETS and SHORT share (§5.1, §12) | the type check |
| API types | `src/api/types.ts` | the indexer responses (`services/indexer/openapi.yaml`) | `types.test.ts` (required fields against the YAML) |
| Design tokens | `src/styles/tokens.css` | colours, type, density (§7) | the contrast table in §7.1 |
| CSP | `src/config/csp.ts`, `vite.config.ts` | the policy and the static host's headers (§9.3) | `csp.test.ts` |

**Status (v1.2, as built).** Every §7.3 flow is implemented on the scaffold: the markets table,
the market page, create market and protocol status (MARKETS); the lend panel and the lender
dashboard (LEND); the venue layer, the short panel, the positions dashboard and the position page
(SHORT); the Playwright suite on a local validator (E2E). The integration applied the tracks'
shared-file requests (§13 "Integration changes"), and the M9 exit is met (§12.3): `pnpm -r
build`, `biome check`, `vitest run` (56 files, 533 tests) and the E2E (19 specs, 50 tests,
including §10.3) pass. Where the build deviates from v1.0, the section says so and §13 lists it.
v1.2 fixes the sixteen findings of the M9 security and UX review (§13 "Review fixes"). **v1.3
(M10):** the WASM facade replaces the TypeScript math ports (FE-15), the validator E2E covers the
post-wipe bundle on the mock venue and the claim-time `sync_lender` (FE-18, partly), withdrawals
pass spl-memo v3 to every Token-2022 destination (TMEMO-9) and the E2E runs mainnet's Token-2022,
the `/security` page and `/.well-known/security.txt` exist, the deposit shows the pool's unpaid
rent (`lender.unpaidRent`), a `DeniedAccount` extras set falls back to the next set, and error
6105 is mapped (§13 "M10 changes"). *M10, eighth round (KI66-R4, KI-67, and the batch's review
fixes):* `vitest run` 60 files, 599 tests passed, 1 opt-in file skipped; the E2E 60 tests pass
(`lend-claim-pot-chain` and `lend-post-wipe` among them). **v1.4 (M10-RESP, 2026-10-09):** the app is responsive
from 320 px (§7.6, §7.7, §13 "M10-RESP changes"); `vitest run` 68 files, 670 tests passed, 1
opt-in file skipped; the default E2E run 77 tests (60 desktop, 17 phone) pass; the responsive gate
(`SHORTWIN_RESPONSIVE_GATE=1`, eight viewports, 55 states each) 80 tests pass with no finding; approved as built by the
owner on 2026-10-10 (thirteenth round). **v1.5 (the owner's
thirteenth round, 2026-10-10):** newcomer help (the intro strip, the glossary, How it works), the
deposit order, one coverage notice, the Claim wording and the 12 px floor (§13 "Thirteenth round");
`vitest run` 75 files, 721 tests passed, 1 opt-in file skipped; the default E2E 87 tests (65 desktop,
22 phone) pass; the responsive gate (eight viewports, 59–61 states each, text ≥ 12 px at every
viewport, the first market on the first screen) 80 tests pass. The new copy was approved as written
by the owner on 2026-10-11 (thirteenth round, §4.3). *The owner's approval and decisions of 2026-10-11*
(§13 "Thirteenth round: the owner's approval and decisions"; with the round-13 review's fixes):
`vitest run` 75 files, 731 tests passed, 1 opt-in file skipped; the default E2E 95 tests (69
desktop, 26 phone) pass; the responsive gate (eight viewports, 61–63 states each, the tap covers
a finding, judged in sheets, sticky panels and dialogs too) 88 tests pass. *The second approval pass
(2026-10-11; §13 "Thirteenth round: the second approval pass"):* the Claim only when syncs can pay,
and the exit state past the deadline as the program takes it; `vitest run` 76 files, 748 tests
passed, 1 opt-in file skipped; the default E2E 95 tests (69 desktop, 26 phone) pass; the responsive
gate (eight viewports, 63–65 states each) 96 tests pass. *The Mobile Wallet Adapter (O-6, lifted by
the owner on 2026-10-13; §5.4 (6), §9.3, §10.2, §13 "Mobile wallets" and FE-32):* built and on by
default behind `VITE_ENABLE_MOBILE_WALLET_ADAPTER`, the library loaded on eligible Android browsers
only; `vitest run` 87 files, 963 tests passed, 1 opt-in file skipped (the fake MWA wallet against the
real library among them). *The first browser runs (M3, 2026-10-13; Playwright's Chromium 1243):*
the default E2E run of 123 tests passes — 121 passed (70 desktop, 26 phone, 25 Android) and 2
skipped (`mobile-mwa`'s flag-off tests) in 17.9 min —, the Android project with the adapter built
off (`SHORTWIN_E2E_MWA=0`) 8 passed and 19 skipped, and the responsive gate (eight viewports,
63–65 states each) 96 tests pass with no finding; the reviewed `e2e/screenshots/android/` set
holds 9 PNGs. The adapter's code needed no change; two layout fixes and a steadier M12 screenshot
did (§13 "Mobile wallets", "The first browser runs"). The owner's real-device test precedes the
release (FE-32). *pump v3 (2026-10-15; §5.11, §13 "pump v3"):* `vitest run` 87 files, 1,021 tests
passed, 1 opt-in file skipped (the curve previews, plans and the liquidation's risk view held to
the program on pump's 2026-10-13 ELF, `pump-vectors.json`); the default E2E run of 124 tests (71 desktop with
`short-close-curve`, 26 phone, 27 Android) passes — 122 passed and 2 skipped in 17.8 min —, the
Android project with the adapter built off 8 passed and 19 skipped, and the responsive gate
(eight viewports, 63–65 states each) 96 tests pass with no finding (errors 0, axe 0).

---

## 0. Decisions and owner items

Decisions this contract takes (binding for the tracks unless the owner rules otherwise):

| # | Decision | Why |
|---|---|---|
| FE-D1 | **Money paths read the chain; browsing reads the indexer.** Every number a transaction depends on (quotes, margin, bounds, allowance, claimable SOL) is computed from accounts read through RPC at `confirmed` with the WASM math; the indexer feeds lists, history, analytics and search. | The indexer can lag or be down (INDEXER §3.2); a preview must equal what the program computes (ARCH §7.3). |
| FE-D2 | **Simulate before every signature** (§5.3): no wallet prompt without a successful simulation of the same instructions less than 20 s old; a stale review is re-simulated and shown again, never signed blind. | SPEC §9, ARCH §7.3. |
| FE-D3 | **No blind signing** (§5.8): only pinned programs at the top level; if the wallet changes the message, only compute-budget instructions may differ. | Wallet-side tampering or a compromised wallet extension. |
| FE-D4 | **One copy of every disclosure** (§4): rendered verbatim from `disclosures.ts`; the acknowledgement checkboxes gate the Sign button. | SPEC §5.3's "exact" sentence; reviewability. |
| FE-D5 | **Mint safety check client-side** (§3.5): the create-market verdict is computed in the browser from the mint account (the program's allow-list rule) and confirmed by simulating `create_market`; no indexer endpoint in M9. | INDEXER §14 IX-7 deferred `GET /v1/mints/{mint}/check` to M9; a client check plus simulation needs no new service. |
| FE-D6 | **Defaults for user bounds** (§5.10): open slippage 1 % (`min_proceeds`), margin and open-fee tolerance +2 % (`max_margin`, `max_open_fee`), close slippage 1 % (`max_buyback_cost`), deposit −0.5 % (`min_shares`), withdraw −0.5 % (`min_tokens`) and `min_sol` = the quoted SOL payout (ARCH §3.3, M1-SEC-2). User-adjustable within [0.1 %, 50 %]. | Memecoin volatility vs sandwich exposure (SPEC R4). |
| FE-D7 | **The mock venue is test-only in the frontend too**: its adapter is compiled in only with `VITE_ENABLE_TEST_VENUE=true`, which the config refuses off localnet (§2.1). | A49 (the mock never ships). |
| FE-D8 | **No remote token images or fonts** (§9.3): token identity is the mint; symbol and name are untrusted text. | Privacy, CSP, ticker impersonation (R-MINT §10.11). |

Owner items (each has a recommendation, which is what the scaffold assumes until the owner rules):

| # | Item | Recommendation |
|---|---|---|
| FE-1 | **CPI Guard deposits.** ARCH §3.3: "a lender whose token account has `CpiGuard.lock_cpi` cannot deposit through our CPI; the frontend detects it. A delegate-based deposit path is v1.1." Options: (a) block and explain (use another account, or turn CPI Guard off in the wallet); (b) additionally offer an opt-in **bracket** in one transaction: Token-2022 `DisableCpiGuard` → `deposit` → `EnableCpiGuard`, all top-level and signed by the owner, so the guard is off only inside that transaction. | (b), opt-in per deposit, with the `token.cpiGuard` disclosure; (a) is the default state of the panel. The E2E covers both (§10.3). |
| FE-2 | **Wallet-modified transactions.** Some wallets add or change priority-fee instructions in `solana:signTransaction`. | Accept compute-budget-only changes (a raised fee is shown after signing), refuse anything else (§5.8). |
| FE-3 | ~~**Curve straddle close by the borrower.**~~ **Closed (2026-10-15, design pump-final §8; owner D1–D4 of 2026-10-13).** It was: a close that would complete the Pump.fun curve failed `CurveStraddle`, the exit a ≥ 3-transaction Jito bundle the browser could not send. Since short.win trades every curve leg on pump's v3, a close of `q > rT` is one `close_short`: `buy_v3` buys `rT` on the curve and the rest from the would-be pool (synthetic migration), priced `B̂_v3`, and the market moves to `PumpMigrating` in that instruction. | The close form previews and sends it like any other close (§5.11); `closeStraddleNotice` and its words are removed. `CurveStraddle` (6031) now means a buyback beyond the curve and its would-be pool, unreachable under the per-market cap (pump-final C8); its message maps if returned. |
| FE-4 | **Geo-restriction (SPEC R6).** | At the CDN edge per legal advice; the app shows no jurisdiction UI of its own in M9 (placeholder footer text only after legal review). |
| FE-5 | **Degraded-market listing.** `/v1/markets` has no flag filter; the status page filters the first pages client-side on `flags.paused_coverage` / `route_broken`. | Accept for M9; an indexer follow-up adds `flag=` filters (INDEXER §14). |
| FE-6 | **Priority-fee ceiling for users.** | 1,000,000 µlamports/CU by default (`VITE_PRIORITY_FEE_MAX_MICROLAMPORTS`); the keeper's own ceiling (O17) is 100,000. |

---

## 1. Scope

**In (SPEC §8, ARCH §7.3):** wallet connect; token search by mint or ticker; permissionless market
creation; lender deposit / withdraw (token + SOL mix) / claim / cover deficit; atomic open short on
the Pump.fun curve, PumpSwap, Raydium AMM v4 / CPMM; close; add margin; the position dashboard
(`Q` owed, vault SOL, CR, liquidation price, P&L, rent accrued, time left); the market page (tier,
inventory, utilisation, rate, open interest, caps, recent liquidations, creator-wallet deposits);
the lender dashboard (shares, SOL claimable, locks, vesting); protocol status (degraded /
coverage-paused markets, the venue upgrade latch, route-broken); every disclosure of §4.

**Not in M9:** liquidation, expiry and `sol_settle` buttons (keeper instructions; the UI shows the
state and history, never sends them); governance / Emergency instructions; Jupiter (v1.1); partial
closes, limit orders, copy trading (SPEC §8 deferred);
charts beyond a price sparkline from `/candles` (optional, MARKETS). *M10-RESP (owner, 2026-10-09):*
the app is no longer laptop-only — phones, tablets and desktops are all targets (§7.6). *O-6 lifted
(2026-10-13):* the Mobile Wallet Adapter is built and on by default on Android browsers (§5.4 (6));
the owner's real-device test precedes the release (§13 FE-32).

---

## 2. Stack and workspace

| Layer | Pin | Notes |
|---|---|---|
| Runtime | Node 24.21.0, pnpm 12.7.0 (`mise.toml`) | `eval "$(mise activate bash)"` |
| Build | Vite **8.3.1**, `@vitejs/plugin-react` 6.1.1, TypeScript 5.9.3 (strict) | `tsc --noEmit` then `vite build` |
| UI | React **19.3.0**, TanStack Router **1.170.41** (code-based routes), TanStack Query **5.104.0** | no CSS framework; plain CSS with tokens (§7) |
| Solana | `@solana/kit` **8.3.0**, `@solana/react` 8.3.0, `@solana/kit-plugin-wallet` **0.20.0**, `@solana/kit-plugin-rpc` **0.19.0**, `@solana-program/token` 0.17.0, `@solana-program/token-2022` 0.19.0; *O-6:* `@solana-mobile/wallet-standard-mobile` **0.7.0** (a Wallet Standard wallet, not wallet-adapter; protocol **3.0.0** pinned through `pnpm-workspace.yaml` overrides; a lazy chunk, §2.2) | **no** web3.js v1, wallet-adapter or `@anchor-lang/core` (ARCH §1.1) |
| Program client | `@shortwin/client` (workspace, Codama) | instruction builders, account decoders, PDAs, error codes |
| Math | `@shortwin/math-wasm` (workspace, `packages/math-wasm`) | wasm-bindgen `--target web` build of `crates/shortwin-math-wasm` (§6.7) |
| Lint / format | Biome **2.5.14** (`biome.json` at the root, scoped to `apps/web` and `packages/math-wasm`) | 4 spaces, single quotes, width 120; a11y rules on |
| Unit tests | Vitest **5.0.2** + jsdom 30.1.1 + Testing Library 16.3.3 | `src/**/*.test.ts(x)` |
| E2E | Playwright **1.63.0** (`@playwright/test`), `@axe-core/playwright` 4.13.0 | system Chromium `/usr/bin/chromium` or Playwright's user-cache browser; never a root install (§10.2) |
| E2E seeding | `@solana-program/system` 0.15.0, `@solana-program/address-lookup-table` 0.15.0 (dev) | Node scripts with Kit and the Codama client |

**Dependencies are frozen for the tracks**: the scaffold installed everything the four tracks need
(`pnpm-lock.yaml` changed by additions only). A track that needs a new package asks the integrator;
no track edits `package.json` or the lockfile.

**Scripts** (`apps/web/package.json`): `wasm` (builds `@shortwin/math-wasm` if stale), `dev`,
`dev:e2e` (`vite --mode e2e`: the dev server against the E2E validator, configured by the
`.env.e2e.local` that `node e2e/validator/start.ts --env` writes), `build` (wasm → type check →
`vite build`), `preview`, `typecheck`, `lint` (`biome check .`), `lint:fix`, `test` (`vitest
run`), `test:watch`, `e2e` (`playwright test`).

**The WASM package.** `packages/math-wasm/scripts/build.mjs` runs `cargo build -p shortwin-math-wasm
--target wasm32-unknown-unknown --release --locked` and `wasm-bindgen --target web` into
`packages/math-wasm/pkg/` (generated, not committed: the `.wasm` embeds absolute registry paths). A
SHA-256 of both crates' sources, manifests and the root `Cargo.lock` is stamped into
`pkg/.inputs-sha256`; a matching stamp skips the build, so `pnpm --filter web build` needs neither
cargo nor wasm-bindgen once `pkg/` is current. wasm-bindgen must be the crate's exact pin
**0.2.129** (`$WASM_BINDGEN`, `PATH` or `~/.cargo/bin`; `cargo install wasm-bindgen-cli --version
=0.2.129 --locked`).

### 2.1 Configuration (`src/config/parseConfig.ts`, `.env.example`)

| Variable | Default (localnet) | Rule |
|---|---|---|
| `VITE_CLUSTER` | `localnet` | `mainnet` \| `devnet` \| `localnet`; the wallet chain is `solana:<cluster>` |
| `VITE_RPC_URL`, `VITE_RPC_WS_URL` | `http://127.0.0.1:8899`, `ws://127.0.0.1:8900` | required off localnet; `https` / `wss` on mainnet; no credentials in the URL (an origin-restricted public key in the path or query is the only acceptable form) |
| `VITE_INDEXER_URL`, `VITE_INDEXER_WS_URL` | `http://127.0.0.1:8080`, `ws://127.0.0.1:8080/v1/ws` | as above |
| `VITE_PROTOCOL_ALT` | none | base58; the protocol lookup table (ARCH §6.4 A28) |
| `VITE_PRIORITY_FEE_MAX_MICROLAMPORTS` | `1000000` | integer µlamports/CU (FE-6) |
| `VITE_ENABLE_TEST_VENUE` | `false` | TEST-ONLY mock venue adapter; refused unless `VITE_CLUSTER=localnet` (FE-D7) |
| `VITE_ENABLE_MOBILE_WALLET_ADAPTER` | `true` | `true` \| `false` (anything else fails the build); the Mobile Wallet Adapter on Android browsers and its CSP sources (§5.4 (6), §9.3). *O-6, 2026-10-13* |
| `VITE_EXPLORER_TX_URL` | Solana Explorer | must contain `{sig}` |

`parseConfig` is pure and lists every problem at once (`ConfigError`); `vite.config.ts` runs it at
build time, so an invalid configuration fails the build, and `main.tsx` renders the error instead
of a half-working app in development. A bare build is a localnet build.

### 2.2 Build output

`apps/web/dist/`: `index.html` with the CSP `<meta>` first in `<head>` (§9.3), hashed JS and CSS,
the math module as a hashed `.wasm` asset (≈ 205 kB, ≈ 74 kB gzip), and `_headers` for the static
host (the header form of the CSP with `frame-ancestors`, and the other security headers). No asset
is inlined (`assetsInlineLimit: 0`), so nothing needs `data:` scripts. *v1.2 (M9-SEC-6):* source
maps are generated `hidden` (no `sourceMappingURL`) and moved out of `dist/` into
`apps/web/dist-sourcemaps/` (gitignored) — upload them privately to an error tracker, or publish
them deliberately (owner choice, §13 FE-23). *O-6 (2026-10-13):* the Mobile Wallet Adapter
(`@solana-mobile/wallet-standard-mobile` 0.7.0 with its protocol and their in-page dependencies) is
a lazy chunk of its own (`index.browser-<hash>.js`, measured 137,863 bytes, 42,677 gzip), imported
only by `src/wallet/mobileWalletAdapter.ts`; only an eligible Android browser with the flag on
fetches it, never a desktop, iOS, a WebView or Firefox. It carries the library's dead QR and nostr
code (`qrcode`, `@noble/curves/secp256k1`; mobile.md R10).

---

## 3. Information architecture and routes

### 3.1 Shell

```
┌ header ─────────────────────────────────────────────────────────────────────────────────┐
│ W ShortWin [LOCALNET]  Markets  Positions  Lend  Status   [ search mint or ticker  / ]   [wallet] │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│ (indexer-down banner)                                                                    │
│                                   routed page                                            │
├ status bar ─────────────────────────────────────────────────────────────────────────────┤
│ ● RPC 412,334,120  ● Indexer live +1s  ● Live open                SOL $142.10  math 0.1.0  localnet │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

- **Header** (`AppShell`): brand, cluster badge off mainnet, nav (Markets, Positions, Lend,
  Status), the search combobox (`SearchBox`: `/v1/search` results by symbol / name prefix, an exact
  mint always offers "Open mint" even before the indexer knows it; `/` focuses it), the wallet
  button (`WalletButton`: Wallet Standard discovery and connect, address and disconnect).
  *Design pass (2026-10-16, P2; as built):* the brand is the mark (§7) and the wordmark, 8 px apart;
  from 640 px the cluster badge sits 4 px closer to the brand and the nav 8 px further off, so brand
  and badge read as one group (P2-2); under 400 px the mark is 30 px alone and "ShortWin" is hidden
  only while the cluster badge shows — on mainnet it shows at every width (P2-2). The header's edge
  is the first layer of `--shadow-header` (a `--line-panel` hairline with a faint drop under it; no
  border), and from 1,680 px the header and the status bar pad in to the page column (P2-4). Under
  1,024 px the header's free space sits before the search, so ⌕ (in `--text-0`) and the wallet button
  form the trailing group (P2-4). Menus and the search results float under `--shadow-pop`; a focused or hovered
  menu item steps to `--bg-4`, and the search's highlighted option is `--bg-4` with a 2 px gold inset
  bar at its left (P2-4).
- **Status bar** (`StatusBar`): RPC slot and health, indexer freshness and lag (§6.6), live-feed
  state, SOL/USD (from `/v1/stats`), the math module's state and version, cluster, and (*M10*) a
  "Security" link to `/security` (`nav-security`). Always visible: stale data is never silent.
  *Thirteenth round:* a "How it works" link before Security (`nav-how-it-works`, §3.11), and a
  glossary ⓘ on the RPC, indexer, live-feed and math items (§4.3).
  *Design pass P2-3 / P2-4:* the bar is inset by the page gutter (`--gutter`, as the header: the
  first dot starts where the title and the panels do), and its items take their state's colour — a
  healthy indexer badge quiet (`--text-1`), a warning or a lagging badge amber (`--warn`), a bad item
  or a stale / down badge red (`--danger`) — so a degraded item is never grey beside a green one.
- **Indexer-down banner**: while `/v1/status` fails, pages fall back to RPC reads (§6.5).
- **Keyboard**: skip link to `#main`; `/` search; table rows reachable by Tab, moved with ↑ / ↓,
  opened with Enter; Escape closes menus and dismissible dialogs.
- *M10-RESP — the document scrolls* (not `main`); the header is sticky, the status bar sticky at the
  bottom. **Tablet (640–1023):** the text nav stays; the search folds into a Search button
  (`search-toggle`) opening a full-screen overlay; the cluster leaves the status bar (the header
  badge has it). *Design pass P2-6:* a landscape phone (a coarse pointer, wider than tall, in this
  tier) gets the phone's status strip below instead of the eight-item bar, which would clip at that
  width; a mouse window keeps the bar with an 8 px item gap, so it fits from ≈ 666 px — a 640–665 px
  mouse window still clips the end of "Security" (no device class, no gate viewport). **Phone (< 640):**

```
┌──────────────────────────────────┐ ← safe area
│ W ShortWin [LOCALNET]  (⌕) [Connect] │ 48 (52 under a coarse pointer) sticky ("ShortWin" hidden < 400 only while the badge shows)
│  page (document scroll)          │
│ [ ▼ Short ]       [  Lend  ]     │ 56 action bar (pages that have one), fixed
│ ● All live · RPC 3,012        ⌃  │ 28 status strip (status-summary), fixed
│  ▤        ◫        ⇅        ⋯    │ 56 + safe area: the same <nav> as a tab bar
│ Markets Positions  Lend    More  │
└──────────────────────────────────┘
```

  The tab bar's More (`nav-more`) opens a bottom sheet with How it works (`more-how-it-works`,
  first; thirteenth round), Status, the security policy, Create market and the network facts
  (SOL/USD, math, cluster); *design pass P2-9 / P9-2:* each row carries a leading 20 px icon and a
  trailing chevron from the registry (`icons.tsx`), on a `--bg-2` sheet under `--shadow-sheet`. The
  status strip says the worst state in
  words with a shape (● ok, ▲ warn, ✕ bad) and expands to every status item (Escape from inside
  it returns focus to the strip). *Design pass P2-3 / P2-5 (as built):* the strip is a `--bg-2` band
  with a 16 px registry chevron (up while folded, down while open); a warn or bad state tints the
  whole band (`--warn-bg` / `--danger-bg`, its top line in the tone) and sets the words in `--text-0`
  at medium weight — tint, weight and words, three cues; the open list (44 px rows; the RPC slot
  grouped, "RPC 2,064") sits over a scrim that dims the page down to the strip's top edge and folds
  the list on its click (never on pointerdown, so a tap cannot fall through to the page once the
  scrim is gone), its two links bordered buttons side by side at the end; the active tab is gold
  with a 32 px bar at the cell's top edge and a heavier icon, the tab bar's edge is `--line-panel`,
  and the action bar floats under `--shadow-up` with no hairline, so action bar, strip and tab bar
  read as panel / status band / tab bar. Toasts drop under the header. The wallet popup is a bottom
  sheet with its title and a ✕ (`wallet-sheet-close`; *P2-8:* its edge the top line only, under
  `--shadow-sheet`, the balance the identity block's one focal number, the no-wallet chooser one
  bordered list of 48 px rows); like the trade sheet it has its own history entry
  (`state.walletSheet`), so Back closes it, closing it any other way pops that entry, and its
  My positions / My lending replace it.

### 3.2 Routes (`src/router.tsx`, frozen)

| Path | Search params (validated) | Component (props) | Owner | Data |
|---|---|---|---|---|
| `/` | — | redirect → `/markets` | SCAFFOLD | — |
| `/markets` | `q` (≤ 64), `venue`, `tier`, `sort` ∈ tvl, oi, util, created, open_positions, `order`, `active` | `MarketsPage` | MARKETS | `/v1/markets`, `markets` channel |
| `/markets/new` | `mint` (base58) | `CreateMarketPage` | MARKETS | RPC (mint, venue accounts), `GlobalConfig` |
| `/m/$mint` | `tab` ∈ overview, positions, lenders, liquidations, pools, coverage, events; `panel` ∈ short, lend; `action` ∈ deposit, withdraw, claim | `MarketPage({ mint })` → `ShortPanel({ mint })`, `LendPanel({ mint, action })`, `CapsSummary({ mint })` | MARKETS (page), SHORT (`ShortPanel`, `CapsSummary`), LEND (`LendPanel`) | `/v1/markets/{mint}` + subresources; RPC `Market`, `MarketVault`, venue |
| `/positions` | `status` ∈ open, closed, liquidated, expired, settled, all | `PositionsPage` | SHORT | `/v1/borrowers/{owner}/positions`; RPC `ShortPosition` + venue |
| `/positions/$position` | — | `PositionDetailPage({ position })` | SHORT | `/v1/positions/{pubkey}`; RPC |
| `/lend` | — | `LenderDashboardPage` | LEND | `/v1/lenders/{owner}`; RPC `LenderPosition`, `Market`, `MarketVault`, `ShareEpoch` |
| `/status` | — | `ProtocolStatusPage` | MARKETS | `/v1/venues`, `/v1/admin/config`, `/v1/status`, markets flags |
| `/security` | — | `SecurityPolicyPage` (*M10*, owner decision Q3 of 2026-10-02: the vulnerability disclosure policy that security.txt's `Policy` names, §9.3) | SCAFFOLD | static |
| `/how-it-works` | — (an anchor: `#short-risks`, `#lend-risks`, …) | `HowItWorksPage` (*thirteenth round*, O-3 (c): §3.11) | GUIDE (`features/guide`) | `GlobalConfig` (the governable numbers) |

A path parameter that is not a base58 address renders Not found. Tracks implement the components
with exactly these props and never edit `router.tsx`; a new search parameter is a contract change.
*As built:* every page is its own chunk (`lazyRouteComponent`); the router loads it before it
renders the route and preloads it on link intent, so no page suspends (the largest chunk is now
the shared vendor one, ≈ 222 kB / 70 kB gzip, instead of one 616 kB app chunk).
Every page keeps `data-testid="page-<name>"` (`T.page`) on its root element — the router smoke
test and the E2E rely on it.

### 3.3 Markets table (MARKETS)

Columns (dense, sortable where the API sorts): token (symbol, short mint; full mint on hover),
venue badge, tier badge, price (`ema_price` as SOL per token, subscript zeros), TVL (SOL, USD on
hover), inventory (tokens) and free, utilisation (bar + %), open interest (SOL), open positions,
flags (paused / coverage / route-broken / unsafe icons with text), created (age). Search box `q`
filters by mint or symbol / name prefix (`/v1/markets?q=`); filters for venue and tier; sort by
TVL (default), OI, utilisation, created, open positions; keyset paging ("Load more"). Rows update
live from the `markets` channel (patched in place, §6.4). Duplicate symbols show the mint suffix in
the token cell (two markets named BONK are visibly different). "Create market" links to
`/markets/new?mint=<q>` when `q` is a mint without a market.

*M10-RESP (§7.6):* columns by priority — token, venue, tier, price, TVL always; utilisation, OI,
positions, flags (2); inventory · free and age (3, hidden under 960 px of table width). Under
560 px each row is a card (symbol and mint, venue and tier at the right; price / TVL, utilisation /
OI, positions / flags), sorted by the table's sort bar. The filter reads "Filter this list"
(`min(380px, 100%)`, mono for a mint); on a phone venue, tier and open positions fold behind
"Filters · n" (`markets-filters-toggle`); an empty filtered list offers Clear filters.

*Design pass (2026-10-16, P5 and P11; as built):* the token cell is a link to the market
(`target-link`: 24 px, 36 under a finger; the row stays activatable by Enter and by a click
elsewhere), its symbol in link blue (P5-4); in the wide table the venue is quiet text — only
Migrating (warn) and Lend only (the ringed neutral pill) stay pills — and a card gives the venue its
grey tag back beside the tier (P5-4); TVL and open interest are padded to two decimals so a column's
decimal points align, and a zero reads muted (P5-4, P4-1); the utilisation track is `--meter-track`,
its fill never thinner than 2 px once there is any utilisation and empty at 0 % with a muted "0.0%"
(P5-5); the age column is `formatAge` ("45s", then "11m", "3h 12m", "2d 4h"; the UTC time in
`title`), so a list that refreshes every 15–30 s never churns seconds past the first minute (P5-5).
When the filter is a mint without a market the empty state carries the action its sentence names —
a gold "Create a market for this mint" (`markets-empty-create`) under it — and the head's Create
market link is the default button (P5-7); the indexer-down form's "Open a market by its mint" is a
sentence, not a tracked label (P5-7). On a phone the toolbar is one row merged into the sort bar —
the filter box, "Filters ▾" and the count, which still wraps to its own right-aligned line at
320 px (P5-1); the compact intro strip on every phone is an owner item (§7.6). *P11:* in the
priority table (under 960 px of table width) the token cell is two lines, symbol over mint, so a
768 px tablet's table fits its frame; a hand-typed `?q=` of digits only is still a filter (the
router's search parser reads it as a number).

*Thirteenth round (O-3 (a), audit finding D-03):* above the list, the **intro strip**
(`markets-intro`, `MarketsIntro`): a heading row ("New to short.win?", How it works, Hide) and three
parts — "Short a token", "Lend a token", "What can go wrong" — of one sentence each, side by side from
1,024 px and stacked under it; the last links to How it works `#short-risks` and `#lend-risks`, where
`borrower.liquidation` and `lender.risks` are rendered verbatim (§3.11). The strip is compact: on a
short phone (under 400 × 700 px) and a phone held sideways (§7.6's landscape-phone tier) it keeps
only the heading row and "What can go wrong" with its sentence and its links (sideways, the title
on the left, the sentence beside it and the links under the sentence; *M3, 2026-10-13:* the heading
row stays one row there — where the three do not fit, "New to short.win?" wraps inside its own box
beside How it works and Hide instead of Hide taking a row of its own), so the first market shows
on the first screen (held sideways, the list's filter: a 390 px screen has no room for a row) —
the responsive gate checks that at every viewport (§10.5); How it works explains the other two
parts. Hide
(`markets-intro-dismiss`) is remembered in `localStorage` (`shortwin:markets-intro:hidden`, inside try
/ catch); where storage throws the strip still renders and Hide hides it for the page session. After
Hide a polite status message says where How it works is, and focus moves to the "Markets" heading.
Its text is §4.3's (`config/copy.ts`). The table's jargon headers carry a glossary ⓘ (§4.3) beside
the header text, never inside its sort button; the flag badges carry one with the banner's text.

### 3.4 Market page (MARKETS; panels SHORT / LEND)

Layout `.layout-trade` (*design pass P2-7:* an alias of the shared `.layout-side` grid — `--side-col`,
360 px and 320 px under 1,180 px — and of `.side-sticky`, which the Manage panel shares, §7.6): left
the market column, right the trade panel (tabs Short | Lend).

- **Header:** symbol, name, mint (copy), tier badge (popover "Why this tier": the tier inputs — pool
  age from `graduated_at` / listing, locked liquidity in USD, SOL/USD age — and the
  `market.tierAge` disclosure), venue badge, flag badges (`marketFlag-*`), price (spot and EMA).
  *As built:* the popover shows the Tier A / B liquidity thresholds in USD and in SOL at the
  oracle price, not the pool's own locked liquidity Λ (FE-14); an `OracleCache` never cranked
  reads "No reading yet".
  *Design pass P5-3 / P5-8 (as built):* the EMA price is the hero (`--fs-hero`: 22 px on a desktop,
  24 on a phone) over the 20 px title, the spot `--fs-lg` in `--text-1`, and the name · mint line and
  the price figures share one baseline (`last baseline` where supported); the price labels are the
  label tier; the mint's quick-copy glyph and the "Why this tier" ✕ are registry icons (`copy` /
  `check`; `close` in the shared 32 px box, 44 under a finger), the popover edged with `--line-panel`
  under `--shadow-pop`; the tier badge keeps its inline ⓘ. The banners sit 8 px apart.
- **Banners** in severity order: `market.mintUnsafe`, `market.routeBroken`, `market.coveragePause`,
  `market.venueUpgraded`, `market.migrating`, `market.graduationHalted`, `market.listOnly`; a
  collection window or a warm-up (`ema_warm > 0`) shows `market.poolSwitch`. *Thirteenth round (O-2,
  audit finding D-23):* the page tells its panels which banners it shows (`PageBannersContext`), and
  each banner is focusable from script (`tabIndex = -1`, a focus ring). On a coverage-paused market the
  Short panel keeps its note ("New shorts unavailable. …", error 6051) and, in place of a second copy
  of `market.coveragePause`, shows a link to the banner (`short-coverage-link`): beside the page "Read
  the full coverage notice" scrolls to the banner (smoothly unless reduced motion) and focuses it; in
  the trade sheet "Close and read the full coverage notice" closes the sheet (as ✕ does), then
  scrolls to and focuses the banner. Without that banner — the 5 s after a clear (the chain's
  `CoveragePaused` with `waitS`, the flag already off) or the indexer's flags missing while the chain
  says paused — the panel keeps the disclosure and shows no link.
- **Stat strip** (SPEC §8): inventory (`inventory_tokens`, free `tokens_free`), utilisation
  (`util_bps`), rate (the APR a new open pays now: `rateBps(base, slope1, slope2, util, kink)` with
  the live tier terms), open interest (`oi_lamports`, `open_positions`, `total_notional_open`), caps
  (`CapsSummary`, SHORT: the binding size cap for the next open), TVL (`tvl_lamports`, `tvl_usd`),
  lenders (`lender_count`).
- **Tabs:** Overview (the stats above in full, fee lines, exit counts and rates, realised lender
  loss by S1 / S2 / S3, `market.creatorSelfDealing`, `market.venueTrust` collapsed); Positions
  (`/positions?status=open`, each with CR and risk computed client-side — *as built* through
  SHORT's `useMarketPositionsLive`: the positions' accounts in one read and the market's
  snapshot, the same `positionMetrics` as the dashboard); Lenders (largest first,
  `is_coin_creator` and `is_market_creator` badges — the **creator-wallet deposits** of SPEC §8 /
  R1, plus `creator_exposure.coin_creator_share_bps` in the header of the tab); Liquidations
  (`/liquidations`: kind, condition S1–S3, Q, buyback, lender amount, keeper paid, borrower payout,
  time, signature); Pools (`/pools`: current pool, candidates with `D`, adoption history, the open
  window); Coverage (`/coverage`: gaps, pauses, last 24 h observations); Events (`/events`).
- **Live:** subscribes `market:<mint>` (and `trades:<mint>` if a sparkline is shown); the panels
  read the chain (FE-D1).
- *M10-RESP (§7.6):* the header puts the prices (EMA, spot) right after the title at the large
  size (*design pass P5-3:* the EMA price at the hero size, the spot at `--fs-lg`), the tier badge reads "Tier C ⓘ" (the mark an inline SVG, not a font glyph), the mint is a Reveal (full mint, Copy, Explorer), and a
  flag that repeats the venue badge ("Lend only", "Migrating") is that badge. The stat strip
  starts with rate, utilisation, TVL and size cap; inventory, OI and lenders are priority 2 (behind
  "More stats" on a phone). Overview: hairline key / value blocks in two columns that share the
  card's width (Market | Exits and the settlements; Fees under both — a third column held a one-liner
  while the others wrapped), zeros muted, an all-zero settlements block as "No settlements yet";
  the creator-wallet figures span their box like the wallet table under them. The Coverage tab's
  key / value blocks use the same grid, so they share the card's width too. Every tab table is
  a DataTable (priority columns, cards when narrow); the Positions tab's columns are priority 1–2
  but the age ("Open for", priority 3: it would push State out of the 680 px column at 1,024 px),
  so a laptop's market column shows borrower, margin and entry; its phone card leaves out the
  borrower and entry (a tap opens the position). Under 1,024 px the page is
  one column and the trade panel a sheet (`?sheet=trade`) opened by the action bar's Short / Lend;
  ✕, Escape, the scrim and Back close it. "Why this tier" is a bottom sheet on a phone.
  *Integration (v1.4):* the header, the action bar and the banners open the market column, so from
  1,024 px the trade panel starts at the top of the page: its sticky top is where it rests, and its
  sticky submit is above the status bar on the first screen (with the header above the panel, the
  panel's viewport-high box reached under the status bar until the page scrolled). The app-wide
  indexer-down banner still sits above both columns.
  *Design pass P5-2 and P11 (as built):* a market that cannot be shorted — `SHORT_BLOCKED`: paused
  opens, coverage paused, route broken, lend only, migrating (the flags under which `openBlocked`
  refuses; warming up, a pool window, a halted graduation, an unsafe mint and an upgraded venue stay
  shortable by default) — opens its trade panel on Lend; an explicit `?panel=` still wins, and the
  Short tab, its notice and the coverage link remain. The default waits for the market's view (its
  flags): until then the panel shows skeleton lines, so the Short panel never mounts and then flips
  to Lend. Under 1,024 px (a tablet included) the action bar's Short steps down to a grey default button (`data-unavailable`;
  the banner above says why, the sheet still opens on it, its hover and press stay grey) and Lend
  takes the gold. *P5-8:* the creator box's disclosure sits on the nested surface (`--surface-nested`),
  edged with `--line-panel` on its plain sides; the stat strip's lender count reads muted at zero.

### 3.5 Create market (MARKETS)

1. Mint input (`createMintInput`; prefilled from `?mint=`). If a market exists (`findMarketPda` +
   RPC), link to it (`createExisting`).
2. **Safety verdict** (`createVerdict`, `data-verdict`): `readMint` → the program's rule (ARCH §3.2
   check 1; *M10, FE-15:* `shortwin_venue_state::mint::check` itself, through the WASM facade's
   `mintCheck` — the further reasons are `check` re-run with each earlier rule lifted; if the math
   module fails to load, the page shows the load error in place of the verdict, and an existing
   market still shows): owner ∈ {Token, Token-2022}; not a native mint; no freeze authority; every extension
   type ∈ {MetadataPointer 18, TokenMetadata 19, GroupPointer 20, TokenGroup 21,
   GroupMemberPointer 22, TokenGroupMember 23}; a malformed TLV rejects. Each failing rule shows as
   `createReason-<ErrorName>` with the message of `config/errors.ts` (6016–6020).
3. **Venue detection** (`detectVenue`, SHORT's venue layer): pump curve / migrating / PumpSwap /
   none, with a refusal reason for unsupported pump coins; for non-pump mints, the Raydium
   candidates (`raydiumCandidates`): pool, kind, real SOL, `D`, λ, fee, eligibility and reasons
   (`createPoolRow-<pool>`). The user picks Auto (list-only or pump) or an eligible Raydium pool.
4. **Cost** (`createCost`): `market_creation_fee` from `GlobalConfig` plus the rent of `Market`
   (3,104 B), `MarketVault` (2,200 B), `pool_vault` and, on pump venues, the trader ATA and UVA
   (`getMinimumBalanceForRentExemption`); `market.create` disclosure.
5. Submit → `create_market(venue_hint, max_creation_fee = the fee read)` through the pipeline;
   `extend_account` prepended for a curve shorter than 115 B (`CurveNeedsExtend`); watched:
   the wallet's SOL. On confirmation, navigate to `/m/<mint>`.

*M10-RESP:* a token the filter rejects (or a venue refuses) replaces the cost with "Cannot be
listed" and its reason, and has no Review (D14). Review sits in a form foot with the reason it is
disabled (`create-submit-reason`): sticky above the bottom bars on a tablet, the page's bottom bar
on a phone. Under 1,024 px one column (mint → verdict → venue → pools as cards → cost); the mint
input takes `addressInputProps` and, under a coarse pointer, Paste (`create-mint-paste`).

*Design pass P5-6 (as built):* the page is on the shared side column (`--side-col`: 360 px, 320 under
1,180 px, the page gap between) like the trade and Manage panels; the left column's fields, the token
facts, the venue choice and the reasons keep a 64ch form measure (`--form-measure`; the pools table
keeps the panel's width); the token facts are hairline rows (`KvLines`, a zero supply muted); the
radios are drawn by the app (§7.4).

### 3.6 Lend panel (LEND)

Tabs Deposit | Withdraw | Claim (`lendTab-*`, `?action=`).

*M10-RESP (§7.6):* the panel lives in the market page's trade column on a desktop and in the trade
sheet under 1024 px. Each form's submit sits in a sticky form foot with the reason it is disabled
(`lend-deposit-reason`, `lend-withdraw-reason`, `lend-claim-reason`; computed from the same predicates
as the submit, first unmet first; a reason that names an unticked acknowledgement or the CPI Guard
box focuses it). Without a wallet the foot offers Connect wallet (`connect-cta-lend`) above the
disabled submit. Withdraw has 25 / 50 / 75 % chips of the holding (Max, what may be withdrawn now,
stays in the field) and a "≈ N TOKEN" line under the shares (display only). *Thirteenth round:* the
deposit follows the Short form's order (O-1, below) and the Claim button says what it does (O-5,
below).

*Design pass P7 and P11 (as built):* Deposit | Withdraw | Claim is the `Tabs` `sub` variant — a
segmented row on a `--bg-2` track, the selection a `--bg-3` fill with an inset ring — under the Short |
Lend underline (P7-2); the position summary is a sunken well (`--surface-well`, edged with
`--line-panel`, 8 / 12 px inset like the Short preview; P7-2, P11); the account select reads at
`--fs-input` like the amount field (13 px with a mouse, 16 under a finger, so iOS no longer zooms the
sheet) and truncates a long label; the tolerance box is `--field-narrow-w` (120 px); "Shares minted"
and "You may withdraw now" are the preview's emphasised figures (`data-emph`, §7.4 `Kv`; P7-2).
*P11:* the key / value rows share their width (labels 1fr, values 1.2fr) at every width and a long
value wraps at its spaces, so the desktop column's labels never collapse to a word a line; a UTC stamp
keeps its date and its time with the zone each on one line (`UtcStamp`, `lend/ui.tsx`), breaking only
between them.

- **Deposit:** source token account select (`readOwnerTokenAccounts`, ATA first; balance; CPI
  Guard flag), amount (`AmountInput`, Max = balance), preview (shares minted
  `previewDeposit` → `lendPreview-shares`, resulting share of the pool `lendPreview-share-bps`,
  token value). *M10 (owner decision of 2026-10-03; ECON §9.4's late-depositor shortfall):* the
  pool's unpaid rent as the deposit meets it — `Market.receivable` plus the lender credit the
  deposit's prologue books over `market_now − last_accrual_ts` (`pendingLenderCredit`, the WASM
  `RentBasis.lenderCredit` on `rent_aggr` with the carried `lender_credit_rem`, AIR-03) — (row
  "Unpaid rent in the pool",
  `lend-preview-unpaid-rent`) and this deposit's part of it should none be paid,
  ⌈receivable·shares/(S + shares)⌉ at the pool's size after the deposit (`previewUnpaidRent`, the
  split in plain integers `unpaidRentExposure`; row "This deposit’s part if it is never paid",
  `lend-preview-unpaid-rent-share` with `data-level` none / info / warn), shown when that
  receivable is > 0. It warns when the part is ≥ `UNPAID_RENT_WARN_BPS` (100 bps) of the deposit's
  value at `P_ema` (⌊amount·P_ema/WAD⌋) or ≥ `UNPAID_RENT_WARN_LAMPORTS` (1 SOL; the only threshold
  without a price): `lender.unpaidRent` expanded and repeated in the review (`TxPlan.disclosures`);
  otherwise collapsed and not in the review. The owner confirmed these thresholds on 2026-10-03
  (fourth round; FE-27 closed). Disclosures: `lender.solRepayment` **on the deposit screen, after
  the risks and right before the sticky submit, and again in the review, with its required
  checkbox** (SPEC §5.3; before signing), `lender.risks`, `lender.lossHistory`,
  `market.creatorSelfDealing` (collapsed), `lender.residuals` (collapsed), `lender.unpaidRent` (M10).
  *Thirteenth round (O-1, audit finding D-09):* the form's order is the Short form's — account (and
  the CPI Guard notice of that account) → amount → tolerance → preview (with the unpaid-rent rows,
  `lender.unpaidRent` and the sweep notice) → the risks (`lender.risks`, expanded on a wallet's first
  deposit; then `lender.lossHistory`, `market.creatorSelfDealing`, `lender.residuals`, collapsed) →
  the "You may be repaid in SOL" acknowledgement → the sticky submit. The texts are verbatim: only
  their place moved (v1.4 had the acknowledgement above the amount and the risks below the submit). A source with CPI Guard
  shows `token.cpiGuard` (`lendCpiGuardNotice`, plain deposit disabled) and the opt-in bracket
  (`lendCpiGuardBracket`, FE-1). `deposit(amount, min_shares)`; `WipeSweepPending` (*AIR-08,
  `2bda5f5`; *AIR2-05:* whenever a required source bit is clear, including a dust flush with
  surviving shares — after a flush or after the last lender's exit, which clears both bits)
  → collect the permissionless sweep bundle (ARCH §3.3,
  §3.9: `sweep_trader`, on pump venues pump's `claim_cashback` first when the UVA holds cashback,
  on PumpSwap pAMM `init_user_volume_accumulator` when missing; *AIR-05, `975f757`:* wherever the
  market's `pump_uva` is set, also on a curve market that left pump, pump's `claim_cashback`
  first and `sweep_trader` with the trader's pump UVA alone as its token leg). With surviving
  shares, offer **Review pool cleanup** (`lend-cleanup-submit`): a separate `lender_cleanup`
  plan containing only the claims and sweep, with `lender.cleanup` in its review. It needs no
  token amount, deposit acknowledgement or lender sync. It shows the wallet fee/account-rent
  change, market SOL vault and remaining vest. Deposit stays disabled until a fresh read shows
  cleanup complete and any actual vest released; keep the entered amount, source and CPI Guard
  choice after cleanup confirms. Review/retry of a deposit refuses surviving-share cleanup;
  review/retry of cleanup never appends a deposit. For an empty pool (`S = 0`), prepend cleanup
  to the deposit in the existing atomic transaction. After the last
  lender's exit that sweep passes no wipe record and books the trader's SOL to the protocol
  (KI-10); the sweep notice says which (`lend-sweep-notice` `data-kind`), and the review title
  names the step "sweep after the last exit", or "post-wipe sweep" after a wipe. *AIR-07:* a
  deposit into a drained pool is refused `PoolDrained` (6108, action `wait`: at most 1/1,000 of
  the pool's tokens unborrowed; `3c0f602`), and the WASM deposit preview refuses it the same way
  (`MATH_ERRORS`, `3f224c2`). *AIR2-04:* the same refusal also holds a deposit while a
  non-flushing settlement's vest remains unreleased; the preview checks the actual vest after
  accrual, including later credits that extend it. A zero legacy hold marker also requires a
  one-time source sweep, even when the stored bits are complete and the vest is empty;
  -1 means initialized without a hold. Pending venue rewards arm a hold before a split claim
  can re-vest income. *AIR2-05:*
  pending-source income after a dust flush is credited immediately to the surviving lenders.
  A split claim can re-vest income through an already completed source; standalone cleanup
  lets that vest start without a refused deposit rolling it back. A lender behind on epochs
  gets `ShareEpoch` remaining accounts (`getShareEpochAccountMetas`) or `sync_lender` first.
- **Withdraw:** shares (or %), Max = the pro-rata allowance (`previewAllowance`, capped at the
  token value of unlocked shares since AIR2-P2-01), preview tokens and
  SOL (`previewWithdraw`), lock and `lock_until` after the withdrawal (`previewLock`,
  `previewLockUntil`), destination account (ATA created idempotently if missing; memo flag →
  `token.memoRequired` and the title; `memo_program` = spl-memo v3 for every Token-2022
  destination — *M10, TMEMO-9:* the program CPIs it only when the destination requires memos at
  execution —, the short.win program id as "none" for SPL Token).
  `lender.exitForfeitsVest` with its checkbox when shares remain and unreleased vest exists.
  `withdraw(shares, min_tokens, min_sol)` with `min_sol` = the quoted SOL (FE-D6).
  `LastSharesBorrowed`, `NegativePending` (→ cover deficit), `AboveProRataAllowance` mapped (§5.9).
  *As built:* the input is in shares, displayed at the token's decimals, with 25 %, 50 % and Max
  (the allowance); `withdraw-preview-sol` is the WASM `previewWithdrawal`'s SOL part on the stored
  state (*M10, FE-15:* the claim right after the burn, a last lender's vest flush included); it is a
  lower bound only because the prologue's rent accrual can add to it, and the exact `min_sol` is
  the market vault's lamport delta in a simulation of the same withdrawal with `min_sol = 0`;
  `withdraw-preview-lock-after` ("Locked shares after"; `data-value` = shares, `data-until` =
  `lock_until`) is the lock `previewWithdrawal` leaves — the borrowed part of the burn added to the
  live lock, capped at the shares left —, next to "Locked shares now" and "A new lock would end"
  (`withdraw-preview-lock-until`, the end a new lock would get); the vest-forfeit box is ticked in the panel and again in the review;
  the destination select is `withdraw-destination-select`.
- **Claim:** claimable now (`previewClaim`), vesting (`previewVestRelease` share), the pot, a
  deficit (`claimValue-*`); `claim_sol()`; *thirteenth round (O-5, audit finding D-21):* the button
  says what the claim does — "Review claim · {c} SOL now · syncs may add up to {g} SOL" on two lines
  (the second from "syncs", `claim-submit-syncs`; one accessible name) when the syncs the search
  picks can add 10,000 lamports or more, {g} = min(shortfall, their gain) with the shortfall
  max(0, pending − own − pot) — credited = claimable + shortfall, so the claim never pays more than
  the two together; else "Review claim · {c} SOL" (the owner's decision below); both amounts with
  `solFrac`; the note above it explains the shortfall (§4.3
  `claim.note.shortfall`). *The dust threshold (the owner's decision of 2026-10-11, thirteenth
  round):* a shortfall under 10,000 lamports (`CLAIM_DUST_LAMPORTS`, `previews.ts`; ECON §9.4's
  rounding dust) counts as none — `previewClaimView` reports it as 0, so with nothing claimable the
  button is the disabled "Review claim · 0 SOL" with "Nothing to claim yet", with something
  claimable it reads "Review claim · {c} SOL" without the note, and the claim gathers no syncs
  (`previews.test.ts`, `LendPanel.test.tsx` and `build.test.ts` at 0, 9,999 and 10,000 lamports, with
  nothing and something claimable; so credited = claimable + shortfall + under 10,000 lamports of
  dust). *Only when syncs can pay (the owner's decision of 2026-10-11, asked after the approval's
  review: "Only when syncs can pay"):* the button works from what the syncs can actually move.
  Before it offers them it runs, in the background, the search the review runs
  (`claim/syncSearch.ts`: `searchPotSyncs` in `build.ts`, `gatherPotSyncs` with `claimPlanFrom`'s
  inputs), only for a shortfall of 10,000 lamports or more, cached by react-query under
  `lendKeys.claimSyncs(mint, owner)` plus the stored state the pick comes from that a price crank
  leaves alone (`searchInputs`: the claimant's own lender fields, the pot, the share epoch and the
  total shares). *Second approval pass's review:* the key held the realised-rent index (`acc_r`),
  which a crank's accrual raises while a vest runs (its release moves `acc` and `acc_r` together,
  so no lender's surplus changes), so every crank searched again and the button fell back to its
  in-flight state; the index and the lenders' SOL (moved by sweeps; a sync's gain does not read it)
  left the key, and so did the view's per-second figures. Searched again when that state changes,
  when a claim, deposit or withdrawal of the market lands (`lendInvalidations`), on a remount after
  15 s and, with nothing claimable now (where the search decides whether the Claim opens), about
  every 60 s while the page is visible — 45–75 s, the period fixed per market so a Lending page's
  rows do not search together (rent paid in raises `acc_r` alone and can give another lender a
  surplus) — never on a price crank. While a new search runs for the same market, owner and own
  position the last result stays (a placeholder from the cache; not fresh, so the review searches
  again): no flicker; after the user's own claim, deposit or withdrawal (whose syncs the claim may
  have run) the first search shows the in-flight state. At most two searches at once app-wide; a
  search whose query is dropped (its inputs replaced, its page left) leaves the queue without
  running, or stops before its next indexer page and frees its slot (react-query's `signal`). A
  search cut short — still running after 20 s (it stops), or a failed read (indexer or RPC down;
  the background search asks for each indexer page once, without the retries) — keeps what its
  pages found, shown but never fresh, so the review searches again; the Lending page runs none while
  it knows the indexer is down. The label's {g} is min(shortfall, the picked syncs' gain); under 10,000 lamports it
  counts as none, so a shortfall no sync can fund (rent the open shorts have not paid in; a market
  whose only lender is you) gives "Review claim · {c} SOL" when something is claimable now, else the
  disabled "Review claim · 0 SOL" with "Nothing to claim yet". The shortfall note keeps the whole
  shortfall as its {g} (what is out of reach, and why). *While a first search runs* nothing more than is
  known is shown: with something claimable now the button is enabled with "Review claim · {c} SOL"
  (its second line appears once the gain is known); with nothing claimable it is disabled and busy
  (`aria-busy`), "Review claim · 0 SOL" with no reason yet — then either the two-line label (enabled)
  or "Nothing to claim yet", so no reason shows that then flips. *The review* takes the button's
  pick while it is fresh, else searches again (as before; Retry always does), and the review's
  simulation decides: one that pays 0 lamports out of the market's SOL vault is not offered for
  signing — Sign stays disabled with "Nothing to claim yet" beside it (`TxPlan.holdSign`,
  `tx-sign-hold`; no new words) and Refresh simulates again (`syncSearch.test.ts`,
  `LendPanel.test.tsx`, `build.test.ts`, `useTxFlow.test.ts`, `TxReviewModal.test.tsx`; E2E
  `lend-claim-sync`). When the pot cannot cover the claim, prepend
  `sync_lender` for other lenders whose touch moves a surplus to the pot (from
  `/v1/markets/{mint}/lenders`, each judged on its on-chain state; ≤ 4 per transaction);
  **cover deficit** (`cover_deficit(lamports)`) when pending < 0. *As built:* the claim tab shows
  "credited" next to "claimable"; a lender more than 8 (`MAX_RECORDS`) share epochs behind (more
  records than one transaction carries) gets a `sync_lender` step first (`lend-sync-submit`).
  *M10 (KI66-R4, the owner's eighth round, 2026-10-08):* the prepended syncs are the claimant's,
  so they are **unsigned** by the other lenders' owners — the program runs the pot-only touch
  `pool::touch_pot` (KI-66 full), which never converts a lender through a rebase record but holds
  it at the record and walks its forward chain, so what an **absent lender behind a rebase record**
  earned before and after the record reaches the pot without its owner. `claimPlanFrom`
  (`build.ts`), the candidates by `gatherPotSyncs` (`quote.ts`; *M10, FE-30, the owner's ninth
  round, 2026-10-08:* behind and chained lenders first): (1) the indexer's lenders of the market
  from `/v1/markets/{mint}/lenders` in its default `active` view (shares > 0), **first those
  behind the share epoch** (`epoch=behind`: `epoch` < the market's share epoch — held at a rebase
  record, parked on a live full-wipe record, and every lender with a forward chain except one
  whose owner's own conversion reached the share epoch inside it, which is current), **then the
  current ones** (`epoch=current`), `SYNC_CANDIDATE_PAGE` = 50 a page (a longer page, from an
  indexer that ignores `limit`, is cut to 50; the indexer's `max_page_limit`, at least 200 by its config
  check, allows it, INDEXER §9.1). The indexer's order within a list, largest raw share count first, is not
  comparable across share epochs and no longer ranks the syncs (the gains do), but it still
  decides which lenders are read first and which fall inside the bound below. The two lists are
  the indexer's split by **its own** share epoch, and every row not read yet is read whatever its
  epoch: an indexer that has not ingested the latest rebase serves the newly behind lenders in its
  current list (read there, after its behind list), an indexer ahead of the RPC serves lenders the
  chain read calls current in its behind list (read there), and an indexer without the filter
  serves the whole list in the first pass (the second then finds nothing new); a lag only
  reorders the reads, never drops a lender. The signer and owners already read are skipped, and
  only accounts that are positions count
  (`readOtherLenders` / `lenderPositionOf`: owned by short.win, the position's size and
  discriminator; a system account someone funded at a lender PDA reads as no position);
  (2) per page, `readRecords` over the union of the records its lenders' syncs need and no earlier
  page read (`potEpochs`: from `fwdEpoch` once it has a forward chain, else from `epoch`; ≤ 8
  each; ≤ 100 per read); (3) `judgePotSyncs` judges each with the WASM `previewTouchPot`
  (`pool::touch_pot` on copies: the pot's gain) and `choosePotSyncs` takes, among every lender read
  so far, the largest gains until the shortfall is covered, ≤ `MAX_SYNCS_PER_CLAIM` = 4 (the
  gains add up: each sync moves only its own lender's surplus); the next page is read only while
  they do not cover it, and at most `MAX_SYNC_CANDIDATE_PAGES` = 4 pages in all, both lists'
  requests counted, an empty one too (≤ 200 lenders):
  each page is three sequential reads (the indexer, the positions, the new records) while the user
  waits for the review, about 12 round trips at the bound (an estimated 2–3 s on a public RPC, not measured), and one claim
  carries 4 syncs at most anyway; a failed read ends the search with what the pages before it
  found (nothing when the first fails: the claim pays what the pot holds); (4) each is `sync_lender(n = its records)` with the other lender's `owner` unsigned and
  its records **read-only** (the pot-only touch writes no record; the program requires them back
  unchanged); (5) while the compiled transaction — without lookup tables, an upper bound of the
  v0 message with the protocol ALT (`wireSizeOf`) — is over 1,232 bytes, the smallest-gain sync is
  dropped, and the expected payout and the close are recomputed with the pot the rest moves
  (`previewClaimOutcome`); (6) a plan with syncs is simulated once more before the review: a sync
  the simulation fails on (its position closed or changed since the read) is dropped and the plan
  rebuilt, so a stale target never blocks the claim; the review then simulates the result like
  every plan, and nothing is signed before it. A sync dropped that way is not deferred (it is no
  longer valid; the next claim's read leaves a closed one out). Without the indexer there are no
  syncs. The dashboard's claims (and "Claim all") build the same plan with the same indexer lists
  (`claim/otherLenders.ts`). The review
  lists the syncs as steps (title "… · N lender syncs first") with `lender.potSyncs` (how many, what
  they move), and `lender.potSyncsDeferred` when lenders with a gain did not fit (the cap or the
  size) while the claim stays short: claim again once it lands. A stale `fwdEpoch` is harmless
  (a sync that finds nothing more to move moves nothing); a lender more than 8 records along
  advances 8 per claim. An unsigned sync costs ≈ 48.0k CU at 8 records (47,989, `m1_cu`; ARCH
  §6.1), so four and a chained claimant's claim stay below ≈ 4 × 48k + 58k ≈ 250k CU; the
  simulation sets the limit (no `m1_cu` row measures the four syncs and the claim together). *M10 (KI-67):* a claim that leaves the
  position current with no shares, `pending` 0, no lock and no forward chain closes it and returns
  its rent (2,568,240 lamports) to the wallet: the review shows `lender.claimCloses`
  (`previewClaimOutcome`), the position's fields read "—" after it, the dashboard drops the row
  once the account is gone (the chain decides, FE-D1), and a claim of a closed position is refused
  by the plan ("You have no lender position in this market …"; on chain a second `claim_sol` of it
  fails `AccountNotInitialized`, 3012). A review built before such a close — the claimant's own
  position closed from another tab or the dashboard, or a sync target closed meanwhile — fails its
  simulation with 3012; `tx/errors.ts` maps short.win's 3012 to a program error with the action
  *retry*, so the review's Retry rebuilds the plan from the chain (`plan.rebuild`: the closed
  target left out, a closed own position refused with the plan's message) instead of re-simulating
  the same one. *Ninth round (FE-30):* once someone sends lamports to such a closed position's
  address, the account there is a system account, and Anchor refuses it with
  `AccountOwnedByWrongProgram` (3007) instead; short.win's 3007 on the instruction's `lender`
  account (the log's "AnchorError caused by account: lender") maps the same way, while a 3007 on
  another account or from another program stays *unknown* (§5.9). The lend panel's read treats an account at the user's lender PDA that is not a
  position (a system account someone funded) as no position, so the panel loads and `deposit`
  takes the account over (`init_if_needed`). *M10 (KI-66 full, the lag-1 CU case, ARCH §6.1):* the
  user's own lender transactions carry at most 8 records (`MAX_RECORDS`), **7**
  (`MAX_RECORDS_CHAINED`) while its position has a forward chain (`fwd ≠ 0`; `recordLimitOf`) — the
  sync notice says how many per transaction. The previews map the chain's four fields
  (`toLenderState`): while a chain is active `snap` / `snapR` are the chain's snapshots, not the
  lender's own.
- **A conversion to no shares during a vest** (*M10*, owner decision of 2026-10-07, seventh
  round; ECON §9.5, known-issues KI-65): every lender transaction of the user's own — deposit,
  withdraw, claim, cover deficit, the signed `sync_lender` — first converts the position through
  the share epochs it is behind. When that conversion can take a position holding shares through
  rebase records only to 0 shares (it is worth less than one new share; whoever converts after
  it can receive the record's rounding unit) while the pool still has income vesting at the
  transaction's time, the review lists `lender.subUnitConversion` last, with the vest's end (UTC),
  advising to wait until then (`previewSubUnitConversion`; not shown with records missing, after
  a full wipe, or with the vest released). "Can": the review judges the worst case, the
  position's floor through the records with every `conv_rem` at 0, not the remainders it reads —
  other lenders' own conversions landing between the review and the transaction move them, and
  no instruction takes a minimum of shares (KI-66 review; KI-65's residual seen from the user's
  side).

### 3.7 Short panel (SHORT) — the open preview

Inputs: size `Q` in tokens (`shortQInput`), slippage (`shortSlippageInput`, default 1 %). Every
field below is recomputed on input and every new slot of the venue accounts (§6.5), from the
chain with `@shortwin/math-wasm` (§6.7), and shown in `shortPreview-<field>`:

| Field | Computation (as the program, ARCH §3.4) |
|---|---|
| `q` | the borrowed quantity (raw tokens) |
| `proceeds` (S) | `S_exp` = venue `sell(q, x, y, realQuoteCap)` (curve: `sellCurve`); `min_proceeds = ⌊S_exp·(10⁴ − slippage_bps)/10⁴⌋` |
| `margin` (M) | `marginD4(S_exp, B̂_spot, B̂_ema, cr_open_bps)` with `B̂_spot` the buyback quote on the post-sale reserves and `B̂_ema` on the newest ring entry's cached reserves; `max_margin = ⌈M·1.02⌉` |
| `open-fee` | `openFee(N_open, open_fee_bps)` with `N_open = B̂_spot` post-open; `max_open_fee = ⌈fee·1.02⌉` |
| `total-cost` | M + open fee + new-account rent (position; exposure on a first open) + network fee; the position's rent returns on close |
| `cr-open`, `cr-ema` | `crBps(vEff(S + M, 0), B̂_spot)`, `crBps(S + M, B̂_ema)` (≥ the tier's opening CR) |
| `liq-price` | `venue.capped(fee cap).liquidationPrice(q, v_eff, cr_liq_bps, L, P)` as SOL per token |
| `rent-rate` | APR after the open: `rateBps(base, slope1, slope2, utilizationBps(T − q, ΣB_Q + q), kink)`; per-day SOL at `P_ema` |
| `min-rent` | `minRent(q, P_ema, …, min_rent_s)` (1 h; Tier D 4 h) |
| `max-duration`, `deadline` | the tier's `max_duration_s`; `now + max_duration_s` (UTC) |
| `tier-terms` | the terms locked into the position at open: opening and liquidation CR, penalty, max liquidation slippage, rate curve, rent protocol share, max duration |
| `caps` | `maxQPosition(conservativeX0(…), impact_pos_bps)`, `oiCap`, `utilOk`, `walletOk` (25 % of the OI cap per key), `curveGammaOk` (Tier D), `notionalOk`; the binding cap first; `shortCapWarning` above it |

Blocked states (`shortBlocked`, `data-code` = the program error that would refuse the open) are
derived before simulation from the chain: paused opens, coverage pause and the 5 s after a clear,
route broken, venue disabled / upgraded / flagged, list-only, migrating, graduation halted, listing
window, market warm-up, EMA warm-up, boost in progress, mint unsafe, price deviates from EMA, no
inventory. Disclosures: `borrower.liquidation` with its required checkbox (placeholders from the
live tier terms), `borrower.residuals` (collapsed), `market.tierAge` (tier popover). Submit →
`open_short(nonce, q, min_proceeds, max_margin, max_open_fee)` with a random u64 `nonce` (the
position PDA is `["position", market, borrower, nonce]`); watched: the wallet's SOL, the new
position (`q`, `s`, `m`, `v`, deadline), the market's free tokens.

An idle market whose observations exceed the program's 40 s coverage gap offers an explicit
price refresh after its normal listing and venue warmup prerequisites pass. `short-price-refresh`
reviews one `crank_price`: first record an observation, then integrate it after a later confirmed
chain-clock second. Each step has its own simulation, fee review and wallet signature; no
background transactions. `short-price-refresh-step` carries `data-step=record|wait|integrate`.
Confirmed market accounts determine the next step, including after reload or another caller's
crank. New shorts wait for the stored EMA and completed observation to be fresh; all other guards,
including price deviation, still apply. Finishing a refresh review keeps the mobile trade sheet
open for the next step. The review watches wallet SOL and the recorded, completed and EMA-update
timestamps, and holds Sign if simulation did not perform the intended observation step.

*As built (M10-RESP, §7.6):* the submit sits in a sticky form foot (`FormFoot`) with the reason it
is disabled in one line (`short-submit-reason`; computed from the same predicates as the enabled
state: wallet, blocked market, size, slippage, quote, cap, acknowledgement, stale read, a review in
progress); while the acknowledgement blocks it, the reason is a button that focuses the box.
Disconnected, the foot offers `connect-cta-short` above the disabled submit. The preview header
keeps the mint's case and wraps, the read age on its own line. The review's position deadline is a
UTC date (`kind: 'timestamp'`) and its confirmed banner leads to the new position (`TxPlan.next`);
a confirmed open, close or add margin also pushes a toast (View position / Explorer).

*Design pass P6-1, P6-4 and P11 (as built):* the preview is the well for read-only figures
(`--surface-well`, edged with `--line-panel`); Total cost, CR at open (spot and EMA) and Liquidation
price are its emphasised rows (`data-emph`: `--fs-md` medium in `--text-0`, their labels white; §7.4
`Kv`), a value's explanatory sub-line is a second `<dd>` spanning the row under its number (the value
`<dd>` keeps its testid and `data-value`), a hairline opens the risk group (CR at open) and the terms
group (Rent rate) and one sits over the caps list, whose non-binding caps read muted while the binding
one keeps its amber — every cap stays exact. On a phone Size takes the sheet's width and Slippage is a
120 px box (`--field-narrow-w`) under it, as from 1,024 px (the 640–1,023 side sheet keeps the two side
by side). The Close form's "You receive" and "P&L" are emphasised the same way, its sub-lines second
`<dd>`s.

### 3.8 Positions, position detail, lender dashboard

- **Positions** (SHORT; SPEC §8): one row per position (`positionRow-<pubkey>`): market, `Q` owed,
  vault SOL (`v`), CR now (`crBps(vEff(v, R_due), B̂_live(q))` at the exit risk fee `min(φ_live, cap)`,
  `exitRiskCapBps`: the position's locked hard cap, on PumpSwap at most the tier at `P_ema` as the
  program's `liquidate` and `sol_settle` price it, AIR-10; the CR on the ring and the liquidation
  price take the same cap), liquidation price, P&L if closed now
  (`previewClose(v, B̂_live, R_due, rent_protocol_bps).borrower − (m + margin_added + fee_open)`),
  rent accrued (`PositionRent.due`), time left (`Countdown` to `deadline`), risk badge
  (`riskLevel(CR, CR_ema, cr_liq_bps)`: safe / watch / at risk / at risk (live price below) /
  liquidatable), status; actions add margin,
  close. Closed positions show the exit (`exit` of the API): kind, condition, payout, P&L. Live:
  `position:<pubkey>` per open row (≤ 40), else a 5 s refetch.
- **Position detail** (SHORT): the row's fields, locked terms, arming state (`armed`), the event
  timeline (`/v1/positions/{pubkey}`); **add margin** (`addMarginInput`, CR after →
  `addMarginPreviewCr`, `add_margin(amount)`); **close** (`closeSlippageInput`; preview buyback
  `B̂_live`, rent, payout, P&L; `close_short(max_buyback_cost = ⌈B̂·(1 + slippage)⌉)`; `InsufficientVault`
  → suggest add margin; past a live curve's `rT` the same preview at `B̂_v3`, §5.11; FE-3 closed).
- **Positions and position detail as built (M10-RESP, §7.6):** the dashboard leads with a totals
  line (`positions-totals`: count, vault, P&L, rent due) and folds the risk explanation into a
  "How risk is graded" toggle with the real badges. Column priorities: 1 market, risk, CR, P&L,
  actions; 2 liq. price, time left, vault, Q owed (and status for "All"); 3 rent (and status under a
  single-status filter). Under 560 px of table width each row is a card (market and risk badge on
  top, the numbers in pairs, Add margin / Close as a button row); the row actions link to the
  position with `?sheet=add-margin|close`. Disconnected: an empty state with
  `connect-cta-positions` (the framed block of §3.10), no table; no open positions: "Open a short"
  (`positions-empty-cta`). The position page reads the position account only when it is a
  short.win `ShortPosition` (owner, discriminator, size); otherwise a not-found state with "Back to
  positions" (`position-not-found-cta`). It shows the health banner (its tone follows the risk) and a
  hero line (CR, P&L, time left) before the details; under 1024 px the Manage panel is a sheet that
  the action bar (`position-action-add-margin`, `position-action-close`) or `?sheet=` opens at the
  chosen form with its heading focused, and ✕ (`manage-sheet-close`), Escape, the scrim or Back
  close; from 1024 px it stays in view beside the details. Each form's submit is in a sticky foot
  with its reason (`add-margin-reason`, `close-reason`).
  *Design pass P6 and P11 (as built):* the totals are a KPI strip spanning the table's width (the
  lender strip's pair: a label over a `--fs-lg` number per cell, hairlines between, the first cell
  "1 OPEN" inline with its number; 2 × 2 on a phone, two cells under the Closed / Liquidated / Expired
  filters), its SOL padded like the table's; the position page's CR is the hero (`--fs-hero`, the
  largest thing on the page) and the title's account id a 12 px regular grey sub-element
  (`.position-id`; its own line on a phone); the Manage panel shares `.layout-side` / `.side-sticky`
  with the trade panel (§7.6), a hairline between its two forms with 16 px above and under it, and
  its last foot alone casting `--shadow-up`; the facts and the timeline sit on the 28 px grid
  (`--kv-row-h`) and do not answer the pointer; the status badges are `.badge-status` (capitalised);
  RISK stays on one line from 1,280 px; vault and rent are padded to their precision (P6-5). The
  no-wallet and not-found states are `Empty` blocks (§3.10).
- **Lender dashboard** (LEND; `/lend`): per market (`lenderRow-<market>`): shares and share of the
  pool, token value (`previewTokenValue`), SOL claimable (`previewClaim`), vesting, lock and
  `lock_until`, creator badge; claim per row and "claim all" (one transaction per market; each
  claim syncs the other lenders it needs first, as Lend → Claim does, §3.6).
  *As built:* plus the share-epoch standing and the indexer's view per row (`lender-epochs-*`,
  `lender-indexer-*`), a per-row `sync_lender`; while the indexer fails, the rows come from
  `getProgramAccounts` on the owner's `LenderPosition`s (memcmp on the owner, `dataSize` 241).
  *M10-RESP (§7.6):* a KPI strip — markets, total value (≈ SOL at each market's indexer EMA price,
  display only; "k of n priced" when a market has none), SOL claimable, rent credited (the
  positions' credited entitlement); the data source is a badge (the freshness badge, or "Chain only"
  while the indexer is unavailable). The epoch standing and the indexer's view share one "Sync"
  cell that shows a badge only when one is not current (priority 2, never hidden). Each row links
  Withdraw / Deposit to the market's lend panel (`?panel=lend&action=…&sheet=trade`: the trade sheet
  under 1024 px), and a disabled Claim says why in text. "Claim all" (with the total) is shown when
  a row's Claim is open (something claimable, or syncs that can add 10,000 lamports or more since
  the thirteenth round's decisions): in the page header on a desktop, in the action bar under 1024 px. Under
  560 px of table width the rows are cards (market and value on top, the actions a button row).
  Without a wallet: Connect wallet; with no positions: Browse markets.
  *Design pass P7 and P11 (as built):* the KPI strip spans the page (four equal cells from a tablet
  up, 2 × 2 on a phone) with SOL claimable as its one hero (`Stat` `hero`: `--fs-xl` medium; `--fs-lg`
  under 400 px so a 2-up cell never clips — an owner item, §7.6), the number the page's only primary
  action repeats; the claimable cell is padded to its row's precision (`solFrac`, 6 or 9, so
  decimals align within rows of equal precision only) and the vesting cell at 6; a card's hero is CLAIMABLE (`--fs-lg`
  medium); the no-wallet state is the shared framed block with its 360 px Connect wallet (§3.10;
  lend.css's own copies are gone, P7-3).
  *Thirteenth round (the owner's decision of 2026-10-11; audit finding D-21):* **a row's Claim is
  the Lend → Claim tab's.** Each row carries the panel's claim view (`previewClaimView`:
  claimable now and the shortfall, 0 under the 10,000-lamport dust threshold, §3.6) and runs the
  panel's search for syncs (`claim/syncSearch.ts`; the same cache, so a row and the tab share a
  market's result; at most two searches at once app-wide, so a lender in many markets queues them
  instead of a request storm; none while the indexer is down); its Claim is enabled whenever the panel's would be — something
  claimable now **or** syncs that can add 10,000 lamports or more (*only when syncs can pay*, the
  owner's decision of 2026-10-11, §3.6) —, builds the same claim (`claimPlanFrom` with the indexer's
  lenders, taking the row's pick while it is fresh) and reads the panel's label (`ClaimLabel`:
  "Review claim · {c} SOL", or the two lines of `claim.label.syncs` with {g} = min(shortfall, the
  syncs' gain) and `claim-submit-syncs` inside the row), "Claim" only until the row's chain is read;
  while its search runs with nothing claimable now it is disabled and busy without a reason; when
  disabled, "Nothing to claim yet" says why, as before. "Claim all" keeps
  its label and its total and claims the same rows, each the same way: one predicate decides both
  a row's Claim and Claim all's count — the Claim is open and the row is not behind the share epoch
  with records still to read (such a row shows Sync instead; one behind with every record read
  claims, and counts) — so a row whose shortfall no sync can fund, or whose search still runs with
  nothing claimable now, is neither counted nor claimed. The total (and the SOL claimable KPI) is
  what is claimable now, so when every enabled row pays only through its syncs the enabled button
  reads "Claim all · 0 SOL" — a known consequence of the decisions (it does not overstate what is
  paid; the syncs add to it, and each review shows what its claim pays). No new string. The action cell fits at every width:
  the two-line label made it the row's widest cell, and on one line it pushed the table past its
  frame (32 px at 1,024 px, 40 px at 1,280 px, the Claim clipped), so where the table runs out of
  width the actions wrap — Claim under Withdraw and Deposit, each label line kept whole — and at
  1,440 px they stay on one line; on a card (under 960 px of table width) the button row wraps as
  it needs (a phone puts Claim on a row of its own). The responsive gate measures the dashboard with that label at all eight
  viewports (`lender-dashboard-syncs`, a second lender with a surplus; and
  `lender-dashboard-shortfall`, the pot market's only lender, now the disabled state; §10.5: no
  table scrolls sideways or clips); E2E `lend-claim-sync` shows the only lender's shortfall as the
  disabled "Review claim · 0 SOL", then claims a row with nothing claimable through the syncs.
  *The owner's decision (2026-10-11, after the approval's review): only when syncs can pay.* The
  shortfall (pending − own − pot) also counts rent credited but not yet paid in by open shorts,
  which no other lender's sync can fund; where no other lender holds a surplus — always for a
  market's only lender — a row and Lend → Claim with nothing claimable now used to show the enabled
  two-line label once the shortfall passed 10,000 lamports, and the claim then paid 0. The owner
  chose to cap {g} by what the syncs the search picks can move: such a claim is now the disabled
  "Review claim · 0 SOL" with "Nothing to claim yet" (§3.6), and a review whose simulation would
  still pay 0 is not offered for signing.
- **Risk levels** (as built; *v1.2, M9-UX-4*): `riskLevel(crBps, crEmaBps, liqCrBps)` returns
  `safe` / `watch` / `danger` / `below` / `critical` — the values of `data-risk` — shown as Safe /
  Watch / At risk / At risk (live price below) / Liquidatable. It judges as the program does
  (`shortwin-math risk::cr_below`): `critical` only when the CR is **strictly** below the line on
  **both** the live and the smoothed leg, `below` when only the live leg is; `danger` / `watch` are
  within 10 % / 30 % above it on the live leg. The exit state (`SettlementRisk` `exitState`, the
  badge's tooltip) is derived from the same function, so the two never disagree. A single CR figure
  (a preview's CR at open) is coloured by `crBand` and never reads "Liquidatable".
  *Past the deadline (the second approval pass, 2026-10-11):* the exit state names the exit the
  program would take now, as `borrower.liquidation` ¶3 as amended says (SPEC §5.1, §5.3; `expire.rs`
  and `liquidate.rs` `plan`, `sol_settle.rs`). It used to return `expired` before checking anything
  else, so a position past its deadline that can be liquidated read "Past its deadline: anyone can
  close this position now, with a {X} expiry fee instead of the penalty." while the program
  liquidates it with the penalty. Now, past the deadline: no route (`!venue.executable`) →
  `no-route` (`liquidate` and `expire` both refuse first with `NoRoute`; S3 counts the expiry as
  liquidatable, `settle::s3_due`); armed (`pos.sArmed`, the S1 / S2 clock `sol_settle` persisted, on
  an executable venue) → `armed` (`sol_settle` keeps that clock past the deadline: it disarms only a
  position neither expired nor liquidatable). The arm is persisted evidence, not the program's live
  refusal: `liquidate` and `expire` refuse with `UseSolSettle` only while the live buyback check fails
  (`B̂_live + ert ≤ V − K_s ∧ σ_live ≤ σ_max`, computed at the call, `s_armed` never read), so a
  keeper may still expire or liquidate an armed position whose live buyback is allowed again. The
  client keeps `armed` first (the choice of the second approval pass's review): it cannot judge S2's
  live slippage, the banner's "Settlement in SOL being considered" stays true while the clock runs,
  and a proxy on S1 alone (`!insufficientVault`) would name an exit the program may still refuse;
  CR-liquidatable on both legs (`riskLevel` `critical`) → `liquidatable`
  (`expire` refuses with `UseLiquidate`, `liquidate` takes the penalty); otherwise `expired`. Before
  the deadline the order is unchanged (armed, no route, then the CR states). No string changed:
  only which existing text shows — the position page's health banner and the Positions page's badge
  tooltip (`exitShort`) follow the same function; the Positions page's status filter reads the
  indexer's status of closed positions (`expired` = closed by `expire`) and does not use it
  (`positions.test.tsx` at each boundary).
- **Which market prices a position** (*v1.2, M9-SEC-2*): the chain's. The live columns use the
  snapshot of the mint of the `Market` the position account names (read from the chain); a
  snapshot of another market is refused. When the indexer's market or mint for a row disagrees,
  the row carries `position-data-mismatch` and the page a banner; the position page reads the
  `Market` at `position.market` whatever the indexer says.

### 3.9 Protocol status (MARKETS)

Global pause (`/v1/admin/config.paused_opens`), the venue programs (`/v1/venues.guards`: name,
upgraded latch, guard slot vs ProgramData slot, flagged at; enabled bit; route-broken bit), the
extras sets in force, markets in degraded mode (coverage-paused) and with route-broken or a running
no-route chain (FE-5), the indexer's health (`/v1/status`: lag, quarantined, missing, truncated
logs), the keeper coverage totals (`/v1/stats.coverage_gap_s`).
*M10-RESP:* "New shorts" is a full-width row at the top; the other panels sit side by side where
420 px fit (`minmax(min(420px, 100%), 1fr)`), 12 px apart, each as tall as its content (*design pass
P5-8:* a stretched frame left empty panel; rows of equal height before); the tables are DataTables
(cards on a phone). *P5-8:* "Open" / "Paused on every market" is a 13 px badge beside its prose, the
indexer state a capitalised `.badge-status` (Live / Lagging / Stale / Unavailable), and every slot —
confirmed, finalized, guard, deploy — is grouped with commas (`formatSlot`).

### 3.10 Loading, empty and error states

Skeleton rows while loading (never a spinner over a table); empty states say what would fill them
and offer the next action; errors show the mapped message with Retry (`ErrorState`); 404 from the
indexer for a market that exists on chain renders the chain view (FE-D1).
*Design pass P8-3 and P4-3 (as built):* an `Empty` with a title and an action (no wallet, Not found,
no positions) is one framed `--bg-1` block edged with `--line-panel`, at least 200 px tall with its
lines centred and its action row spanning it (a Connect wallet 360 px wide, the whole row on a
narrower phone); a table's empty line stays centred and muted in place of the rows and hides the
table and its sort bar with it (the axe fix: nothing is left for axe to require rows of, and the sort
buttons are no Tab stops). `ErrorState` is a left-bar note in the danger tint (`--text-0` on
`--danger-bg`) with Retry beside the message, an unbroken token wrapping instead of pushing the box
past 320 px; in a flush panel or tab body it keeps the body's 12 px inset (P11).

### 3.11 How it works (GUIDE; the owner's thirteenth round, O-3 (c))

`/how-it-works`, `HowItWorksPage` (`features/guide`), its own chunk; no wallet. Centred at 760 px.
The page is `config/copy.ts`'s `HOW_IT_WORKS`, top to bottom: the lead, an "On this page" contents
nav, then Shorting a token (`#shorting`), Lending a token (`#lending`), What can go wrong
(`#what-can-go-wrong`: If you short `#short-risks` — `borrower.liquidation` expanded with Tier C's live
terms and a caption, `borrower.residuals` collapsed `#borrower-limits`; If you lend `#lend-risks` —
`lender.risks` and `lender.solRepayment` expanded, the late-depositor shortfall, `lender.residuals`
collapsed `#lender-limits`; In every market — `market.coveragePause`, `market.venueTrust` collapsed),
Fees and rent (`#fees`), Before you sign (`#before-you-sign`) and Read more (`#read-more`). The
embedded disclosures are verbatim, without a checkbox (nothing is signed there; §4.1 rule 3).
Governable numbers are placeholders filled from `GlobalConfig` (`guideVars`: the open fee, the rent's
protocol share, the penalty split, the expiry fee, the settlement keeper reward, the creation fee,
Tier C's opening CR, the tiers' shortest and longest duration); until it is read each shows a
skeleton, never a word. An anchor in the URL (the intro strip's risk links) scrolls to its heading
and focuses it once the numbers are in; a link to a collapsed disclosure opens it. Linked from the
intro strip, the status bar and the phone's More sheet (`page-how-it-works`, `how-it-works-section-<anchor>`).

*Design pass P8 (as built):* a 15 px lede (`--fs-lg`, `--text-1`) on a 60ch measure, 14 px / 1.55
paragraphs and lists (`--fs-prose` / `--lh-prose`; 15 px on a phone) with the embedded risk boxes at
the prose size, 18 px sections (`--fs-xl`; 18 px on a phone too, under the 20 px title — P11) with
32 px above and sub-headings with 16 px above; the contents box is edged with `--line-panel`, wraps
into rows of 44 px links on a phone so the first section starts on the first screen, and from
1,280 px is a sticky 200 px rail beside the body (a grid; DOM order kept, no `order`; the first
section's hairline level with the rail's top). Security reads the same way (the lede, the prose tier,
16 px above a sub-heading; the Scope note keeps its 12 px).

---

## 4. Disclosures

### 4.1 Rules

1. The copy below is **exact**. Components render it only through `<Disclosure id=… />` from
   `src/config/disclosures.ts`; no component paraphrases, truncates or restyles a disclosure into
   other words. `{name}` placeholders are filled with formatted values; a missing value throws (it
   never renders `{name}`).
2. SPEC §5.3's sentence is verbatim, on the deposit screen, **before signing**, with a required
   checkbox whose label is the sentence itself; the review modal repeats it and the Sign button
   stays disabled until it is ticked (`canSign`).
3. Disclosures with an acknowledgement text gate the Sign button of the flow that lists them in
   its `TxPlan.disclosures`. Acknowledgements are per transaction (never remembered).
4. Changing a text is a contract change: edit `disclosures.ts` and this section together
   (`disclosures.test.ts` fails otherwise).
5. Numbers quoted in disclosures come from SPEC / ECON / ARCH as cited in *Source*; every number
   governance can change (ECON §15 bounds) is a placeholder: the live tier terms and `GlobalConfig`
   parameters fill it on the panels, the terms a position locked at open fill it on that position's
   pages (*v1.2, M9-UX-3:* `{expiryFee}`, `{s3Wait}`, `{keeperReward}`, `{vestPeriod}`,
   `{lockGrace}`; `{maxDuration}` of an open position is its own `deadline − t_open`; filled by
   `src/config/disclosureVars.ts` and the features' `*DisclosureVars`).

### 4.2 Catalogue

#### `lender.cleanup` — Clean up pending lender income

*Severity:* info · *Acknowledgement:* none · *Source:* AIR2-05; ARCH §3.3, §3.9
*Shown:* Deposit when cleanup is pending with surviving shares, and its separate review.

> This transaction collects pending income for the pool’s existing lenders. You pay its network fee and any account rent needed to claim venue cashback.

> Your tokens stay in your wallet. Once cleanup confirms, deposits wait until any income still vesting has released. You review and sign your deposit separately.

#### `lender.solRepayment` — You may be repaid in SOL

*Severity:* danger · *Acknowledgement:* required · *Source:* SPEC §5.3 (verbatim, required checkbox)
*Shown:* Lend → Deposit, after the preview and the risks, right before the submit (thirteenth round, O-1), and in the review modal (required checkbox before signing)

> If a borrower can't be bought back, you will be repaid in SOL, which may be worth less than the tokens you lent.

Checkbox: “If a borrower can't be bought back, you will be repaid in SOL, which may be worth less than the tokens you lent.”

#### `lender.risks` — How lending works here

*Severity:* warn · *Acknowledgement:* none · *Source:* SPEC §2.4, §5.4, §7; ARCH §3.3, §7.3; ECON §9.3–§9.5, §15
*Shown:* Lend → Deposit (always visible, collapsible after first view per wallet), lender dashboard header; `{lockGrace}`, `{vestPeriod}` from `GlobalConfig` (`lock_grace_s`, `lump_vest_s`)

> Lending is not staking and not risk-free. You earn rent from borrowers who short this token.
>
> Losses stay in this market: if a position cannot be bought back, every lender in this pool shares the loss pro-rata. There is no insurance fund and no treasury backstop.
>
> You can withdraw only from tokens that are not lent out, as a pro-rata mix of tokens and SOL. While tokens are borrowed, part of your shares can stay locked until the positions they stand for have expired, plus up to {lockGrace}.
>
> Rent is credited continuously. One-off income (your share of liquidation penalties, expiry fees, settlement proceeds above market value and swept balances) vests over {vestPeriod}.
>
> SOL that arrives while this market has no lenders goes to the protocol.

#### `lender.lossHistory` — What gaps cost lenders

*Severity:* info · *Acknowledgement:* none · *Source:* ECON §11.6 (alpha = 2 column); INDEXER §8.4
*Shown:* Lend → Deposit and market page → Lenders tab; `{tier}`, `{breakEvenDays}` (ECON §11.6 α = 2: A 535, B 205, C 84, D 28), `{settlements}`, `{realisedLoss}` from `/v1/markets/{mint}`

> If the price jumps past a position faster than it can be liquidated, the lenders are repaid in SOL worth less than the tokens. In the protocol’s model, lending in Tier {tier} breaks even if such a jump happens on this token less than about once every {breakEvenDays} days. These figures are illustrative and not yet calibrated on real data.
>
> This market so far: {settlements} SOL settlements, realised lender loss {realisedLoss} SOL.

#### `lender.exitForfeitsVest` — Unvested income stays in the pool

*Severity:* warn · *Acknowledgement:* required · *Source:* ARCH §3.3 (last lender, R4B-4)
*Shown:* Lend → Withdraw, when the withdrawal leaves shares behind and the vest is unreleased (`previewVestRelease`); `{vesting}` in SOL; required checkbox

> You have {vesting} SOL of income still vesting. If you withdraw now, the unvested part stays with the lenders who remain.

Checkbox: “I understand I give up my unvested income.”

#### `lender.residuals` — Known limits (accepted by the protocol)

*Severity:* info · *Acknowledgement:* none · *Source:* ARCH §8.5, §10 O19 (OWNER-M7-11, R4B-10); ECON §8.2, §9.3, §9.5 (KI-65, KI65-REV-SEC1), §12.9–§12.11; owner-selected policies of 2026-10-21 (AIR2-P2-02, AIR2-12, AI3-F06, AI3-L10)
*Shown:* Lend → Deposit (collapsed `<details>`), FAQ link. *v1.2 (M9-UX-2):* the three O19 residuals that fall on lenders — the liquidation that reserves nothing (ARCH §8.5's Tier A figures), Raydium pool pinning by a standing minimum position (R4B-10), the notional-cap exceedance of M7-SEC-4 in a lender's framing — were missing and are added. *M10 (owner decision of 2026-10-07, seventh round: approved):* the last paragraph, KI-65's conversion-order residual after a rebase (known-issues KI-65). *FE-31 (approved by the owner on 2026-10-13):* the liquidation paragraph's Tier A figures are the held model's under AIR-22's lower-median growth (ECON §8.2, §12.6; `sim/out/attacks/sandwich.out`, no reserve: fresh +3.340, aged 25 d at 90 % +4.543 SOL; before AIR-22 2.06 / 8.02 SOL)


> If lenders withdraw ahead of a loss, those who leave first can take more than their fair share and those who stay less: about 4% of value at 90% utilisation, up to about 11% at 50–60%.
>
> A borrower can pay to delay its own liquidation by disturbing the price check every second. It only delays, at the borrower’s cost.
>
> Settlement amounts can be pushed by up to ±1%, and a settlement can be delayed (about 13 seconds per second an attacker controls the price observations, or indefinitely at a cost paid every second).
>
> If the keeper loses coverage, 5 consecutive seconds of attacker-controlled observations can turn a capped settlement into a full one.
>
> Small positions may need short.win’s keeper as a backstop, which needs operating funds. A liquidation pays its keeper only from equity left after the buyback and rent. When enough remains, it pays at least the SOL-settlement keeper reward; otherwise it can pay less or zero. Timely execution by a third-party keeper is not guaranteed.
>
> A liquidation sets nothing aside for the lenders: when buying the tokens back uses up the position’s vault, the lenders and the protocol lose the penalty and, on a position open for a long time, the rent the vault no longer covers. In the protocol’s Tier A model this is about 3.34 SOL on a fresh position and about 4.54 SOL on one open for 25 days at 90% utilisation.
>
> A minimum-size position held open can stop a Raydium market from moving to a deeper pool, so the market can stay on a thinner pool than it could use. The emergency multisig can pause new shorts to clear it.
>
> A sale bundled into an open can make that open record up to 10% less notional than an honest quote, so the market’s notional cap — the limit on how much can be shorted while a market is new — can be exceeded by up to about 11%.
>
> Tiny dust amounts can be left behind when a pool rebases.
>
> After a pool rebase, only you can convert your position, but a position worth less than one new share can convert to none when you next claim, deposit or withdraw. A lender converting after you, at any later time, can receive that share, and if that leaves it holding every share, its exit takes the income still vesting.
>
> Claims use the SOL available to each position and a shared market pot. Claiming from that pot earlier can leave less cash for later claimants, even when both have credited income; unpaid rent does not guarantee an equal cash payout. Some credited income may never be paid if the shortfall is never covered.
>
> Venue cashback sent outside the protocol’s supported collection accounts may remain unrecoverable. Do not count it as guaranteed lending income.
>
> The lender who withdraws the final shares receives the remaining vesting income and whole-lamport rounding carry. These amounts can include income from before that lender joined; other lenders’ recorded claims are not erased.
>
> While deposits wait for old income to vest, further external donations can extend that wait. There is no guaranteed reopening time while donations continue; the hold protects existing lenders’ income.

#### `lender.unpaidRent` — Rent this pool has not been paid yet

*Severity:* warn · *Acknowledgement:* none · *Source:* ECON §9.4 (a shortfall is debited pro-rata to current shares); the owner’s decision of 2026-10-03 (*M10*)
*Shown:* Lend → Deposit when the pool has unpaid rent at the deposit (`Market.receivable` plus the credit of the deposit's prologue > 0): collapsed, expanded when this deposit's part is material (≥ 1 % of its value at `P_ema` or ≥ 1 SOL), and then also in the review; `{receivable}`, `{exposure}` in SOL (9 decimals), `{share}` this deposit's shares' share of the pool after it

> This pool has credited {receivable} SOL of rent that borrowers have not paid yet; it is paid when their positions close. If a position closes without enough SOL left to pay its rent, the unpaid part is taken from the lenders pro-rata to the shares they hold at that moment — including lenders who joined after the rent was credited.
>
> At the pool’s current size, this deposit would carry {exposure} SOL of it ({share} of the pool from this deposit). If other lenders withdraw first, that share — and this deposit’s part of such a loss — grows.
>
> Such a loss is taken from your SOL balance in this pool. If it leaves that balance below zero, you must pay the difference in SOL (Cover deficit, on the Claim tab) before you can withdraw your tokens.

#### `lender.subUnitConversion` — Your position may convert to no shares

*Severity:* warn · *Acknowledgement:* none · *Source:* ECON §9.5 (KI-65, KI65-REV-SEC1); the owner’s decision of 2026-10-07 (seventh round); the text approved as written by the owner on 2026-10-08 (ninth round) (*M10*)
*Shown:* in the review of the user's own lender transaction — Deposit, Withdraw, Claim, Cover deficit, Sync position — when the conversion it runs first can take a position holding shares through rebase records only (`1/1,000^k`) to 0 shares while the pool still has income vesting at the transaction's time (`previewSubUnitConversion`: the position's floor through the records, `⌊s/1,000^(Σk)⌋`, is 0 — the worst case over the other lenders' conversions, which can land before the transaction and move each record's `conv_rem`; no instruction takes a minimum of shares — the WASM `previewTouch` accepts the records at hand, and the unreleased vest `vest_bal − previewVestRelease` > 0); last in `TxPlan.disclosures`; `{vestEnd}` `Market.vest_end` in UTC

> This pool’s shares were rebased since your position last changed, and your position is worth less than one new share: this transaction can convert it to 0 shares. Whether it does depends on other lenders’ conversions that land before it, which this review cannot see. A lender converting after you can receive that share.
>
> Income is vesting in this pool until {vestEnd}. A lender that ends up holding every share is paid all of the income still vesting when it leaves. Unless you need to act now, we advise waiting until {vestEnd} before you claim, deposit or withdraw here.

#### `lender.potSyncs` — Other lenders are synced first

*Severity:* info · *Acknowledgement:* none · *Source:* ECON §9.4 R3A-2, §9.5; ARCH I-P6, §3.3 sync_lender (KI-66 full, KI66-R4); the owner’s decision of 2026-10-08 (eighth round); the text approved as written by the owner on 2026-10-08 (ninth round) (*M10*)
*Shown:* in the claim's review when the plan prepends other lenders' `sync_lender` (§3.6 Claim, KI66-R4); `{count}` the syncs in this transaction, `{lenders}` "lender" / "lenders", `{gain}` what `previewTouchPot` says they move to the pot, in SOL (9 decimals)

> This market’s pot cannot pay all of the SOL credited to you yet. This transaction first syncs {count} other {lenders} of this market whose realised SOL is more than their own credit; the excess, {gain} SOL by this review, moves to the pot and your claim draws on it.
>
> A sync you send for another lender never converts its shares through a share rebase: behind one it only books what the position has earned and moves the excess, and the conversion stays with its owner. A sync that finds nothing more to move moves nothing.

#### `lender.potSyncsDeferred` — Not every sync fits this transaction

*Severity:* info · *Acknowledgement:* none · *Source:* FRONTEND.md §3.6 (KI66-R4: at most 4 syncs per claim, 1,232 bytes per transaction); the text approved as written by the owner on 2026-10-08 (ninth round) (*M10*)
*Shown:* in the claim's review when the pot still cannot pay the claim with the syncs it carries and other lenders' syncs with a gain were left out — beyond the cap of 4 (`choosePotSyncs`' `more`, among the lenders the claim read: ≤ 200, FE-30) or dropped to fit 1,232 bytes (a sync the pre-review simulation failed on is not counted: it was dropped as invalid, not deferred); `{count}` / `{lenders}` those lenders, `{gain}` what they would move, in SOL

> Syncing {count} more {lenders} would move about {gain} SOL more to the pot, but one transaction carries at most four syncs within its size limit. This claim pays what it can now; claim again once it has landed, and the next claim syncs them first.

#### `lender.claimCloses` — This claim closes your position

*Severity:* info · *Acknowledgement:* none · *Source:* ARCH §3.3 claim_sol (KI-67); the owner’s decision of 2026-10-08 (eighth round); the text approved as written by the owner on 2026-10-08 (ninth round) (*M10*)
*Shown:* in the claim's review (Lend → Claim and the dashboard's claims) when `claim_sol` leaves the position closable (`previewClaimOutcome`: current, 0 shares, `pending` 0, no lock, no forward chain; the pot raised by the plan's syncs); before `lender.subUnitConversion`; `{rent}` the account's rent-exempt minimum in SOL (`LENDER_POSITION_RENT`, 0.00256824)

> After this claim your position in this market holds no shares and no SOL, so the claim closes it and returns its account rent ({rent} SOL) to your wallet. Depositing here again later opens a new position.

#### `borrower.liquidation` — How this short can end

*Severity:* danger · *Acknowledgement:* required · *Source:* SPEC §4, §5, §5.3, §6.1; ARCH §3.4–§3.8; ¶3: `expire.rs` (precondition: **not** CR-liquidatable, else `UseLiquidate` — an expired position that can be liquidated pays the penalty through `liquidate`, ECON §8.4); *¶3 amended by the owner on 2026-10-11 (thirteenth round)*: "Unless it can be liquidated at that point," added before the expiry fee (it read "You pay a {expiryFee} expiry fee instead of the penalty."); the amended text approved as written
*Shown:* Short panel above the submit button and in the review modal (required checkbox); placeholders from the live tier terms and parameters (`GlobalConfig`: `{expiryFee}` = `expiry_fee_bps`, `{s3Wait}` = `s3_wait_s`); on the position page from the position's locked terms (its `expiry_fee_bps`, `s3_wait_s`, `{maxDuration}` = `deadline − t_open`)

> Your collateral is your margin plus the sale proceeds, locked in the position’s vault. Collateral ratio (CR) = vault SOL ÷ the SOL needed to buy the tokens back now, including price impact.
>
> Liquidation: if CR falls below {liqCr} on both the live price and the smoothed price, anyone can liquidate. The tokens are bought back with your vault SOL, a {penalty} penalty is taken, and the rest comes back to you.
>
> Expiry: after {maxDuration} the position can be closed by anyone. Unless it can be liquidated at that point, you pay a {expiryFee} expiry fee instead of the penalty.
>
> Settlement in SOL, when the tokens cannot be bought back:
>
> S1 — the price jumped past your vault: the lenders take all vault SOL minus the keeper reward. You receive nothing.
>
> S2 — buying back would exceed the maximum liquidation slippage ({maxSlippage}): the lenders take the vault SOL minus the keeper reward, but no more than fair value (the buyback at the smoothed price plus the penalty and rent; the {expiryFee} expiry fee instead of the penalty for an expired position). Anything above that comes back to you.
>
> S3 — no route to buy back for {s3Wait} while the position is liquidatable (venue halted, pool drained, migration stuck): as S2.
>
> Rent accrues every second on the borrowed tokens’ value; at least {minRent} of rent is charged.

Checkbox: “I understand my position can be liquidated or settled in SOL.”

#### `borrower.tokenRepay` — Repay the full debt with your tokens

> This transaction returns the full debt of {q} tokens ({qRaw} raw units) from your selected token account. It does not buy tokens or use a live swap quote.
>
> You authorize at most {maxRent} SOL ({maxRentLamports} lamports) of accrued rent from the position vault. The rest of the vault and refundable account rent return to you; you also pay the network fee.
>
> The cost of obtaining these tokens outside short.win is unknown. The returned SOL is not a profit figure, so economic P&L is unavailable for this repayment.

Checkbox: “I understand that the full token debt will be repaid and external token acquisition costs are not included.”

The borrower’s token-repayment review shows the exact raw debt, selected account,
explicit rent limit and simulated source/pool deltas. Its data read uses the
position, market, mint and chain clock without a venue or oracle quote. The rent
limit starts empty, cannot exceed the position vault, and is preserved on retry;
there is no automatic increase. Account token-program/owner/mint/balance checks,
a wallet SOL check, the simulated network-fee budget and the normal simulation, acknowledgement and explicit
wallet-signature pipeline remain required. CPI Guard accounts must be disabled
by their owner outside this flow or another source account selected. This is a
borrower repayment path; keeper liquidation is a separate integration.

#### `borrower.residuals` — Known limits (accepted by the protocol)

*Severity:* info · *Acknowledgement:* none · *Source:* ARCH §8.5, §10 O19 (OWNER-M7-11); ECON §8.2, §12.2, §12.5, §12.6
*Shown:* Short panel (collapsed `<details>`), position detail; `{keeperReward}` from `GlobalConfig` (`settle_keeper_fixed`, `_bps`, `_cap`) on the panel and from the position's locked terms on its page, `{maxDuration}` as for `borrower.liquidation`


> A keeper that liquidates you may pay up to 2% more than the protocol’s reference price for the buyback; that comes out of what is returned to you.
>
> If someone holds the price up while your position is being liquidated, the keeper can take most of what would otherwise come back to you.
>
> If enough equity remains after the buyback and rent, a liquidating keeper receives at least {keeperReward}, with any top-up taken from what would come back to you. Otherwise the reward can be smaller or zero; timely execution by a third-party keeper is not guaranteed.
>
> If the venue halts or breaks, a healthy position can be neither closed nor settled until the route returns, an adapter upgrade lands (48-hour timelock), or the position reaches its deadline (up to {maxDuration}).
>
> A sale bundled into an open can make that open record up to 10% less notional than an honest quote, so a market’s notional cap can be exceeded by up to about 11%. Your margin is not affected.
>
> On low-fee Raydium pools, a basket of positions at the market’s open-interest cap can still be profitable to manipulate.

#### `market.creatorSelfDealing` — The token’s creator can be on both sides

*Severity:* warn · *Acknowledgement:* none · *Source:* SPEC §7, §12 R1; ARCH §7.3 (C0-18), §8.5; ECON §12.8
*Shown:* Market page → Overview (always), Lend → Deposit (collapsed); `{creatorFee}` = `creator_exposure.creator_fee_income_lamports` in SOL

> The creator of this token can lend its own supply here and short it from another wallet — in effect selling into the market. If the token then rises, the creator may be repaid in SOL on its own lending.
>
> Deposits from the creator’s wallet are shown on this page. Other wallets the creator controls cannot be identified, and the per-wallet cap does not stop one person from using several wallets.
>
> The creator also earns the venue’s creator fee on every short.win trade of this token: {creatorFee} so far.

#### `market.tierAge` — Why this tier

*Severity:* info · *Acknowledgement:* none · *Source:* SPEC §4 (O13, O13b)
*Shown:* Market page → tier badge popover (“Why this tier”) and Short panel tier terms

> Tiers are set automatically from on-chain liquidity and pool age, and checked again on every trade.
>
> short.win can only prove a pool’s age from the moment it first saw it: the real graduation time for tokens listed before they graduated, otherwise the listing time. A Raydium CPMM pool created after mid-2024 records its own creation time, which is used instead.
>
> An older pool listed today therefore starts in Tier C for 24 hours and is at most Tier B until day 7. This is conservative: higher margins, smaller caps and shorter durations.

#### `market.coveragePause` — New shorts paused: keeper coverage

*Severity:* warn · *Acknowledgement:* none · *Source:* SPEC §11 (O17, M7-3, M7-13/14)
*Shown:* Market page banner and Short panel (replaces the form; while the page shows the banner, the panel links to it instead of repeating it — thirteenth round, O-2) while `flags.paused_coverage`; status page

> short.win’s keeper cannot currently cover this market (degraded mode), so new shorts are paused.
>
> Closing, adding margin, liquidations, settlements, withdrawals and claims continue. New shorts resume automatically once coverage is back.
>
> While coverage is degraded, positions here may be liquidated later than usual.

#### `market.poolSwitch` — This market’s pool can change

*Severity:* info · *Acknowledgement:* none · *Source:* SPEC §14 (#99, M4R-1, M4F-1); ARCH §2.6
*Shown:* Market page → Pools tab; banner while a collection window is open (`/pools.window`) or `ema_warm > 0`

> If the current pool fails (halted, drained or too expensive), short.win can move this market to a deeper pool.
>
> Open positions move only if each one fits the new pool’s normal opening limits. Otherwise they stay and settle in SOL at fair value at their deadline (S3).
>
> Right after a change, the new pool’s prices are confirmed for 8 seconds before liquidations and new shorts resume.

#### `market.routeBroken` — Route marked broken

*Severity:* danger · *Acknowledgement:* none · *Source:* SPEC §11 (O6)
*Shown:* Market page banner and Short panel while the market or venue route-broken bit is set; status page; `{s3Wait}` from `GlobalConfig` (`s3_wait_s`)

> The emergency multisig marked this market’s trading route as broken. New shorts are refused.
>
> Closes and liquidations are never paused by this flag, but they can fail while the venue cannot trade. Positions that are liquidatable settle in SOL after {s3Wait} without a route (S3).

#### `market.venueUpgraded` — Venue upgraded, under review

*Severity:* warn · *Acknowledgement:* none · *Source:* ARCH §0 (8), §3.1 (ADOPTED Q13)
*Shown:* Market page banner and Short panel while the venue guard latches the market’s venue; status page

> A venue program this market trades on was upgraded and has not been re-validated yet. New shorts wait for the review; exits are never blocked by it.

#### `market.migrating` — Graduating to its pool

*Severity:* warn · *Acknowledgement:* none · *Source:* SPEC §5.3; ARCH §5.4
*Shown:* Market page banner and position rows while the venue is `pump_migrating`

> This token is migrating from the Pump.fun curve to its pool. Closes and liquidations pause during the migration and resume once the pool’s prices are confirmed.
>
> If the migration gets stuck, positions settle in SOL (S3) after the migration pause.

#### `market.graduationHalted` — Graduation halted

*Severity:* warn · *Acknowledgement:* none · *Source:* SPEC §14 M6-SEC-3; ARCH §3.11 (6104)
*Shown:* Short panel on a curve market while PumpSwap `disable_flags` bit 0 is set

> PumpSwap pool creation is disabled, so this curve cannot graduate. New curve positions are refused until it is re-enabled; open positions are unaffected.

#### `market.listOnly` — Lend only

*Severity:* info · *Acknowledgement:* none · *Source:* SPEC §3
*Shown:* Market page and Short panel on a `none` venue

> This token has no pool short.win can price on-chain yet (for example it trades only on Orca, Meteora DLMM or Raydium CLMM). It can be listed and lent, but not shorted.

#### `market.venueTrust` — Venue trust assumptions

*Severity:* info · *Acknowledgement:* none · *Source:* ARCH §8.5; SPEC §12 R5
*Shown:* Market page → Overview (collapsed), create-market page

> Pump.fun, PumpSwap and their fee program can be upgraded at any time by one upgrade authority, and are upgraded several times a month.
>
> Raydium AMM v4 and CPMM are upgraded by a 3-of-4 multisig with no timelock and have no verified builds. A 2-of-3 pool admin can halt a pool or raise its fees.
>
> Token-2022 is upgradeable. short.win itself is upgradeable by its governance multisig after a 48-hour timelock.
>
> A malicious venue upgrade can harm exits; no design can prevent that.

#### `market.mintUnsafe` — Token flagged unsafe

*Severity:* danger · *Acknowledgement:* none · *Source:* SPEC §3; ARCH §3.9 flag_unsafe_mint
*Shown:* Market page banner, Short and Lend panels while `flags.mint_unsafe`

> This token no longer passes the safety filter (freeze authority, permanent delegate, transfer hook or transfer fee). Deposits and new shorts are blocked; withdrawals, claims and exits continue.

#### `market.create` — Creating a market

*Severity:* info · *Acknowledgement:* none · *Source:* SPEC §3; ARCH §3.2
*Shown:* Create-market page and its review modal; `{fee}` from `GlobalConfig`, `{rent}` from `getMinimumBalanceForRentExemption` of the accounts

> Anyone can create a market for any token that passes the safety filter. There is no approval and no special status for the creator.
>
> You pay the creation fee ({fee}) and the rent for the market’s accounts ({rent}). The rent stays in the accounts.

#### `market.priceRefresh` — Refresh the market price

*Severity:* info · *Acknowledgement:* none · *Source:* open_short stale-price guard; crank_price observation buckets
*Shown:* Short panel when idle-market observations are stale, and each price-refresh review

> An idle market may need two transactions: record a price observation, then update the smoothed price after the chain clock advances. You review and sign each separately and pay its network fee.
>
> Opening a short is a separate action. Refreshing the price does not guarantee that new shorts are available.

#### `token.cpiGuard` — CPI Guard is on for this token account

*Severity:* warn · *Acknowledgement:* none · *Source:* ARCH §3.3; R-MINT §8.4
*Shown:* Lend → Deposit when the source token account has `CpiGuard.lock_cpi` (blocks the plain deposit; offers the bracket, FE-1)

> CPI Guard blocks transfers that programs make on your behalf, so short.win cannot deposit from this token account.
>
> Deposit from another account of this token without CPI Guard, or turn CPI Guard off for this one deposit: the transaction turns it off, deposits and turns it back on, and your wallet shows all three steps.

#### `token.memoRequired` — Memo required

*Severity:* info · *Acknowledgement:* none · *Source:* ARCH §3.3 (M1-SEC-7)
*Shown:* Lend → Withdraw when the destination has `MemoTransfer.require_incoming_transfer_memos` (informational; `memo_program` is passed)

> This token account only accepts transfers with a memo. short.win adds the memo when it pays tokens to it.

#### `tx.simulation` — Simulated result

*Severity:* info · *Acknowledgement:* none · *Source:* SPEC §9; ARCH §7.3
*Shown:* Every review modal (collapsed)

> These changes come from simulating the transaction against the current chain state. The result can differ if the market moves before it lands; the limits you set protect you.

### 4.3 Newcomer copy and the glossary (thirteenth round)

The owner's thirteenth round (2026-10-10; audit findings D-03, D-21, D-23) adds copy that is not a
disclosure: the markets list's intro strip (§3.3), the glossary, How it works (§3.11), the links to
it, the Short panel's link to the coverage banner (O-2, §3.4) and the Claim button (O-5, §3.6).
**Every string below but the mobile and pump groups — all 143 — was
approved as written by the owner on 2026-10-11 (thirteenth round)**, word for word, as the file
`apps/web/scripts/owner-copy.ts` prints it (with each string's id, where it shows and the sources
of each claim). **The mobile group's 7 strings** (the wallet chooser on a phone and the Mobile
Wallet Adapter's review notices, §5.4 (6); `src/config/copyWallet.ts`, the owner file's section 8)
**were approved as drafted by the owner on 2026-10-13 (O-6 lifted)**, byte for byte: straight
apostrophes and no final full stops. **The pump group's 2 strings** (the program's error texts
6109 `CurveCashbackCoin` and 6031 `CurveStraddle`, §5.9; `src/config/copyPump.ts`, the owner
file's section 9) **were approved as drafted by the owner on 2026-10-13 (pump-final D4)**, byte
for byte as the program returns them: a straight apostrophe and no final full stop on
`CurveStraddle`. 152 strings in all. Like an approved disclosure's text
(§4.1), a string changes only with the owner's approval, and the code and this section change
together.

Rules (as §4.1 for disclosures): the text has one home — `src/config/copy.ts` (`COPY`, each string
with its id, where and sources, the mobile group spread in from `copyWallet.ts`, which the shell's
wallet chooser imports alone; `HOW_IT_WORKS`, the page's order) and `src/config/glossary.ts`
(`GLOSSARY`); components render it through `copy()` / the glossary control and never retype it;
`copy.test.ts` and `glossary.test.ts` fail unless this section carries every string verbatim
(regenerate it with `node scripts/owner-copy.ts --doc` after a change, in the same change). A
governable number is a `{placeholder}` filled from `GlobalConfig`. A glossary entry that says the
same as an approved disclosure quotes its paragraphs (`quote`), rendered from `disclosures.ts`;
the catalogue prints the quoted words under the entry's reference, so a paragraph added, dropped or
moved in the disclosure changes this section too (the round-13 review).

**The glossary control** (`components/Glossary.tsx`): `GlossaryTip` is an ⓘ button (24 px box, the
tier badge's inline-SVG mark; 44 px touch area under a coarse pointer) beside a label —
`aria-label` "What is {term}?", `aria-expanded`, `aria-controls` — that opens the definition on a
click, a tap, or Enter / Space with focus on it, never on hover. The definition is a non-modal
`role="dialog"` named by its title, rendered in a portal (the open sheet or dialog it sits in, else
`<body>`) at a fixed position computed from the ⓘ (CSSOM), so a table's or a panel's overflow never
clips it; on a phone a bottom sheet over a scrim. Focus moves into it; Escape and ✕ close it and
return focus to the ⓘ, a tap outside closes it, Shift+Tab or Tab past ✕ leaves it from the ⓘ.
Its own clicks, pointer presses and keys (but Tab) stop at it: a portal's events still bubble
through the React tree, and a table row that opens on a click, Enter or an arrow key must not see
them. On a phone the scrim closes the sheet on its own click (not on the press: the tap's click
would land on the card under it) and returns focus to the ⓘ.
`Term` is a label with its ⓘ when the glossary knows that exact label: the shared renderers use it —
`Kv`, `KvLines`, `Stat`, the stat strip, `Field` (the ⓘ outside the `<label>`, so the input's name
stays the label), `DataTable` headers (beside the sort button, never in it; `Column.term` overrides; not in card mode, whose header row is clipped — the cards' badges keep theirs),
the Short preview rows, the size caps, the tier terms, the position's facts — and the flag, creator,
epoch and settlement badges and the status bar name their entry.

**The Claim label (O-5)** — what its figures are (from the code; the owner file's §6): *credited*
is the lender's `pending` after a touch; *claimable now* what `claim_sol` pays with the pot as it
stands (`previewClaim`, no syncs); the *shortfall* max(0, pending − own − pot), the credited SOL
that neither the SOL allocated to the lender's shares nor the pot covers. *The owner's decision of
2026-10-11:* a shortfall under 10,000 lamports (`CLAIM_DUST_LAMPORTS`) is rounding dust (ECON §9.4)
and counts as none — no `claim.label.syncs`, no `claim.note.shortfall`, no syncs gathered, and with
nothing claimable the disabled "Review claim · 0 SOL" with "Nothing to claim yet" (§3.6); and the
Lending page's row Claim shows the same label for the same claim (§3.8). *Only when syncs can pay
(the owner's decision of 2026-10-11, after the approval's review):* `claim.label.syncs`'s {g} is
min(shortfall, what the syncs the search picks move to the pot), run before the button offers them;
under 10,000 lamports it counts as none, so a shortfall no sync can fund (unpaid rent; a market's
only lender) shows `claim.label.full`, or with nothing claimable the disabled "Review claim · 0 SOL"
with "Nothing to claim yet"; `claim.note.shortfall`'s {g} stays the whole shortfall; a review whose
simulation would pay 0 is held with "Nothing to claim yet" (§3.6). No string changed or added.

<!-- copy:begin (generated by apps/web/scripts/owner-copy.ts --doc; do not edit by hand) -->

**Intro strip, How it works, links, O-2, O-5** (`src/config/copy.ts`):

- `intro.heading` (Intro strip: heading (an h2)):
  > New to short.win?
- `intro.short.title` (Intro strip: part 1 title):
  > Short a token
- `intro.short.body` (Intro strip: part 1):
  > Borrow a token and sell it. You gain if the price falls by more than the rent, fees and price impact; if it rises, you can lose all your margin and the fees you paid.
- `intro.lend.title` (Intro strip: part 2 title):
  > Lend a token
- `intro.lend.body` (Intro strip: part 2):
  > Lend your tokens for rent paid by the people who short them. It is not staking, and not risk-free.
- `intro.risks.title` (Intro strip: part 3 title):
  > What can go wrong
- `intro.risks.body` (Intro strip: part 3):
  > A short can be liquidated or settled in SOL. Lenders can be repaid in SOL worth less than the tokens they lent, and can owe SOL for rent borrowers never pay.
- `intro.risks.linkShort` (Intro strip: part 3, link 1 → How it works #short-risks (D:borrower.liquidation verbatim)):
  > How a short can end
- `intro.risks.linkLend` (Intro strip: part 3, link 2 → How it works #lend-risks (D:lender.risks verbatim)):
  > How lending works here
- `intro.more` (Intro strip: link in the heading row → /how-it-works):
  > How it works
- `intro.dismiss` (Intro strip: the dismiss button (visible text)):
  > Hide
- `intro.dismiss.aria` (Intro strip: the dismiss button’s accessible name (contains the visible “Hide”, WCAG 2.5.3)):
  > Hide the introduction
- `intro.dismissed.note` (Markets page: a polite status message after hiding (screen readers; focus moves to the “Markets” heading)):
  > Introduction hidden. You can still open How it works from the status bar or the More menu.
- `howItWorks.title` (How it works: h1):
  > How it works
- `howItWorks.lead` (How it works: lead paragraph):
  > short.win lets you short a Solana token by borrowing it from the people who lend it, and lets holders lend their tokens for rent. Both sides carry real risk. This page covers the basics; each market shows its own terms before you sign.
- `howItWorks.toc.aria` (How it works: the contents nav (accessible name and visible heading)):
  > On this page
- `howItWorks.short.h` (How it works: h2 #shorting):
  > Shorting a token
- `howItWorks.short.p1` (How it works: Shorting ¶1):
  > You choose a size in tokens and a slippage limit. In one transaction, short.win borrows the tokens from the market’s lending pool and sells them on the token’s venue: the Pump.fun bonding curve, PumpSwap or Raydium.
- `howItWorks.short.p2` (How it works: Shorting ¶2):
  > The sale proceeds stay locked in your position’s vault together with the margin you post. You never hold the borrowed tokens or the proceeds.
- `howItWorks.short.p3` (How it works: Shorting ¶3):
  > Your margin is the SOL it takes to bring your collateral ratio (CR) up to the market’s opening CR, for example {exCrOpen} in Tier {exTier}. The preview shows the exact margin, fee and costs before you sign.
- `howItWorks.short.p4` (How it works: Shorting ¶4):
  > To close, short.win buys the tokens back with your vault’s SOL, returns them to the pool, pays the rent, and sends you the rest. You gain only if the price has fallen by more than the venue fees, price impact, rent and the open fee together; otherwise you get back less than you put in.
- `howItWorks.short.p5` (How it works: Shorting ¶5):
  > A position pays rent every second and can stay open up to the market’s maximum duration ({durMin} to {durMax} depending on the tier). After that, anyone can close it: if it is still above its liquidation CR, you pay an expiry fee and a small keeper reward instead of the liquidation penalty.
- `howItWorks.lend.h` (How it works: h2 #lending):
  > Lending a token
- `howItWorks.lend.p1` (How it works: Lending ¶1):
  > You deposit tokens into the market’s pool and receive pool shares. Every lender in a market shares the same pool, so income and losses are shared pro-rata.
- `howItWorks.lend.p2` (How it works: Lending ¶2):
  > Borrowers pay rent on the tokens they borrow. Your share is credited to you as it accrues and is paid in when their positions end (closed, liquidated, expired or settled in SOL); rent a position never pays is taken back (see What can go wrong). You claim it on the market’s Lend → Claim tab or on the Lending page.
- `howItWorks.lend.p3` (How it works: Lending ¶3):
  > Before you deposit into a pool with unpaid rent, the Lend tab shows that rent. It also shows how much of it your deposit would carry if it is never paid.
- `howItWorks.lend.more` (How it works: Lending, last line):
  > The rules for withdrawals, locks and income are under What can go wrong, below.
- `howItWorks.risk.h` (How it works: h2 #what-can-go-wrong):
  > What can go wrong
- `howItWorks.risk.lead` (How it works: What can go wrong, lead):
  > Below are the main risk texts the app shows on its market pages and before you sign, word for word, with short notes on others. Each market page and each review shows every text that applies there; read them before you trade or lend.
- `howItWorks.risk.short.h` (How it works: h3 #short-risks):
  > If you short
- `howItWorks.risk.shortCaption` (How it works: caption above the embedded borrower.liquidation):
  > Shown with Tier {exTier}’s current terms as an example. Each market’s Short panel shows the same text with that market’s terms.
- `howItWorks.risk.lend.h` (How it works: h3 #lend-risks):
  > If you lend
- `howItWorks.risk.lend.p2` (How it works: If you lend, after the embedded lender.solRepayment):
  > Rent that borrowers never pay is taken from the lenders who hold shares when the position ends, including lenders who deposited after that rent was credited. Such a loss is taken from your SOL balance in this pool. If it leaves that balance below zero, you must pay the difference in SOL (Cover deficit, on the Claim tab) before you can withdraw your tokens. The amount is not limited to what you have received from the pool; a market’s Lend tab shows any unpaid rent before you deposit.
- `howItWorks.risk.lend.p1` (How it works: If you lend, before the collapsed lender.residuals):
  > Each market’s Lend tab also shows what price gaps cost lenders in its tier, and the losses that market has had so far.
- `howItWorks.risk.all.h` (How it works: h3):
  > In every market
- `howItWorks.risk.all.p1` (How it works: In every market ¶1):
  > A token’s creator can lend and short its own token from different wallets — in effect selling into the market — and the per-wallet cap does not stop one person from using several wallets. Each market’s Overview shows the deposits made from the creator’s wallet.
- `howItWorks.fees.h` (How it works: h2 #fees):
  > Fees and rent
- `howItWorks.fees.open` (How it works: Fees list):
  > Open fee: {openFee} of the position’s notional, to the protocol. There is no close fee.
- `howItWorks.fees.venue` (How it works: Fees list):
  > Venue fees: the venue charges its own trading fee on the sale when you open and on the buyback when the position ends; on some venues part of it goes to the token’s creator. The preview’s figures include it.
- `howItWorks.fees.rent` (How it works: Fees list):
  > Rent: a yearly rate on the value of the borrowed tokens, charged every second. It rises with the market’s utilisation along the tier’s curve: gently at first, steeply once most of the pool is lent out. Every position pays at least its minimum rent. Lenders receive the rent less the protocol’s share ({rentProtocol}).
- `howItWorks.fees.rate` (How it works: Fees list):
  > The Rate (APR) on each market page is what a new short pays at the market’s utilisation now.
- `howItWorks.fees.penalty` (How it works: Fees list):
  > Liquidation penalty: set by the tier and split between the lenders ({penLender}), the protocol ({penProtocol}) and the keeper that liquidates ({penKeeper}). If enough equity remains after the buyback and rent, the keeper’s share is topped up to the settlement keeper reward from what would come back to you. Otherwise its reward can be smaller or zero.
- `howItWorks.fees.expiry` (How it works: Fees list):
  > Expiry fee: {expiryFee} of the buyback cost, paid instead of the penalty by a position closed after its deadline while still above its liquidation CR, all to the lenders. The keeper that closes it is also paid a small, capped reward from the position.
- `howItWorks.fees.settle` (How it works: Fees list):
  > Settlement in SOL: the keeper is paid {keeperReward}; the lenders take the rest of the vault, capped at fair value in S2 and S3.
- `howItWorks.fees.network` (How it works: Fees list):
  > Network fees and account deposits: every transaction pays Solana’s network fee. A new position, or your first deposit in a market, also pays a refundable deposit for a new account (Solana’s account rent, not the rent lenders earn). It comes back when that account closes.
- `howItWorks.fees.create` (How it works: Fees list):
  > Creating a market: a creation fee of {creationFee} plus Solana’s account rent for the market’s accounts, which stays in the accounts.
- `howItWorks.sign.h` (How it works: h2 #before-you-sign):
  > Before you sign
- `howItWorks.sign.p1` (How it works: Before you sign ¶1):
  > Every transaction is first simulated against the chain as it is now. You review what it will change before your wallet asks you to sign. If the market moves before the transaction is confirmed, it can still go through at a worse price, up to the limits you set (slippage and tolerances); beyond them it fails instead.
- `howItWorks.more.h` (How it works: h2 #read-more):
  > Read more
- `howItWorks.more.security` (How it works: Read more list):
  > [Security policy](/security): how to report a vulnerability, and the audit reports once they are published.
- `howItWorks.more.limits` (How it works: Read more list (the links open the two collapsed disclosures above)):
  > Known limits: the residual risks the protocol accepts, [for lenders](#lender-limits) and [for borrowers](#borrower-limits).
- `howItWorks.more.status` (How it works: Read more list):
  > [Status](/status): the indexer, the venues and any markets with paused shorts.
- `howItWorks.more.glossary` (How it works: Read more list):
  > Terms: tap or click ⓘ next to a label for a short definition.
- `howItWorks.back` (How it works: the foot link → /markets):
  > Back to markets
- `howItWorks.configError` (How it works: an alert above the sections when the GlobalConfig read fails or finds no account (with Retry); also in place of the risk texts that need those numbers):
  > The protocol’s current numbers could not be read, so they are missing on this page.
- `nav.howItWorks` (Status bar (the app’s footer; desktop and tablet, and the phone’s expanded status list), before “Security”):
  > How it works
- `more.howItWorks.title` (Phone: the More sheet’s first row):
  > How it works
- `more.howItWorks.sub` (Phone: the More sheet row’s muted subtitle):
  > Shorting, lending and the risks
- `coverage.link.inline` (Short panel beside the page (≥ 1,024 px), under the “New shorts unavailable” note: scrolls to the banner and focuses it):
  > Read the full coverage notice
- `coverage.link.sheet` (Trade sheet (< 1,024 px), under the same note: closes the sheet, then scrolls to the banner and focuses it):
  > Close and read the full coverage notice
- `claim.label.full` (Lend → Claim submit and each Lending page row’s Claim when the syncs cannot add 10,000 lamports or more (no shortfall, dust under 10,000 lamports, or a shortfall no sync can fund; the owner’s, 2026-10-11), or while the search for syncs runs with something claimable now (unchanged; also “Review claim · 0 SOL”, disabled, with “Nothing to claim yet”)):
  > Review claim · {c} SOL
- `claim.label.syncs` (Lend → Claim submit and each Lending page row’s Claim when the syncs the search picks can add 10,000 lamports or more of what is credited but not claimable now (CLAIM_DUST_LAMPORTS; the owner’s, 2026-10-11: only when syncs can pay); two lines, the second from “syncs”):
  > Review claim · {c} SOL now · syncs may add up to {g} SOL
- `claim.note.shortfall` (Lend → Claim: the note above the button, appended when there is a shortfall (10,000 lamports or more: CLAIM_DUST_LAMPORTS, owner, 2026-10-11) (replaces “{g} SOL credited to you is not paid in yet. A claim first syncs other lenders whose surplus can fund it, where there are any.”)):
  > {g} SOL of what is credited to you cannot be claimed now. Some of it may be rent that open positions still owe: it is paid when they end, and taken back from the lenders if it never is. Some may be allocated to other lenders’ shares beyond their own credit. A claim first syncs those lenders where it finds any, which moves that excess to the pot; the review shows what the claim will pay.

**Mobile wallets** (`src/config/copyWallet.ts`; O-6, approved as drafted by the owner on 2026-10-13 (O-6 lifted)):

- `wallet.openIn.more` (A phone or tablet with wallets listed (header sheet and inline under Connect wallet): the label above “Open in Backpack / Phantom / Solflare”, after the entries (`wallet-open-elsewhere`)):
  > Or open this page in a wallet app
- `wallet.webview.open` (A social app’s in-app browser with no wallet (Telegram, X, Instagram, Facebook, Line, an Android WebView): the chooser’s first line (`wallet-in-app-browser`)):
  > Open this page in your browser
- `wallet.webview.copy` (The button after it: copies this page’s address (“Copied” for 1.2 s, as “Copy address”) (`wallet-copy-link`)):
  > Copy link
- `wallet.mwa.label` (The Mobile Wallet Adapter’s entry (Android browsers): its label. The wallet’s own name, “Mobile Wallet Adapter”, stays its test id and shows in the account menu and the review):
  > Use installed wallet
- `wallet.mwa.hint` (The Mobile Wallet Adapter entry’s second line (`wallet-mwa-hint`)):
  > Opens your wallet app on this phone
- `wallet.mwa.localNetwork` (The Mobile Wallet Adapter entry’s second line while Chrome’s local-network permission is not granted (asked or denied); and beside Sign in the review when the connected wallet is the Mobile Wallet Adapter and the permission is missing (`tx-sign-notice`, `data-reason="local-network"`)):
  > Allow 'Apps on device' so this page can reach your wallet app
- `wallet.mwa.retry` (The review, beside Sign, when the wallet app could not be launched; the review stays open and nothing was sent (`tx-sign-notice`, `data-reason="launch"`)):
  > Couldn't open your wallet app. Tap Sign again

**pump’s v3 adapter: the program’s error texts** (`src/config/copyPump.ts`; approved as drafted by the owner on 2026-10-13 (pump-final D4)):

- `error.curveCashbackCoin` (Create market: the listing refusal of a live cashback curve, above its code (`create-check-reason-CurveCashbackCoin`); the review’s error when the program refuses a transaction with 6109 `CurveCashbackCoin` (src/config/errors.ts)):
  > pump.fun's current trades do not support cashback coins on the curve. This coin can be shorted once it graduates to PumpSwap.
- `error.curveStraddle` (The review’s error when the program refuses a close with 6031 `CurveStraddle` (src/config/errors.ts): a buyback past what the live curve and its would-be pool hold, which the per-market cap keeps out of reach (pump-final C8)):
  > Buyback exceeds what the curve and its post-migration pool can deliver

**The glossary control** (`src/config/glossary.ts` `GLOSSARY_UI`):

- `glossary.trigger.aria` (the ⓘ):
  > What is {term}?
- `glossary.popover.close` (the ⓘ):
  > Close

**Glossary entries** (`src/config/glossary.ts`; *beside*: the labels it attaches to):

- `glossary.cr` (CR (the collateral ratio); beside “CR”, “CR now”, “CR after”, “CR (spot)”, “CR (EMA)”, “CR at open (spot)”, “CR at open (EMA)”):
  > Collateral ratio (CR) = vault SOL ÷ the SOL needed to buy the tokens back now, including price impact. If it falls below the position’s liquidation CR on both the live and the smoothed price, anyone can liquidate the position.
- `glossary.crOpen` (the opening CR; beside “Opening CR (D4)”):
  > The CR a new position must have when it opens, checked at both the live and the smoothed price. The tier sets it; your margin is the SOL it takes to reach it.
- `glossary.crLiq` (the liquidation CR; beside “Liquidation CR”):
  > The CR below which the position can be liquidated, on both the live and the smoothed price. It is set by the market’s tier and locked into the position when it opens.
- `glossary.ema` (the smoothed price (EMA); beside “Price (EMA)”):
  > The smoothed price: an average that follows the live price over a few seconds, updated on every trade and by the keeper. A liquidation needs both prices past the line, so a brief spike in the live price cannot by itself liquidate a position whose smoothed price is still healthy.
- `glossary.spot` (the spot price; beside “Spot”):
  > The live price on the token’s venue now, as short.win reads it from the curve or pool.
- `glossary.liqPrice` (the liquidation price; beside “Liquidation price”, “Liq. price (SOL)”, “Liq. price”):
  > The token price at which this position’s CR would reach its liquidation CR. Liquidation also needs the smoothed price to cross, and rent accruing over time moves this price closer.
- `glossary.risk` (the risk grade; beside “Risk”):
  > How close the CR is to the liquidation CR: Safe, Watch, At risk, At risk (live price below), or Liquidatable.
- `glossary.q` (Q (tokens owed); beside “Q”, “Q owed”, “Borrow”):
  > The number of tokens a short borrowed and must give back to the pool. Closing buys exactly this many back.
- `glossary.sumQ` (ΣQ (tokens borrowed); beside “Tokens borrowed (ΣQ)”):
  > All the tokens currently borrowed by the open shorts in this market.
- `glossary.tokensFree` (free tokens; beside “Tokens free (T)”, “Free inventory”):
  > The pool’s tokens that are not lent out. New shorts borrow from them and lenders withdraw from them.
- `glossary.inventory` (inventory; beside “Inventory”, “Inventory · free”, “Inventory value”):
  > All the tokens lenders have in this pool: the free ones plus the ones lent out.
- `glossary.saleProceeds` (sale proceeds; beside “Sale proceeds (S)”, “Sale proceeds”):
  > The SOL the borrowed tokens sold for. It stays locked in the position’s vault until the position ends.
- `glossary.margin` (margin; beside “Margin (D4)”, “Margin”, “Margin (incl. added)”):
  > The SOL you post when you open, plus any margin you add later. The tier’s opening CR sets how much you need.
- `glossary.vault` (the vault; beside “Vault”):
  > The position’s own SOL account: your margin plus the sale proceeds. Only a close, a liquidation, an expiry or a settlement in SOL can move this SOL out.
- `glossary.notional` (notional; beside “Notional open”):
  > A position’s size in SOL: what buying its tokens back would cost right after it opened. The open fee is charged on it.
- `glossary.slippage` (slippage; beside “Slippage”):
  > How much worse than the quote a trade may execute before it fails instead. A wider limit fails less often but can cost more.
- `glossary.openFee` (the open fee; beside “Open fee”):
  > A fee to the protocol on the position’s notional, paid when it opens. There is no close fee.
- `glossary.totalCost` (the total cost; beside “Total cost”):
  > Margin, open fee and refundable deposits for the position’s accounts (Solana’s account rent, not the rent paid to lenders). The deposits come back when the position ends, or when your last position in this market ends. The network fee is shown in the review.
- `glossary.rentApr` (the rent rate (APR); beside “Rate (APR)”, “Rent rate (APR)”, “Rate”):
  > The yearly rent rate a short pays on the value of its borrowed tokens. It rises with utilisation along the tier’s curve: gently at first, steeply once most of the pool is lent out.
- `glossary.rentCurve` (the rent curve; beside “Rent: base / slope 1 / slope 2”):
  > The three numbers of the rent curve: the rate at zero utilisation, how fast it rises up to a set utilisation (the kink), and how fast it rises above it. A position keeps the curve it opened with; its rate still moves with the market’s utilisation.
- `glossary.minRent` (the minimum rent; beside “Minimum rent”):
  > The least rent a position pays, counted as a stretch of time at its rate, even if it closes sooner.
- `glossary.rentDue` (rent due; beside “Rent due”):
  > Rent this position owes so far; it is paid when the position ends.
- `glossary.rentProtocol` (the protocol’s rent share; beside “Rent to the protocol”):
  > The protocol’s share of the rent; the lenders receive the rest.
- `glossary.maxDuration` (the maximum duration; beside “Max duration”, “Deadline”, “Time left”):
  > How long a position may stay open; the tier sets it. After the deadline anyone can close it: if it is still above its liquidation CR, you pay the expiry fee and a small keeper reward instead of the liquidation penalty; if it is below, it is liquidated and pays the penalty.
- `glossary.expiryFee` (the expiry fee; beside “Expiry fee”):
  > What a position pays when it is closed after its deadline while still above its liquidation CR, in place of the liquidation penalty. It goes to the lenders; the keeper that closes it is also paid a small, capped reward from the position.
- `glossary.penalty` (the liquidation penalty; beside “Liquidation penalty”, “Penalty”):
  > Taken from the vault when a position is liquidated, and split between the lenders, the protocol and the keeper that liquidated.
- `glossary.maxLiqSlippage` (the maximum liquidation slippage; beside “Max liquidation slippage (S2)”, “Max liquidation slippage”):
  > The most a liquidation’s buyback may move the price. If buying back would move it more, the position settles in SOL instead (S2).
- `glossary.buyback` (the buyback; beside “Buyback”):
  > The SOL it costs to buy the borrowed tokens back.
- `glossary.keeper` (a keeper; beside “Keeper paid”):
  > A bot that liquidates, expires and settles positions. Anyone can run one. Small positions can depend on a short.win backstop with operating funds; timely third-party execution is not guaranteed.
- `glossary.keeperReward` (the settlement keeper reward; beside “Keeper reward (settlement)”, “Settlement keeper reward”):
  > What the keeper that settles a position in SOL is paid from the vault: a fixed amount plus a share of the vault, capped.
- `glossary.venueFeeHardCap` (the venue fee hard cap; beside “Venue fee hard cap”):
  > The venue trading fee above which short.win treats the venue as unusable for buying back, which can lead to a settlement in SOL (S3). This position’s risk figures count the venue fee at no more than the cap locked when it opened.
- `glossary.settlement` (a settlement in SOL (S1, S2, S3); beside “Settled in SOL (S1 / S2 / S3)”, “Condition”):
  > When the tokens cannot be bought back, the position settles in SOL: the lenders take SOL from its vault instead of tokens. S1: the price jumped past what the vault can pay. S2: buying back would move the price too far. S3: there has been no way to buy back for a while. “How this short can end” has the exact rules.
- `glossary.armed` (an armed settlement; beside “Settlement watch”, “Settlement armed”, “Settlement in SOL being considered (S1 / S2)”):
  > The program has started collecting the price observations an S1 or S2 settlement needs. It is not a settlement yet, and it can still be cleared.
- `glossary.pnl` (P&L; beside “P&L”, “P&L if closed now”):
  > Profit or loss in SOL: what you would get back by closing now (or got back), less your margin, any added margin and the open fee.
- `glossary.tier` (the tier; beside “Tier”):
  > Tiers A to D set a market’s terms: margin, liquidation CR, penalty, maximum duration, rent curve and size caps. Tiers are set automatically from on-chain liquidity and pool age, and checked again on every trade. A position keeps the terms it opened with.
- `glossary.venue` (the venue; beside “Venue”):
  > Where short.win trades the token: the Pump.fun bonding curve, PumpSwap, or Raydium AMM v4 or CPMM.
- `glossary.poolAge` (the pool age; beside “Pool age from”):
  > When short.win starts counting the pool’s age for its tier: the graduation it saw, the listing time, or the pool’s own recorded creation time.
- `glossary.utilisation` (utilisation; beside “Utilisation”, “Util.”, “Utilisation cap”):
  > The share of this pool’s tokens that is lent out now. Rent rises with it, and a new short may not take it above the protocol’s utilisation cap.
- `glossary.tvl` (TVL (total value locked); beside “TVL”):
  > Total value in the market, in SOL: the free tokens at the smoothed price, the SOL held for lenders, and the SOL in open positions’ vaults.
- `glossary.oi` (open interest; beside “Open interest”, “Market open-interest cap”):
  > The value of all borrowed tokens in this market at the smoothed price. The market’s open-interest cap keeps the tokens borrowed small enough that buying them all back would move the price by no more than the tier allows.
- `glossary.sizeCap` (the size cap; beside “Size cap”, “Size caps”):
  > The largest short you can open right now. Several caps apply (price impact, the market’s open interest, utilisation, your wallet, free inventory, the curve guard on Tier D, and a new market’s notional cap); the smallest, marked “binding”, sets it.
- `glossary.positionImpactCap` (the position impact cap; beside “Position impact cap”):
  > A single position may not be so large that buying it back would move the price more than the tier allows.
- `glossary.walletCap` (the per-wallet cap; beside “Per-wallet cap”):
  > The most one wallet may hold of the market’s open-interest cap. It counts each wallet address separately, so it does not stop one person from using several wallets.
- `glossary.notionalCap` (the market notional cap; beside “Market notional cap”):
  > The limit on how much can be shorted while a market is new.
- `glossary.curveGuard` (the curve graduation guard; beside “Curve graduation guard”):
  > On a token still on the Pump.fun bonding curve, the open shorts together may borrow only a set share of the tokens the curve still has for sale, so that buying them back never needs more tokens than the curve holds.
- `glossary.lockedDepth` (locked depth; beside “Locked depth D”):
  > The part of the pool’s liquidity that cannot be withdrawn, because its LP tokens (the pool’s receipts) are burned or locked. short.win bases its size caps on this part, not on the whole pool.
- `glossary.coinCreator` (the coin creator; beside “Coin creator”):
  > This wallet created the token. It can lend and short its own token, so it can be on both sides of this market.
- `glossary.marketCreator` (the market creator; beside “Market creator”):
  > This wallet created the short.win market. That gives it no special rights.
- `glossary.flagMintUnsafe` — Mint unsafe; beside “Mint unsafe”: *verbatim, D:market.mintUnsafe ¶1*
  > This token no longer passes the safety filter (freeze authority, permanent delegate, transfer hook or transfer fee). Deposits and new shorts are blocked; withdrawals, claims and exits continue.
- `glossary.flagRouteBroken` — Route broken; beside “Route broken”: *verbatim, D:market.routeBroken ¶1*
  > The emergency multisig marked this market’s trading route as broken. New shorts are refused.
- `glossary.flagOpensPaused` (Opens paused; beside “Opens paused”):
  > New shorts are paused (on this market or globally). Exits continue.
- `glossary.flagCoveragePaused` — Coverage paused; beside “Coverage paused”, “Markets in degraded mode”: *verbatim, D:market.coveragePause ¶1, ¶2, ¶3*
  > short.win’s keeper cannot currently cover this market (degraded mode), so new shorts are paused. Closing, adding margin, liquidations, settlements, withdrawals and claims continue. New shorts resume automatically once coverage is back. While coverage is degraded, positions here may be liquidated later than usual.
- `glossary.flagVenueUpgraded` — Venue upgraded; beside “Venue upgraded”: *verbatim, D:market.venueUpgraded ¶1*
  > A venue program this market trades on was upgraded and has not been re-validated yet. New shorts wait for the review; exits are never blocked by it.
- `glossary.flagMigrating` — Migrating; beside “Migrating”: *verbatim, D:market.migrating ¶1*
  > This token is migrating from the Pump.fun curve to its pool. Closes and liquidations pause during the migration and resume once the pool’s prices are confirmed.
- `glossary.flagGraduationHalted` — Graduation halted; beside “Graduation halted”: *verbatim, D:market.graduationHalted ¶1*
  > PumpSwap pool creation is disabled, so this curve cannot graduate. New curve positions are refused until it is re-enabled; open positions are unaffected.
- `glossary.flagListOnly` — Lend only; beside “Lend only”: *verbatim, D:market.listOnly ¶1*
  > This token has no pool short.win can price on-chain yet (for example it trades only on Orca, Meteora DLMM or Raydium CLMM). It can be listed and lent, but not shorted.
- `glossary.flagWarmingUp` (Warming up; beside “Warming up”):
  > The market’s prices are being confirmed after a listing, a graduation or a pool change; keeper liquidations and new shorts wait until they are, and you can still close your own position.
- `glossary.flagPoolWindow` — Pool switch window; beside “Pool switch window”: *verbatim, D:market.poolSwitch ¶1, ¶2*
  > If the current pool fails (halted, drained or too expensive), short.win can move this market to a deeper pool. Open positions move only if each one fits the new pool’s normal opening limits. Otherwise they stay and settle in SOL at fair value at their deadline (S3).
- `glossary.shares` (pool shares; beside “Shares”, “Your shares”, “Shares minted”):
  > A deposit receives pool shares; they are your part of the pool’s tokens and SOL.
- `glossary.poolShare` (the pool share; beside “Pool share”, “Your share of the pool”, “Share”, “Its share of the pool”):
  > Your shares as a percentage of all of this pool’s shares. Income and losses are shared in this proportion.
- `glossary.tokenValue` (the token value; beside “Token value”, “Value”, “Total value”):
  > What your shares are worth in tokens now. On the dashboard, total value is in SOL at each market’s smoothed price and is for display only.
- `glossary.minSharesTolerance` (the tolerance; beside “Min shares tolerance”, “Min tokens tolerance”):
  > How many fewer shares (or tokens) than the quote you accept. If the pool changes more than this before the transaction is confirmed, it fails instead.
- `glossary.credited` (credited SOL; beside “Credited to you”, “Rent credited”):
  > Rent and released income credited to your shares that you have not claimed yet. Credited rent is not guaranteed: rent a position never pays is taken back from the lenders who hold shares at that moment. Credited income does not guarantee a particular cash payout from the shared pot.
- `glossary.claimable` (claimable SOL; beside “Claimable now”, “SOL claimable”, “Claimable”):
  > What a claim would pay you now, from the SOL allocated to your shares and the market’s pot as they stand. Rent is paid in only when borrowers’ positions end, so rent credited to you may not be claimable yet. A claim that first syncs other lenders (see Market pot) can pay more.
- `glossary.vesting` (vesting; beside “Still vesting (your share)”, “Vesting”, “Vest you give up”):
  > Income credited to you bit by bit over {vestPeriod} instead of all at once: one-off income, such as your share of liquidation penalties and expiry fees, and rent paid above what accrued (the minimum rent of an early exit). If you withdraw before it has vested, the unvested part stays with the lenders who remain.
- `glossary.pot` (the market pot; beside “Market pot”):
  > Spare SOL in this market: SOL allocated to other lenders’ shares beyond their own credit, moved here when they are synced. When the SOL allocated to your shares does not cover what is credited to you, a claim can draw on the pot, and it can first sync such lenders. Claims draw on it in transaction order; an earlier claim can leave less cash for later claims against credited income. Some credited income may never be paid if the shortfall is never covered.
- `glossary.deficit` (a deficit; beside “Deficit”):
  > A negative SOL balance in this pool: rent that borrowers never paid was taken from the lenders, and your part was more than your SOL balance here. You must pay the difference in SOL (Cover deficit, on the Claim tab) before you can withdraw your tokens.
- `glossary.locked` — locked shares; beside “Locked”, “Locked shares now”, “Locked shares after”, “Lock ends”, “Current lock ends”, “Locked until”, “A new lock would end”: *verbatim, D:lender.risks ¶3*
  > You can withdraw only from tokens that are not lent out, as a pro-rata mix of tokens and SOL. While tokens are borrowed, part of your shares can stay locked until the positions they stand for have expired, plus up to {lockGrace}.
- `glossary.unpaidRent` (unpaid rent; beside “Unpaid rent in the pool”, “This deposit’s part if it is never paid”):
  > Rent the pool has credited that borrowers have not paid yet. If a position ends without enough SOL to pay it, the unpaid part is taken from the lenders who hold shares at that moment. That includes new deposits.
- `glossary.shareEpoch` (the share epoch; beside “Share epoch”):
  > How many times this pool’s shares have been reissued at a new scale. That happens after settlements in SOL, when each share stands for very few tokens or the pool has none left. Your position converts to the new shares at your next claim, deposit or withdrawal.
- `glossary.sync` (a sync; beside “Sync”):
  > Brings your position up to date with the pool’s latest share epoch and income. Your own claim, deposit or withdrawal does this too.
- `glossary.poolWiped` (a wiped-out pool; beside “Pool wiped out”):
  > Every token in this pool was lost to settlements in SOL; its lenders now hold SOL only, which they can claim.
- `glossary.realisedLoss` (the realised loss; beside “Realised loss”, “Loss / token value”):
  > What lenders lost in this market’s settlements in SOL: for each settlement that paid them less than the tokens’ value at settlement, the difference, added up.
- `glossary.lendersPnl` (the lenders’ P&L; beside “Lenders’ P&L”):
  > The SOL paid to lenders in this market’s settlements in SOL, less the value of the tokens they gave up at settlement; negative when they lost.
- `glossary.rpc` (the RPC; beside its status bar item):
  > Your connection to a Solana node, with the latest slot it has seen. Every amount you sign is read from the chain through it.
- `glossary.indexer` (the indexer; beside its status bar item):
  > short.win’s database of markets and history. Live means at most 10 seconds behind the chain, lagging at most 60, stale beyond; when it is unavailable, lists and history pause and the app reads the chain only.
- `glossary.liveFeed` (the live feed; beside its status bar item):
  > The live feed that pushes market updates from the indexer to this page. “open” means connected, “idle” that this page needs no live updates; pages also refresh on a timer, so they keep updating while it reconnects.
- `glossary.math` (the math version; beside “Math”):
  > The version of the calculation module that runs in your browser. It is the protocol’s own Rust code compiled for the web, so previews use the same calculations as the on-chain program.

<!-- copy:end -->


---

## 5. Transaction lifecycle

Every transaction goes through one pipeline (`src/tx/`), driven by `useTxFlow` and shown in
`TxReviewModal`:

```
feature ── TxPlan ──▶ simulating ──▶ review ──(Sign: ok ∧ acks ∧ < 20 s)──▶ signing ──▶ confirming ──▶ confirmed
                         ▲   │  ▲         │                                   │              │
                         │   ▼  └─Refresh─┘                                   ▼              ▼
                         └─ failed ◀──────────────────────────────────── (mapped error, Retry per `retryMode`)
```

### 5.1 Building (features; `@shortwin/client`; `src/venues`)

- **Instructions** come only from the Codama builders of `@shortwin/client`
  (`getOpenShortInstructionAsync`, `getDepositInstructionAsync`, …; PDAs from its `find*Pda`
  helpers, `ShareEpoch` metas from `getShareEpochAccountMetas`), from `@solana-program/token` /
  `token-2022` (idempotent ATA creation, CPI Guard enable / disable) and from the venue layer.
  No hand-encoded instruction data. Every signer input of a builder (`owner`, `borrower`,
  `creator`, the fee payer) is the connected wallet's `signer` from `useWallet()` — the same object
  for the whole plan — never a bare address, so Kit's `signTransactionMessageWithSigners` finds
  exactly one signer: the wallet.
- **Venue accounts** (`src/venues`, SHORT; frozen interface `src/venues/types.ts`): `ReadSet ++
  TradeExtras ++ Extras` for `open_short` / `close_short` per venue (ARCH §3.0, §3.12) — a
  TypeScript port of `services/keeper/src/venue.rs` (the same slot order and writability), with the
  slots the program checks against live state read at the same slot (the pump / PumpSwap fee and
  buyback recipients picked at random from the live `Global` / `GlobalConfig` lists, ARCH §7.3 —
  the keeper instead spreads its own exits by the mint's first byte —, and the coin-creator vault of
  the live curve / pool). Extras: the `current` set of `VenueExtras`; on a failure with
  `BadExtras` / `BadRemainingAccounts` / `DeniedAccount` (`EXTRAS_RETRY_CODES`, 6003–6005: the
  refusals of the selected set itself; *M10 batch 3:* a set naming the position's borrower or the
  adopted pool's own accounts is `DeniedAccount`), retry with the next set — `current`, none,
  `previous`, `stable` (`extrasCandidates`; ARCH §7.1 duty 10). A denied set costs one extra
  simulation until the Emergency key replaces it; nothing filters it out before sending. `detectVenue` and `raydiumCandidates` serve create-market (§3.5).
- **Prepended instructions** (idempotent, each shown in the review): ATA creation for a withdrawal
  destination; pump `extend_account` for a short curve; PumpSwap T11 `create_idempotent` after a
  coin-creator change (ARCH §5.4 case 4); `sync_lender` / `ShareEpoch` records for a lender behind on
  epochs; the post-wipe sweep bundle for a deposit into a wiped pool (§3.6); the CPI Guard bracket
  (FE-1).
- **Plan** (`TxPlan`, `src/tx/types.ts`): `kind`, `title`, `instructions` (no compute-budget ones),
  `watch` (accounts whose before / after the review shows: `lamports`, `token`, or `program` with a
  decoder and field units), `disclosures`, `disclosureVars`, extra `lookupTables`, `invalidate`
  (query keys to refetch after confirmation).

### 5.2 Message

`buildMessage` (`src/tx/message.ts`): v0 with the protocol ALT (`VITE_PROTOCOL_ALT`, ARCH §6.4
A28, built with `scripts/build-alt.sh`) plus the plan's tables, compressed with Kit's
`compressTransactionMessageUsingAddressLookupTables`; fee payer = the connected wallet's signer;
`SetComputeUnitLimit` and `SetComputeUnitPrice` first; the plan's instructions in order. Legacy
(without tables) only when the wallet cannot sign v0 (`supportedTransactionVersions`). A compiled
transaction over 1,232 bytes is refused before simulation (`kind: 'size'`; the PumpSwap open with 4
extras fits A28 only when the wallet pays its own fee, OWNER-M7-1 — it always does here).

### 5.3 Simulation and review

`simulatePlan` (`src/tx/simulate.ts`):

1. Refuse any instruction whose program is not in `TOP_LEVEL_ALLOWLIST` (§5.8).
2. Build the message exactly as it will be signed, with the compute limit at 1,400,000 and the
   chosen priority fee.
3. In parallel: `getMultipleAccounts(watch, base64, confirmed)` (pre-state) and
   `simulateTransaction(wire, { encoding: 'base64', sigVerify: false, replaceRecentBlockhash: true,
   commitment: 'confirmed', accounts: { addresses: watch, encoding: 'base64' } })`.
4. `computeDeltas(watch, pre, post)`: SOL balances, token amounts (offset 64 of a token account),
   decoded program fields (e.g. position `q`, `v`, `m`; lender `shares`, `lock`; market `T`); the
   compute-unit limit for signing = `⌈unitsConsumed·1.15⌉ + 10,000` within [20,000, 1,400,000]; the
   fee = 5,000 lamports + `⌈limit·price/10⁶⌉`.
5. A failing simulation maps its error (§5.9) and the flow stops in `failed` (Sign never enabled).
6. *v1.2 (M9-UX-1).* **Retry** does what fixes the failure (`useTxFlow` `retryMode`): a transient
   one (RPC, an expired blockhash, a wallet rejection) re-simulates the same plan; a refusal a
   fresh plan fixes — a short.win error whose action is `retry` (a moved quote under its `min_*` /
   `max_*` bounds, changed venue accounts or epoch records, a missing memo / sync / sweep step) or
   Token-2022's memo (36) / CPI Guard (42) refusal — runs `TxPlan.rebuild()`: the same user inputs
   on a fresh chain read, a new quote and new bounds, the steps re-derived (`features/lend/build.ts`
   for deposit / withdraw / claim / cover / sync, `features/short/trades.ts` + `useTradeFlow` for
   open / close), shown with `tx-rebuilt-notice` and new acknowledgements ("Retry with a new
   quote"); a plan without `rebuild` (add margin, create market) closes the review and invalidates
   its `invalidate` keys so the form re-reads and builds a new one ("Close and review again"). A
   transaction that may still land (`unknown-outcome`) or has landed (a post hoc integrity
   incident) offers no Retry. The refusal copy says "a new review …" (§5.9).

The review shows: title, stage, *v1.2:* **who signs** (`tx-wallet`: "Signing with <wallet icon and
name> · <short account>", and the caveat for a sending-only wallet; M9-SEC-8), simulation status,
**the plan's instructions** (`tx-instruction-<i>`:
program and instruction name decoded from each instruction's own program id and discriminator by
`src/tx/describe.ts` — the Codama / `@solana-program` identifiers, pump and PumpSwap by their
Anchor discriminators — never from the feature's description; *added at integration*), the delta
table (before → after → change, signed and coloured, units), compute units and fee, the priority
choice (*design pass P9-4:* a Normal / Fast chip pair — both visible, the pressed chip the selected
state, a 36 px chip with a 44 px touch area (`.hit`) under a finger — in place of a select; p75 / p95, re-simulates), the plan's
disclosures with their checkboxes,
`tx.simulation` (collapsed; *v1.2:* in every review, M9-UX-3), and errors with their action.
Titles and delta labels name the token by symbol **and** short mint ("Open short · 1,000 BONK ·
DezX…B263", "Borrowed (BONK · DezX…B263)"; M9-SEC-5). **Watched accounts per flow** (minimum): create market — wallet SOL; deposit —
wallet SOL, source token account, lender position (`shares`, `lock`); withdraw — wallet SOL,
destination token account, lender position; claim / cover — wallet SOL, lender position (`claimed`,
`pending`); open — wallet SOL, position (`q`, `s`, `m`, `v`, `deadline`), market (`tokens_free`);
close — wallet SOL, position (closed → absent); add margin — wallet SOL, position (`m`, `v`).
Decoded fields carry the generated client's names (`tokensFree`, `q`, `shares`, …), which is what
`tx-delta-<address>:<field>` uses.

### 5.4 Signing (Wallet Standard)

Discovery, connection and account selection are `@solana/kit-plugin-wallet` (`walletWithoutSigner`:
the plugin never signs implicitly; the pipeline passes the connected `signer` as fee payer). At Sign:

1. If the review is older than `STALE_REVIEW_MS` (20 s), re-simulate and return to the review with a
   notice; the user signs only numbers they saw.
2. Rebuild the message with the measured compute limit and the review's `confirmed` blockhash:
   fetched with the simulation and refreshed every 10 s while the review waits
   (`LIFETIME_REFRESH_MS`; never older than the 20 s review); the lookup tables are the
   simulation's. *2026-10-13:* nothing awaits the network between the Sign tap and the wallet —
   Android launches a wallet app (the Mobile Wallet Adapter) only within the tap's user activation.
3. A wallet with `solana:signTransaction` (Kit `TransactionModifyingSigner`) signs through
   `signTransactionMessageWithSigners`; the returned message is checked by `assertSameIntent` (§5.8)
   before anything is sent. A wallet with only `solana:signAndSendTransaction` sends itself
   (`signAndSendTransactionMessageWithSigners`); the app then confirms by signature status and checks
   the landed transaction post hoc (a mismatch is reported, not prevented — such wallets are
   labelled in the wallet menu). *As built v1.2 (M9-SEC-1):* `send.ts` `checkLanded` reads the
   landed transaction (`getTransaction`, base64, `maxSupportedTransactionVersion: 0`, `confirmed`),
   decompiles it with the same tables and runs `assertSameIntent` on it (any lifetime, the fee
   ceiling); a mismatch, or a transaction that cannot be read back, is an integrity incident
   (`LandedIntegrityError` → `integrity`, no Retry). The app cannot see which blockhash such a
   wallet sent with, so a signature not seen by the app's blockhash bound + 151 blocks (or the
   confirmation timeout) ends as `unknown-outcome` ("check the explorer before trying again"),
   never as `expired`. The wallet menu badges such a wallet (`wallet-sends-itself-badge`) and the
   review says it cannot be checked before it is sent.
4. A rejection in the wallet maps to `wallet-rejected` (`action: retry`), not to an error banner.
   *v1.2 (M9-UX-7):* only what the wallet call itself throws (`send.ts` wraps it in
   `WalletCallError`) or a wallet-specific code / name (4001, `WalletSignTransactionError`,
   `WalletRejectedError`) can read as a rejection; an RPC's "access denied", a rate limit's "request
   denied" or a load balancer's "cancelled" maps to `rpc`.
5. *v1.2 (M9-SEC-8).* Wallet identity: two registered wallets with the same name are told apart in
   the menu (keyed by identity, `wallet-option-<name> (2)`, a `wallet-duplicate-badge` with the
   registration rank and feature set); a silent reconnect (the plugin's stored `name:address`) is
   undone when another installed wallet uses that name or the wallet came back with another
   account, and the user picks the wallet explicitly.
6. *Mobile wallets (M10-RESP; O-6 lifted by the owner on 2026-10-13).*

   - **In-app browsers.** Inside a wallet app's own browser (Backpack, Phantom, Solflare), Wallet
     Standard is injected and the path above works unchanged.
   - **Open-in links.** On a mobile device (`isMobileDevice`: client hints, the UA, iPadOS's
     Macintosh-with-touch, a coarse non-hovering pointer) the chooser (`WalletChooser`, shared by
     the header and `ConnectCta`) offers "Open in <wallet>". Each is the wallet's universal browse
     link to this very page, listed alphabetically (`src/wallet/deepLinks.ts`; formats re-checked
     on 2026-10-13, re-check at each release): `https://backpack.app/ul/v1/browse/<url>?ref=<origin>`,
     `https://phantom.com/ul/browse/<url>?ref=<origin>`, `https://solflare.com/ul/v1/browse/<url>?ref=<origin>`
     (URL-encoded; rendered only as a tapped `<a>`, never for pasting).
     - Without a wallet, they are the dialog's content, with the cluster hint off mainnet ("Set
       your wallet to <cluster> first."), the wallets' sites and Reload. A desktop without a wallet
       gets the sites and Reload.
     - With wallets listed, they form a block after the entries under "Or open this page in a
       wallet app" (`wallet-open-elsewhere`), so the route survives a wallet registering.
     - In a social app's in-app browser without a wallet (`isInAppBrowser`, FE-33), the chooser
       first says "Open this page in your browser", with "Copy link".
     - These are top-level, user-initiated navigations.
   - **The Mobile Wallet Adapter** (`@solana-mobile/wallet-standard-mobile` 0.7.0, protocol 3.0.0).
     - It is on by default; `VITE_ENABLE_MOBILE_WALLET_ADAPTER=false` turns it off (§2.1).
     - It registers a Wallet Standard wallet in a secure context on Android browsers that are
       neither a WebView nor Firefox (`mwaEligible`). The package is loaded lazily by
       `src/wallet/mobileWalletAdapter.ts`, from `App.tsx` (a framed page never registers it).
     - It uses `appIdentity` ShortWin, the origin and `/icon-192.png`, the build's chain, and a
       chain-scoped authorization cache. It has no remote / QR association and no library dialogs.
     - Its entry reads "Use installed wallet" over "Opens your wallet app on this phone". Chrome's
       "Apps on device" permission is asked by the app from a tap, before Connect and before Sign
       (`src/wallet/localNetwork.ts`; while it is missing the entry and the review say "Allow 'Apps
       on device' so this page can reach your wallet app").
     - The library's own UI is never shown: its spinner and modal hosts are removed as they are
       inserted (`removeLibraryUi`), and the app passes its own `onWalletNotFound` (no library
       error modal; the header chooser reopens with the open-in links instead).
     - A reload reconnects silently from the cached authorization (local storage only; the wallet
       is not asked), without opening the wallet app. An account changed in the wallet shows up at
       the next Sign, which then ends with "Connect a wallet that can sign." Disconnect forgets the
       authorization locally; the wallet's token stays valid until it expires or is revoked in the
       wallet.
     - An MWA wallet signs only through `solana:signTransaction`, checked by `assertSameIntent`
       before anything is sent. One that cannot (no `solana:signTransactions`, an MWA 1.x wallet
       without sign-and-send, or per-account features that hide `signTransaction` from Kit) gets
       no signer, like a read-only wallet (`mwaSignerOk`, `useWallet`). The sending-only path
       (FE-11, FE-21) is never used for MWA, and its entry never carries the "sends itself" badge.
     - The tap reaches the wallet with no network call in between (step 2). If Android still
       refuses the launch, the review stays open with "Couldn't open your wallet app. Tap Sign
       again".
     - The library has calls that never settle (FE-35). While the review waits for an MWA wallet,
       Cancel is enabled after 5 s and closes the review (nothing is sent); a connect the wallet
       never answers is ended 10 s after the page is back in front (at most 120 s), and the
       library's connect lock is reset (`src/wallet/mwaCall.ts`).
     - Errors are as in §5.9.
   - No WalletConnect / Reown.

### 5.5 Send and confirm

Kit's `sendAndConfirmTransactionFactory({ rpc, rpcSubscriptions })` with preflight at `confirmed`
(the second simulation catches state that moved since the review) and confirmation at `confirmed`
within the blockhash's `lastValidBlockHeight`; the signature is shown as soon as it is known
(stage `confirming`), with an explorer link (`VITE_EXPLORER_TX_URL`). Block-height expiry maps to
`expired` (Retry re-simulates and signs again: a new transaction, never a re-broadcast of an expired
one). On confirmation: the plan's `invalidate` keys and the wallet balance are invalidated, a success
toast is pushed, the modal shows Done. The modal cannot be dismissed while signing or confirming.
*M10-RESP:* a flow inside a sheet (the trade sheet, Manage) passes `TxDoneContext`: closing a
confirmed review there — Done, its ✕ or Escape — also closes the sheet (back or replace, like the
sheet's ✕), so the user lands on the page with the toast whole; a review closed before it is
confirmed (Cancel, a wallet cancel, a failure) leaves the sheet open. On a desktop nothing else
closes. A wallet cancel's stage reads "Cancelled".
The confirmed banner links the flow's next step (View position); the explorer link is the
signature row's "Open in explorer" only.
*v1.2 (M9-SEC-4):* the lifetime confirmed against is always explicit (`confirmationLifetime`): ours
when the wallet kept our blockhash; when it chose its own, that one only while `isBlockhashValid`
says so, bounded by the current block height + 151 (Kit would otherwise confirm a changed message
against `lastValidBlockHeight` = 2⁶⁴ − 1, forever). Confirmation has a hard timeout
(`CONFIRM_TIMEOUT_MS`, 120 s) after which the review is dismissible in `unknown-outcome`. When the
wallet changed the compute budget, the fee as it signed it is shown (`tx-final-fee`, FE-2).

### 5.6 Compute budget and priority fee

Compute limit from the simulation (§5.3). Priority fee (`src/tx/priorityFee.ts`): the 75th (Normal)
or 95th (Fast) percentile of `getRecentPrioritizationFees` over the transaction's writable accounts,
clamped to [1,000, `VITE_PRIORITY_FEE_MAX_MICROLAMPORTS`] µlamports per CU (FE-6); 0 on localnet.
The review shows the resulting fee in SOL; a wallet that raises it (FE-2) is shown the final fee.

### 5.7 Token accounts: CPI Guard, memo-required destinations, ATAs (ARCH §3.3; R-MINT §8.4)

`src/tx/tokenAccount.ts` parses token accounts by hand (base layout 165 B, Token-2022 account-type
byte 2 at 165, TLV from 166): `CpiGuard` (extension 11, `lock_cpi`) and `MemoTransfer` (extension 8,
`require_incoming_transfer_memos`).

- **Deposit from a CPI-Guarded account:** the program's `transfer_checked` CPI with the owner's
  signature fails Token-2022 error 42 (`CpiGuardTransferBlocked`). The panel detects the flag before
  building (`readOwnerTokenAccounts`), disables the plain deposit, shows `token.cpiGuard`, and offers
  (FE-1) the bracket `[DisableCpiGuard(owner), deposit, EnableCpiGuard(owner)]` — all top-level,
  owner-signed, in one transaction; the review shows all three. If a deposit nevertheless fails with
  42 (the flag changed between read and send), the error maps to the CPI Guard message.
- **Withdrawal to a memo-required account:** `withdraw`'s `memo_program` = spl-memo v3
  (`MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr`), so the program CPIs a memo right before its
  `transfer_checked` (M1-SEC-7); otherwise the short.win program id ("none"). Without it the
  program refuses `MemoProgramRequired` (6095); Token-2022's `NoMemo` (36) maps to the same advice.
  *M10 (TMEMO-9):* every Token-2022 withdrawal passes spl-memo v3 — the program CPIs it only when
  the destination requires memos at execution —, so a destination that turns memo-required after
  the review still lands; the memo flag only drives `token.memoRequired` and the title. Verified on
  the mainnet Token-2022 and spl-memo v3 ELFs (T-MEMO, M10): LiteSVM `security::sec_7_*` and the
  E2E, whose validator runs the same ELFs (§10.2). A mainnet transaction is M11's (TMEMO-10).
- **ATAs:** a missing withdrawal destination is created with `createAssociatedTokenIdempotent` for
  the mint's token program (`mint.owner`; never assume SPL Token, R-MINT §8.5).

### 5.8 Integrity: no blind signing (`src/tx/integrity.ts`)

1. `assertAllowedInstructions` (*v1.2, M9-SEC-7:* per instruction, not per program;
   `src/tx/allowlist.ts`): every top-level instruction of a plan is one of short.win's own
   (identified by its discriminator), ATA `CreateIdempotent`, Token-2022 `EnableCpiGuard` /
   `DisableCpiGuard` (the bracket, FE-1), pump `extend_account` / `claim_cashback`, PumpSwap
   `init_user_volume_accumulator` / `claim_cashback` (the post-wipe bundle). Compute-budget
   instructions are the pipeline's own (`message.ts`), never a plan's. System and SPL Token are
   not allowed (no plan invokes them at the top level; `TOP_LEVEL_ALLOWLIST` lists the five
   programs; the `complete_curve` helper is retired, design pump-final D2). Anything else is
   refused before simulation; `describe.ts` names the review's instructions from the same table
   and marks a refused one.
2. `assertSameIntent(expected, signed, tables)`: identical message bytes pass; otherwise both are
   decompiled and must have the same fee payer and the same non-compute-budget instructions
   (program, accounts with roles, data) in the same order (FE-2), and (*v1.2, M9-SEC-4*) the same
   lifetime token unless the caller validates a changed one (§5.5). Any other change throws and
   nothing is sent.
3. The review always shows what was simulated; amounts are never taken from the wallet's display.

### 5.9 Error mapping (`src/tx/errors.ts`, `src/config/errors.ts`)

| Source | Mapping |
|---|---|
| `{ InstructionError: [i, { Custom: n }] }`, n ∈ 6000–6109 (*M10:* 6105 `AuthoritiesNotDistinct`, action `admin`; 6106 `EpochRecordReadOnly`, `retry`; 6107 `RebaseNeedsOwner`, `blocked`; *AIR-07:* 6108 `PoolDrained`, `wait`; *AIR-08:* 6084 `WipeSweepPending`'s message also covers a pool every lender left; *pump v3 (D1 (a), D4):* 6109 `CurveCashbackCoin`, `blocked`, and 6031 `CurveStraddle`'s new text, both the program's own as drafted, §4.3), failing program short.win (from the logs' **first** "Program … failed": the innermost failing program logs first and every caller up the CPI stack repeats the code after it; v1.0 said "last", which attributed a CPI'd Token-2022 error to short.win — fixed at integration) | `config/errors.ts`: name, message, action (`retry`, `adjust`, `wait`, `blocked`, `report`, `admin`) |
| same, failing program Token / Token-2022 | 1 insufficient tokens, 4 owner mismatch, 17 frozen, 36 memo required, 42 CPI Guard |
| `InstructionError` built-ins | `InsufficientFunds`, `ComputationalBudgetExceeded`, others → `report` |
| transaction-level | `InsufficientFundsForFee`, `InsufficientFundsForRent`, `AccountNotFound`, `BlockhashNotFound`, `AccountInUse`, block / account cost limits, `ProgramAccountNotFound` |
| Kit `SolanaError` with a preflight `transactionError` and logs | as above |
| wallet | rejection (`4001`, "rejected", `AbortError` — *v1.2:* only from the wallet call, `WalletCallError`) → `wallet-rejected`; another wallet failure → `wallet`; a refused intent check (`IntegrityError`, §5.8) → `integrity` (`report`); a landed transaction that differs → `integrity` (no Retry) |
| transport | network, HTTP status, "denied" / "cancelled" / rate limit outside the wallet → `rpc` (retry); block height exceeded → `expired`; *v1.2:* no confirmation in time, or a sending wallet's signature not seen → `unknown-outcome` (no Retry) |
| plan | *v1.2:* a rebuild that the fresh chain refuses (`PlanError`) → `plan` (`adjust`) |
| the Mobile Wallet Adapter (*O-6, 2026-10-13*; §5.4 (6)) | found on the wallet call's `cause` chain (at most 6 levels, by `name` and `code`, never `instanceof`: the package is lazy), first in the `WalletCallError` branch. `SolanaMobileWalletAdapterProtocolError` −3 (`ERROR_NOT_SIGNED`) on `sign_transactions` → `wallet-rejected`; its other codes (−2, −4, −5, −100), `ERROR_SESSION_CLOSED`, `ERROR_ILLEGAL_TRANSPORT_STATE`, `ERROR_INVALID_PROTOCOL_VERSION`, `ERROR_FORBIDDEN_WALLET_BASE_URL`, `ERROR_SECURE_CONTEXT_REQUIRED` → `wallet` (generic, Retry: the signing path sends nothing itself); `ERROR_ASSOCIATION_CANCELLED` with an event (a library modal closed, reachable only in the local-network race) → `wallet-rejected`, without one → `wallet`; `ERROR_WALLET_NOT_FOUND` → `wallet-launch`; `ERROR_LOOPBACK_ACCESS_BLOCKED` → `local-network`; `ERROR_SESSION_TIMEOUT` → `local-network` while Chrome's permission is missing, else `wallet`. `wallet-launch` and `local-network` never reach the failed state: the review stays open, Sign enabled, with its notice beside Sign (`tx-sign-notice` `data-reason` ∈ `launch` "Couldn't open your wallet app. Tap Sign again", `local-network` "Allow 'Apps on device' so this page can reach your wallet app"). A failed *reauthorization* at Sign never reaches the app as an error (the library disconnects and its call never settles): the connected MWA account changing or disconnecting mid-sign ends the sign at once as `wallet-changed` (`blocked`, "Connect a wallet that can sign.", retry mode `close`: "Close and review again"; R-S2). Any MWA transport error on the sending branch (`WalletCallError.sending`) → `unknown-outcome` (unreachable: MWA never sends itself). While the review waits for an MWA wallet, Cancel (and ✕, Escape) is enabled 5 s after the wallet call starts and closes the review; the abort is not reported as a rejection (R-S1) |
| Anchor framework errors, failing program short.win | *M10:* 3012 `AccountNotInitialized` (a lender position closed meanwhile, KI-67) → `program`, `retry` (the review rebuilds the plan); *ninth round (FE-30):* 3007 `AccountOwnedByWrongProgram` when the logs name the `lender` account (a closed position's address someone funded) → the same; 3007 on another account, or either code from another program → `unknown` (`report`) |

The `txError` element carries `data-kind` and `data-code`. `action` drives the UI: `retry` →
Retry (re-simulate a transient failure; rebuild the plan, or close for a new review, after a
refusal a fresh plan fixes — §5.3 step 6), `adjust` → focus the input, `wait` → explain and keep
the form, `blocked` → disable the action with the reason, `report` → show the code and logs
(copyable). *v1.2 (M9-UX-1):* the refusal copy promises only what happens: "Review again with a
new quote", "A new review adds that step" — never "Try again: the app adds it" for a re-simulation.

*M10 batch 4 (PUMP-2026-10-2; disclosed, the owner's decision of 2026-10-03):* a plan may carry
`refineError(e)`, which the flow applies to a failed simulation's mapped error — the message
only; kind, code and action stay. The open plan sets it when the venue account its sale would
grow is short (`venueGrowth`: a PumpSwap pool under 287 bytes, P0, or a curve under 166 bytes,
C0, of the snapshot's ReadSet — 141 until pump's 2026-10-08 redeploy, which grows every shorter
curve to 166 on its next trade, 76,200–259,080 lamports from the listable 151–115 bytes at 5,080
per byte, PUMP-2026-10-2): since the pump venues' 2026-10-02 redeploys the first trade on
such an account grows it and charges that trade the rent (81,280–386,080 lamports measured),
which the program nets out of the proceeds, so an open whose tolerance cannot cover it fails
`ProceedsBelowMin` though no price moved. short.win's `ProceedsBelowMin` there reads "This pool
needs a first trade to grow its account: PumpSwap charges that trade the account’s rent, which
this open’s slippage tolerance cannot cover at this size. Try again after another trade on the
pool, or with a larger size." (the curve's names the bonding curve and pump;
`VENUE_GROWTH_MESSAGES`). PumpSwap's own 6063 (`InsufficientRealQuoteReserves`, the same number)
fails in pAMM and maps to `unknown`, untouched (`growth.test.tsx`). The refinement applies only
when the error's logs name short.win as the failing program: the mapper also reads a 6063 whose
logs name none (truncated logs) as short.win's, which could be PumpSwap's, so such a refusal keeps
the generic message (batch 4's review). The length is the snapshot's, so a trade that grows the
account between the plan and the simulation leaves the specific message on a `ProceedsBelowMin`
the price move caused; its advice, to retry, still leads to a correct review (accepted, batch 4's
review: the only case where the message is not about the growth).

### 5.10 Per-instruction parameters

| Instruction | Arguments from the UI | Bounds |
|---|---|---|
| `create_market(venue_hint, max_creation_fee)` | Auto 0, Raydium v4 4, CPMM 5 (Mock 6 test-only) | `max_creation_fee` = the fee read from `GlobalConfig` |
| `deposit(amount, min_shares)` | amount | `min_shares = ⌊previewDeposit·(1 − 0.5 %)⌋` |
| `withdraw(shares, min_tokens, min_sol)` | shares | `min_tokens = ⌊tokens·(1 − 0.5 %)⌋`, `min_sol` = quoted SOL (vest incl.) |
| `claim_sol()` | — | — |
| `cover_deficit(lamports)` | the deficit | exact |
| `sync_lender(max_epochs)` | — | ≤ the epochs one transaction holds |
| `open_short(nonce, q, min_proceeds, max_margin, max_open_fee)` | q, slippage | §3.7; nonce random u64 |
| `close_short(max_buyback_cost)` | slippage | `⌈B̂_live·(1 + slippage)⌉` |
| `add_margin(amount)` | amount | ≤ wallet SOL − fee reserve (0.01 SOL kept) |

*pump v3 (2026-10-15; design pump-final §6.3, §8, owner D1 (a)):* pump's v3 trades refuse a
cashback coin (6094), so a **live** cashback curve cannot be listed: `classifyPump` refuses it with
`CurveCashbackCoin` (6109) as the program's `classify_pump_with` does, on the live branch only, and
Create market shows the program's text as drafted (§4.3) above the code; a live curve of depth ≠ 0
is refused `CurveQuoteNotSol` (defence in depth). A complete cashback curve still lists
(`PumpMigrating`, then PumpSwap once it graduates), and on a market whose curve is a live cashback
one the venue is not executable (`readVenueState`: observed only once complete, §6.1.6).

### 5.11 A close past the curve's `rT` (FE-3 closed)

*v1.0–2026-10-13:* a close of `q > rT` on a live curve failed `CurveStraddle`, and the position page
showed `closeStraddleNotice`; the exit was the keeper's 3-transaction Jito bundle. **Since the
v3 adapter (2026-10-15, design pump-final §5.2, §8; owner D3):** it is an ordinary close. One
`close_short` buys `rT` on the curve and `q − rT` from the would-be pool by pump's synthetic
migration (`buy_v3`) and moves the market to `PumpMigrating` in the same instruction. The preview
prices `B̂_v3` — the curve leg and the pool leg at the curve's fee on the mint's live supply, `φ_s`
(`CurvePricingInputs.syntheticFees`; the facade's `withSynthetic`) — and the transaction, the
review and `max_buyback_cost` are the usual close's. A curve without that route (cashback, mayhem, a
read without the mint's supply) has no buyback quote past `rT`, so the form says "No buyback quote
now" and offers nothing to sign. The notice and its words are removed (D4); `CurveStraddle`'s new
text maps if the program ever returns it (a buyback beyond what the curve and its would-be pool
hold, unreachable under the per-market cap, pump-final C8). The risk view prices the same path
capped (CR, liquidation price), and on a `PumpMigrating` market the EMA leg and FV price the pool
`migrate_v2` will create: after a synthetic pool part its tokens are the live `curve_base_ata` and
its SOL includes `pcQ`, FV ρ-scaled, and no FV when the post-complete fields are not known
(`fvRepriced`, the facade's `migratingFv`; pump-final §5.4, §6.1.5).

---

## 6. Data layer

### 6.1 Sources

| Source | Used for | Client |
|---|---|---|
| Indexer REST (`/v1/*`, INDEXER §9) | lists, search, history, analytics, creator exposure, coverage, venue status | `src/api/client.ts` (`createApiClient`), `src/api/queries.ts` |
| Indexer WebSocket (`/v1/ws`) | live invalidation and in-place market rows | `src/api/ws.ts` (`IndexerSocket`), `src/api/live.tsx` |
| RPC (Kit) | every account a transaction or preview depends on; the fallback when the indexer is down | `src/rpc/*`, `@shortwin/client` decoders |
| RPC subscriptions | the slot (staleness), account changes of the open market / positions (SHORT, LEND) | `src/rpc/slot.ts`; features |
| WASM math | quotes, margin, CR, liquidation price, rent, caps, tiers, lender previews | `src/math/*` |

The API is GET-only and CORS-exposes `X-Indexer-Slot` / `X-Indexer-Finalized-Slot`
(`services/indexer/src/api/mod.rs`). Requests carry no credentials and no referrer.

### 6.2 Query keys (`src/api/keys.ts`)

Two roots. `['indexer', …]`: `status`, `search(q)`, `markets(params)` (`params.cursor` for keyset
pages: each page is its own query, so the `markets` channel patches rows on every loaded page),
`market(mint)` and its
sub-keys (`positions`, `lenders`, `pools`, `coverage`, `liquidations`, `events`, `snapshots`,
`trades`, `candles`, `stats`), `position(pubkey)`, `borrower(owner, …)`, `lender(owner)`, `stats`,
`liquidations`, `settlements`, `keepers`, `admin/*`, `venues`, `tx(sig)`. `['chain', …]`: `slot`,
`account(addr)`, `config`, `venue-guard`, `oracle`, `market(mint)` and `…/vault`, `…/venue`,
`…/lender/<owner>`, `…/exposure/<owner>`, `position(pubkey)`, `balance(owner)`,
`token-accounts(owner, mint)`, `mint(mint)`, `alt(list)`. Components never write key literals.
*As built:* the protocol accounts several pages show are read through one hook each in
`src/rpc/queries.ts` (`useChainMarket`, `useChainConfig`, `useChainVenueGuard`, `useChainOracle`),
so a key always holds one shape (`MaybeAccount<…>` from `rpc/accounts.ts`); feature reads hang
their keys below these (`[...qk.marketAccount(mint), 'lend', owner]`, `…'short-snapshot'`), so a
prefix invalidation after a confirmed transaction refreshes them all. short.win's event authority
(every builder names it) is derived once in `src/rpc/pda.ts`.

### 6.3 Timings

| Data | staleTime | Refetch |
|---|---|---|
| `/v1/status` | 2 s | every 10 s (freshness) |
| `/v1/stats` | 2 s | every 10 s and the `stats` channel |
| lists and market detail | 2 s | WS invalidation; no focus refetch |
| `/v1/admin/config` | 60 s | — |
| `/v1/venues` | 2 s | every 30 s and the `admin` / `coverage` channels |
| chain accounts behind a preview | 0 | on the account subscription, at most once per slot |
| protocol ALT | 10 min | — |

Indexer retries: 429 / 503 / network up to 4 times honouring `Retry-After`; other 4xx never.

### 6.4 Live updates

`IndexerSocket` subscribes reference-counted channels (`useLiveChannels`), resumes with
`since = last event id` after a reconnect, closes a connection silent for 45 s (heartbeats are
20 s), backs off exponentially with jitter (1 s → 30 s; 16 s+ after a policy close 1008).
`applyWsMessage`: `market:<mint>` events invalidate `['indexer','market',mint]`; `position:<pk>`
the position and borrower lists; `lender:<owner>` the lender; `liquidations` the global list;
`admin` / `coverage` venues and config; a `markets` summary patches every cached markets page in
place; `stats` replaces the stats query; `rollback`, a lagged close (4008) or a replay error
invalidates every indexer query. Channels per page: markets — `markets`; market page —
`market:<mint>` (+ `trades:<mint>`); positions — `position:<pk>` per open row (≤ 40); lend —
`lender:<owner>`; status — `admin`, `coverage`.

### 6.5 RPC reads and the fallback

`src/rpc/accounts.ts` reads `Market`, `MarketVault`, `LenderPosition`, `WalletExposure`,
`ShortPosition`, `GlobalConfig`, `VenueGuard`, `OracleCache` through the generated `fetchMaybe*`
at `confirmed`; `src/rpc/mint.ts` and `src/rpc/tokens.ts` read mints and the owner's token
accounts. While the indexer is down (`fallbackBanner`): the market page renders from the chain
(no history / analytics tabs); positions and the lender dashboard use `getProgramAccounts` with a
`memcmp` on the borrower / owner field and the account size; the markets list is replaced by
"search a mint" (no `getProgramAccounts` over all markets on mainnet). Previews always use RPC.

### 6.6 Staleness

`lib/staleness.ts`: lag = (RPC slot − `X-Indexer-Slot`) × 0.4 s; `live` ≤ 10 s, `lagging` ≤ 60 s,
`stale` beyond, `down` when `/v1/status` fails, `unknown` before the first response. On devnet only,
when the chain confirms `GlobalConfig` is absent and `/v1/status` has responded recently, the
indexer shows a neutral **awaiting activity** state: no protocol markets can emit transactions yet,
so the last indexed slot is not a useful measure of current ingest coverage. This state is not
labelled live and expires if status responses stop. The status bar
always shows it; indexer-backed panels show a `staleBadge` when not `live`, with the age in seconds when applicable
(*v1.2, M9-UX-6:* the markets page, the market header, the status page, the positions list, the
lender dashboard and the position timeline). A preview's account reads show their slot age when it
exceeds 2 slots behind the confirmed tip (the RPC's slot less its usual 2-slot confirmation lag;
`shell/SlotAge.tsx`, `slot-age-badge`): the Short panel (its snapshot), the Lend panel (its chain
read — current while every account subscription is live) and the position page; above 75 slots
or 30 s the read is `stale` and the panel's Submit waits for a fresh one. A polled list without a
slot (the positions' accounts) shows its age above 10 s.

### 6.7 Previews with the WASM math

`src/math/load.ts`: `loadMath()` initialises `@shortwin/math-wasm` once (Vite emits the `.wasm`
as an asset; CSP `'wasm-unsafe-eval'`), `useMath()` exposes it; `initMathSync(bytes)` for tests.
Rules: (1) the frontend never re-implements protocol math — every protocol number comes from an
export of the facade (crate README "Exports"); (2) inputs are the live on-chain state and the live
`GlobalConfig` terms (a position's locked terms for an open position); `defaultTierTerms` (the
compiled SPEC table) is for explanations only; (3) WASM objects are freed after use (`free()` or
`using`); (4) an `undefined` result (e.g. a quote with `q ≥ X`) renders "—" and disables Submit.
*M10 (FE-15):* the last second copies are gone — the open guards (`spotNearEma`, `pOpenMax`), the
pool-selection comparisons (`lambdaBps`, `lockedDepth`, `lockedSol`, `lockedFloorOk`), the mint
verdict (`mintCheck`) and the full withdrawal (`previewWithdrawal`) are facade exports; the static
flag and timer guards and the boost gate, which the facade does not export, stay in
`features/short/previews.ts`.

---

## 7. Design system — dark trading terminal

**The product's name** (the owner's decision of 2026-10-11, thirteenth round): running text says
**short.win**, as the disclosures (§4.2) and the copy of §4.3 ("New to short.win?") spell it;
**ShortWin** is the brand mark only: the header's wordmark (`AppShell`, "ShortWin home"), the tab
title and the page description (`index.html`), and the start-up messages — `index.html`'s
`<noscript>` ("ShortWin needs JavaScript."), `main.tsx`'s "ShortWin could not start." and
`FrameGuard.tsx`'s "ShortWin does not run inside another page.". No string changed with the rule.

**The brand mark** (`src/shell/Logo.tsx`, the design pass of 2026-10-16): the W of "Win" drawn as a
price chart — a full-height fall and a small bounce in the text colour (the short) joined through a
low, sharp apex to a check in the accent gold (the win). An inline 24 × 24 SVG with presentation
attributes only (CSP): the fall half is `currentColor` (the `.brand` link's `--text-0`), the win half
`.brand-mark-win { fill: var(--accent) }`; 24 px beside the wordmark (8 px gap), 30 px alone under
400 px (cap and baseline on whole pixels at every DPR). Loss red stays with the numbers: the mark uses
none. The same W on a `--bg-1` tile is the favicon (`public/favicon.svg`, rebuilt on the 16 px grid:
a 4-unit stroke so the stems are two solid pixels at 16 px; `favicon-16.png` / `favicon-32.png` /
`favicon.ico` for Safari and legacy browsers), the iOS home-screen icon (`apple-touch-icon.png`, 180 px)
and the web app manifest's icons (`manifest.webmanifest`: `icon-{192,512}-any.png` and
`icon-512-any.svg` with rounded transparent corners for surfaces that do not mask; full-bleed
`icon-{192,512}-maskable.png` with the W inside the central 80 % for Android's masks). It replaced
the text "▼" glyph and the falling-line favicon.

### 7.1 Colour tokens (`src/styles/tokens.css`)

*Design pass P1 (2026-10-16, the tokens package):* the rows marked *P1* were added or changed by it; a row names
the package that consumes the token (all landed: P2–P9 and the P11 cleanup). Every ratio below was recomputed
(WCAG 2.x relative luminance) when the row was written.

| Token | Value | Use | Contrast (on `--bg-0` / `--bg-1` / `--bg-3`) |
|---|---|---|---|
| `--bg-0` | `#0b0e11` | app background | — |
| `--bg-1` | `#12161b` | panels, table body | — |
| `--bg-2` | `#181d23` | table header, menus, notes (disclosure, banner, creator box); *P1:* hover and press states step to `--bg-hover` / `--bg-4` instead | — |
| `--bg-3` | `#1f252c` | inputs, selected row | — |
| `--line` | `#2a323b` | hairlines (decorative) | — |
| `--line-strong` | `#66727f` | input borders | 3.94 / 3.70 / 3.15 (≥ 3:1) |
| `--text-0` | `#e8ecf1` | primary text | 16.31 / 15.31 / 13.02 |
| `--text-1` | `#aab4bf` | labels | 9.21 / 8.64 / 7.35 |
| `--text-2` | `#919cab` (*P1*; was `#8a95a1`) | muted | 6.96 / 6.53 / 5.55 (6.09 on `--bg-2`, 5.08 on `--bg-4`) |
| `--accent` | `#f0b90b` | primary action, active nav | 10.74 / 10.07 / 8.57 |
| `--focus` | `#8ab4ff` | focus ring | 9.26 / 8.69 / 7.40 |
| `--gain` | `#2ebd85` | P&L gain, healthy | 8.05 / 7.55 / 6.43 |
| `--loss` | `#ff5c6c` | P&L loss, short action, danger | 6.45 / 6.05 / 5.15 |
| `--risk-safe` / `-watch` / `-danger` / `-critical` | `#2ebd85` / `#f0b90b` / `#ff8a3d` / `#ff5c6c` | position health | ≥ 5.15 on each tinted badge background |
| `--info` | `#5aa2ff` | links, info banners | 7.42 / 6.96 / 5.92 |
| `--tier-a` … `--tier-d` | `#36c5a0`, `#5aa2ff`, `#f0b90b`, `#ff6b8a` | tier badges (A safest → D riskiest) | ≥ 5.68 |
| *P1* `--bg-4` | `#242c35` | hover / pressed / selected on a bg-2 surface (menus, search options, chips, default buttons); never behind an input border (`--line-strong` is 2.88 on it) | on it: `--text-0` 11.91, `--text-1` 6.72, `--text-2` 5.08, `--info` 5.41, `--loss` 4.71, `--gain` 5.87, `--accent` 7.84 |
| *P1* `--bg-hover` | `#1f252c` (= `--bg-3` by design) | hover on an interactive bg-1 row; the press colour of tabs, the nav, the phone's More and interactive rows (`base.css`). A selected row, if one is ever introduced, takes `--bg-4` | 1.18 on `--bg-1`; retune here, no lighter than `#262d36` (so `--text-2` keeps 4.5:1 on it), never by pointing rows at `--bg-4` |
| *P1* `--surface-well` / `--surface-note` / `--surface-nested` | `var(--bg-0)` / `var(--bg-2)` / `var(--bg-3)` | the three surface roles, one meaning each: read-only figures (previews, tx steps, the lend position) / a note (disclosure, banner, menu, creator box) / a box inside a note | as their values |
| *P1* `--line-panel` | `#363f4b` | the outer edge of panels, cards, wells and popovers (`--line` stays for in-panel hairlines) | 1.70 on `--bg-1` (decorative) |
| *P1* `--meter-track` | `#3d4753` | the utilisation track | 1.92 on `--bg-1` (decorative) |
| *P1* `--scrollbar-thumb` | `var(--line-strong)` | panel scrollbars | — |
| *P1* `--tag` / `--tag-bg` | `#b9c6d6` / `#1c242e` | informational badges, so link blue is reserved for links | 9.03 on `--tag-bg` |
| *P1* `--icon-muted` | `#737e8a` | the resting glossary ⓘ and other icon-only marks — **SVG icons only**, stroked in `currentColor`, never a text glyph (below AA for text on bg-2 / bg-3) | 4.68 / 4.39 / 3.74 (4.10 on `--bg-2`): ≥ 3:1 as non-text |
| *P1* `--warn` | `#ffa726` (was `#f0b90b`) | caution is amber: every `--warn` consumer is a caution surface (the binding size cap, "Coverage paused", a lagging badge, the status strip's ▲ state, the high-utilisation meter fill, the intro's "What can go wrong", caution left bars) and none puts `--text-inverse` on it; `--risk-watch`, `--tier-c` and `--accent` keep gold, so gold means the primary action or the selected state only | 9.96 / 9.35 / 7.95; 7.39 on `--warn-bg` (which stays `#33290a`) |
| *P1* `--accent-active` | `#d9a60a` | the primary button's press (P3-3) | `--text-inverse` 8.67 on it |
| *P1* `--loss-hover` | `#ff7281` | the Short button's hover / press (P3-3) | `--text-inverse` 7.35 on it |
| *P1* `--edge-highlight` | `inset 0 1px 0 rgb(255 255 255 / 3%)` | a panel's 1 px top highlight | — |
| *P1* `--shadow-header` / `--shadow-up` / `--shadow-pop` / `--shadow-modal` / `--shadow-sheet` / `--shadow-toast` | `0 1px 0 var(--line-panel), 0 6px 16px rgb(0 0 0 / 35%)` / `0 -12px 20px -12px rgb(0 0 0 / 65%)` / `0 12px 32px rgb(0 0 0 / 55%)` / `0 16px 48px rgb(0 0 0 / 60%)` / `0 -12px 32px rgb(0 0 0 / 60%)` / `0 8px 24px rgb(0 0 0 / 50%)` | one shadow per floating layer: the header (its 1 px line first, so it keeps a hairline edge), the action bar and the sticky form foot, menus / popovers / search results, the modal, a sheet, a toast | — |
| *P1* `--title-fs` / `-fw` / `-ls` / `-color` | `var(--fs-md)` / `var(--fw-bold)` / `0.06em` / `var(--text-0)` | the panel-title tier: 13 px bold white uppercase (never confused with `--fs-title`, the 20 px page h1) | — |
| *P1* `--section-fs` / `-fw` / `-ls` / `-color` | `var(--fs-sm)` / `var(--fw-bold)` / `0.05em` / `var(--text-1)` | the section tier: 12 px bold light grey uppercase (13 px on a phone) — the base `h3` | — |
| *P1* `--label-fs` / `-fw` / `-ls` / `-color` | `var(--fs-xs)` / `var(--fw-medium)` / `0.04em` / `var(--text-2)` | the label tier: 12 px medium muted uppercase | — |
| *P1* `--fs-title` | `20px` (every width) | the page `h1` (`base.css`, −0.01em; it was 18 px on a desktop) | — |
| *P1* `--fs-hero` | `var(--fs-2xl)` (22 px; 24 on a phone) | a hero number — the market head's EMA price (P5-3) and the position page's CR (P6-2) — so a hero number always outranks the title | — |
| *P1* `--fs-prose` / `--lh-prose` | `14px` (15 on a phone) / `1.55` | reading-page prose (How it works, Security; P8) | — |

Semantics: **gain / loss are the user's P&L** (for a short, a price fall is a gain); price direction
in charts uses the same pair with an arrow. Risk: `riskLevel(CR, CR_ema, cr_liq)` (§3.8) —
critical strictly below the liquidation CR on both legs, below on the live leg alone, danger within
+10 %, watch within +30 %, safe beyond. Colour never carries meaning alone (signs, arrows, words:
§8). Prices carry a visible unit (`Price`: "0.0₇28 SOL", SOL per token in `title` and the
screen-reader text; *v1.2, M9-UX-8*).

### 7.2 Typography and numbers

UI: system stack (`Inter` if installed); numbers: monospace stack (`JetBrains Mono`, `IBM Plex
Mono`, `ui-monospace`, …) with `font-variant-numeric: tabular-nums slashed-zero` (`.num`). Base
13 px (`--fs-md`), 12–22 px scale. *Thirteenth round (O-4):* **no text under 12 px at any width** —
`--fs-xs` is 12 px from 640 px up as it already was on a phone (so `--fs-xs` = `--fs-sm` = 12 px on a
tablet and a desktop), and a unit after a number is `max(0.9em, 12px)`; no density switch, and no
other size changed (the terminal stays dense). The responsive gate checks it at every viewport
(§10.5). What it costs on a desktop: a table header may wrap (`.tbl th` `white-space: normal` at
every width; the values stay on one line): the table's auto layout wraps a header only when the
table would not fit otherwise — the markets list at 1,024 px (two or three header lines), the market
tabs beside the trade panel and the lender dashboard at 1,280 px; at 1,440 px the headers are one
line. Where a table is at its narrowest a header's ⓘ may take a line of its own under the label —
at 1,280 px (the market tabs beside the trade panel, the lender dashboard), 1,024 px (every table)
and 768 px; at 1,440 px none does. Keeping the ⓘ with the header's last word (or beside the sort
button) widens each such column by the ⓘ, and the verification round measured what that costs
(responsive gate at 768, 1,024 and 1,280 px): a table scrolls sideways in seven page states — at 768 px the markets
list by 66 px (its Flags column, with the "Coverage paused" badge, clipped) and the positions table
by 62 px; at 1,024 px the market's Positions tab by 17 px, its Liquidations tab by 59 px and the
lender dashboard by 3 px; at 1,280 px the lender dashboard by 14 px (its action cell clipped). So
the column fit wins: a header's ⓘ alone on its line is an accepted, cosmetic cost — *accepted by
the owner on 2026-10-11 (thirteenth round) as built, to revisit in a design pass after the
audit*.
The ⓘ overhangs its line instead
of growing it (a 24 px box with −5 px block margins, centred on the capitals), so a label with an ⓘ
is as tall as one without and keeps the line's baseline; a key / value list with an ⓘ spaces its
rows 24 px apart so two rows' ⓘ never overlap. The positions table's headers wrap like the others'
(only its values stay on one line). The market's stat strip keeps each label on one line from
1,280 px: in its one-row case (≥ 980 px wide) a cell is at least as wide as its label, so the
values share one line. Every table fits its width without sideways scroll at 1,024, 1,280 and
1,440 px with the trade panel where the base did (the responsive gate's h-scroll counts, §10.5; *design pass
P1:* re-measured at 1,280 and 1,440 px with the 12 px gutter and the reserved scrollbar gutter — 0 sideways
scrollers, §7.3). Number rules (`lib/format.ts`, `components/Num.tsx`): amounts
are `bigint` end to end and formatted once; balances truncate (never round up); SOL 4 decimals in
tables; tokens by their decimals (≤ 4 shown); tiny prices with subscript zeros (`0.0₅1234`), the
plain value in `title` and for screen readers; P&L always signed with U+2212 and ▲ / ▼; numeric
columns right-aligned; every number element carries `data-value` with the raw integer.

*Design pass P1 (2026-10-16):* **three label tiers** do all the labelling (`tokens.css`, §7.1): a panel
title (`--title-*`, 13 px bold `--text-0` uppercase, 0.06em) outranks a section heading (`--section-*`, 12 px
bold `--text-1` uppercase, 0.05em — the base `h3`, one pixel smaller than before; 13 px on a phone) which
outranks a label (`--label-*`, 12 px medium `--text-2` uppercase, 0.04em). The page `h1` is `--fs-title`
(20 px at every width, −0.01em; it was 18 px on a desktop and a tablet) so it stands clear of the 15 px
wordmark, and a hero number is `--fs-hero` (= `--fs-2xl`, 22 / 24 px), so a hero number outranks the title
(the market's EMA price, P5-3; the position's CR, P6-2); reading prose is `--fs-prose` 14 px (15 on a
phone) at `--lh-prose` 1.55 (P8). `h3` lands exactly on the 12 px floor on a desktop (the gate
fails only below 12). **Press states** (`base.css`): a pressed default / danger / ghost button, menu item or
search option steps to `--bg-4`; a pressed tab, nav link, the phone's More button (new: it had no press
feedback) and an interactive row step to `--bg-hover`; the filled primary and short buttons are excluded
and take their own presses (P3-3).

*Design pass P4, P5–P7 and P11 (numbers, as built):* an amount that reads as zero carries `data-zero`
and is muted (`--text-2`, its unit with it; never hidden, and `inherit` inside a button so "Claim all ·
0 SOL" stays legible on gold) — judged by the displayed digits, so dust that rounds to "0.0000" is as
quiet as an exact zero while `data-value` keeps the raw integer; a feature's plain-span zero (a count, a
utilisation of 0 %) sets it too, so a row never mixes grey and white zeros. SOL columns are padded to
their column's precision (`Sol` / `Tokens` `pad`: every fraction digit kept, so the decimal points
align — the markets list's TVL and OI at 2, the market tabs' and the positions' SOL at 4, rent at 6, the
lender's claimable at its row's `solFrac`); whole-token quantities stay unpadded, and so do the review's
delta rows (`formatFixed`, where a padded round "before" would be noise). Slots are grouped with commas
(`formatSlot`; "—" for none) and ages read "45s", "11m", "3h 12m", "2d 4h" (`formatAge`; `Countdown`
and `formatDuration` keep their seconds). In a card the sorted column is the hero figure
(`td[data-sorted]`: `--fs-lg` medium, its label 12 px; a skeleton row carries no `data-sorted`), and in
a table the sorted column's header is white over a 2 px gold line. A unit is an atomic inline (a line
may break between a number and its unit, never inside the unit). Hairline key / value rows and the
position's facts sit on the 28 px grid (`--kv-row-h`), and a preview's decisive figure (`data-emph`) is
`--fs-md` medium in `--text-0` with its label white and its unit 12 px.

### 7.3 Density and layout

Compact rows 28 px, controls 30 px; 4 px grid; header 44 px (48 under 1,024 px; *design pass P1:* 52 px
under a coarse pointer at any width, so the 44 px controls get 4 px of air — the nav height, `scroll-padding`
and the sheet head follow `--header-h`), status bar 24 px (a 28 px strip on a phone; *P1:* 28 px on a
landscape phone too — `(min-width: 640px) and (max-width: 1023.98px) and (pointer: coarse) and (orientation:
landscape)` — so the 844 × 390 chrome is 52 + 56 + 28 = 136 px); *M10-RESP:* no minimum width (§7.6) and, under a coarse pointer at any
width, 44 px controls and rows, 16 px inputs (the unused `data-density` block is gone; a density
toggle is owner item O-4); `.page` stops at 1,680 px and centres; the trade layout is a fluid market column plus a
360 px panel (320 px under 1,180 px). Panels: 6 px radius, a `--line-panel` edge with a 1 px top highlight
(`--edge-highlight`), 13 px bold white uppercase titles (the title tier; *design pass P3-1*; 12 px light grey
before).
*As built (the visual pass, §10.2 `screens`):* the trade panel is sticky and scrolls on its own
(its submit stays a short scroll from the inputs while the market column scrolls); table cells
tighten to 4 px padding under 1,280 px so the markets table fits 1,024 px without sideways
scroll; the stat strip wraps as flex rows that fill the width (no empty cells, no clipped values;
the size cap is compact there, exact in its title and the panel); a ticker or a mint inside an
uppercase label keeps its case (`.case-keep`: a mint is case-sensitive). *M10-RESP:* the stat strip
on a desktop is a wrapping flex row: one row where the strip is ≥ 980 px wide, else four per row,
and a short last row's cells grow to fill it — never an empty cell (seven stats: 4 + 3 at 1,024
and 1,280 px, one row at 1,440, with or without a page scrollbar; the switch is a `clamp()` on the
flex basis, not a container query); a tablet's strip is under 980 px, so the same rule gives
4 + 3 there (the lender KPIs one row of four); 2-up on a phone. Numeric columns keep their
value on one line and, from 1,280 px, 16 px from the column before (the spec's D15 `width: 1%`
packed the numbers into one block and left the text columns empty: dropped, every column shares
the spare width; the review's delta table keeps its numbers shrink-wrapped, and a date there may
break between its day and its time). A position's facts fill up to three columns top to bottom, in groups (size,
risk now, rent and dates). The trade and Manage panels reserve their scrollbar gutter on both
edges (`stable both-edges`, thin), so the insets are symmetric (*design pass P2-7:* one recipe,
`.side-sticky`, §7.6 — the thumb in `--scrollbar-thumb`, `scroll-padding` clearing the sticky foot, the last
foot under `--shadow-up`). The status bar's top line is a
shadow, so a 24 px target in it never makes a short page 1 px taller than the screen. A
`details` summary has a bordered chevron (right / down) in `--text-1`.

*Design pass P1 (2026-10-16) — structural measures as tokens* (`tokens.css`; the later packages consume them):
`--page-gap` 12 px between boxes and `--page-head-gap` 16 px under the page head; `--kv-row-h` 28 px (key /
value rows are not controls: they stay 28 px under a coarse pointer, unlike `--row-h`); `--side-col` 360 px
(320 px under 1,180 px, the `(max-width: 1179.98px)` block) for the trade / Manage / create-market column;
`--page-max` 1,680 px; `--sheet-head-h` = `--header-h` + 4 px; `--tabbar-indicator-w` 32 px; `--field-h` =
`--control-h` (46 px under coarse, so the 44 px input fits its 1 px border); `--select-arrow-w` 28 px;
`--check` 18 px (20 under coarse); `--icon` 16 px and `--icon-sm` 14 px; `--icon-btn` 32 px (44 under coarse:
`.modal-close` / `.sheet-close` are primary controls in the gate); `--form-measure` 64ch; `--field-narrow-w`
120 px. **Gutter:** `--gutter` is 12 px from 1,280 px as well as under 1,024 px (a desktop is never tighter
than a tablet); 1,024–1,279 px keeps 8 px because its tables are measured to the pixel. Measured before it
landed (P1; the dev build in headless Chromium at 1,280 × 800 and 1,440 × 900, with §7.6's scrollbar gutter
reserved, so the content is 1,241 / 1,401 px wide): the markets list, every market tab beside the trade panel
(Overview, Positions, Lenders, Liquidations, Pools, Coverage, Events), the positions table, the lender
dashboard (the claimer wallet), protocol status, the position page, create market, How it works and
Security — 0 sideways scrollers and 0 px of document overflow before and after.

### 7.4 Components (`src/components`, scaffold; tracks compose them)

| Component | Contract |
|---|---|
| `Button` | variants default / primary / short / danger / ghost, sizes sm / md, `busy`; *design pass P3-2 / P3-3 (as built):* a disabled button keeps its shape (`--bg-2`, a `--line-strong` edge, `--text-2`) at regular weight, so it is told from an enabled default button beside it, and with a Connect CTA in the same foot it is a quiet caption-like row (no box, 24 px with a mouse, 44 under a finger); `busy` (`aria-busy`, only while it works) spins a ring before the text (`cursor: progress`); under the pointer a default or ghost button steps to `--bg-4`, the primary presses to `--accent-active`, short hovers and presses to `--loss-hover`, danger hovers to its own tint (`--danger-bg`); the shared close ✕ (`.modal-close`, `.glossary-close`, `.mk-popover-close`) is one `--icon-btn` box (32 px; 44 under a finger) in `--text-0` — the sheet's ✕ keeps its 44 px box, the toast's its small ghost box (it shows the word "Dismiss" over an open sheet) |
| `Panel` | titled `<section>`, actions slot, flush body; *P3-1:* a `--line-panel` edge with `--edge-highlight`, the title on the title tier (13 px bold white, `--title-*`) |
| `DataTable` | columns (align, sortable, width), sticky header, `aria-sort`, row activation by click / Enter, ↑ / ↓ between rows, skeleton rows, empty state, caption; *as built:* `rowAttrs` (`data-*` state on a row, e.g. `data-status`), and a click or key on a link / button / input inside a cell is that control's, not the row's; *design pass P4 and P11 (as built):* the header sits on the label tier in a 28 px row (a 24 px sort button plus 2 + 2) and from 1,024 px sticks under the app header except in the positions and lend tables (the wrap stops being its scroll container; rows and in-row controls carry `scroll-margin-top: calc(var(--header-h) + 3 * var(--row-h))`, so a Shift+Tab-focused row — the `focus.spec.ts` walk over the markets list, Tab and Shift+Tab — clears a stuck header up to three lines tall; the review's deltas table has no wrap); the sorted column's header is white over a 2 px gold line and the sort marks are registry icons (`sort` on a sortable column, `chevron` on the sort — turned for ascending — and an `arrow` in the card sort bar before "Descending" / "Ascending"); an interactive row (`onRowActivate`) lifts to `--bg-hover` under the pointer and shows an inset focus ring, a read-only row does not (P11); an empty list hides its table and sort bar and shows its message alone (P4-3, the axe fix); a card's sorted cell is its hero (P4-4) |
| `Sol`, `Tokens`, `Bps`, `Price`, `Pnl` (`Num.tsx`) | formatted numbers with `data-value` and screen-reader text; *P4 / P11:* `data-zero` on an amount that reads as zero, `pad` on `Sol` / `Tokens` (§7.2) |
| `Badge`, `TierBadge`, `RiskBadge`, `VenueBadge`, `riskLevel` | tone / risk / tier semantics; *P3-8:* an info badge is a grey tag (`--tag` on `--tag-bg`: link blue stays a link's), a neutral badge is ringed (`--bg-2` with an inset `--line-strong` hairline), and `className="badge-status"` capitalises a single-word enum (position status, indexer state) without changing its text; the tier badge is untouched |
| `Modal` | native `<dialog>` (focus trap, Escape, focus return), non-dismissible mode; *P9-2 / P9-5:* under `--shadow-modal` with its `--line-strong` edge; open, a column whose body takes the height the dialog's own `max-height` leaves (no 200 px constant); the `sheet` variant a `--bg-2` sheet under `--shadow-sheet` with a `--bg-2` head; its ✕ the registry's `close` icon in the shared box |
| `ToastProvider` / `useToast` | polite and assertive live regions; errors persist; *as built:* bottom-left (the trade panel is the right column), the empty region lets clicks through; *P9 / P11:* under `--shadow-toast`; over an open sheet on a phone the toast is a strip sized to the sheet head and 4 px clear of its ✕ box (§7.6); on a tablet the column stops 12 px short of the side sheet; the gap between the two live-region stacks is a margin only while both hold a toast |
| `Field`, `AmountInput`, `Checkbox`, `Select` | labelled inputs, strict amount parsing, Max, hints and errors wired with `aria-describedby`; *P3-4 / P3-7:* an error never replaces the hint (the hint carries the limit the error is about): error first, then the hint, both in `aria-describedby`; the select, the checkbox and a radio are drawn by the app (`appearance: none` — the registry chevron in a `--select-arrow-w` gutter at the field's height `--field-h`; a `--check` box, 18 px and 20 under a finger, that fills gold with a drawn tick, a radio a gold ring and dot); a disabled field sinks to `--bg-1` with a hairline and muted text; Max is a `--text-1` text control, not gold |
| `Tabs`, `TabPanel` | WAI-ARIA tabs (arrows, Home / End); *P3-6:* a hover (white text, a grey underline), a fade at the edge where tabs are out of view (`data-overflow` = start / end / both) and `variant="sub"` — a segmented row on a `--bg-2` track, the selection a `--bg-3` fill with an inset ring, 44 px under a finger (Deposit / Withdraw / Claim) |
| `Disclosure` | verbatim disclosure with optional acknowledgement, collapsible; a disclosure whose acknowledgement is its whole text (SPEC §5.3's sentence) shows it once, as the checkbox's label; *P3-1 / P3-5:* a `--surface-note` box edged with `--line-panel` and its 3 px severity bar; the acknowledgement is a control row — a bordered `--bg-3` row that turns gold when ticked, one focus ring on the row (keyboard only), its box 24 px (`max(24px, --check + 4px)`); a `details` chevron is a 7 px, 2-stroke mark |
| `Empty`, `ErrorState`, `Skeleton`, `Stat`, `Kv` | states, stat strips, key / value previews; *P4-5 / P4-7 / P8-3:* `Stat` `hero` (`data-hero`: `--fs-xl` medium, the strip's one headline) and the strip's hairlines between cells (a cell is transparent, the strip paints `--bg-1` and clips the outer edges); `Kv` rows on the 28 px grid under `data-lines`, with `dd[data-emph]` / `dd > [data-emph]` for a decisive figure, and no hover; the `Empty` block and the left-bar `ErrorState` (§3.10) |
| `AddressText`, `Countdown` | short address with copy; live time left; *P3-10:* the copy glyph is the registry's `copy` (`check` in `--ok` while "Copied"), muted beside the text |
| `TxReviewModal` (`src/tx`) | the review of §5.3; *M10-RESP:* the error under the stage, the simulation badge gone once the wallet has it, "Cancelled in your wallet" + Try again, timestamp deltas as dates, the sign hint, the confirmed link (`TxPlan.next`); full screen on a phone; *P9-2 / P9-4:* the instructions on the sunken well (`--surface-well`), the priority fee as a Normal / Fast chip pair (§5.3) |
| *M10-RESP* `DataTable` | column `priority` (1–3), `label`, `card` (title / aside / actions); every cell `data-col`, `data-label`, `data-priority`, `data-card`; explicit table roles; a sort bar (`table-sort-<id>`, `table-sort-dir-<id>`) in card mode; `className`, `sortLabel` |
| *M10-RESP* `useSheetMode`, `SheetHead`, `Scrim` (`Sheet.tsx`) | an element that becomes a modal sheet under a query (default compact) while open: dialog role, `aria-modal`, the rest inert (not `[data-sheet-keep]` / an open `<dialog>`), focus in and back, Escape; nothing on a desktop; *P9-1 / P9-5:* the head carries the notch inset itself (`top: 0`, `--safe-t` as padding inside a `min-height` of `--sheet-head-h` + `--safe-t`), so it keeps its height in a wallet's in-app browser; its ✕ the `close` icon |
| *M10-RESP* `FormFoot`, `ActionBar` | the sticky submit with a `role=status` reason (a button to the control to fix with `reasonTarget`); the page's fixed thumb bar under 1,024 px; *P3-2 / P2-5:* the submit wraps instead of overflowing a 320 px foot, a 12 px fade sits above the foot's hairline (not on a sheet's static foot or the create page's fixed one), a disabled submit under a Connect CTA is a caption (`Button`), the reason-button is link blue; the action bar floats under `--shadow-up` with no hairline |
| *M10-RESP* `Reveal` | a truncated value → popover with the full value, Copy, Explorer (`reveal-<kind>`); opt-in on `TokenId` / `AddressText` (`reveal`); *P3-10:* the dotted line is a text underline on the baseline (the 24 px box kept), the popover edged with `--line-panel` under `--shadow-pop`, Explorer with the `external` icon |
| *M10-RESP* `ConnectCta`, `WalletChooser` (`src/wallet`) | an enabled Connect wallet on a surface and the shared chooser (one open at a time, `walletUi.ts`); mobile without a wallet: open-in-wallet links (§5.4); *P2-8:* the chooser's links are one bordered list of 48 px rows (the focus ring inside a row), the cluster hint an info note (`.wallet-note`), the open-in and explorer links with the `external` icon |
| *M10-RESP* `Modal` `variant`, `Empty` `title` / `action`, `Stat` `priority`, `Panel` `ref` / `head`, `AmountInput` Max in the field, `addressInputProps`, `Toast` kind word / `links`, `Tabs` sideways scroll | as §7.6 |
| *Thirteenth round* `GlossaryTip`, `Term` (`Glossary.tsx`) | the glossary control (§4.3); *P3-10:* the resting ⓘ is `--icon-muted` at 12 px (the text colour under the pointer, `--info` while open; its hit areas unchanged), its ✕ the `close` icon in the shared box |
| *Thirteenth round* `PageBannersContext`, `focusBanner` (`PageBanners.tsx`) | which banners the page shows, whether its panel is a sheet, and a way to a banner (O-2, §3.4) |
| *Thirteenth round* `.link-button` | a button that reads as a link (an in-page action), 24 px target, underlined |
| *Design pass P2-1* `NavIcon` (`shell/icons.tsx`) | the one glyph registry: the tab bar's icons, `info`, and `close`, `chevron`, `arrow`, `sort`, `copy`, `check`, `external`, `shield`, `plus` — 20 px inline SVG stroked 1.6 in `currentColor`, presentation attributes only, aria-hidden (the control's text or `aria-label` names it); outside the tab bar it takes `.icon` (`--icon`, 16 px), `.icon-sm` (`--icon-sm`, 14 px) or its own class (the More rows' 20 px icon and 16 px chevron, the status strip's 16 px chevron, the 12 px sort marks, glossary ⓘ and tier ⓘ); every close, copy, external, sort and chevron glyph in a control is its (the P&L ▲ / ▼, the status strip's ● ▲ ✕ and the action bar's "▼ Short" stay text). The brand mark is `Logo.tsx`, not an icon |

Track-specific components live in the track's directory; a component two tracks need is proposed
to the integrator for `src/components`.

### 7.5 Motion

Short transitions (80–140 ms) on hover and focus only; no number flashing or animated tickers
by default; skeleton shimmer; everything off under `prefers-reduced-motion`. *M10-RESP:* sheets
slide in over `--t-sheet` (140 ms; 0 under reduced motion). *Design pass (as built):* a button's
background steps over `--t-fast` (80 ms); a busy button's ring spins (0.8 s, linear) and stands still as
a half-tone mark under reduced motion (P3-2); the status strip's chevron turns over `--t` (140 ms), a
`details` chevron over `--t-fast`; hover and press are colour steps on the P1 tokens, no new transition.

### 7.6 Responsive conventions (M10-RESP)

The owner's request (2026-10-09): "optimize the site for phone and make it look good and intuitive
both on pc and phone", keeping the dark terminal. The rules every track follows:

- **One DOM.** Every width switch is CSS; nothing renders twice, so every `data-testid` stays
  unique. JavaScript reads a breakpoint only for behaviour (a sheet's inert / focus handling, the
  mobile wallet links), through `src/styles/breakpoints.ts`; jsdom gets a desktop `matchMedia`
  (`src/test/media.ts`, `setMedia()`).
- **Three tiers.** Phone < 640 px: the app shell (tab bar, status strip), cards, full-screen
  sheets, an action bar. Tablet 640–1,023: the text nav, one column, priority columns or cards by
  container width, a right-side sheet. Desktop ≥ 1,024: the terminal as before.
- **The closed breakpoint set** (top of `tokens.css`; `breakpoints.test.ts` rejects anything else):
  `@media` `(max-width: 399.98px | 639.98px | 1023.98px | 1179.98px | 1279.98px)`,
  `(min-width: 640px | 1024px | 1280px | 1680px)`, `(pointer: coarse)`, `(hover: hover)`,
  `(prefers-reduced-motion: reduce)` — no range syntax; `(orientation: landscape)` only with
  coarse in the tablet tier — a landscape phone: its side sheet keeps its form foot in the flow
  (a ~390 px sheet cannot lift a sticky foot over a form that starts low; it covered the fields
  and cut off the submit; *the fix round's review:* with `z-index: auto` too, as a flex item's
  z-index stacks in the flow and the Manage sheet's Add margin foot painted over the sticky head
  and its ✕), (thirteenth round) the markets intro strip is compact (§3.3), *(design pass P2-6, P11)* the
  status bar is the phone's strip with its open list two items a row (§3.1), and *(P11)* the review
  dialog takes all but 24 px of the height with 8 px of block padding in its head and foot (its body
  242 px, from 186);
  *thirteenth round:* once, `(max-height: 699.98px)` with
  `(max-width: 399.98px)`: a short phone (320 × 640, 375 × 667), where the markets list's intro strip
  keeps only its heading row and "What can go wrong" (§3.3) — `MEDIA_HEIGHTS`, the only height
  allowed, always with a width; the whole query, `(max-width: 399.98px) and (max-height: 699.98px)`,
  **approved by the owner on 2026-10-11 (thirteenth round)** and pinned as `SHORT_PHONE`
  (`breakpoints.ts`; `breakpoints.test.ts` refuses a height in any other query). **Containers only on `.tbl-wrap` (`tbl`:
  960 / 560 px) and `.modal-body` (`dialog`: 480 px).**
- **Input decides density, width decides layout.** 44 px controls / rows and 16 px inputs only
  under `(pointer: coarse)`; hover styles only under `(hover: hover)`; WCAG 2.5.8's 24 px floor at
  every width (`.hit` for a small control's larger hit area). *Thirteenth round (the last review):*
  **a control's touch area never covers another control.** The glossary ⓘ's hit area grows
  sideways only — 44 × 24 px under a finger, 32 × 24 px with a mouse (grown on all sides it reached
  the ⓘ of the next key / value row, a Copy button or a field above or below it — the market
  header's mint, the creator's address, the withdraw amount — so a tap or a click near that edge hit
  the wrong control); a table header's ⓘ, 2 px beside its sort button, keeps its own 24 px box with
  any pointer; the Short preview's size caps are rows 24 px apart, like a key / value list with ⓘ.
  The focus walk fails on a sample point that lands on another control outside the page's bars
  (below), and the responsive gate measures every control's touch area at all eight viewports
  (`tapCovers`, §10.5) — inside an open sheet, the sticky trade panel, a sticky table header and a
  dialog too (the round-13 review: exempting every fixed or sticky box had hidden the covers there).
- **Tokens first.** Breakpoints change tokens (`--gutter`, `--tabbar-h`, `--actionbar-h`,
  `--sheet-w`, `--tap-min`, `--tap-sm`, `--fs-input`, `--safe-*`, `--z-bars` / `--z-sheet` /
  `--z-wallet-sheet`, `--t-sheet`; the phone type scale one notch up: 12 / 13 / 14 / 16 / 20 / 24
  px) and a few structural rules in `components.css`; feature CSS places only its own pieces; no
  fixed width wider than the screen (`min(Npx, 100%)`).
- **Reading order = DOM order.** No CSS `order`, no `row-reverse`; sheets leave the flow but are
  never moved; an action bar follows the page header in the DOM. On a phone the review's Sign —
  last in the DOM — gets the full-width bottom row.
- **Sheets.** A page panel becomes a sheet with `useSheetMode` + `.sheet-host`, opened by
  `?sheet=` (`MarketSearch.sheet` 'trade', `PositionSearch.sheet` 'add-margin' | 'close';
  `useSheetParam`): opening pushes a history entry, so Back closes it; a deep link closes by
  replacing the URL. A review `<dialog>` opened from a sheet stays visible even if Back drops the
  key.
- **Within thumb reach.** At most one `ActionBar` per page; every form submit in a sticky
  `FormFoot` with a one-line reason while it is disabled. A submit that is only busy — the Claim
  while its first search for syncs runs with nothing claimable now — shows no reason until the
  result is known, so no reason appears and then flips (§3.6).
- **Never hide risk or staleness.** Disclosures verbatim and in place; simulate → review → sign
  unchanged; staleness in words at every width (the status strip; panel badges).
- **Focus Not Obscured (2.4.11).** `scroll-padding` / `scroll-margin` keep a focused control
  clear of the sticky header and the fixed bars. *Thirteenth round:* `focus.spec.ts` (desktop) and
  `mobile-focus.spec.ts` (phone) walk the markets list (intro strip shown), a market's Overview
  (on the phone with its action bar), Lending and How it works with Tab and then Shift+Tab, and
  fail when a focused control is entirely hidden (`elementFromPoint` at a 5 × 5 grid of its
  on-screen rects hits nothing of it; partial covers are annotations; `e2e/focus-obscured.ts`).
  *Thirteenth round, the last review:* they also fail when a sample point lands on a **different
  interactive control** outside the page's bars — another control's touch area over this one;
  covers by the header (with the phone's tab bar), the status bar, an action bar, a sticky form
  foot, a sheet's head or the toasts stay annotations (only those: a control in another fixed or
  sticky box — an open sheet, the sticky trade panel, a sticky table header, a dialog — is judged,
  the round-13 review). *Second approval pass's review:* a bar counts only while it is over the
  page (fixed, or sticky and stuck away from its place in the flow), the same rule as the tap-cover
  check (§10.5), and the dialog and menu heads left the list; no walk currently lands a point on a
  bar's control, so the list has no current case — it keeps a scroll-under from failing a walk. Run on the CSS before the ⓘ fix, the phone's market Overview failed at 4
  stops (the venue ⓘ under the tier ⓘ's area, 4 of 25 points; the creator's Copy under the market
  creator ⓘ's, 5 of 25; each by Tab and by Shift+Tab); after it, every walk passes.
  Either guard alone passes the walk (the html `scroll-padding` without the `:focus-visible`
  `scroll-margin`, and the reverse); with both removed it fails on the market page (desktop and
  phone), and in one of two runs also on the phone's markets list (not every run leaves a control
  there entirely hidden; in the other, its covers were partial). Both stay: the `scroll-padding`
  also serves anchor jumps and a focus that is not `:focus-visible`.
- **Viewport.** `width=device-width, initial-scale=1, viewport-fit=cover` (safe areas); pinch zoom
  never disabled; `theme-color` = `--bg-1`.
- **Shared rules the pages rely on** (integration, v1.4): a card keeps priority-3 cells hidden; a
  text field is 44 px inside its border under a coarse pointer; action-bar buttons are ≥ 44 px; a
  `FormFoot` in the page sticks above the fixed bars under 1,024 px and at the bottom of an open
  sheet; an open sheet's `scroll-padding` clears its head and its foot (2.4.11); on a phone, a toast
  over an open sheet takes the sheet head's title strip (one line, sized to the head — `top` =
  `--safe-t`, `min-height` = `--sheet-head-h` — and 4 px clear of its 44 px ✕ box, 4 px past the
  title box, so it covers text, never a control, and no title ink shows under it; *design pass
  P9-3*, 16 px clear and shorter than the head before; a success or a note has no ✕ of its own
  there, an error keeps its one-line reason and a "Dismiss" word); a toast's 6 s count only while it is
  in view (not under a
  dialog or a sheet); the acknowledgement checkbox takes an id
  (`Disclosure ackId`, `Checkbox id`) for `FormFoot reasonTarget`. *Fix round:* a card whose
  title has no aside takes the whole first row; a card table's clipped header shows while focus is
  in it (its sort buttons are never an invisible Tab stop); the sort bar's direction button names
  the order ("Descending" / "Ascending" after an `arrow` icon — down for descending, turned for
  ascending; `data-order`; *design pass P4-3, P11*), no pressed state; a sheet that
  unmounts as it closes (the wallet popup) or has no opener (a deep link) returns focus to the
  opener or `#main`, never `<body>`; a shortened id ("8oBK…6cTC") in a review title or account
  label never splits across lines (`.nowrap`).
- **Scrollbar gutter** (*design pass P1, 2026-10-16*). From 1,280 px, `html { scrollbar-gutter: stable }`
  (`base.css`): the document keeps its scrollbar gutter while a dialog or sheet locks scrolling
  (`html:has(dialog[open])`, `html[data-sheet-open]` → `overflow: hidden`), so opening the review no longer
  shifts the header, nav and trade panel by the scrollbar width on classic-scrollbar platforms, and Cancel /
  Sign stay under a pointer heading for them. Overlay-scrollbar platforms (macOS default, phones) are
  unchanged. The cost is an empty `--bg-0` strip at the right of a short page with classic scrollbars — and
  of every page from 1,280 px in headless Chromium, whose hidden scrollbars still reserve 15 px: the desktop
  `screens` PNGs show it (they are written, never compared) and the responsive gate reads a document
  15 px narrower than the screen, never wider. 1,024–1,279 px keeps the old behaviour: its tables are measured
  to the pixel and `screens.spec` asserts the markets table fits at 1024 × 720. Never unconditional, never
  `both-edges`; if the owner wants 1,024–1,279 covered too, the only safe form there is on the lock rule itself
  (`scrollbar-gutter: stable` beside `overflow: hidden`), which costs nothing at rest but moves the jump to
  short pages at open time.
- **The shared side column** (*design pass P2-7, P5-6, P6-3*). `.layout-side` is the two-column grid of a
  page with a side panel — `minmax(0, 1fr)` and `--side-col` (360 px; 320 under 1,180 px), `--page-gap`
  between, one column under 1,024 px — and `.side-sticky` the panel's recipe from 1,024 px: sticky under
  the header, its own thin scroller (`--scrollbar-thumb`, the gutter on both edges), `scroll-padding`
  clearing its sticky foot, the last form's foot under `--shadow-up`. The market page's `.layout-trade` /
  `.trade-panel` are aliases of the same declarations; the Manage panel carries the classes, and the
  create page's own grid takes the same tokens.
- **The sheet head under the notch** (*P9-1*). The head carries the inset itself: `top: 0` with `--safe-t`
  as padding inside a `min-height` of `--sheet-head-h` + `--safe-t` (a scroll container's padding-top scrolls
  away with its content, and a sticky head with a negative `top` was pinned above the scrollport: in a
  wallet's in-app browser only ~5 px of the title and its ✕ were left once the form scrolled); the host's
  `scroll-padding-top` keeps the 8 px clearance under it for a focused control (2.4.11). The side sheet
  follows through the same rule; nothing changes at a 0 inset.
- **Owner items from the design pass** (*2026-10-16; each left as built until the owner rules*). (1) *The
  compact intro strip on every phone (P5-1):* the strip is compact on a short phone and a landscape phone
  only (above, §3.3); adding `(max-width: 639.98px)` to the markets.css prelude (keeping the approved
  short-phone query) would compact it on every phone, so the first market shows higher at the cost of
  "Short a token" and "Lend a token", which How it works carries — not applied without an explicit yes
  (the toolbar's one-row recipe landed, §3.3). (2) *The scrollbar gutter at 1,024–1,279 px:* off (the bullet
  above); the only safe form there is on the lock rule itself. (3) *The lend KPI hero under 400 px (P7-1):*
  the strip's hero is `--fs-lg` (16 px) under 400 px so a 2-up cell never clips "0.016481 SOL" inside the
  strip's `overflow: hidden`, and `--fs-xl` from 400 px (20 px on a phone, 18 from 640 px) — so a 390 px
  phone shows the 16 px hero, the card's size, not the plan's larger one; it reads fine as built.

### 7.7 Supported widths and the pages at each (M10-RESP, as built)

There is no minimum width: the app is laid out from **320 px** up (the responsive gate's
`phone-xs`), with no sideways document scroll at any width, and at 1,680 px and wider the page
stops and centres. The tier is the viewport width; density is the pointer (§7.6).

| Width | Shell | Tables | The trade panel / Manage | Type |
|---|---|---|---|---|
| **< 640 px** (phone; under 400 px "ShortWin" is hidden only while the cluster badge shows) | 48 px header (52 under a coarse pointer: the mark — 30 px alone under 400 — and the wordmark, the cluster badge, then ⌕ and the wallet as the trailing group), the `<nav>` as a bottom tab bar with More (the active tab gold with a 32 px bar), the status strip (worst state in words, tinted when degraded, expands over a scrim) | cards (container < 560 px) with the sorted column as each card's hero, sort bar; priority-3 hidden | a full-screen sheet from the page's action bar (`?sheet=`), Back closes it; its head keeps its height under the notch | one notch up: 12–24 px |
| **640–1,023 px** (tablet, landscape phone) | text nav, Search button → overlay, the status bar as on a desktop (no cluster; an 8 px item gap, so a 640–665 px mouse window still clips the end of "Security") — a landscape phone (a coarse pointer, wider than tall) gets the phone's status strip instead, its open list two items a row | priority mode (container < 960 px: p3 hidden), cards where the container is < 560 px | a right-side sheet (`min(440px, 100vw)`) over a scrim; on a landscape phone its submit sits under the fields, not sticky (usable, cramped: the header, action bar and status strip take 136 px of 390, §7.3 — a device check, §10.3) | as a desktop |
| **≥ 1,024 px** (laptop, desktop) | the terminal header and status bar, inline search with `/` | full tables, the header stuck under the app header except in the positions and lend tables (the market page's 680 px column at 1,024 is a priority-mode container) | in the page: the sticky right column (`.layout-side` / `.side-sticky`; 360 px, 320 under 1,180) | 12–22 px (thirteenth round; 11–22 before); the page title 20 px, a hero number 22 px |

**Pages** (the §3 sections have the details):

| Page | Phone | Tablet | Desktop |
|---|---|---|---|
| `/markets` (§3.3) | cards (symbol and mint, venue and tier; price / TVL, utilisation / OI, positions / flags; the sorted figure the hero), one toolbar row with the "Filters · n" toggle and the count, sort bar | priority table | the table (the token a link, the venue quiet text); "Filter this list" (mono for a mint), Clear filters or "Create a market for this mint" |
| `/m/$mint` (§3.4) | the 24 px EMA price under the title, 2-up stats with "More stats", sideways tabs, tab tables as cards; action bar Short / Lend → trade sheet (Lend gold and Short grey on a market that cannot be shorted, which opens on Lend) | one column, stats 4 + 3, side sheet; the action bar as on a phone | two columns, the trade panel sticky (Lend first when shorting is blocked) |
| `/markets/new` (§3.5) | one column; Review is the page's bottom bar with its reason; Paste under a coarse pointer | one column, sticky foot | two columns on the shared side column, the form on a 64ch measure; a rejected token says "Cannot be listed" and has no Review |
| `/positions` (§3.8) | totals strip 2 × 2, position cards with Add margin / Close | priority table | the totals strip over the table, "How risk is graded" |
| `/positions/$position` (§3.8) | health banner, hero line (the CR at 24 px, P&L, time left), facts in one column; action bar → Manage sheet at the chosen form | facts in two columns, side sheet | Manage sticky beside the facts (`.side-sticky`) |
| `/lend` (§3.8) | KPIs 2 × 2 (SOL claimable the hero), lender cards (CLAIMABLE the hero) with Withdraw / Deposit / Claim, Claim all in the action bar | cards (the lender table is cards under 960 px of container) | the KPI strip spanning the table, Claim all in the header |
| `/status`, `/security` (§3.9) | one column, tables as cards; identifiers wrap | one column | panels side by side, each as tall as its content; the policy centred at 920 px |
| `/how-it-works` (§3.11) | one column: the contents wrapped into rows of 44 px links, the prose 15 px | one column, at most 760 px | the prose at 760 px with, from 1,280 px, the contents as a sticky 200 px rail beside it (*design pass P8-2*) |

**Mobile wallet entry** (§5.4 (6)): on a phone or tablet the chooser offers "Open in Backpack /
Phantom / Solflare" universal links (alphabetical) to the same page — the content without a wallet
(with the cluster hint off mainnet, the install sites and Reload), a block after the entries with
wallets listed, and after "Open this page in your browser" with Copy link in a social app's in-app
browser; inside a wallet's in-app browser Wallet Standard works unchanged. On Android browsers the
Mobile Wallet Adapter (O-6, on by default since 2026-10-13) adds the entry "Use installed wallet" /
"Opens your wallet app on this phone", which opens the wallet app installed on the phone.

---

## 8. Accessibility (WCAG 2.2 AA)

- **Contrast:** the token table of §7.1 (text ≥ 4.5:1, input borders and focus ≥ 3:1).
- **Keyboard:** every action reachable and operable; visible focus (`--focus` 2 px ring); skip
  link; `/` search; tables, tabs and menus follow the WAI-ARIA patterns; dialogs trap focus and
  return it (to the opener; when it is gone or disabled — a submit after the form resets on a
  confirmed send — to an open sheet's title, else `#main`, never `<body>`); no keyboard trap;
  Escape closes dismissible layers. *Thirteenth round:* the glossary's ⓘ opens on click, tap, Enter
  or Space (never hover-only) and returns focus on Escape / ✕ (§4.3); a dismissed intro strip moves
  focus to the page heading; the coverage link moves focus to the banner it names.
- **Semantics:** landmarks (`header`, `nav`, `main`, `footer`), headings per page, tables with
  captions and `scope`, sortable headers as buttons with `aria-sort`, the search as a combobox with
  `aria-activedescendant`, form controls with labels and `aria-describedby` for hints and errors,
  `aria-invalid` on invalid input, live regions for toasts (polite) and errors (assertive) and for
  the transaction stage.
- **Non-colour cues:** P&L sign and arrow and a spoken "gain" / "loss"; risk and tier badges with
  words; flags with text.
- **Numbers for screen readers:** plain values with units (no subscript notation, no glyph
  arrows).
- **Motion:** `prefers-reduced-motion` honoured.
- **Checks:** Biome's a11y rules in lint; `@axe-core/playwright` on every page of the E2E (no
  serious or critical violation); one keyboard-only E2E per main flow (§10.2).

---

## 9. Security

### 9.1 Pinned programs and accounts

`src/config/programs.ts` pins short.win, the venue, token, memo, ATA, System and ComputeBudget
programs; `programs.test.ts` holds them to `constants.rs` and the Codama client (*2026-10-15:* the
`complete_curve` helper and its `declare_id!` check are gone with it, design pump-final D2). Which
of their instructions a plan may put at the top level is `src/tx/allowlist.ts` (§5.8). The program id is never read from configuration, the indexer or a URL.
PDAs are derived locally (`find*Pda`); an indexer-supplied address is used only for display and
navigation, and every account a transaction touches is re-derived or read from the chain.

### 9.2 No blind signing

§5.3, §5.8: simulation of the exact instructions before any wallet prompt; a review older than
20 s is refreshed; the top-level allowlist; the signed message is compared with the built one; the
user-set bounds (`min_*` / `max_*`) are always present so a moved market reverts instead of
filling badly.

### 9.3 Content Security Policy and headers

Production builds inject (as `<meta>`) and write to `dist/_headers` (for the host; the CDN must
send the header form):

```
default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; img-src 'self' data:;
font-src 'self'; connect-src 'self' <RPC http + ws origins> <indexer http + ws origins>;
manifest-src 'self'; worker-src 'self'; base-uri 'none'; form-action 'none'; object-src 'none';
frame-ancestors 'none' (header only); upgrade-insecure-requests (mainnet)
X-Frame-Options: DENY · X-Content-Type-Options: nosniff · Referrer-Policy: no-referrer ·
Permissions-Policy: camera=(), microphone=(), geolocation=(), payment=(), usb=(), serial=(), hid=() ·
Cross-Origin-Opener-Policy: same-origin · Strict-Transport-Security: max-age=63072000; includeSubDomains
```

*With the Mobile Wallet Adapter on (the default since 2026-10-13; `VITE_ENABLE_MOBILE_WALLET_ADAPTER`,
§2.1):* `style-src-elem 'self' 'sha256-9y+0omBcAXmVSpYuadfUp/k8GxCIDiw2wrEyqM1cQnc='`,
`style-src-attr 'unsafe-hashes' 'sha256-fpg83YZxyBh7zNCICwdKi5D7eUkxQ9wk3mpUiNPhP3g='` (the library's
loading spinner, which the app removes as it is inserted: the hashes only keep its insertion free of
violations; recomputed from the installed package by `mwaPackage.test.ts`) and `connect-src …
ws://localhost:* http://localhost` (the association socket, random port 49152–65535, and the request
that raises Chrome's "Apps on device" prompt). Any same-origin script may then reach any loopback
port, on every platform; only Chrome gates it (its local-network permission); accepted because
`script-src 'self'` holds and the app has no HTML sinks. The MWA transport authenticates no wallet
end (a local app on the port can answer as the wallet; encryption guards against passive observers
only): the wallet's own UI for each signature and `assertSameIntent` on what comes back are the
defences. The library's other UIs are never shown (the app's own `onWalletNotFound` and
local-network step, and the host observer), so no `'unsafe-inline'`, font or image host; no
`frame-src` (Firefox for Android unsupported, FE-34); Permissions-Policy and COOP unchanged — never
list `local-network-access` / `loopback-network` (their default allowlist, `'self'`, is what the
adapter needs). `upgrade-insecure-requests` kept (Chromium does not upgrade localhost; the E2E's
T-UIR probe confirms it in Blink — Chromium 1243, since its first run on 2026-10-13 (§10.2); a
device check is §13 FE-32). Flag off: the policy is byte for byte the one above. `csp.test.ts` pins both,
in full.

No inline scripts or styles in the HTML, no `eval`, no third-party scripts, fonts or images (wallet
icons are `data:` URIs from the Wallet Standard registry). **Clickjacking:** `frame-ancestors 'none'`
and `X-Frame-Options: DENY`; the app never renders inside a frame. *As built v1.2 (M9-SEC-6):* a
`<meta>` CSP cannot carry `frame-ancestors` and a host may ignore `dist/_headers`, so (1)
`main.tsx` does not mount the app when `window.top !== window.self` and renders `frame-guard` ("open
short.win directly") instead; (2) `vite preview` (the E2E server) sends `X-Frame-Options: DENY` and
`frame-ancestors 'none'` too; (3) a deploy check, `pnpm --filter web check:headers <https://origin>`
(`scripts/check-headers.ts`, `missingSecurityHeaders` in `csp.ts`), fails unless the live origin
sends them (and `nosniff`, HSTS) and publishes no source map; the E2E `frame.spec.ts` frames the
build both with and without the headers.

*As built (M10 SECTXT-4, owner decision Q3 of 2026-10-02):* `public/.well-known/security.txt`
(RFC 9116: `Contact: mailto:security@short.win`, `Policy: https://short.win/security`,
`Preferred-Languages: en`, `Canonical`, `Expires: 2027-09-30` — rotate it before 2027-08-31) is
checked by `src/config/securityTxt.ts`: Contact mailto: or https:; exactly one RFC 3339 Expires;
an https Policy and the Canonical at `/.well-known/security.txt`; registry fields only. An OpenPGP
cleartext-signed file (RFC 9116 §2.3) is checked on its signed text; the signature is not
verified. Every build runs the check; only a `VITE_CLUSTER=mainnet` build and `check:headers`
(production checks) also refuse `OWNER-FILL` placeholders and an Expires 30 days or less, or more
than a year, ahead, so other builds never fail because the date nears; the unit test checks the
file on a clock pinned 90 days before its own Expires. `dist/_headers` (`csp.ts` `headersFile`,
pinned by `csp.test.ts`) serves it as `text/plain; charset=utf-8`. `Policy` is the app's own
`/security` page (§3.2; `SecurityPolicyPage`: scope, how to report, response, safe harbour,
audits; the repository's `SECURITY.md` carries the same values). The program embeds its own
security.txt with the same contact and policy (ARCH §8.5; the retired `complete_curve` helper
carried one too until 1.0.0).

### 9.4 Secrets

None in the app or the repository. Every `VITE_*` value is public (compiled into the bundle); the
RPC URL must be an origin-restricted public key; `.env*` files are gitignored except `.env.example`;
the config refuses URLs carrying credentials. E2E keypairs are generated at run time into the
gitignored `apps/web/.e2e/` and never committed (program-id keypairs stay in `keys/`, never read by
the app or the E2E).

### 9.5 Input validation

Amounts: `lib/amount.ts` (digits and one dot, ≤ the mint's decimals, no sign / exponent / grouping,
≤ u64); slippage in bps within bounds; addresses: base58 via Kit's `isAddress` (`lib/address.ts`)
before any RPC or API use; route params and search params validated in `router.tsx`; API path
segments validated (`keySegment`); `q` ≤ 64 chars. Indexer strings (symbols, names) are rendered as
text only (React escaping; `noDangerouslySetInnerHtml` is a lint error) and never used as keys.
*v1.2 (M9-SEC-5):* they are normalised once where indexer data enters (`api/client.ts`, `api/ws.ts`
→ `lib/displayText.ts`: controls, bidi embeddings / overrides / isolates, zero-width and other
invisible characters removed, NFC, whitespace collapsed, symbols ≤ 16 and names ≤ 64 code points;
the indexer's refresher strips the same set), rendered isolated (`<bdi>`, `unicode-bidi: isolate`),
and wherever a symbol names the token of a trade the short mint stands next to it (`TokenId`,
`tokenLabel`: review titles and delta labels, both trade panels, position and lender rows, search
results, which also mark a symbol two results share). The market header marks a symbol another
listed market uses (`market-duplicate-symbol`). The market page takes the token's identity from the
route and its decimals, venue, tier, flags and creator from the chain; a detail the indexer serves
for another mint is not used (`market-indexer-mismatch`; M9-SEC-3).

### 9.6 Dependencies and supply chain

Exact versions in `package.json`; the lockfile is the workspace's (`pnpm install --frozen-lockfile`
in CI); pnpm's supply-chain policy applies; no `postinstall` downloads (Playwright browsers are
installed explicitly into the user cache, never as root).

*O-6 (2026-10-13):* `@solana-mobile/wallet-standard-mobile` **0.7.0** is the one new runtime
dependency; it and all its imports run only in its lazy chunk (§2.2). pnpm 12 reads overrides only
from `pnpm-workspace.yaml` (it ignores package.json's `pnpm` field), which pins the protocol,
`@solana-mobile/mobile-wallet-adapter-protocol` **3.0.0**, and the protocol's own `@solana/kit` to
8.3.0 (one Kit in the lockfile). `minimumReleaseAge: 20160` (14 days, repo-wide) keeps every install
from resolving anything younger, verified by a probe at install (a young version is refused, an
older one resolved); the two `@solana-mobile` versions the owner chose to install now are excluded
by exact version until 2026-10-21 (remove both exclusions then). 21 versions locked before the
gate and published 2026-09-29/30 were excluded the same way until they were 14 days old, and the
exclusions were removed on 2026-10-16 (pump-final P8; `pnpm install --frozen-lockfile` passes
without them): pnpm 12 also verifies the existing lockfile against the gate. Every new lockfile entry was reviewed with its publish date (M1's
commit lists them). `src/wallet/mwaPackage.test.ts` is the drift test: the package files' sha256
(wallet-standard-mobile's `index.browser.js`, the protocol's `index.browser.js` and both encoding
files), the in-page transitive versions, the two CSP hashes recomputed from the package, the one
interpolated `innerHTML` inside `populateError`, the host shapes the app removes and the library
facts the way out relies on — a package update fails it until someone re-reads the library.

---

## 10. Test plan

### 10.1 Unit tests (Vitest; `src/**/*.test.ts(x)`; jsdom)

Present in the scaffold (87 tests in 23 files): config (`parseConfig`, CSP, disclosures verbatim and documented,
errors complete against the client, programs pinned, test ids unique and documented), lib
(formatting, amount parsing, staleness), api (client errors and headers, WebSocket resume /
back-off / watchdog, live cache updates, types against `openapi.yaml`), tx (token-account
extensions, error mapping, deltas, integrity, compute budget and fees, the flow's gating), math (the
WASM module loads and computes SPEC numbers), mint parsing, components (numbers, badges,
disclosure, table, amount input) and a router smoke test (every route renders in the shell).

*As built:* 50 files (one, the live-validator suite of SHORT, skipped unless `SHORTWIN_LIVE_RPC`
points at a validator), 476 tests (*v1.2:* 56 files, 533 tests — the review's regressions in
`src/m9-review/` and `features/markets/m9-review.test.tsx`, §13): the scaffold's, plus MARKETS 56, LEND 68, SHORT 261 (venue
readers and account lists against keeper vectors and `tests/fixtures`, previews and plans equal to
what the program booked on LiteSVM), and the integration's (`tx/describe.test.ts`, the error
mapping on real nested logs and `IntegrityError`, the oracle with no reading, the SPEC §5.3
sentence shown once).

*M10:* 59 files (the live-validator one skipped), 555 tests on the integrated tree before program
batch 3, which adds 10 (565 passed, 1 skipped, on the final tree); program batch 4 adds 5 (PUMP-2026-10-1: `readers.test.ts`, `state.test.ts`; PUMP-2026-10-2: `growth.test.tsx`, 3). New: `src/m10-fe15/parity.test.ts` (FE-15: the facade against the
TypeScript ports it replaced, frozen in `legacy.ts` as the oracle — both sides of the spot/EMA edge
included — and against the Rust crates' vectors and every dumped mint and pool), `config/securityTxt.test.ts`
and `config/securityPolicy.test.ts` (SECTXT-4, the `/security` values), `errors.test.ts` with 6105;
batch 3: `features/lend/previews.test.ts` "the pool’s unpaid rent at a deposit" (the split, the
thresholds, the prologue's pending credit), `LendPanel.test.tsx` (no rows on a paid-in pool, info
collapsed, warn expanded and in the plan, a stored-zero accruing pool, a top-up's labels) and
`features/short/plans.test.ts`'s extras fallback (`EXTRAS_RETRY_CODES` is exactly 6003–6005).

*O-6 (the Mobile Wallet Adapter, 2026-10-13):* new files `wallet/platform.test.ts` (the
`isMobileDevice` matrix, `isInAppBrowser`, `mwaEligible`), `wallet/mwaEligible.agree.test.ts` (the
real package registers exactly where `mwaEligible` says, Firefox for Android the one intended
difference), `wallet/mobileWalletAdapter.test.ts` (gating, `registerMwa`'s arguments, verification
against the plugin's list, `onWalletNotFound`, `chainScopedCache`, `removeLibraryUi`, the icon's
pixels), `wallet/mwaPackage.test.ts` (the drift test, §9.6), `wallet/mwaSigner.agree.test.ts` (the
signer policy with the real package and plugin), `wallet/mwaCall.test.ts` (R-C1 on fake timers),
`wallet/mwaConnect.agree.test.ts` (R-C1 with the real plugin), `wallet/localNetwork.test.ts`; and
extended `parseConfig`, `csp` (both flag states, in full), `wallet.test.tsx` (the chooser states
(a)–(l)), `useTxFlow` (no awaited I/O before the wallet, the 10 s refresh, the notices, R-S1, R-S2,
the signer policy), `errors` (every MWA row of §5.9), `m9-review/tx` (the lifetime path, no send
after an abort), `TxReviewModal` (the notices, Cancel while signing) and `copy` (70 ids, 150
strings). Vitest also runs `e2e/**/*.test.ts`: `e2e/wallet/fake-mwa.test.ts` holds the E2E's fake
MWA wallet (§10.2) to the real wallet-standard-mobile 0.7.0 and protocol 3.0.0, in-process — the
handshake and its key derivation, the encrypted framing and sequence numbers, connect, sign after
the reauthorization, and every behaviour and variant the Android specs use.

Each track adds, at minimum: MARKETS — the safety verdict for every rule (one fixture per rejected
extension and for freeze authority / native mints), the market page's mapping of API fields, the
markets table sorting and paging, the status page's flag aggregation; LEND — deposit / withdraw /
claim previews against `pool_model`-derived vectors through the WASM, CPI-Guard and memo branches
of plan building, the bracket's instruction order, `min_sol` / `min_tokens`, the sync / sweep
prepends; SHORT — the venue account lists against vectors produced by the keeper's `venue.rs` for
each venue (fixtures in `tests/fixtures/accounts`), venue-state readers against the fixture
accounts, the open preview against `shortwin_sim`-derived numbers (margin, fee, CR, liquidation
price, caps), close / add-margin previews, P&L and risk.

### 10.2 End-to-end (Playwright 1.63; `apps/web/e2e`; E2E track)

**Environment.** Everything runs locally, without Docker or network:

- `solana-test-validator` from `~/.local/share/solana/install/active_release/bin`, with
  `--reset --ledger apps/web/.e2e/ledger` (on disk, gitignored; never `/tmp`), dedicated ports
  (RPC 18899, WS 18900, faucet 19900) so a developer validator can run alongside, and:
  `--upgradeable-program 5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK
  target/test-venue/deploy/shortwin.so <generated authority>` (the test-venue build,
  `scripts/build-test-venue.sh`; `initialize_config` requires the upgrade authority's signature),
  `--bpf-program BPAZFPkmVtYEWEZHCzFUns7oybh1VWfAvmu3ofM2mQi7 target/deploy/mock_cpamm.so`
  (*2026-10-15:* no longer `complete_curve.so`, retired with pump's v3 exits, design pump-final D2),
  and optionally the dumped venue programs and accounts of `tests/fixtures` (`--bpf-program` /
  `--account`) for venue-specific specs. *M10 (T-MEMO):* Token-2022 is mainnet's ELF
  (`--upgradeable-program` from `tests/fixtures` group `spl_programs` with its mainnet upgrade
  authority; the built-in is a different build; `SHORTWIN_BUNDLED_SPL=1` keeps the built-in).
  spl-memo v3 is the validator's built-in, which in Agave 4.1.2 already is mainnet's BPFLoader2
  account byte for byte. Global setup (`checkSplPrograms`) holds both to the mainnet hashes. SPL
  Token and the ATA program are the built-ins. CPMM is the 2026-10 redeploy.
- **Seed** (`e2e/validator/seed.ts`, Node with Kit and `@shortwin/client`): generated keypairs
  (authority, governance, pause, fee recipient, lenders, the E2E wallet); `initialize_config`
  (`venue_enabled` with the mock bit; windows and warm-ups at their compiled minima; the live
  ProgramData slots as `initialize_config` takes them); a protocol ALT (`@solana-program/
  address-lookup-table`) for the v0 path; an SPL mint with a mock pool and a market
  (`venue_hint = 6`), lent by a seed lender, cranked until opens are allowed; a Token-2022 mint
  (metadata only) with a list-only market; for the wallet: SOL, tokens of both mints, a Token-2022
  account **with CPI Guard enabled** and one **requiring incoming memos**; a market in coverage
  pause (`set_coverage_pause` with the generated coverage authority) for the status / blocked
  specs. Writes `apps/web/.e2e/state.json` (addresses and the generated keys' paths).
- **App:** `vite build` with `VITE_CLUSTER=localnet`, `VITE_ENABLE_TEST_VENUE=true`, the E2E ports
  and the seeded ALT, served by `vite preview` (Playwright `webServer`).
- *M10-RESP* **Projects:** `<profile>` (desktop 1440 × 900; every spec but `mobile-*.spec.ts`) and
  `<profile>-phone` (390 × 844, `isMobile`, touch, DPR 3, an iPhone user agent, the same Chromium;
  `mobile-*.spec.ts` only), both in the default run. Phone specs tap (`locator.tap()`, no DOM-click
  fallback), run axe at 0 on every state, sign only with the seeded `phone` wallet (SOL and E2EMAIN
  like `trader`) and write the reviewed phone PNG set `e2e/screenshots/phone/` (CSS scale) with
  `SHORTWIN_E2E_SCREENSHOTS=update`. `mobile-shell.spec.ts` (foundation): the tab bar and More,
  the status strip, the search overlay (a mixed-case mint kept as typed), the open-in-wallet hrefs
  without a wallet (never followed), the connected wallet sheet, not found; `mobile-markets`,
  `mobile-trade`, `mobile-lend` belong to the page tracks; *thirteenth round:* `mobile-guide` and
  `mobile-focus` (2.4.11, §7.6). `screens.spec` captures grow to the
  document's scroll height (the document scrolls since M10-RESP).
- *O-6 (2026-10-13)* **The Android project** `<profile>-android` (`devices['Pixel 7']`: 412 × 839,
  DPR 2.625, `isMobile`, touch, an Android Chrome user agent; the same Chromium): `mobile-shell`
  and `mobile-mwa` (the phone project ignores `mobile-mwa.spec.ts`), in the default run. The app
  is built with `VITE_ENABLE_MOBILE_WALLET_ADAPTER` from `SHORTWIN_E2E_MWA` (default `1`; `0`
  builds it off — the build's `VITE_*` are in `sourceFingerprint`, so switching rebuilds). A seeded
  wallet `android` (SOL and E2EMAIN, like `phone`) is signed with only by the fake MWA wallet.
  `mobile-shell` runs there too: without an injected wallet its chooser shows the adapter's entry
  and the open-elsewhere block (with the flag off, the iPhone's dialog); its PNGs go to
  `e2e/screenshots/android/` (with the flag off, to `.e2e/screenshots/android-mwa-off/` only, so
  CI's artifact keeps the default build's set). *Status (M3, 2026-10-13; Playwright's Chromium
  1243, headless shell):* the Android project passes — 25 passed and 2 skipped (the flag-off
  tests) in 3.2–3.4 min, global setup's seed and build included —, and with `SHORTWIN_E2E_MWA=0`
  8 passed and 19 skipped in 2.0–2.2 min. Every spec below passed on its first run with the
  adapter's code unchanged (§13 "Mobile wallets", "The first browser runs").
  - **The intent and the app switch** (`e2e/wallet/mwa-hooks.js`, an init script the `mwa` fixture
    installs): `URLSearchParams.prototype.set` is wrapped to record each association as the
    protocol builds the intent (`association`, `port`, `navigator.userActivation.isActive`,
    `performance.now()`); `window.addEventListener` is wrapped so that the first `blur` listener
    after an association (the protocol's 3 s launch detection) is followed 50 ms later by a
    synthetic `blur` — the wallet app coming to the front — unless `__mwaE2E.launch =
    'not-found'`; `__mwaE2E.returnToPage()` dispatches `focus`. Headless Chromium has no app for
    `solana-wallet:`, and the page stays (M2 asserts the URL unchanged after the intent; it held
    from the first run, so CDP's `Page.frameRequestedNavigation` is not needed).
  - **The wallet** (`e2e/wallet/fake-mwa.ts`): a protocol-level MWA wallet app in Node behind
    `page.routeWebSocket(/^ws:\/\/localhost:\d+\/solana-wallet$/)`, with `node:crypto` only:
    HELLO_REQ verified against the launched association key (ECDSA P-256, IEEE-P1363) and port,
    ECDH P-256 and HKDF-SHA-256 to an AES-128-GCM session key, HELLO_RSP with the session
    properties `{"v":"v1"}`, then encrypted JSON-RPC (4-byte sequence number as the AAD, 12-byte
    IV): `get_capabilities` (`features: ['solana:signTransactions']`), `authorize` (fresh, or with
    the `auth_token` of the reauthorization before each sign), `sign_transactions` (Kit's
    `partiallySignTransaction` with `android`'s key), `sign_and_send_transactions` (never called:
    the specs assert it), `deauthorize`. One-shot behaviours on the call they name: `decline`
    (−3), `tamper` (the first data byte XOR 1 of the last instruction that is neither
    compute-budget nor without data — not simply the last instruction: a compute-budget change is
    one the app accepts, §5.8, and an empty one has no byte), `close`, `silent` (on
    `sign_transactions`); `reauthDecline` (−1),
    `reauthClose`, `reauthSwitch` (another account; the next sign declined −2) on the
    reauthorization; `silentConnect`. Variants: `signFeature: false`, `accountFeatures`. It
    records every call (`fakeMwa.calls`), session and signature; a message it cannot understand
    fails the test. The routed socket is a page-side mock that bypasses `connect-src`, so the CSP's
    MWA sources are probed natively (M11).
  - **`mobile-mwa.spec.ts`** (`wallet: null`: the adapter is the only wallet; axe at 0 on each
    state; the zero-CSP guard, which takes `allowCsp` for the flag-off probe only): M1 the chooser
    (`android/wallet-mwa.png`), M2 connect (one association with the tap's activation still
    active; the library's spinner host inserted and removed at once), M3 the silent reconnect (no
    association) and Disconnect, M4 sign and confirm (a new association, the reauthorization, then
    `sign_transactions`; the signature lands; `tx-wallet` names the adapter), M5 the activation
    reaches the wallet although every `getLatestBlockhash` is held 6 s (the prefetch, §5.4 step 2),
    M6 decline, M7 tamper (`integrity`, nothing lands), M8 a launch nobody answers (the `launch`
    notice, Sign again confirms; then a second unanswered launch on a new page load, Cancel, and
    Disconnect: the chooser stays closed, and the test asserts that Disconnect came within the
    10 s for which a recorded not-found would reopen it, so a slow run cannot pass vacuously), M9
    the session closed (`wallet`, Retry), M10 / M10b a sending-only wallet and per-account
    features in MWA ids (connected, no signer: the deposit form's no-signer state; no "sends itself" badge; neither
    sign call made), M11 the CSP probes, M12 a Telegram in-app browser (no entry; Copy link writes
    the page's URL, and reads "Copy link" again after its confirmation, so the PNG shows the chooser
    at rest; `android/wallet-in-app-browser.png`), M13 / M14 the reauthorization declined or
    closed (`wallet-changed` within 1 s of the wallet's answer — the fake's `answeredAt` against
    the page's `performance.timeOrigin + performance.now()`, one clock —, "Close and review
    again", the header "Connect wallet"), M15 the wallet never answers the sign (Cancel enabled
    after 5 s; a new Sign confirms), M16 / M17 the
    wallet never answers the connect (the button enabled 10 s after `returnToPage()`, a new
    association after; meanwhile a tap mid-screen changes nothing and no spinner host is in the
    DOM), M18 another account at the reauthorization (`wallet-changed`; the header shows the new
    account), T-UIR (Blink keeps `ws://localhost` and `http://localhost` under
    `upgrade-insecure-requests` on a route-fulfilled `https://` page; its `fetch` is bounded at
    2 s in case Chromium's own local-network check holds it — in Chromium 1243 it does not: the
    whole test takes about 1.1 s, so the fetch settled at once, and the test needs neither
    `--disable-features=LocalNetworkAccessChecks` nor Playwright 1.63's `local-network-access`
    permission). Flag off
    (`SHORTWIN_E2E_MWA=0 pnpm --filter web e2e --project chain-android`): no entry, today's
    no-wallet dialog, no script
    carrying the library (its cache key, `SolanaMobileWalletAdapterDefaultAuthorizationCache`, not
    the wallet name, which the app's own bundle carries), and the M11 probes refused (`connect-src`).
    `wallet.spec.ts` (desktop): no adapter entry with the flag on.
- **Indexer:** profile `chain` (CI): the indexer URL points at a Playwright-routed mock
  (`page.route`) that serves OpenAPI-shaped responses built from the seed's on-chain state, or at
  nothing for the fallback specs. Profile `full` (local, optional): `shortwin-indexer run` against
  the validator and a user-space PostgreSQL (the zonky binaries of INDEXER §11.1), same specs.
- **Browser:** `chromium.launch({ executablePath: process.env.SHORTWIN_CHROMIUM ?? '/usr/bin/chromium' })`,
  or Playwright's own Chromium installed into the user cache (`npx playwright install chromium`,
  never `--with-deps` as root). Checked here: Playwright 1.63 drives `/usr/bin/chromium` 152.

**Injected test wallet** (`e2e/wallet/injected-wallet.js`, plain browser JS added with
`page.addInitScript`): a Wallet Standard wallet named **`ShortWin E2E`** registered through the
`wallet-standard:register-wallet` / `wallet-standard:app-ready` events, chains `solana:localnet`,
features `standard:connect`, `standard:disconnect`, `standard:events`, `solana:signTransaction`
(versions `legacy` and `0`), `solana:signMessage`; Ed25519 signing with WebCrypto from a secret key
passed as the init-script argument (generated per run). Test controls on `window.__shortwinE2E`:
`rejectNext()` (the wallet rejects), `tamperNext(fn)` (the wallet changes the message — the app
must refuse, §5.8), `raiseFeeNext(price)` (a compute-budget-only change — the app must accept),
`signed()` (the transactions signed so far).

**Specs** (each selects only by the §11 ids; `axe` on every page):

| Spec | Covers |
|---|---|
| `smoke.spec.ts` | shell, nav, search by mint and ticker, wallet connect / disconnect, status bar states, math ready |
| `markets.spec.ts` | table, filters, sort, live row update, open market, create-market link |
| `create-market.spec.ts` | verdicts (ok, freeze authority, disallowed extension, existing), list-only listing through the pipeline |
| `lend-deposit.spec.ts` | SPEC §5.3 text verbatim, Sign disabled until ticked, simulated deltas, confirm, dashboard updated; *M10 batch 3:* the unpaid-rent rows after an empty `sweep_trader` credits the seeded rent (`actions.accrue`): the panel's receivable ≥ the stored one, the exposure and level recomputed from it (`unpaidRentExposure`); the `info` level (`warn` is unit-tested) |
| `lend-cpi-guard.spec.ts` | **CPI Guard UX** (§10.3) |
| `lend-withdraw-memo.spec.ts` | **memo UX** (§10.3) |
| `lend-claim.spec.ts` | claimable after rent accrues, claim, vest-forfeit warning on a partial exit |
| `lend-post-wipe.spec.ts` | *M10 (FE-18):* an organic full wipe on the mock venue (every free token withdrawn, a 10× gap, keeper seconds until S1 rolls `num = 0`); a plain deposit refused `WipeSweepPending`; the wiped lender's claim through the live record; a new lender's deposit lands exactly `[sweep_trader, deposit]` (the sweep notice, the review's steps, `TraderSwept` to the record, initial-rate shares); the swept SOL claimed through the final record — *KI-67:* that claim closes the position (`lender.claimCloses` in the review, the account gone, its 2,568,240 lamports of rent back in the wallet with the SOL paid, less the fee; the claim tab then says there is no position) |
| `lend-claim-sync.spec.ts` | *M10 (FE-18):* a claim the pot cannot pay lands `[sync_lender(other lender), claim_sol]`: `pot_draw > 0`; paid = the claim alone (simulated) + the draw; the pot afterwards = before + the other lender's surplus (simulated `sync_lender` alone) − the draw. *Thirteenth round (owner, 2026-10-11):* the claimer's credit is first booked to ≥ 40,000 lamports (its shortfall stays above the 10,000-lamport dust threshold); Lend → Claim shows the two-line label; the claim alone lands from Node, and then the **Lending page's row** — nothing claimable, the shortfall only — claims through the syncs: exactly `[sync_lender(other), claim_sol]`, paid = the review's simulated `claimed` = the pot draw. *Second approval pass (owner, 2026-10-11, "only when syncs can pay"):* first, while the claimer is the market's only lender, its shortfall gives the disabled "Review claim · 0 SOL" with "Nothing to claim yet" on Lend → Claim and on the Lending page's row, and no Claim all |
| `lend-claim-pot-chain.spec.ts` | *M10 (KI66-R4, KI-66 full):* on mock market `chain` the absent lender `chainAbsent` keeps a thousandth of the claims free, a 100× gap settles the borrowed tokens S1 and rolls a **rebase** record (`num = 1`, `1/1,000`); `chainClaimer` lends in the new epoch, a position borrows again, rent is credited for 90 s (*thirteenth round:* a 1,000× gap and 300 s, so the claimer's shortfall — ≈ 470 lamports before — clears the 10,000-lamport dust threshold; the seeding payer is funded with 400,000 SOL for the gap's buy), the claimer takes part out and the rent is paid in — the absent lender's unconverted shares hold realised SOL beyond its credit. The claim lands exactly `[sync_lender(absent, 1), claim_sol]`: `max_epochs = 1`, the absent owner unsigned, the record read-only (not writable in the message); `lender.potSyncs` in the review; paid = the claim alone + the pot draw, `pending` ≤ 2 lamports after it; the absent lender's shares, epoch, lock and the record's `conv_rem` unchanged, `fwd = 1` with `fwdEpoch` the share epoch; a second sync moves nothing |
| `lend-sweep-cashback.spec.ts` | *M10 (FE-18):* the bundle's pump / PumpSwap cashback claims built by `sweep.ts` (`readSweepFacts` / `cashbackClaimIxs`: pump and PumpSwap `claim_cashback`, pAMM `init_user_volume_accumulator`), landed by themselves on the dumped ELFs on an unused trader PDA — no wipe, sweep or deposit |
| `short-open.spec.ts` | every preview field present and equal to the simulated position (`q`, `s`, `m`, `v`), the borrower disclosure gate, blocked states (coverage pause) |
| `short-close.spec.ts` | P&L after a mock price move, close, payout delta |
| `short-close-curve.spec.ts` | *pump v3 (2026-10-15, design pump-final §8; FE-3 closed):* the dumped near166 curve listed and lent from Node, an open of 1.5 M tokens in the Short panel (`sell_v3`, the 10 v3 TradeExtras), a third party's `buy_v3` down to `q/2`, then the position page's close at `B̂_v3` — one `close_short` completes the curve by synthetic migration (`pcB = q − rT`) and moves the market to `PumpMigrating`; axe at 0 on the panel, the position page and the close review |
| `add-margin.spec.ts` | CR preview and the confirmed CR |
| `positions.spec.ts` | dashboard fields, risk badge after a price move, time left |
| `status.spec.ts` | coverage-paused market listed, venue rows |
| `tx-errors.spec.ts` | wallet rejection, a failing simulation (Sign never enabled, mapped message, `data-code`), a stale review refreshed at Sign, `tamperNext` refused, `raiseFeeNext` accepted (*v1.2:* the review names the signing wallet, `tx-wallet`, and shows the fee as signed, `tx-final-fee`) |
| `fallback.spec.ts` | indexer down: banner, market page from the chain, a deposit still works |
| `keyboard.spec.ts` | deposit and open completed with the keyboard only |
| `focus.spec.ts` | *thirteenth round:* Focus Not Obscured (WCAG 2.2 2.4.11, §7.6): Tab and Shift+Tab walks over the markets list (intro strip shown), a market's Overview, Lending and How it works; no focused control entirely under the sticky header or the status bar, and none partly under another control's touch area outside the page's bars (the phone's walk, with the tab bar, the status strip and the action bar: `mobile-focus.spec.ts`) |
| `harness.spec.ts` | *as built:* the seeded scenario is on chain; the routed indexer's REST and WebSocket shapes; the Node-side actions |
| `wallet.spec.ts` | *as built:* the injected wallet's Wallet Standard features, reject / tamper / raise-fee controls; *O-6:* no Mobile Wallet Adapter entry on the desktop |
| `mobile-mwa.spec.ts` | *O-6 (the Android project):* the Mobile Wallet Adapter against the fake wallet app, M1–M18 and T-UIR, and the flag-off build (above) |
| `frame.spec.ts` | *v1.2 (M9-SEC-6):* the preview sends `X-Frame-Options: DENY` and `frame-ancestors 'none'`; a framing page gets no app — the browser refuses it, and with the headers stripped (a host ignoring `_headers`) the app renders `frame-guard` instead; *M10 (SECTXT-4):* it GETs `/.well-known/security.txt` (200, `text/plain`, valid; the preview server only — `vite preview` ignores `_headers`) |
| `screens.spec.ts` | *as built:* every page and state (markets, search, create market ok / rejected / pump curve, every market tab, the tier popover, coverage-paused and list-only markets, 1,024 px, the short preview and its review, the blocked panel, positions, a position, deposit / withdraw / claim, the lender dashboard, the CPI Guard notice and the bracket's review, the indexer-down views; *M10:* the `/security` page, `60-security`) saved as PNGs with axe on each; asserts no sideways scroll at 1,024 px |

**As built** (v1.1):

- The validator and the preview are started by Playwright's global setup, not `webServer` (the
  build needs the seeded protocol ALT, and `webServer` starts first). Extra ports: gossip 18901,
  dynamic 18910–18960, preview 14173; `--limit-ledger-size 200000000` (the default keeps minutes
  of history, which breaks profile `full`'s backfill).
- One seeded wallet per independent spec group (`trader`, `lender`, `guard`, `memo`, `memoRace`,
  `claimer`, `creator`, `closer`, `margin`, `holder`, `errors`, `keyboard`; *M10, FE-18:* `wiped`,
  `wipeBorrower`, `rejoin`, `potClaimer`, `potLate`, `potBorrower`; *M10-RESP:* `phone`; *O-6:*
  `android`), so specs do not depend
  on their order; one worker (one validator); specs that change shared state undo it.
- *M10 (FE-18):* two more seeded mock markets, `wipe` and `pot`, each lent by one spec wallet
  alone, with positions after the warm-up (five SPL mock markets, five open positions). Under
  `SHORTWIN_E2E_REUSE=1` they are spent after one run: the FE-18 specs then skip with a message.
  The markets / status reference PNGs (`01-markets`, `03-status`, `16-markets-1024`,
  `51-fallback-markets`) predate the two markets; a refresh (`SHORTWIN_E2E_SCREENSHOTS=update`)
  shows two more rows. The PNGs are not asserted.
- *M10:* 22 specs, 58 tests; about 9 minutes on a fresh validator (the seed ≈ 100 s; the wipe
  spec's keeper loop ≈ 60 s). On a host whose `/tmp` is a small tmpfs run with `TMPDIR` on disk
  (Chromium's profile; docs/DEVELOPMENT.md).
- `SHORTWIN_E2E_REUSE=1` reuses the validator and its seed, and the build only while its sources,
  configuration, WASM stamp and `VITE_*` values are unchanged (`sourceFingerprint`); a rebuild
  restarts the preview.
- The release program is optional (`SHORTWIN_E2E_PROGRAM=release`: the trading specs skip); the
  wallet's `tamperNext` takes `'data' | 'drop' | fn`; tags `@scaffold`, `@markets`, `@lend`,
  `@short`, `@screens`, and `@responsive` (§10.5; not in the default run).
- `screens.spec.ts` writes `.e2e/screenshots/`; `SHORTWIN_E2E_SCREENSHOTS=update` refreshes the
  reviewed set committed in `e2e/screenshots/`.
- For manual work the same environment runs the app: `node e2e/validator/start.ts --env`,
  `node e2e/validator/fund.ts <wallet>`, `pnpm --filter web dev:e2e` (docs/DEVELOPMENT.md).

### 10.3 CPI Guard and memo UX tests (the M9 exit's named tests)

- **CPI Guard:** with the CPI-Guarded account selected, `lend-cpi-guard-notice` is shown with the
  `token.cpiGuard` text and the plain deposit is disabled; selecting the ATA re-enables it; opting
  into the bracket (`lend-cpi-guard-bracket`) yields a review listing DisableCpiGuard / deposit /
  EnableCpiGuard (*as built:* `tx-instruction-0…2` with `data-program` / `data-name`, and checked
  again on the landed transaction's top-level instructions); the deposit lands and the account
  still reads `lock_cpi = true` afterwards.
  **Race:** the spec selects an unguarded account, then enables CPI Guard on it from Node before
  Sign; the simulation fails and the review shows the mapped CPI Guard message (`tx-error`
  `data-kind="token"`, `data-code="42"`) with Sign disabled; Cancel and re-entry re-read the account
  and show the notice.
- **Memo:** withdrawing to the memo-required account shows `withdraw-memo-notice`; the transaction
  passes `memo_program` and lands; the destination's balance rises by the previewed tokens.
  **Race:** the spec selects a plain Token-2022 destination, then requires memos on it from Node
  after the review simulated. *M10 (TMEMO-9):* every Token-2022 withdrawal already passes
  `memo_program`, so Refresh still simulates and the withdrawal lands with the memo CPI'd before
  Token-2022's transfer; re-entry shows the notice. (Through v1.2 the simulation failed with
  `MemoProgramRequired`, `data-code="6095"`, and Retry, `data-mode="rebuild"`, rebuilt the
  withdrawal with `memo_program`, M9-UX-1; that rebuild path keeps its unit tests.) Since M10 both
  memo tests run on mainnet's Token-2022 and spl-memo v3 ELFs (§10.2).

### 10.4 CI

The scaffold adds the `web` job to `.github/workflows/ci.yml`: install wasm-bindgen 0.2.129, build
`@shortwin/math-wasm`, `pnpm --filter web lint`, `typecheck`, `test`, `build`. The E2E track adds
`web-e2e` (needs `anchor-build` and the test-venue build; Agave 4.1.2 for `solana-test-validator`;
Playwright's Chromium in the user cache; profile `chain`). *As built:* `web-e2e` runs every spec
(no tag filter since the integration) and uploads the `screens` PNGs on every run, the report,
traces and logs on failure. *Since 2026-10-13* the `web` job also runs the facade's Node smoke
test after the math-wasm build (`crates/shortwin-math-wasm/js/smoke.cjs` on a `wasm-bindgen
--target nodejs` build of the same `.wasm`). The app's own facade calls run under Vitest, which
loads the real `.wasm`; a facade function no app code calls, such as `previewSettlement` (whose
trailing `executable` a JS caller that leaves it out passes as `false`), was until then checked
from JS only by the smoke test, which no job ran. *O-6 (2026-10-13):* after the full run,
`web-e2e` runs the Android project again with `SHORTWIN_E2E_MWA=0` (the app rebuilt with the Mobile
Wallet Adapter off: `mobile-mwa`'s flag-off describe and `mobile-shell`'s iPhone-like no-wallet
dialog); the `web` job's Vitest also runs `e2e/**/*.test.ts` (the fake MWA wallet against the real
library, §10.1).

### 10.5 Responsive capture (M10; `e2e/specs/responsive.spec.ts`, `@responsive`)

A measuring pass over the same states as `screens` plus the ones it skips (the wallet menu
disconnected, connected and with no wallet; the header search; the markets loading skeleton, a 500
error state and an empty filter; the route 404 and an unlisted mint; the empty positions and lender
pages; a position that does not exist; the pages that need a wallet, without one; the review after a
wallet rejection, before Sign and confirmed; the success toast; the lender dashboard from the chain),
at six Chromium-emulated viewports: `phone-s` 360×780 (mobile, touch, DPR 2), `phone` 390×844
(mobile, touch, DPR 3), `tablet` 768×1024 (touch, DPR 2), `laptop-s` 1024×768, `laptop` 1280×800,
`desktop` 1440×900 (phones and the tablet send a mobile user agent). Per state and viewport it
writes `$SHORTWIN_SHOTS_DIR/<viewport>/<name>.png` (the page unrolled: `main`'s scroll height, then
the full document, capped at 16,000 device px; on a phone also `<name>-screen`, the first screen
at load; with a dialog open, the viewport plus `<name>-full` with the dialog body unrolled) and
records in `$SHORTWIN_SHOTS_DIR/metrics.json` (`e2e/responsive/metrics.ts`). *M10-RESP:* the
document scrolls (the capture grows to the larger of `main`'s and the document's scroll height),
the tablet also writes `<name>-screen`, two more viewports run — `phone-xs` 320 × 640 and
`phone-land` 844 × 390 (mobile) — and each state also records `matchMedia('(pointer: coarse)')`,
the tab / action bar heights and the primary controls under 44 px with a coarse pointer:

- **layout width:** a page wider than the phone is laid out wider (`innerWidth` > the screen) and
  shown from its left edge at scale 1; sizes below are as seen (× `visualViewport.scale`);
- **overflow:** the document's sideways scroll (against the screen width) and `main`'s, the
  outermost elements past the viewport's edges (not inside a scrolling or clipping box), the boxes
  that scroll sideways on their own and the `overflow-x: hidden` boxes that cut their content
  (ellipsis counted);
- **touch targets:** interactive elements under 44 × 44 CSS px and under WCAG 2.5.8's 24 × 24 (a
  checkbox counts its label's box; links inside running text are exempt and counted apart), and
  overlapping targets — inside the open dialog when there is one; *thirteenth round:* **tap
  covers** (`collectTapCovers`, `tap-cover` in `summary.ts`) — each visible control wholly in view,
  sampled at the focus check's 5 × 5 grid per line box, whose point lands on a different control
  outside the page's bars (the focus check's list; an enlarged `.hit` area counts as its control's),
  measured on the unrolled page with the touch emulation on; a gate finding. *Second approval
  pass's review:* only the points inside the box a control can be seen in are judged (`clipBox`:
  the boxes that clip it on its containing-block chain, up to the top layer — the review dialog
  sits in the DOM inside the trade panel or sheet, whose box clipped nothing it draws, and had
  skipped every dialog control at 768 px and wider); a bar is exempt only while it is over the page
  (fixed, or sticky and stuck away from its place in the flow: a form foot in the flow is judged),
  and the dialog and menu heads, which never covered a point, left the list. Where unrolling would
  change the layout — a landscape phone grown taller than wide turns portrait, a 640 px phone
  grown past 700 px loses the short-phone rules (`phone-land`, `phone-xs` on a tall page; the
  queries compared are the short-phone one and the landscape-phone rules' own, so a laptop or
  desktop page taller than wide is still unrolled) — the page is not unrolled: its `<name>.png` is
  Chromium's full-page capture at the real size (the orientation and height queries kept, but the
  capture drops the touch emulation, so it draws the fine pointer's sizes and the sticky form foot,
  and the fixed bars sit where they are on the first screen), and `<name>-scroll-N.png` slices
  (touch on) show what the device shows: an open sheet's body, else the document, scrolled 75 % of
  what shows between the bars fixed or stuck over its top and bottom per slice, so the slices
  overlap by a quarter of that; the tap covers are measured at the real size in half-screen scroll
  steps (the document, then each box that scrolls on its own, such as an open sheet's body), and
  axe runs at the real size. *The pass's second review:* a dialog or overlay state (the review
  dialog, the Manage and trade sheets captured as the viewport) is measured at the real size with
  each box that scrolls in it stepped too (a dialog's body, half its height at a time when lower
  than half a screen; an open sheet's body; in an open dialog only its own), and an open sheet's
  slices are written there as well, so a control below the fold of a dialog's body or a sheet is
  judged and pictured; over the steps `judged` counts each control once. *The verification after
  it:* the unrolled page, and a page that fits its screen, step each box that scrolls its own
  content too (the side trade panel from 1,024 px, whose end, where its form foot is no longer
  stuck, was otherwise never judged);
- **text under 12 px**, grouped by element and size;
- **hover-only affordances:** `title` tooltips, `cursor: help`, `:hover` rules that reveal
  (display / visibility / opacity / size / transform without a `:focus` twin);
- the header and status bar heights and the height left for content;
- **axe** (WCAG 2.2 A/AA) violations per viewport (*v1.4:* on a full-page state, run on the unrolled
  page — every target in view, the fixed bars past the content — so a target is not judged by
  what scrolls under a fixed bar at one moment; a dialog or overlay state, or a page whose layout
  unrolling would change, at the viewport's size).

Findings never fail it (a step that cannot run is recorded with its error and a PNG of where it
stopped; a control Playwright's tap cannot hit is clicked through the DOM and noted); only the
fixtures' CSP check can. Writes on chain: the lend screens' `accrue` and one
1-token deposit by the `errors` wallet per viewport (the confirmed review and its toast). It is left
out of the default run (`grepInvert` unless `SHORTWIN_E2E_RESPONSIVE=1` or `SHORTWIN_SHOTS_DIR` is
set); `SHORTWIN_SHOTS_VIEWPORTS=phone,desktop` limits the viewports; `node
e2e/responsive/summary.ts <dir> [--pages]` prints the counts, the axe rules and the worst states.

*M10-RESP — the gate.* With `SHORTWIN_RESPONSIVE_GATE=1` each test fails on a finding: sideways
document overflow, a layout wider than the screen, a zoomed-out page, an element off screen (a
tablist or a table still in table mode may scroll inside itself), an axe violation, a target under
24 px, a primary control under 44 px with a coarse pointer, text under 12 px — *thirteenth round
(O-4):* at **every** viewport (v1.4 checked the phones only, `phone-xs`, `phone-s`, `phone`, while
the 640 px-and-up type scale went down to 11 px) —, a control under another control's touch area
(*thirteenth round*, the tap covers above), a failed step or a DOM-click fallback. Required before each responsive
phase merges. *Integration (v1.4):* under 1,024 px the capture opens the trade panel as a user
does — the page first (the header's wallet is outside the sheet), then the action bar — and adds
two states there: `market-trade-sheet` and `position-manage-sheet`. A full-page screenshot clears Chromium's touch
emulation (`(pointer: coarse)` then reads false until the page closes), so the capture turns it
back on after every shot on a touch viewport; before v1.4 the tablet and the landscape phone were
measured, and their later PNGs drawn, with a mouse's density after their first full-page shot.
*Thirteenth round:* new states — `markets` now shows the intro strip; `markets-intro-hidden` (after
Hide), `glossary-open` (a market page's "Rate (APR)" ⓘ open; on a phone the bottom sheet),
`how-it-works` and `how-it-works-risks` (the page at `#short-risks`), `short-coverage-link` (the
paused market's Short panel with its link; under 1,024 px in the trade sheet) and
`coverage-banner-focused` (after the link: the banner focused, the sheet closed). *The owner's
decisions of 2026-10-11:* `lender-dashboard-shortfall` and `lend-claim-shortfall` (wallet
`potClaimer`, the pot market's only lender, whose rent credited is not paid in yet: since "only when
syncs can pay" the disabled "Review claim · 0 SOL" with "Nothing to claim yet"), and — in a second
pass over all eight viewports, after every viewport's first (it changes the pot market: `potLate`
lends there and `potBorrower` closes, so `potLate` holds a surplus its sync can move) —
`lender-dashboard-syncs` and `lend-claim-syncs` (the row's and the panel's two-line Claim label), so
the Lending page's action cell is still measured with its widest label at every viewport.
*O-6 (2026-10-13):* `phone-s` (360) and `phone-xs` (320) send the Pixel 7 user agent, so with the
Mobile Wallet Adapter on (the default build) their `wallet-menu-disconnected` and
`wallet-menu-no-wallet` states show its entry ("Use installed wallet" over its second line, a
row at least 48 px tall in the phone's sheet) and the open-elsewhere block — the Android wallet states at 320 and 360 px, with the
Android project's at 412 px (§10.2); `phone` (390, iPhone) keeps the iOS states. The gate must
stay at zero findings on them (44 px rows under a coarse pointer, text ≥ 12 px — the hint is
`--fs-xs`, the O-4 floor —, no tap covers); their PNGs are new baselines (the capture's, in
`$SHORTWIN_SHOTS_DIR`: no committed set). *M3 (2026-10-13):* the gate's first run on the flag-on
build found one state, `phone-xs/markets` — not a wallet state: the first market row 10.6 px too
low (its top at 578.6 px, the fixed bars at 584 px). The M3 host has no Inter, `--font-ui`'s first
family, so Chromium takes the next, `system-ui`, which fontconfig resolves to DejaVu Sans, its
default sans-serif (the host also has the Ubuntu fonts, which no family in the stack names).
DejaVu Sans is wider than the fonts the gate passed with before and set the intro's Hide on a
row of its own. `markets.css` now keeps that heading row one row on a short phone (§3.3, the
intro strip). The gate then passed: 96 tests, eight viewports, 63–65 states each, no
finding — the wallet states at 320 and 360 px with the adapter's entry and the open-elsewhere
block among them.

---

## 11. `data-testid` contract

Ids come from `src/config/testids.ts` (`T`); specs import the same constants. A parametrised id
appends `-<key>` (an address, a signature, an enum value or a disclosure id). State is exposed in
data attributes on the same element (`data-state`, `data-stage`, `data-ok`, `data-kind`,
`data-code`, `data-verdict`, `data-risk`, `data-value` with the raw integer), never only in text.
Adding an id is free; renaming or removing one changes this table in the same change
(`testids.test.ts`).

### 11.1 Shell, transaction review and disclosures (SCAFFOLD)

| id | constant | element and state |
|---|---|---|
| `app-header` | `T.header` | app header |
| `cluster-badge` | `T.clusterBadge` | cluster name off mainnet |
| `nav-<…>` | `T.nav` | `<key>` ∈ markets, positions, lend, status, security (*M10:* the status bar's link to `/security`); `aria-current="page"` when active |
| `search-input` | `T.searchInput` | combobox input (mint or ticker); `/` focuses it |
| `search-results` | `T.searchResults` | listbox |
| `search-result-<…>` | `T.searchResult` | `<mint>`; an option (a valid mint always yields one) |
| `wallet-button` | `T.walletButton` | opens the wallet menu; `aria-expanded` |
| `wallet-menu` | `T.walletMenu` | role=menu when it lists wallets or account actions; *M10-RESP:* without a wallet, `role="dialog"` "Connect a wallet" (install / open-in links); *O-6 (2026-10-13):* with the Mobile Wallet Adapter on Android, its entry reads "Use installed wallet" (`data-mwa`, `data-local-network`) |
| `wallet-option-<…>` | `T.walletOption` | `<wallet name>` as the wallet reports it (the E2E wallet: `ShortWin E2E`); *v1.2:* a second wallet with the same name is `<name> (2)`, and so on (M9-SEC-8); `data-duplicate` = its rank |
| `wallet-address` | `T.walletAddress` | connected address; `data-value` = full base58 |
| `wallet-disconnect` | `T.walletDisconnect` | menu item |
| `wallet-duplicate-badge` | `T.walletDuplicate` | *v1.2:* on an entry whose name another registered wallet uses: registration rank and feature set (M9-SEC-8) |
| `wallet-sends-itself-badge` | `T.walletSendsItself` | *v1.2:* on an entry with only `solana:signAndSendTransaction` (checked after landing, §5.4) |
| `frame-guard` | `T.frameGuard` | *v1.2:* the notice rendered instead of the app inside a frame (§9.3) |
| `slot-age-badge` | `T.slotAge` | *v1.2:* the age of a preview's chain read (§6.6); `data-state` ∈ fresh, aging, stale; `data-slots` |
| `status-bar` | `T.statusBar` | bottom bar |
| `status-indexer` | `T.statusIndexer` | `data-state` ∈ live, lagging, stale, down, unknown, idle |
| `status-rpc` | `T.statusRpc` | `data-state` ∈ ok, bad |
| `status-ws` | `T.statusWs` | `data-state` ∈ idle, connecting, open, reconnecting, closed |
| `status-sol-usd` | `T.statusSolUsd` | SOL/USD from `/v1/stats` |
| `status-math` | `T.statusMath` | `data-state` ∈ loading, ready, error (the WASM math module) |
| `status-github` | `T.statusGithub` | *2026-10-18 (owner):* the status bar's GitHub icon (tablet and desktop) → the source repository, a new tab |
| `status-discord` | `T.statusDiscord` | *2026-10-18 (owner):* the status bar's Discord icon (tablet and desktop) → the community Discord, a new tab |
| `status-x` | `T.statusX` | *2026-10-18 (owner):* the status bar's X icon (tablet and desktop) → @short_dot_win on X, a new tab |
| `stale-badge` | `T.staleBadge` | per-panel freshness badge; `data-state` as statusIndexer |
| `fallback-banner` | `T.fallbackBanner` | shown while the indexer is unreachable (RPC fallback) |
| `devnet-preview-banner` | `T.devnetPreviewBanner` | shown on devnet while the protocol configuration account is absent; market creation and trading are unavailable |
| `toast-region` | `T.toastRegion` | live regions container |
| `toast` | `T.toast` | `data-kind` ∈ success, error, info |
| `page-<…>` | `T.page` | `<page>` ∈ markets, create-market, market, positions, position, lend, status, security (*M10*); the page root — every track keeps it on its page |
| `route-stub-<…>` | `T.routeStub` | `<route>`; placeholder until a track replaces the page (removed by then) |
| `tx-modal` | `T.txModal` | review dialog |
| `tx-stage` | `T.txStage` | `data-stage` ∈ simulating, review, signing, confirming, confirmed, failed |
| `tx-sim-status` | `T.txSimStatus` | `data-ok` ∈ true, false |
| `tx-delta-<…>` | `T.txDelta` | `<address>:<field>` (`lamports`, `amount`, or a decoded field); `data-value` = signed change |
| `tx-cu` | `T.txCu` | `data-value` = compute-unit limit |
| `tx-fee` | `T.txFee` | `data-value` = fee lamports |
| `tx-sign` | `T.txSign` | enabled only after a successful simulation and every required acknowledgement |
| `tx-cancel` | `T.txCancel` | disabled while signing / confirming; *O-6 (2026-10-13):* enabled 5 s after a Mobile Wallet Adapter wallet call starts, closing the review (nothing is sent); `data-cancellable` ∈ true, false |
| `tx-retry` | `T.txRetry` | Refresh (review) / Retry (failed); *v1.2:* `data-mode` ∈ resimulate, rebuild ("Retry with a new quote"), close ("Close and review again") — `retryMode`, §5.3; absent when the outcome is unknown or the transaction landed |
| `tx-error` | `T.txError` | `data-kind` (TxErrorKind), `data-code` (custom error code) |
| `tx-signature` | `T.txSignature` | `data-value` = signature |
| `tx-explorer-link` | `T.txExplorer` | explorer link |
| `tx-wallet` | `T.txWallet` | *v1.2:* "Signing with <wallet> · <account>"; `data-value` = address, `data-sends-itself` (M9-SEC-8) |
| `tx-final-fee` | `T.txFinalFee` | *v1.2:* the fee as the wallet signed it, when it changed the compute budget; `data-value` = lamports (FE-2) |
| `tx-rebuilt-notice` | `T.txRebuilt` | *v1.2:* Retry rebuilt the plan from a fresh chain read (§5.3) |
| `tx-instruction-<…>` | `T.txInstruction` | `<i>` = index in the plan (compute-budget instructions excluded); `data-program` (`short.win`, `Token-2022`, …), `data-name` (the program client's instruction name, e.g. `DisableCpiGuard`, `Deposit`) |
| `disclosure-<…>` | `T.disclosure` | `<disclosure id>` (§4); rendered verbatim |
| `disclosure-ack-<…>` | `T.disclosureAck` | `<disclosure id>`; the acknowledgement checkbox |
| `nav-more` | `T.navMore` | *M10-RESP:* the phone tab bar's fourth cell (hidden from 640 px); opens the More sheet; `aria-haspopup="dialog"`, `aria-expanded` |
| `more-sheet` | `T.moreSheet` | *M10-RESP:* the More bottom sheet (`<dialog>`) |
| `more-status` | `T.moreStatus` | *M10-RESP:* More → `/status` |
| `more-security` | `T.moreSecurity` | *M10-RESP:* More → `/security` |
| `more-create-market` | `T.moreCreateMarket` | *M10-RESP:* More → `/markets/new` |
| `more-github` | `T.moreGithub` | *2026-10-18 (owner):* More → the source repository on GitHub, a new tab |
| `more-discord` | `T.moreDiscord` | *2026-10-18 (owner):* More → the community Discord, a new tab |
| `more-x` | `T.moreX` | *2026-10-18 (owner):* More → @short_dot_win on X, a new tab |
| `more-sol-usd` | `T.moreSolUsd` | *M10-RESP:* More's network block: SOL/USD |
| `search-toggle` | `T.searchToggle` | *M10-RESP:* under 1024 px, the header's Search button; opens the search overlay; `aria-expanded` |
| `search-close` | `T.searchClose` | *M10-RESP:* the overlay's Cancel (focus returns to `search-toggle`) |
| `status-summary` | `T.statusSummary` | *M10-RESP:* the phone's status strip (a button; hidden from 640 px): the worst state in words; `data-state` ∈ ok, warn, bad; `aria-expanded` shows every `status-*` item |
| `not-found-cta` | `T.notFoundCta` | *M10-RESP:* "Browse markets" on the not-found page |
| `table-sort-<…>` | `T.tableSort` | *M10-RESP:* `<table test id>`; a DataTable's sort select (shown in card mode, or while a sortable column is hidden) |
| `table-sort-dir-<…>` | `T.tableSortDir` | *M10-RESP:* `<table test id>`; the sort direction button: its label names the order ("Descending" / "Ascending"), `data-order` ∈ desc, asc; a tap flips it |
| `connect-cta-<…>` | `T.connectCta` | *M10-RESP:* `<surface>` ∈ short, lend, positions, lend-dashboard, position; an enabled "Connect wallet" that opens the inline chooser below it |
| `reveal-<…>` | `T.reveal` | *M10-RESP:* `<kind>` (e.g. `mint`, `address`, `wallet`); a truncated value's popover with the full value, Copy and Explorer |
| `wallet-open-in-<…>` | `T.walletOpenIn` | *M10-RESP:* `<wallet>` ∈ backpack, phantom, solflare; on a mobile device without a wallet: the wallet's universal "browse" link to this page (§5.4); *O-6:* also after the wallet entries on a mobile device (`wallet-open-elsewhere`) |
| `wallet-open-elsewhere` | `T.walletOpenElsewhere` | *O-6 (2026-10-13):* a mobile device with wallets listed: "Or open this page in a wallet app", the three `wallet-open-in-<…>` links and, off mainnet, the cluster hint — after the entries, outside `role="menu"` (§5.4 (6)) |
| `wallet-in-app-browser` | `T.walletInAppBrowser` | *O-6:* a social app's in-app browser without a wallet (`isInAppBrowser`): "Open this page in your browser", first in the chooser |
| `wallet-copy-link` | `T.walletCopyLink` | *O-6:* its "Copy link" button: copies this page's address ("Copied" for 1.2 s) |
| `wallet-mwa-hint` | `T.walletMwaHint` | *O-6:* the Mobile Wallet Adapter entry's second line: "Opens your wallet app on this phone", or the local-network permission text while Chrome's permission is not granted |
| `wallet-install-<…>` | `T.walletInstall` | *M10-RESP:* `<wallet>` as above; the wallet's website |
| `wallet-reload` | `T.walletReload` | *M10-RESP:* "Already in a wallet's browser? Reload" / "I installed one: reload" |
| `wallet-copy-address` | `T.walletCopyAddress` | *M10-RESP:* connected menu: copy the full address |
| `wallet-explorer` | `T.walletExplorer` | *M10-RESP:* connected menu: the account on the explorer |
| `wallet-my-positions` | `T.walletMyPositions` | *M10-RESP:* connected menu → `/positions` |
| `wallet-my-lending` | `T.walletMyLending` | *M10-RESP:* connected menu → `/lend` |
| `wallet-sheet-close` | `T.walletSheetClose` | *M10-RESP:* the header wallet popup's ✕ on a phone (its bottom sheet; Back closes it too) |
| `tx-sign-hint` | `T.txSignHint` | *M10-RESP:* "Tick the acknowledgement above to sign" — a button that focuses the first unticked acknowledgement; only while one blocks signing |
| `tx-sign-hold` | `T.txSignHold` | *Thirteenth round (second approval pass, 2026-10-11):* why a successful simulation is not offered for signing (`TxPlan.holdSign`) — a claim whose simulation pays 0 lamports reads "Nothing to claim yet" (§3.6); beside the disabled Sign, in place of `tx-sign-hint` |
| `tx-confirmed-link` | `T.txConfirmedLink` | *M10-RESP:* after landing: the flow's next step (e.g. View position), when the plan has one (the explorer is `tx-explorer-link`) |
| `tx-sign-notice` | `T.txSignNotice` | *O-6 (2026-10-13):* the Mobile Wallet Adapter's notice beside an enabled Sign, the review kept open: `data-reason` ∈ local-network ("Allow 'Apps on device' so this page can reach your wallet app"), launch ("Couldn't open your wallet app. Tap Sign again") (§5.4 (6)) |

### 11.2 Markets table, market page, create market, protocol status (MARKETS)

| id | constant | element and state |
|---|---|---|
| `markets-table` | `T.marketsTable` | table |
| `markets-row-<…>` | `T.marketsRow` | `<mint>`; row, Enter / click opens the market |
| `markets-filter-venue` | `T.marketsFilterVenue` | select |
| `markets-filter-tier` | `T.marketsFilterTier` | select |
| `markets-sort-<…>` | `T.marketsSort` | `<key>` ∈ tvl, oi, util, created, open_positions; header button, `aria-sort` on its th |
| `markets-empty` | `T.marketsEmpty` | empty state |
| `markets-load-more` | `T.marketsLoadMore` | keyset paging button |
| `markets-create-link` | `T.marketsCreateLink` | link to /markets/new (prefilled with the searched mint) |
| `markets-empty-create` | `T.marketsEmptyCreate` | the empty state's "Create a market for this mint" link on /markets when the filter is a mint without a market (design pass P5-7) |
| `markets-search` | `T.marketsSearch` | filter input `q` (mint, or symbol / name prefix; ≤ 64 chars) |
| `markets-filter-active` | `T.marketsFilterActive` | checkbox: only markets with open positions (`active`) |
| `markets-open-mint` | `T.marketsOpenMint` | “open a mint” input, shown instead of the list while the indexer is unreachable (§6.5) |
| `market-header` | `T.marketHeader` | page head |
| `market-symbol` | `T.marketSymbol` | ticker |
| `market-mint` | `T.marketMint` | `data-value` = mint |
| `market-tier-badge` | `T.marketTier` | tier badge; opens the `market.tierAge` popover |
| `market-venue-badge` | `T.marketVenue` | venue badge |
| `market-flag-<…>` | `T.marketFlag` | `<flag>` ∈ paused-opens, coverage-paused, route-broken, venue-upgraded, migrating, graduation-halted, list-only, mint-unsafe, warming-up, pool-window; present only while true |
| `market-stat-<…>` | `T.marketStat` | `<stat>` ∈ inventory, utilisation, rate, oi, caps, tvl, price, lenders; `data-value` raw |
| `market-tier-why` | `T.marketTierWhy` | tier explanation |
| `market-creator-deposits` | `T.marketCreatorDeposits` | creator-wallet deposits block (SPEC R1) |
| `market-liquidations-table` | `T.marketLiquidations` | recent liquidations table |
| `market-liquidation-row-<…>` | `T.marketLiquidationRow` | `<signature>`; `data-kind` ∈ liquidation, expiry, settlement; `data-condition` S1–S3 |
| `market-tab-<…>` | `T.marketTab` | `<tab>` ∈ overview, positions, lenders, liquidations, pools, coverage, events |
| `market-panel-tab-<…>` | `T.marketPanelTab` | `<panel>` ∈ short, lend (trade panel tabs, rendered by the market page) |
| `market-source` | `T.marketSource` | `data-value` ∈ indexer, chain (chain: the indexer is down or does not know the market, FE-D1) |
| `market-not-found` | `T.marketNotFound` | no market for this mint, on chain or in the indexer; links to `/markets/new?mint=` |
| `market-banner-<…>` | `T.marketBanner` | `<flag>` as in `market-flag-<…>`; one banner per flag in §3.4's severity order, `data-severity` |
| `market-spot-price` | `T.marketSpotPrice` | spot price (`spot_wad`); `data-value` = WAD |
| `market-indexer-mismatch` | `T.marketIndexerMismatch` | *v1.2:* banner while the indexer answers for this mint with another token's detail; the page shows the chain's view (M9-SEC-3) |
| `market-duplicate-symbol` | `T.marketDuplicateSymbol` | *v1.2:* another listed market uses this symbol (R-MINT §10.11) |
| `market-fees-table` | `T.marketFees` | fee lines (Overview) |
| `market-lender-loss` | `T.marketLenderLoss` | realised lender loss by S1 / S2 / S3 (Overview); `data-value` = total lamports |
| `market-positions-table` | `T.marketPositions` | open positions (Positions tab) |
| `market-position-row-<…>` | `T.marketPositionRow` | `<position>`; opens the position page |
| `market-lenders-table` | `T.marketLenders` | lenders, largest first (Lenders tab) |
| `market-lender-row-<…>` | `T.marketLenderRow` | `<owner>`; `data-coin-creator`, `data-market-creator` ∈ true, false (SPEC R1) |
| `market-creator-share` | `T.marketCreatorShare` | the coin creator's share of the pool (Lenders tab header); `data-value` = bps |
| `market-pools-table` | `T.marketPools` | pools and their adoption history (Pools tab) |
| `market-coverage` | `T.marketCoverage` | keeper coverage: gaps, pauses, observations (Coverage tab) |
| `market-events-table` | `T.marketEvents` | the market's events, newest first (Events tab) |
| `market-route-chain-<…>` | `T.marketRouteChain` | `<chain>` ∈ no-route, pool-fail: the running route chain's start (Overview); `data-value` = unix seconds, 0 when none |
| `create-mint-input` | `T.createMintInput` | mint input |
| `create-check-verdict` | `T.createVerdict` | `data-verdict` ∈ ok, reject, existing, checking |
| `create-check-reason-<…>` | `T.createReason` | `<code>` = the program error name the filter would return (e.g. MintFreezeAuthority) |
| `create-venue-detected` | `T.createVenue` | detected venue; `data-value` = VenueName |
| `create-pool-candidates` | `T.createPools` | Raydium candidates table |
| `create-pool-row-<…>` | `T.createPoolRow` | `<pool>`; `data-eligible` |
| `create-cost` | `T.createCost` | fee + rent; `data-value` lamports |
| `create-submit` | `T.createSubmit` | opens the review |
| `create-existing-market` | `T.createExisting` | link to the existing market of this mint |
| `create-mint-info` | `T.createMintInfo` | the mint's program, decimals, supply, authorities and extensions |
| `create-choice-<…>` | `T.createChoice` | `auto` or `<pool>`: the listing choice (radio) |
| `status-page` | `T.statusPage` | page root |
| `status-indexer-lag` | `T.statusIndexerLag` | indexer lag; `data-state` |
| `status-paused-opens` | `T.statusPausedOpens` | global pause; `data-value` true/false |
| `status-venues-table` | `T.statusVenues` | venue table |
| `status-venue-row-<…>` | `T.statusVenueRow` | `<venue program name>` (pump, pump_amm, pump_fees, raydium_v4, raydium_cpmm); `data-upgraded`, `data-enabled`, `data-route-broken` |
| `status-degraded-markets` | `T.statusDegraded` | coverage-paused markets table |
| `status-degraded-row-<…>` | `T.statusDegradedRow` | `<mint>` |
| `status-route-broken` | `T.statusRouteBroken` | route-broken markets / venues list |
| `status-route-broken-row-<…>` | `T.statusRouteBrokenRow` | `<mint>` or `<venue>`; `data-kind` ∈ market, venue |
| `status-scanned` | `T.statusScanned` | markets scanned for the degraded and route-broken lists (FE-5); `data-value` = count |
| `status-indexer-health` | `T.statusIndexerHealth` | `/v1/status` lag and data-quality counts |
| `status-coverage-gap` | `T.statusCoverageGap` | keeper coverage gaps (`/v1/stats.coverage_gap_s`); `data-value` = seconds |
| `status-extras-table` | `T.statusExtras` | the venue extras sets in force (`/v1/venues.extras`) |
| `markets-filters-toggle` | `T.marketsFiltersToggle` | *M10-RESP:* phone: shows / hides the venue, tier and open-positions filters ("Filters · n" = active filters); `aria-expanded`; hidden from 640 px |
| `markets-clear-filters` | `T.marketsClearFilters` | *M10-RESP:* in an empty filtered list: clears the text filter, venue, tier and open-positions filters |
| `market-action-short` | `T.marketActionShort` | *M10-RESP:* under 1024 px, the market page's action bar: opens the trade sheet at Short (`?sheet=trade&panel=short`); accessible name "Short <symbol>" |
| `market-action-lend` | `T.marketActionLend` | *M10-RESP:* as above, at Lend (`panel=lend`); "Lend <symbol>" |
| `trade-sheet-close` | `T.tradeSheetClose` | *M10-RESP:* the trade sheet's close button (Back and Escape close it too) |
| `market-stats-more` | `T.marketStatsMore` | *M10-RESP:* phone: shows the stat strip's secondary stats (inventory, open interest, lenders); `aria-expanded`; hidden from 640 px |
| `market-tier-why-close` | `T.marketTierWhyClose` | *M10-RESP:* closes the "Why this tier" popover (a bottom sheet on a phone) |
| `create-mint-paste` | `T.createMintPaste` | *M10-RESP:* coarse pointer only: pastes the mint from the clipboard (fails silently) |
| `create-submit-reason` | `T.createSubmitReason` | *M10-RESP:* why Review is disabled, above it in the form foot; empty while it is enabled |

### 11.3 Lend panel and lender dashboard (LEND)

| id | constant | element and state |
|---|---|---|
| `lend-panel` | `T.lendPanel` | `data-mint`, `data-action` |
| `lend-tab-<…>` | `T.lendTab` | `<action>` ∈ deposit, withdraw, claim |
| `lend-amount-input` | `T.lendAmountInput` | deposit amount (tokens) |
| `lend-max` | `T.lendMax` | fills the source account balance |
| `lend-token-account-select` | `T.lendTokenAccount` | source account select (lists the owner’s accounts of the mint, ATA first) |
| `lend-preview-<…>` | `T.lendPreview` | `<field>` ∈ shares, share-bps, value, unpaid-rent, unpaid-rent-share (*M10 batch 3*; the latter's cell carries `data-level` none / info / warn); `data-value` raw |
| `lend-submit` | `T.lendSubmit` | opens the review; disabled until the SPEC §5.3 box is ticked |
| `lend-cpi-guard-notice` | `T.lendCpiGuardNotice` | `token.cpiGuard` notice; `data-state` ∈ blocked, bracket |
| `lend-cpi-guard-bracket` | `T.lendCpiGuardBracket` | opt-in checkbox for the disable → deposit → enable bracket (FE-1) |
| `withdraw-shares-input` | `T.withdrawSharesInput` | shares to withdraw (or % of position) |
| `withdraw-max` | `T.withdrawMax` | fills the pro-rata allowance |
| `withdraw-preview-<…>` | `T.withdrawPreview` | `<field>` ∈ tokens, sol, allowance, lock, lock-after (*M10, FE-15*: `data-value` shares, `data-until` the lock's end), lock-until, vesting-forfeit |
| `withdraw-memo-notice` | `T.withdrawMemoNotice` | `token.memoRequired` notice |
| `withdraw-submit` | `T.withdrawSubmit` | opens the review |
| `claim-value-<…>` | `T.claimValue` | `<field>` ∈ claimable, vesting, pot, deficit; `data-value` lamports |
| `claim-submit` | `T.claimSubmit` | opens the review (`sync_lender` prepended when needed) |
| `cover-deficit-submit` | `T.coverDeficitSubmit` | cover_deficit |
| `lender-dashboard` | `T.lenderDashboard` | dashboard table |
| `lender-row-<…>` | `T.lenderRow` | `<market>` |
| `lender-cell-<…>` | `T.lenderCell` | `<field>` ∈ shares, value, claimable, vesting, lock, lock-until, creator |
| `lend-blocked-reason` | `T.lendBlocked` | why a lend action is refused now; `data-code` = the program error name (e.g. MintUnsafeFlagged, LenderBehindEpochs, NegativePending) |
| `lend-position-<…>` | `T.lendPosition` | the panel's position summary; `<field>` ∈ shares, share-bps, value, claimable, lock; `data-value` raw |
| `lend-slippage-input` | `T.lendSlippageInput` | tolerance in % for `min_shares` / `min_tokens` (default 0.5, FE-D6) |
| `lend-sweep-notice` | `T.lendSweepNotice` | `data-kind` ∈ wipe (after a full wipe), emptied (*AIR-08:* every lender left; the sweep books to the protocol), lenders (*AIR2-05:* surviving shares, including legacy migration). Empty pools prepend the bundle to deposit; surviving shares get a separate cleanup review |
| `lend-cleanup-submit` | `T.lendCleanupSubmit` | **Review pool cleanup**: venue claims plus `sweep_trader`, no deposit; keeps the form's amount and waits for cleanup confirmation and any held vest to release |
| `lend-sync-submit` | `T.lendSyncSubmit` | `sync_lender` of the own position when it is behind more share epochs than one transaction carries |
| `withdraw-destination-select` | `T.withdrawDestination` | destination token account select (ATA first; a missing ATA is created idempotently); `data-memo` true/false |
| `withdraw-percent-<…>` | `T.withdrawPercent` | `<pct>` ∈ 25, 50, 75 (*M10-RESP*: 75 added; a chip, `aria-pressed` while the input holds that share): fills that share of the holding |
| `lender-epochs-<…>` | `T.lenderEpochs` | `<market>`; `data-state` ∈ current, behind, parked; `data-value` = epochs behind |
| `lender-indexer-<…>` | `T.lenderIndexer` | `<market>`; `data-state` ∈ ok, unsynced (indexer shares ≠ chain), quarantined (the market has quarantined events), missing (chain only) |
| `lender-claim-<…>` | `T.lenderClaim` | `<market>`; per-row claim — *thirteenth round (owner, 2026-10-11):* the Lend → Claim tab's claim and label, enabled with SOL claimable or syncs that can add 10,000 lamports or more (only when syncs can pay; disabled and `aria-busy` while its search runs with nothing claimable, §3.8) |
| `lender-sync-<…>` | `T.lenderSync` | `<market>`; per-row `sync_lender` when behind |
| `lender-claim-all` | `T.lenderClaimAll` | claims every market with SOL claimable (*thirteenth round:* or syncs that can add 10,000 lamports or more — the rows whose own Claim is enabled, not behind the share epoch), one reviewed transaction per market |
| `lender-empty` | `T.lenderEmpty` | empty state (no wallet / no positions) |
| `lend-deposit-reason` | `T.lendDepositReason` | *M10-RESP:* the deposit's FormFoot reason (`role=status`; empty while the submit is enabled; a button that focuses the acknowledgement or the CPI Guard box when that is what is missing) |
| `lend-withdraw-reason` | `T.lendWithdrawReason` | *M10-RESP:* the withdrawal's FormFoot reason (as above; focuses the vest acknowledgement) |
| `lend-claim-reason` | `T.lendClaimReason` | *M10-RESP:* the claim's FormFoot reason |
| `withdraw-approx` | `T.withdrawApprox` | *M10-RESP:* "≈ N TOKEN" under the shares input (the preview's tokens; display only) |
| `lend-empty-cta` | `T.lendEmptyCta` | *M10-RESP:* the dashboard's empty state: Browse markets (→ `/markets`) |
| `lender-kpi-<…>` | `T.lenderKpi` | *M10-RESP:* `<field>` ∈ markets, value (≈ SOL at each market's EMA price; `data-priced` = rows with a price), claimable, credited (`data-value` lamports) |
| `lender-source` | `T.lenderSource` | *M10-RESP:* the dashboard's "Chain only" badge while the indexer is unavailable |
| `lender-withdraw-<…>` | `T.lenderWithdraw` | *M10-RESP:* `<market>`; link to the market's Lend → Withdraw (`?panel=lend&action=withdraw&sheet=trade`) |
| `lender-deposit-<…>` | `T.lenderDeposit` | *M10-RESP:* `<market>`; link to the market's Lend → Deposit (`sheet=trade` as above) |
| `lender-claim-reason-<…>` | `T.lenderClaimReason` | *M10-RESP:* `<market>`; why that row's Claim is disabled, in text — after the row's buttons (Withdraw, Deposit, Claim: the same small button), on their line |

### 11.4 Short panel, positions and position detail (SHORT)

| id | constant | element and state |
|---|---|---|
| `short-panel` | `T.shortPanel` | `data-mint` |
| `short-q-input` | `T.shortQInput` | size Q in tokens |
| `short-slippage-input` | `T.shortSlippageInput` | max slippage in % (bps inside) |
| `short-preview-<…>` | `T.shortPreview` | `<field>` ∈ q, proceeds, margin, open-fee, total-cost, cr-open, cr-ema, liq-price, rent-rate, min-rent, max-duration, deadline, tier-terms, caps; `data-value` raw (`caps`: the largest `Q` the program accepts now, with `data-cap` = the binding cap; `tier-terms`: the tier letter) |
| `short-blocked-reason` | `T.shortBlocked` | why opens are refused now; `data-code` = the program error name |
| `short-cap-warning` | `T.shortCapWarning` | size above a cap; `data-cap` ∈ position, market, utilisation, wallet, gamma, inventory, notional |
| `short-submit` | `T.shortSubmit` | opens the review |
| `positions-table` | `T.positionsTable` | positions table |
| `positions-filter` | `T.positionsFilter` | status filter |
| `position-row-<…>` | `T.positionRow` | `<position>`; `data-status` |
| `position-cell-<…>` | `T.positionCell` | `<field>` ∈ market, q, vault, cr, liq-price, pnl, rent, time-left, risk, status; `data-value` raw, risk `data-risk` |
| `position-add-margin` | `T.positionAddMargin` | row action (links to the position page's add-margin form; the page itself uses `add-margin-submit`) |
| `position-close` | `T.positionClose` | row action (links to the position page's close form; the page itself uses `close-submit`) |
| `position-detail` | `T.positionDetail` | detail page root; `data-value` = position |
| `position-data-mismatch` | `T.positionDataMismatch` | *v1.2:* the indexer's market or mint for a position disagrees with the chain's (dashboard row badge, position page banner; M9-SEC-2) |
| `add-margin-input` | `T.addMarginInput` | SOL amount |
| `add-margin-preview-cr` | `T.addMarginPreviewCr` | CR after; `data-value` bps |
| `add-margin-submit` | `T.addMarginSubmit` | opens the review |
| `close-slippage-input` | `T.closeSlippageInput` | max slippage on the buyback |
| `close-preview-<…>` | `T.closePreview` | `<field>` ∈ buyback, rent, payout, pnl |
| `close-submit` | `T.closeSubmit` | opens the review (*2026-10-15, pump v3:* `close-straddle-notice` is removed with FE-3, §5.11) |
| `repay-panel` | `T.repayPanel` | price-independent borrower token repayment form |
| `repay-q` | `T.repayQ` | full raw principal; `data-value` raw units |
| `repay-rent` | `T.repayRent` | stored-basis rent estimate; `data-value` lamports |
| `repay-source` | `T.repaySource` | borrower token-account selector |
| `repay-max-rent` | `T.repayMaxRentInput` | explicit SOL rent bound; initially empty |
| `repay-use-estimate` | `T.repayUseEstimate` | user action copying the current rent estimate |
| `repay-exact-limit` | `T.repayExactLimit` | exact SOL and lamport authorization bound |
| `repay-pnl-disclosure` | `T.repayPnlDisclosure` | external token cost and unavailable economic P&L notice |
| `repay-reason` | `T.repayReason` | why repayment review is disabled |
| `repay-submit` | `T.repaySubmit` | builds the SDK repayment plan and opens simulation/review |
| `short-submit-reason` | `T.shortSubmitReason` | *M10-RESP:* why `short-submit` is disabled, one line in the form's sticky foot (`role=status`; `data-reason` while there is one); while the acknowledgement blocks it, a button that focuses the box |
| `positions-totals` | `T.positionsTotals` | *M10-RESP:* the dashboard's totals line (a KPI strip since design pass P6-2) — count, vault, P&L (sign and arrow), rent due |
| `positions-empty-cta` | `T.positionsEmptyCta` | *M10-RESP:* "Open a short" (to `/markets`) when there are no open positions |
| `position-not-found-cta` | `T.positionNotFoundCta` | *M10-RESP:* "Back to positions" on a position page with no position at the address |
| `position-action-add-margin` | `T.positionActionAddMargin` | *M10-RESP:* the position's action bar (under 1024 px; open positions): opens the Manage sheet at Add margin (`?sheet=add-margin`) |
| `position-action-close` | `T.positionActionClose` | *M10-RESP:* the same, at Close (`?sheet=close`) |
| `manage-sheet-close` | `T.manageSheetClose` | *M10-RESP:* the Manage sheet's close button (Back and Escape close it too) |
| `add-margin-reason` | `T.addMarginReason` | *M10-RESP:* why `add-margin-submit` is disabled (as `short-submit-reason`) |
| `close-reason` | `T.closeReason` | *M10-RESP:* why `close-submit` is disabled (as `short-submit-reason`) |

### 11.5 Newcomer help, the coverage link and the Claim label (the owner's thirteenth round)

| id | constant | element and state |
|---|---|---|
| `markets-intro` | `T.marketsIntro` | the intro strip above the markets list (absent once hidden) |
| `markets-intro-dismiss` | `T.marketsIntroDismiss` | its Hide button (remembered in `localStorage`; for the session where storage throws) |
| `markets-intro-link-<…>` | `T.marketsIntroLink` | `<link>` ∈ how-it-works, short-risks, lend-risks (to `/how-it-works`, `#short-risks`, `#lend-risks`) |
| `glossary-tip-<…>` | `T.glossaryTip` | `<id>` = the glossary entry: the ⓘ beside a label; `aria-expanded`, `aria-controls` while open |
| `glossary-popover-<…>` | `T.glossaryPopover` | `<id>` = the glossary entry: its open definition (`role=dialog`; `data-placed` once positioned) |
| `glossary-close` | `T.glossaryClose` | the open definition's ✕ |
| `nav-how-it-works` | `T.nav('how-it-works')` | the status bar's How it works link (`nav-<…>`, §11.1) |
| `more-how-it-works` | `T.moreHowItWorks` | the phone More sheet's How it works row |
| `page-how-it-works` | `T.page('how-it-works')` | the How it works page root (`page-<…>`, §11.1) |
| `how-it-works-section-<…>` | `T.howItWorksSection` | `<anchor>` ∈ shorting, lending, what-can-go-wrong, short-risks, lend-risks, every-market, fees, before-you-sign, read-more: a section heading (focusable from script) |
| `how-it-works-config-error` | `T.howItWorksConfigError` | How it works: the alert with Retry when the GlobalConfig read fails or finds no account (the governable numbers show a dash, the borrower disclosures the same sentence) |
| `short-coverage-link` | `T.shortCoverageLink` | the Short panel's link to the page's coverage banner (O-2): "Read the full coverage notice", or in the trade sheet "Close and read the full coverage notice" |
| `claim-submit-syncs` | `T.claimSubmitSyncs` | the Claim button's second line, "syncs may add up to {g} SOL", while the syncs the search picks can add 10,000 lamports or more ({g} = min(shortfall, their gain); O-5 and the owner's decisions of 2026-10-11, §3.6) — in Lend → Claim and, inside its row, in each Lending page row's Claim (`lender-claim-<market>`, §3.8) |

---

## 12. Tracks and file ownership

### 12.1 Tracks

| Track | Builds | Depends on |
|---|---|---|
| **MARKETS** | markets table (§3.3), market page (§3.4), create market (§3.5), protocol status (§3.9) | the scaffold; `src/venues` `detectVenue` / `raydiumCandidates` (SHORT; mocked in MARKETS' tests until it lands); `ShortPanel`, `LendPanel`, `CapsSummary` by their frozen props |
| **LEND** | lend panel — deposit, withdraw, claim, cover deficit (§3.6) — and the lender dashboard (§3.8) | the scaffold (pipeline, token-account and mint readers, WASM lender previews) |
| **SHORT** | the venue layer (§5.1), short panel and open preview (§3.7), `CapsSummary`, positions dashboard and position detail with add margin and close (§3.8) | the scaffold |
| **E2E** | local validator bootstrap and seed, injected test wallet, Playwright config and specs (§10.2–§10.3), the `web-e2e` CI job | the scaffold now (smoke, shell, wallet, pipeline); each track's pages as they land, selecting only by §11 ids |

### 12.2 File ownership

Paths are under `apps/web/` unless absolute from the repository root. **SCAFFOLD** files are frozen:
a track that needs a change asks the integrator (a contract change updates this document).

| Owner | Files |
|---|---|
| SCAFFOLD (integrator) | `/biome.json`; `/pnpm-lock.yaml`; `/packages/math-wasm/**`; `/.github/workflows/ci.yml` job `web`; `package.json`, `tsconfig.json`, `vite.config.ts`, `vitest.config.ts`, `index.html`, `.env.example`, `public/**`; `src/main.tsx`, `src/App.tsx`, `src/router.tsx`; `src/config/**` (`disclosures.ts`, `errors.ts`, `programs.ts`, `env.ts`, `parseConfig.ts`, `csp.ts`; `testids.ts` — see the append rule below); `src/styles/**`; `src/components/**`; `src/shell/**`; `src/lib/**`; `src/api/**`; `src/rpc/**`; `src/wallet/**`; `src/math/**`; `src/tx/**`; `src/test/**`; `src/venues/types.ts`; this document's §0–§11 |
| MARKETS | `src/features/markets/**` — `MarketsPage.tsx`, `MarketPage.tsx`, `CreateMarketPage.tsx`, `ProtocolStatusPage.tsx` and new files beside them (e.g. `table/`, `market/`, `create/` with `mintSafety.ts`, `status/`, `markets.css`, `*.test.ts(x)`) |
| LEND | `src/features/lend/**` — `LendPanel.tsx`, `LenderDashboardPage.tsx` and new files (e.g. `deposit/`, `withdraw/`, `claim/`, `plans.ts` — the `TxPlan` builders for deposit, withdraw, claim, cover deficit, sync, the post-wipe sweep bundle and the CPI Guard bracket —, `previews.ts` — `PoolState` / `LenderState` / `ShareEpochs` from the accounts —, `lend.css`, `*.test.ts(x)`) |
| SHORT | `src/features/short/**` — `ShortPanel.tsx`, `CapsSummary.tsx`, `PositionsPage.tsx`, `PositionDetailPage.tsx` and new files (e.g. `open/`, `positions/`, `plans.ts` for open / close / add margin, `previews.ts`, `short.css`, `*.test.ts(x)`); `src/venues/**` except `types.ts` (`index.ts`, readers, account lists, extras, `*.test.ts` with keeper-derived vectors) |
| E2E | `playwright.config.ts`; `e2e/**` (`global-setup.ts`, `global-teardown.ts`, `validator/` start and seed scripts, `wallet/injected-wallet.js`, `fixtures.ts`, `indexer-mock.ts`, `specs/*.spec.ts`); `/.github/workflows/ci.yml` job `web-e2e` (a new job only); `/docs/DEVELOPMENT.md` a new "Web E2E" section (append only) |

**Append rule for shared contract files.** `src/config/testids.ts` and §11 of this document have one
section per track (the comments `// ---- … (MARKETS)` etc.). A track may **add** ids, and their §11
rows, inside its own section only; renaming or removing an id goes through the integrator. No track
edits `disclosures.ts` (copy changes need the owner) or `errors.ts`.

**Dependencies** are frozen (§2): no track edits `package.json` or the lockfile.

### 12.3 Integration and the M9 exit

1. SHORT lands `src/venues` first (`detectVenue`, `raydiumCandidates`, `tradeContext`); MARKETS
   switches from its mocks.
2. E2E lands the environment, the wallet and `smoke` / `tx-errors` against the scaffold; the other
   specs follow each track.
3. Exit (ARCH §9 M9): every §7.3 flow implemented with simulation before signing and the §4
   disclosures; `pnpm --filter web build`, `biome check`, `vitest run` green; the E2E profile
   `chain` green, including `lend-cpi-guard` and `lend-withdraw-memo` (§10.3) and axe on every page;
   this document updated "as built" (deviations flagged in §13).

**The exit is met (2026-10-02), with this evidence** (the integrated tree; Node 24.21.0, pnpm
12.7.0, Chromium 152 at `/usr/bin/chromium`, Agave 4.1.2's `solana-test-validator`):

| Criterion | Evidence |
|---|---|
| Every §7.3 flow, simulated before signing | market search, create market (verdict, venue detection, Raydium candidates, cost), deposit / withdraw / claim / cover deficit / sync, open / close / add margin, the dashboards, the market page and protocol status — each transaction through `useTxFlow` → `simulatePlan` (exact instructions, < 20 s) → `TxReviewModal` (instructions, deltas, disclosures) → Wallet Standard signing with `assertSameIntent`; E2E `short-open`, `short-close`, `add-margin`, `lend-deposit`, `lend-claim`, `lend-withdraw-memo`, `create-market`, `keyboard` land them on the validator; `tx-errors` proves Sign stays disabled on a failing simulation, a stale review is re-simulated, a tampered message is refused and a raised fee accepted |
| The §4 disclosures | one copy in `disclosures.ts`, held to this document and SPEC §5.3 verbatim by `disclosures.test.ts`; `lend-deposit` checks the SPEC §5.3 sentence on the deposit screen and that its checkbox gates Review and Sign |
| CPI Guard UX (§10.3) | `lend-cpi-guard`: notice and blocked plain deposit, the ATA re-enables it; the bracket's review lists `DisableCpiGuard` / `Deposit` / `EnableCpiGuard` and the landed transaction is exactly that, guard still on; the race maps Token-2022 error 42 (`data-kind="token"`, `data-code="42"`), Sign disabled, re-entry shows the notice |
| Memo UX (§10.3) | `lend-withdraw-memo`: the notice, `memo_program` passed, the memo program invoked before Token-2022's transfer, the destination rises by the previewed tokens; the race fails `MemoProgramRequired` (6095) and re-entry lands with the memo (*M10, TMEMO-9:* the race now lands with the memo, on mainnet's Token-2022) |
| Accessibility | axe (WCAG 2.2 A / AA) on every page and state the specs visit, `screens` included (0 violations); keyboard-only deposit and open |
| Security | the production build under its CSP: every spec fails on any CSP violation (none); the top-level allowlist and the intent check in `tx-errors` |
| Builds and tests | `pnpm -r build`, `biome check` (root and `apps/web`), `tsc --noEmit`, `vitest run` (*v1.2:* 56 files, 533 tests, 1 opt-in file skipped), `pnpm --filter @shortwin/client check` (28); `pnpm --filter web e2e`: **50 passed** (19 specs) on a fresh validator and seed |
| Visual pass | `screens.spec.ts`: 32 PNGs of every page and state, reviewed; the layout fixes are in §7.3; the reviewed set is in `apps/web/e2e/screenshots/` (*v1.2:* refreshed — price units, token mints, the signing wallet, the slot age) |
| Deviation | ARCH §9 names "Playwright E2E on Surfpool"; the E2E runs on `solana-test-validator` (FE-7) |

---

## 13. Open items and known limits

| # | Item | Status |
|---|---|---|
| FE-1 … FE-6 | Owner items of §0 | open (owner) |
| FE-7 | ARCH §9's M9 row names "Playwright E2E on Surfpool"; Surfpool needs live mainnet and the network (INDEXER §11.3), so the E2E runs on `solana-test-validator` with the test-venue build and dumped fixtures (§10.2). A Surfpool profile can be added later for the real venues. | flagged deviation |
| FE-8 | The mock venue drives the trading E2E; real-venue trading paths are covered by the SHORT track's unit vectors (keeper-derived account lists, fixture readers) and by the program's fork tests, not by a browser test against a live venue. *pump v3 (2026-10-15):* one exception, `short-close-curve` opens and closes on the dumped pump curve (pump's 2026-10-13 ELF) through the UI; the other real venues stay as before | by design |
| FE-9 | Mainnet acceptance of `withdraw`'s CPI'd memo (T-MEMO, U17) | met on the mainnet build (M10: LiteSVM and the E2E run mainnet's Token-2022 and spl-memo v3 ELFs, §5.7, §10.2); a mainnet transaction is M11's (TMEMO-10) |
| FE-10 | Indexer follow-ups: `flag=` filters on `/v1/markets` (FE-5); a `borrower:<owner>` WebSocket channel would replace per-position subscriptions (≤ 40 rows live today) | open (indexer) |
| FE-11 | Wallets with only `solana:signAndSendTransaction` cannot be checked before sending (§5.4); they are labelled and checked post hoc (*v1.2:* implemented — `checkLanded`, `wallet-sends-itself-badge`, `unknown-outcome` instead of `expired`; M9-SEC-1). Refusing such wallets for writes altogether is the stricter alternative (FE-21) | by design |
| FE-12 | Token-2022 metadata (name / symbol) on mints without Metaplex metadata depends on the indexer's refresher (INDEXER IX-16); the UI falls back to the short mint | open (indexer) |
| FE-13 | Price charts: an optional sparkline from `/candles`; a full chart is not in M9 | by design |
| FE-14 | "Why this tier" shows the Tier A / B thresholds in USD and SOL, the pool's age and its basis, and SOL/USD with its age, but not the pool's own locked liquidity Λ in USD: that needs live reserves from the venue layer's `tradeContext` or an indexer field | open (MARKETS follow-up) |
| FE-15 | WASM facade gaps — **closed in M10**: `spotNearEma`, `pOpenMax`, `lambdaBps`, `lockedDepth`, `lockedSol`, `lockedFloorOk`, `mintCheck` (over `shortwin_venue_state::mint::check`; the further reasons are `check` re-run with each earlier rule lifted) and `previewWithdrawal` (withdraw, `LastSharesBorrowed`, claim, as the handler composes them: the tokens, the SOL part and the lender afterwards, so the panel shows the lock a withdrawal leaves, §3.6) are exported; the TypeScript ports are deleted (frozen in `src/m10-fe15/legacy.ts` as the test oracle of `parity.test.ts`). Frontend only, no program bytecode | closed (M10) |
| FE-16 | The vector generators `venues/vectors/keeper_vectors.rs` and `features/short/vectors/program_vectors.rs` run outside the web build (copied into `services/keeper/examples` and `tests/litesvm/examples`, instructions in each header; *pump v3:* `program_vectors.rs pump` writes `pump-vectors.json`); moving them into those crates would let CI regenerate the vectors | open |
| FE-17 | `raydiumCandidates` runs the keeper's four `getProgramAccounts` discovery calls from the browser; public mainnet RPCs may refuse or throttle them on AMM v4 — an indexer endpoint or a dedicated discovery RPC | open (M11) |
| FE-18 | Validator E2E (M10): `lend-post-wipe` lands the post-wipe bundle `[sweep_trader, deposit]` after an organic full wipe on the **mock** venue, and the wiped lender claims through the live and then the final record; `lend-claim-sync` lands `[sync_lender(other), claim_sol]` with a pot draw (the pot afterwards exactly before + the simulated surplus − the draw); `lend-sweep-cashback` lands the bundle's venue claim encodings (pump and PumpSwap `claim_cashback`, pAMM `init_user_volume_accumulator`) **by themselves** on the dumped ELFs, on an unused trader PDA. **Open:** one transaction composing the pump / PumpSwap venue claims with `sweep_trader` (token leg, UVA) and `deposit` on a wiped pump / PumpSwap market — it needs a pump / PumpSwap wipe on the validator (Node trade encoders and a cashback-coin curve fixture); LiteSVM covers it today (`m5_sec_curve_it`, `pamm_it::gates`) | partly closed (M10): the mock-venue bundle, the claim-time sync and the venue claim encodings on a validator; the pump / PumpSwap composite bundle is open (M11) |
| FE-19 | While the indexer is down the chain-only views fill `{creatorFee}` with "n/a (indexer unavailable)" and the `lender.lossHistory` numbers with "—"; the indexer mock's lender deposit / withdrawal history is 0 (it derives responses from chain state; profile `full` serves the real values) | by design |
| FE-20 | `tradeContext` (frozen interface) is implemented in `venues/index.ts` on SHORT's snapshot loader; the trading panels build from that snapshot directly, so the venue layer imports `features/short/snapshot.ts` | note |
| FE-21 | Sending-only wallets (M9-SEC-1): v1.2 implements the documented post hoc check — a wallet that lands something else is reported after the fact, not prevented. The stricter option is to refuse such wallets for writes (every major wallet also offers `solana:signTransaction`) | open (owner) |
| FE-22 | Slot-age thresholds (§6.6): shown above 2 slots behind the confirmed tip, Submit blocked above 75 slots / 30 s; the Lend panel counts as current while its account subscriptions are live. Tune after mainnet latency is measured | open (M11; needs mainnet latency) |
| FE-23 | Source maps (M9-SEC-6): built `hidden` into `dist-sourcemaps/`, not deployed; whether to upload them to a private error tracker or publish them deliberately | open (owner) |
| FE-24 | The deploy header check (`check:headers`) exists but no deployment pipeline runs it yet; the CDN must send `_headers`' header form. *M10:* it also checks `/.well-known/security.txt` (served as plain text, the production rules of §9.3) | open (M11 deployment) |
| FE-25 | Homoglyph tickers ("USDС" with a Cyrillic С) survive normalisation by design; the defence is the mint next to every symbol and the duplicate-symbol marks — no confusable-skeleton detection | by design |
| FE-26 | Retry rebuilds (M9-UX-1) cover deposit, withdraw, claim, cover deficit, sync, open and close; add margin (no quote bound) and create market close the review for a new one instead | by design |
| FE-27 | The deposit's unpaid-rent warning (M10 batch 3, §3.6): thresholds `UNPAID_RENT_WARN_BPS` = 100 (1 % of the deposit's value at `P_ema`) and `UNPAID_RENT_WARN_LAMPORTS` = 1 SOL, confirmed by the owner on 2026-10-03. The preview uses the stored `util_kink_bps` (the prologue first syncs a pending kink change), and rent keeps accruing between the preview and the landing; the exposure is this deposit's shares' part at the pool's current size (it grows if others withdraw first, as the text says) | closed (owner, 2026-10-03) |
| FE-28 | `lender.risks` (§4.2) lists the one-off income that vests but not "rent paid above what accrued, such as the minimum rent of an early exit", which vests since M10-8C-1 (ECON §9.4; the Claim form's note already says it). The copy lives in `disclosures.ts` and this document together (`disclosures.test.ts` pins them verbatim), so both change in one commit | open (copy) |
| FE-29 | `/.well-known/security.txt`'s `Expires` (2027-09-30) must be rotated before 2027-08-31: a mainnet build and `check:headers` refuse 30 days or less ahead | open (operations) |
| FE-30 | The claim-time pot syncs (KI66-R4, §3.6): the size check compiles without lookup tables (an upper bound: a v0 wallet with the protocol ALT could carry a sync more), and a sync is dropped only when the claim's own pre-review simulation fails on it — a target that closes or changes between that simulation and a stale review's re-simulation at Sign fails the review with `AccountNotInitialized` (3012), or `AccountOwnedByWrongProgram` (3007) on the `lender` account once someone funded the closed address, both of which the review's Retry rebuilds (`tx/errors.ts`; the rebuild leaves the closed target out, or refuses a claim of the claimant's own closed position). The judgement covered the indexer's first 24 lenders, largest raw share count first, so a large current lender could push an absent surplus holder out. *Ninth round (owner, 2026-10-08):* the lenders behind the share epoch (or with a forward chain) first, then the rest, paged further than 24 when needed, ranked by the simulated gain; 3007 mapped like 3012. *As built (the builder's choices under that decision, confirmed by the owner on 2026-10-09, tenth round, with the indexer's `max_page_limit` floor of 200):* the indexer's additive `epoch=behind` / `current` filter (INDEXER §9.2), 50 a page, paged further while the best ≤ 4 do not cover the shortfall, ≤ 4 pages (≤ 200 lenders; the bound's reason in §3.6); every row of either list is read whatever its epoch, so an indexer lagging or leading the RPC's share epoch only reorders the reads. What stays: a market with more than 200 lenders behind its share epoch is judged on the first 200 of them (largest raw share count first); the behind list's request counts toward the 4 pages even when it is empty or unfiltered, so a market with no lender behind its share epoch is judged on at most 150 current lenders, and an indexer without the filter spends the pages on its whole list in raw share order; an indexer that has not ingested a rebase lists the newly behind lenders among its current ones; and a sync that moved a surplus leaves its lender listed as behind, so a later claim re-reads it at no gain | closed (owner, 2026-10-08; the filter, the bound and the page-limit floor confirmed 2026-10-09) |
| FE-31 | `lender.residuals` (§4.2) quotes the held-model loss to lenders and the protocol from before AIR-22 ("about 2.06 SOL on a fresh position and about 8.02 SOL on one open for 25 days"); under AIR-22's lower-median growth the sim gives about 3.34 and 4.54 SOL (ECON §8.2, §12.6; known-issues KI-03). The copy lives in `disclosures.ts` and this document together (`disclosures.test.ts`; `m9-review/ux.test.tsx` pins the two figures) and a copy change needs the owner (§12.2), so the three change in one commit once the owner approves the new figures. *Approved by the owner on 2026-10-13:* the paragraph now reads "about 3.34 SOL on a fresh position and about 4.54 SOL on one open for 25 days at 90% utilisation" in `disclosures.ts` and §4.2, and `ux.test.tsx` pins the new figures and that the old ones are gone | closed (owner, 2026-10-13) |
| FE-32 | **The Mobile Wallet Adapter** (O-6 lifted on 2026-10-13): built, and on by default (`VITE_ENABLE_MOBILE_WALLET_ADAPTER`).<br>**Playwright covers it with a protocol-level fake wallet** (§10.2): the Android project, its flag-off run, the whole default run and the responsive gate passed on 2026-10-13 (M3; §13 "Mobile wallets", the E2E row), and the fake is held to the real library in Vitest.<br>**What only a phone shows:** the Android intent and the app switch, Chrome's "Apps on device" prompt, `upgrade-insecure-requests` on `ws://localhost`, a discarded background tab, which real wallets implement MWA and which signer row each is in (only the `solana:signTransaction` path signs, §5.4 (6)), how wallets answer a revoked token or a switched account at the reauthorization, and returning to Chrome without deciding. | **built** (the E2E passed on 2026-10-13) |
| FE-33 | **Social-app in-app browsers** are detected by user agent only (`; wv)`, Telegram, Twitter, Instagram, FBAN / FBAV / FB_IAB, Line). Apps that open links in SFSafariViewController or Custom Tabs are not detected, but those handle universal links. A universal link may not leave a WebView, so the app offers Copy link. MWA does not register in WebViews. | by design |
| FE-34 | **Firefox for Android:** MWA is not registered there, because its protocol frames a `solana-wallet:` URL that the CSP refuses (no `frame-src`). The open-in links remain. | by design |
| FE-35 | **MWA library calls that never settle** (wallet-standard-mobile 0.7.0): a failed reauthorization at Sign, a wallet that never answers after the association, the library spinner closed after the handshake, a Disconnect during a sign. The app bounds each one: Cancel while signing (after 5 s), the sign ended when the connected MWA account changes, the connect ended 10 s after the page is back in front (at most 120 s) with MWA's connect lock reset, and the library's hosts removed at insertion. The abandoned library call keeps its socket until the wallet closes it or the page unloads. Re-check at each package update (the drift test forces it). | mitigated; upstream fix wanted |

### pump v3 (2026-10-15; design pump-final §8, PUMP-V3 P5)

The web side of pump's v3 adapter, on the owner's decisions of 2026-10-13 (D1 (a): a live cashback
curve is refused; D2: the straddle bundle and `complete_curve` are retired; D3: `B̂_v3` is the risk
and exit price above `rT`; D4: the two error texts as drafted). P4 had already ported the curve
TradeExtras (`lists.ts`: pump's 10 v3 keys, `T4`–`T8` writable; `lists.test.ts` held to the
regenerated keeper vectors). The web deploys after the program upgrade (pump-final §15 step 3):
the 10-key list is refused by shortwin 1.0.0.

| Item | Change | Tests |
|---|---|---|
| Reader | `readCurve` reads the 166-byte layout's six fields (@125 `creator_fee`, @133 `protocol_fees`, @141 `depth` u8, @142 `ivqr`, @150 `pcB`, @158 `pcQ`; 0 past a shorter curve's end) and `quoteOffsetConsistent` (`vQ − rQ = ivqr`, vacuous on a legacy or migrated curve); `PUMP_GLOBAL_LEN` stays 1,087 | `readers.test.ts`: every dumped curve's fields against the manifest, `synthetic_stuck`, `graduated_synthetic`, zeros on 141 / 151 B, `mayhem166` inconsistent, the 1,088-byte `Global` |
| Venue state (`venues/state.ts`) | `listable` needs `depth = 0`; `executable` refuses a live cashback curve (a complete one stays observed, §6.1.6). `CurvePricingInputs` gains `syntheticFees` (`φ_s`: the curve's tier on the mint's **live** supply, the facade's `resolveAtSupply`; `null` on a complete, cashback or mayhem curve or without the supply), `postCompleteQuoteIn`, `postComplete`, `postCompleteKnown`; `curvePricing` is `null` on a live curve carrying `pc*` (G-3). The curve venue passes `pcQ` and, with a route, `withSynthetic(φ_s)`: past `rT` a live curve quotes `B̂_v3`, without a route nothing (`null`, not `B̂_str`). The EMA leg's pool tokens are 206.9e12, or the live `curve_base_ata` after a pool part (§5.4). `fvRepriced` ports `Quoter::fv_repriced`: on a migrating curve the facade's new `migratingFv` (ρ-scaled onto the pool `migrate_v2` creates), `null` when the post-complete fields are not known. The fee gates and the caps keep the 10¹⁵ `φ` (`feePpm`, `feePpmAtEma`), as the program's `oneway_ppm` does | `state.test.ts` "pump v3": near166's `rT + 10e12` at 22,944,563,746 (pump charged 22,944,563,744 on the 2026-10-13 ELF), continuity at `rT`, the capped view, `afterSale` (under G0's seven-tier curve `FeeConfig` a sale moves `φ_s` from 80 / 25 to 90 / 28: re-resolved, and equal to a fresh read of the post-sale accounts), no route without the supply; the cashback, depth, mayhem and G-3 guards; 98u8RJ's completion forged onto near166 — the EMA leg = PumpSwap's buy on `(X₁, Y₁)` exactly, FV ρ-scaled within 1 bps of that pool's buy, unchanged without a pool part, lower after a donation, capped, and `null` for the three unknown states |
| Listing (`venues/detect.ts`) | `classifyPump`, on the live branch only: a cashback coin `CurveCashbackCoin` (6109), a depth ≠ 0 `CurveQuoteNotSol` (§5.9) | `state.test.ts` (cashback166 refused, complete cashback lists as `pump_migrating`; depth 1 refused) |
| Previews, close (`previews.ts`, `trades.ts`, `CloseForm.tsx`) | `PositionMetrics.straddle` and the `CurveStraddle` refusal are gone: a close past `rT` previews and sends at `B̂_v3` (§5.11). The notice, its `close-straddle-notice` test id and its words are removed (D4, G-8: nothing new). `insufficientVault` no longer counts a missing quote as a thin vault ("No buyback quote now") | `vectors/pump-vectors.json` (new; `program_vectors.rs pump`: the **release** build on pump's 2026-10-13 ELFs in LiteSVM, near166): the open preview equals `open_short` (`sell_v3` proceeds, `N_open`, `B̂_ema`, margin, fee, vault, tier, deadline); the close below `rT` and the close past it equal `close_short` — the preview is the program's own `b_live` (`B̂_v3` past `rT`, 692,998,557 against pump's charge of 692,998,555), the payout what the program paid less that slack; the keeper's `liquidate` past `rT` (T-SYNTH-2's ECON D@80 % on the fresh curve, the crowd's buy down to `q/2`): the position metrics equal its event — `B̂_v3` at the live fee (`b_live`) and at the exit risk cap (`b_live_risk`), the EMA leg's CR on the newest ring entry (`cr_ema_bps`) and `R_due` — and the liquidation price is the spot (the event's spot CR is on the robust spot, which the web does not model); `plans.test.ts` builds both opens and closes exactly as the program executed them (the 7-account ReadSet, the 10 TradeExtras); `positions.test.tsx` |
| The D4 strings | `src/config/copyPump.ts` holds the program's two texts as drafted (§4.3's pump group): `errors.ts` takes 6031 and 6109 from it — byte for byte the program's `#[msg]`, a straight apostrophe and no final full stop on `CurveStraddle` | `copy.test.ts` (the drafted texts, the program's `errors.rs`, `errors.ts` and §4.3 held to each other; 72 / 152), `errors.test.ts` |
| D2 (`complete_curve` retired) | `programs.ts`, `describe.ts`, the `/security` page and its security.txt, `SECURITY.md`'s scope table; the E2E validator no longer deploys `complete_curve.so`; since P6 (the indexer's ingestion retired) the `full` profile's indexer config no longer names its id either | `programs.test.ts`, `securityPolicy.test.ts` |
| WASM | `Venue.migratingFv(q, p, state)` (`quote::migrating_fv`); the package rebuilt | `quotes::tests::migrating_fv_follows_the_post_complete_pool`; `js/smoke.cjs` |
| E2E | `short-close-curve.spec.ts`: near166 listed and lent from Node (the lender's tokens planted at genesis: pump's mints have no authority), an open in the Short panel on the curve, a third party's `buy_v3` down to `q/2`, the close on the position page at `B̂_v3`, the curve complete with `pcB = q − rT` and the market `PumpMigrating`; axe at 0, no CSP violation | §10.2 |

### Mobile wallets (2026-10-13; O-6 lifted)

The owner lifted O-6 on 2026-10-13 (decided, not reopened; design: the mobile track's design,
revision 2, after its security review): build the Mobile Wallet Adapter and switch it on (default
on, a build flag to turn it off; the owner's real-device test precedes the release), install
`@solana-mobile/wallet-standard-mobile` 0.7.0 with the protocol pinned to 3.0.0 through pnpm
overrides (Kit stays 8.3.0), ship the mobile chooser fixes with the drafted strings as they are
(§4.3's mobile group), no WalletConnect / Reown. The review's safeguards are taken as decided: the
adapter signs only through `solana:signTransaction`, a repo-wide 14-day `minimumReleaseAge` with the
two packages excluded until 2026-10-21, Disconnect local only, and the `ws://localhost` CSP residual
accepted (§9.3). Web only: no program, keeper, indexer or client change.

| Item | Change | Tests |
|---|---|---|
| Dependency, flag, CSP | the package (a lazy chunk, §2.2), the overrides and the release-age gate in `pnpm-workspace.yaml` (§9.6); `VITE_ENABLE_MOBILE_WALLET_ADAPTER` (strict `true` / `false`, §2.1, `.env.example`); flag on: `connect-src` + `ws://localhost:*` `http://localhost` and the spinner's two style hashes (§9.3), flag off byte for byte the old policy | `parseConfig`, `csp` (both states, in full), `mwaPackage` (the drift test) |
| Registration | `wallet/platform.ts` (`mwaEligible`, `isInAppBrowser`), `mwa.ts`, `localNetwork.ts` (Chrome's local-network permission, asked from a tap), `mwaCall.ts` (R-C1), `mobileWalletAdapter.ts` (`registerMwa` once from `App.tsx`, verified against the plugin's list; the chain-scoped cache; the app's `onWalletNotFound`; `removeLibraryUi`); `public/icon-192.png` (`scripts/icon-192.ts`) | `platform`, `mwaEligible.agree`, `mobileWalletAdapter`, `mwaCall`, `localNetwork` |
| The chooser and the copy | `copyWallet.ts` (the 7 strings, §4.3), "Use installed wallet" (unique name only), the open-elsewhere block, the in-app-browser block with Copy link, the not-found reopen, no "sends itself" badge for MWA, the connect guard; five test ids (§11.1) | `wallet.test.tsx` (a)–(l), `mwaConnect.agree`, `copy` (70 / 150) |
| Sign path, way out, errors | the prefetch for every wallet (§5.4 step 2); the local-network and launch notices (`tx-sign-notice`); R-S1 (Cancel after 5 s), R-S2 (`wallet-changed`), no send after an abort; the signer policy (`mwaSignerOk`, `useWallet`, asserted again at Sign); the MWA error rows (§5.9); `WalletCallError.sending` | `useTxFlow`, `errors`, `m9-review/tx`, `TxReviewModal`, `mwaSigner.agree` |
| E2E | the `<profile>-android` project, `SHORTWIN_E2E_MWA`, the seeded `android` wallet, the init-script hooks and the protocol-level fake wallet app, `mobile-mwa.spec.ts` (M1–M18, T-UIR, the flag-off describe), `mobile-shell` on Android, `wallet.spec.ts` on the desktop; CI's `web-e2e` adds the flag-off Android run (§10.2, §10.4) | `e2e/wallet/fake-mwa.test.ts` (the fake against the real library, Vitest); *M3 (2026-10-13; Playwright's Chromium 1243, once the host's missing `libnspr4`, `libnss3` and `libasound2` were installed with the owner's approval):* the Android project 25 passed, 2 skipped (the flag-off tests); `SHORTWIN_E2E_MWA=0` 8 passed, 19 skipped; the whole default run 121 passed, 2 skipped (17.9 min); the responsive gate 96 passed with no finding (§10.5); the reviewed `e2e/screenshots/android/` set (9 PNGs) |
| The first browser runs (M3, 2026-10-13) | Every Android spec passed on its first run with the adapter's code unchanged: the tap's user activation at the intent (M2, M5), M5's 1 s with every `getLatestBlockhash` held 6 s, M13 / M14's 1 s after the wallet's answer, the lend sheet's selectors, T-UIR without a Local Network Access flag or permission, and M8's new page load, silent reconnect, Cancel and Disconnect inside the 10 s; `location.assign('solana-wallet:…')` leaves headless Chromium's page where it is. Three fixes, none weakening an assertion: (1) the responsive gate's only finding, `phone-xs/markets` (DejaVu Sans, `system-ui` on a host without Inter (§10.5), set the intro's Hide on a row of its own and the first market row 10.6 px under the first screen): the intro's heading row stays one row on a short phone (`markets.css`, §3.3); (2) the in-app-browser block nested in the no-wallet chooser sat 4 px inside the open-in links (two `.wallet-chooser` paddings): the inner one has none (`components.css`); (3) M12 waited for nothing after Copy link, so its PNG raced the 1.2 s "Copied" confirmation: it now waits for "Copy link" to return (an added assertion) | E2E (`mobile-mwa`, `mobile-shell`, the whole run), the `responsive` gate; `e2e/screenshots/android/` reviewed by eye at 412 px, the gate's Android wallet PNGs at 320 and 360 px (rendered in DejaVu Sans, the M3 host's fontconfig default sans-serif, which `--font-ui` falls through to as the host has no Inter; the phone and desktop sets came from another host's fonts) |
| Docs | §1, §2, §2.1, §2.2, §5.4 step 2 and (6), §5.9, §7.7, §9.3, §9.6, §10.1, §10.2, §10.5, §11.1, §13 FE-32 … FE-35; AUDIT §7's O-6 row; ARCHITECTURE §1.1; DEVELOPMENT "Web E2E" and "Mobile wallets on a real device" (the owner's checklist) | `copy.test.ts`, `testids.test.ts` |

### AI review follow-ups (2026-10-12/13; one approved text changed, FE-31, with the owner's approval)

The web side of the AI review's fixes (`docs/audit/ai-review/`; `3f224c2` … `abce2b1`):

- AIR-07 (`3f224c2`, `3c0f602`): `PoolDrained` (6108, action `wait`) in `errors.ts` and in the
  WASM previews' `MATH_ERRORS`; `errors.test.ts` now holds the table to the program's
  `ShortwinError` enum, so it passes before and after the client regeneration; the lend vectors
  regenerated on the fixed `pool_model` (84 cases, a `last_share_borrowed` state among them).
- AIR-08 (`2bda5f5`): the sweep before a deposit whenever `S = 0`, the sweep notice's
  `data-kind` and the review title "sweep after the last exit" (§3.6, §11).
- AIR-05 (`975f757`): pump's `claim_cashback` and the pump-UVA token leg on any market whose
  `pump_uva` is set (§3.6).
- AIR-09 (`abce2b1`): the WASM `previewSettlement` prices the tokens at 0 on a venue that cannot
  execute (its new `executable` argument has no caller yet); AIR-21's S1 vest was already in it.
- The generated client (`clients/js`) is regenerated from this branch's build (`caa5852`:
  `PoolDrained` 6108, `SolSettle.vestFlushed`, `revalidate_venue`'s `expectedElfHash`, the new
  `VenueGuard` and `Market` fields), and `errors.test.ts` again requires one table row per
  client constant (`2fea2c5`).
- AIR-10 (`ed014ce`; the final review of the follow-ups, 2026-10-13): a position's risk view —
  the CR, the CR on the ring, the liquidation price, and the open preview's liquidation price —
  takes the program's exit risk cap (`exitRiskCapBps` = `VenueState::exit_risk_cap_bps`: on
  PumpSwap at most the tier at `P_ema`), so a PumpSwap position below a tier boundary no longer
  reads riskier than `liquidate` judges it (§3.8; `state.test.ts`, `previews.test.ts`).
- AIR-03 (`b1d86d4`; the same review): the deposit's unpaid-rent preview (`pendingLenderCredit`)
  passes the market's carried `lender_credit_rem` to the WASM `RentBasis.lenderCredit`, which now
  takes it (`rent::lender_credit_carried`), so the pending credit is the prologue's to the
  lamport (it could read 1 lamport low; §3.6; `previews.test.ts`, the facade's `smoke.cjs`).
- CI (`33491a3`): the `web` job runs the facade's Node smoke test (§10.4).
- FE-31 (approved by the owner on 2026-10-13): the one approved text these follow-ups change.
  `lender.residuals`' liquidation paragraph quotes the held model's Tier A figures under
  AIR-22's lower-median growth, about 3.34 SOL fresh and 4.54 SOL aged (were 2.06 / 8.02;
  §4.2, §13 FE-31).

### Thirteenth round (v1.4 → v1.5)

The owner's decisions of 2026-10-10 (binding), on the M10-RESP redesign's audit (D-03, D-09, D-21,
D-23). Every `data-testid` kept (new ones in §11.5), every flow and every approved disclosure text
unchanged; the new copy is §4.3's, approved as written by the owner on 2026-10-11 (thirteenth round).

| Item | Change | Tests |
|---|---|---|
| O-3 newcomer help (D-03) | the markets list's intro strip (§3.3); the glossary — `config/glossary.ts`, the ⓘ control and its attachment through the shared label renderers (§4.3); How it works, a new code-split route linked from the strip, the status bar and the More sheet (§3.2, §3.11) | `Glossary.test.tsx`, `glossary.test.ts`, `copy.test.ts`, `MarketsIntro.test.tsx`, `HowItWorksPage.test.tsx`, `router.test.tsx`; E2E `guide`, `mobile-guide`, `responsive` |
| O-1 deposit order (D-09) | account → amount → tolerance → preview → risks → the SOL-repayment acknowledgement → the sticky submit; texts verbatim (§3.6, §4.2) | `LendPanel.test.tsx` (DOM order); E2E `lend-deposit`, `mobile-lend`, `screens` |
| O-2 coverage notice once (D-23) | the Short panel links to the page's coverage banner instead of repeating `market.coveragePause`; from the trade sheet the link closes it and focuses the banner; without the banner the disclosure stays (§3.4, §4.2) | `ShortPanel.test.tsx`, `coverageLink.test.tsx`; E2E `guide`, `mobile-guide` |
| O-5 Claim wording (D-21) | "Review claim · {c} SOL now · syncs may add up to {g} SOL" on two lines while there is a shortfall, and the shortfall note rewritten; {g} with `solFrac` (§3.6, §4.3) | `LendPanel.test.tsx` (four states), `copy.test.ts` |
| Focus Not Obscured (2.4.11) | an E2E check of the `scroll-padding` / `scroll-margin` the build relied on untested (§7.6) | E2E `focus`, `mobile-focus` |
| O-4 12 px floor | `--fs-xs` 12 px from 640 px up (phones had it), `.unit` never under 12 px; no density switch; the responsive gate checks text ≥ 12 px at every viewport (§7.2, §7.7, §10.5) | E2E `responsive` (gate) |
| Not built | O-6 Mobile Wallet Adapter (after the audit; *lifted by the owner on 2026-10-13: built and on by default, §13 "Mobile wallets"*) | — |

### Thirteenth round: the owner's approval and decisions (2026-10-11)

The owner approved the thirteenth round's copy on 2026-10-11 and took the recommended option on all
four questions: the 143 strings of §4.3 approved as written, with recommendations 6–10 — Tier C stays
How it works' example, the risk texts render there without tick boxes, the short.win / ShortWin rule
is recorded (§7), the glossary cuts stand, "Lending page" stays —; `borrower.liquidation` ¶3 amended
as proposed; both Claim changes built (the dust threshold, the Lending page's Claim); the short-phone
height breakpoint approved; the lone header ⓘ accepted until a design pass after the audit. No other
user-facing string changed or was added; every `data-testid` kept (none new), simulate → review →
sign, the CSP, the breakpoint set and code splitting unchanged. Gates: biome, typecheck,
`vitest run` (75 files, 731 passed, 1 opt-in file skipped; with the round-13 review's fixes), `pnpm build` (no `<style>` tag or
`style` attribute in `dist/index.html`); the default E2E 95 passed (69 desktop, 26 phone); the
responsive gate 88 passed at all eight viewports (61–63 states each, two new) — against the round's
earlier capture: 0 document overflow, 0 elements off screen, axe 0, 0 targets under 24 px, 0 text
under 12 px, 0 tap covers (judged in the open sheets, the sticky trade panel, the sticky table
headers and dialogs too; 62 on the CSS before the review's fix); the sideways scrollers unchanged (the market page's tab strip on the
phones, which scrolls by design, and the Pools tab's table at 320 px, 21 px — both as before).

| Item | Change | Tests |
|---|---|---|
| Copy approval | §4.3's heading drops the *pending owner approval* marker and its text says "approved as written by the owner on 2026-10-11 (thirteenth round)", like §4.2's approved disclosures; the v1.5 status and the round's log likewise; `scripts/owner-copy.ts` prints the approved state and its §7 records the decisions (the owner file regenerated); a string changes only with the owner's approval, code and §4.3 together | `copy.test.ts` (each string under its id and `where`, exactly; §4.3's catalogue equals `owner-copy.ts --doc`'s block; 63 + 2 + 78 = 143 strings), `glossary.test.ts` (the control's two strings; each entry's term, labels and text, or the disclosure and paragraphs it quotes and, under it, the quoted words as the entry shows them — so a paragraph added, dropped or moved in the disclosure fails until §4.3 changes too —, with §4.2 carrying those paragraphs verbatim) |
| `borrower.liquidation` ¶3 amended (owner, 2026-10-11) | "Expiry: after {maxDuration} the position can be closed by anyone. Unless it can be liquidated at that point, you pay a {expiryFee} expiry fee instead of the penalty." — the expiry fee only when the position cannot be liquidated (SPEC §5.3; `expire.rs`'s `UseLiquidate` precondition); `disclosures.ts` (its source line) and §4.2 together; no glossary entry quotes ¶3 | `disclosures.test.ts` (¶3 pinned; §4.2 verbatim), `m9-review/positions.test.tsx` (the locked 5 % fee in the amended sentence) |
| The Claim's dust threshold (owner, 2026-10-11) | a shortfall under 10,000 lamports counts as none: `CLAIM_DUST_LAMPORTS` beside `previewClaimView` (`previews.ts`; ECON §9.4's rounding dust), so the label, the note and the syncs all follow — with nothing claimable the disabled "Review claim · 0 SOL" with "Nothing to claim yet", with something claimable "Review claim · {c} SOL" without the note, no syncs gathered (§3.6, §4.3) | `previews.test.ts`, `LendPanel.test.tsx`, `build.test.ts` (0, 9,999 and 10,000 lamports, with nothing and something claimable) |
| The Lending page's Claim (owner, 2026-10-11; audit D-21) | a row's Claim is the Lend → Claim tab's: the same claim (`claimPlanFrom`, the syncs for a shortfall), enabled with something claimable or a shortfall, the panel's label (`ClaimLabel`, now its own module; two lines with `claim-submit-syncs` in the row); "Claim all" keeps its label and claims the same rows — one predicate (`canClaim`) for a row's button and Claim all's count, total and the SOL claimable KPI, so a row behind the share epoch with every record read counts; with only shortfall rows the total reads 0 SOL (a known consequence). A shortfall no sync can fund (unpaid rent, no other lender with a surplus) still enabled a claim that paid 0 — put to the owner, who decided "only when syncs can pay" (the second approval pass, below) | `LenderDashboardPage.test.tsx` (labels, enabled on a shortfall alone, the builder per row and for Claim all, a behind row with its records read counted); E2E `lend-claim-sync` (after the claim alone lands, the row claims `[sync_lender, claim_sol]` and pays what the review simulated), `mobile-lend` |
| The short-phone breakpoint (owner, 2026-10-11) | `(max-width: 399.98px) and (max-height: 699.98px)` approved; `SHORT_PHONE` in `breakpoints.ts` (§7.6) | `breakpoints.test.ts` (a height only in that query) |
| The product's name (owner, 2026-10-11) | running text says short.win; ShortWin is the brand mark (header, tab title and description, start-up messages); recorded in §7, no string changed | — |
| The lone header ⓘ (owner, 2026-10-11) | accepted as built, to revisit in a design pass after the audit (§7.2) | — |
| The E2E scenarios above the dust threshold | the claim-time sync scenarios produced shortfalls of a few hundred lamports, now dust: `lend-claim-sync` books ≥ 40,000 lamports of credit first; `lend-claim-pot-chain` gaps 1,000× (not 100×) and credits rent for 300 s (not 90), a ≈ 12,000–15,000-lamport shortfall, the seeding payer funded with 400,000 SOL; `mobile-trade` waits for the trade sheet's slide-in before its first tap (in two runs after other specs the sheet was found scrolled at the axe check, its first labels under the sheet head, and axe's target-size judged the Slippage ⓘ obscured by the sheet's ✕; with the wait it was not — a likely race, not proven) (§10.2) | E2E `lend-claim-sync`, `lend-claim-pot-chain`, `mobile-trade` |
| Touch areas over a neighbour (the round's last two reviews) | the glossary ⓘ's hit area grows sideways only at every pointer (44 × 24 px under a finger, 32 × 24 px with a mouse; a table header's ⓘ keeps its 24 px box with any pointer); the Short preview's size caps are rows 24 px apart: grown on all sides an ⓘ covered the ⓘ of the next row, a Copy button or a field above or below; the focus walk fails on a point that lands on another control outside the page's bars (the header and tab bar, status bar, action bar, form foot, a sheet's, dialog's or menu's head, the toasts — no longer every fixed or sticky box, which had exempted the open sheets, the sticky trade panel, the sticky table headers and dialogs); the responsive capture measures every control's touch area (`collectTapCovers`, a gate finding; a control partly scrolled out of a dialog's body is not judged) (§7.6, §10.2, §10.5) | E2E `focus`, `mobile-focus` (before the first fix: the phone's market Overview failed at 4 stops; after: pass; with the page-bars list they pass before and after the second fix, whose covers sit in the trade sheet and panel the walks do not open); the responsive gate with the final check — on the CSS before the second fix 25 of 88 failed, 62 controls covered (3 at each phone and the tablet: the cap rows' ⓘ; 15–17 at 1,024–1,440 px: the lend position's, the Claim tab's and the Short preview's ⓘ rows, the withdraw amount under the shares ⓘ, the TVL sort button under its ⓘ), after it 88 of 88 passed, 0 tap covers at all eight viewports |

### Thirteenth round: the second approval pass (2026-10-11)

After the approval's review the owner decided the question it left open — "Only when syncs can
pay" — and the review's finding on the expired banner was fixed. No user-facing string changed or
was added (§4.3's 143 strings byte-identical; the owner file and §4.3 regenerated for their notes);
one `data-testid` added (`tx-sign-hold`, §11); simulate → review → sign, the CSP, the breakpoint
set and code splitting unchanged; no new dependency. Gates: biome, typecheck, `vitest run` (76 files, 748 passed, 1 opt-in
file skipped), `pnpm build` (no `<style>` tag or `style` attribute in `dist/index.html`); the
default E2E 95 passed (69 desktop, 26 phone; the reference screenshots refreshed); the responsive
gate 96 passed at all eight viewports (63–65 states each, two new: the second lender's pass) —
against the approval's capture: 0 document overflow, 0 elements off screen, axe 0, 0 targets under
24 px, 0 text under 12 px, 0 tap covers; the sideways scrollers unchanged (the market page's tab
strip on the phones, by design, and the Pools tab's table at 320 px).

| Item | Change | Tests |
|---|---|---|
| The Claim only when syncs can pay (owner, 2026-10-11) | the Claim works from what the syncs can move: before the button offers them, the review's search (`searchPotSyncs`, `gatherPotSyncs` with `claimPlanFrom`'s inputs) runs in the background (`claim/syncSearch.ts`) for a shortfall of 10,000 lamports or more — react-query under `lendKeys.claimSyncs`, keyed by the stored state the pick comes from that a price crank leaves alone (not the per-second figures; the realised-rent index left the key in the review below), 15 s fresh, searched again when a claim, deposit or withdrawal of the market lands and every 60 s while shown, at most two at once app-wide, 20 s timeout, a failure counted as no syncs; {g} = min(shortfall, the picked syncs' gain), none under 10,000 lamports, so a shortfall no sync can fund (unpaid rent; a market's only lender) is `claim.label.full` or the disabled "Review claim · 0 SOL" with "Nothing to claim yet"; the note keeps the whole shortfall; while the search runs, the full label if something is claimable now, else disabled and busy with no reason (none flips); the review takes the fresh pick, else searches again, and a simulation that pays 0 lamports is not offered for signing (`TxPlan.holdSign`: "Nothing to claim yet" beside the disabled Sign, `tx-sign-hold`); the Lending page's rows and Claim all on the same offer — Claim all counts, totals and claims exactly the enabled rows, and reads 0 SOL when every enabled row pays only through its syncs (§3.6, §3.8, §4.3, §10.5, §11) | `syncSearch.test.ts` (gain 0, under 10,000, from 10,000, above the shortfall; in flight; a failed search; two at once; the timeout), `LendPanel.test.tsx` (the tab's states, in flight with and without something claimable, a failed search, the review taking the fresh pick and holding a 0 payout), `LenderDashboardPage.test.tsx` (the rows' states, in flight, Claim all's rows, count and total, each claim with its row's pick, the concurrency), `build.test.ts` (the shared search; the pick reused without a second search; the hold), `useTxFlow.test.ts`, `TxReviewModal.test.tsx`; E2E `lend-claim-sync` (the only lender's disabled state first; then `[sync_lender, claim_sol]` paying exactly the review's simulated amount), `mobile-lend`, `screens`, the responsive gate (`lender-dashboard-shortfall` / `lend-claim-shortfall` now disabled; `lender-dashboard-syncs` / `lend-claim-syncs`, a second lender's surplus, at all eight viewports) |
| The expired banner matches the program | `exitState` returned `expired` before checking anything else, so a liquidatable position past its deadline read "… with a {X} expiry fee instead of the penalty." while the program liquidates it (`expire.rs` `UseLiquidate`, SPEC §5.3; the inaccuracy ¶3's amendment fixed). Past the deadline now: no route → `no-route` (`liquidate` and `expire` refuse with `NoRoute`; S3 counts the expiry), armed → `armed` (`sol_settle`'s persisted clock runs on; the program refuses with `UseSolSettle` only while its live buyback check fails, so a keeper may still close an armed position; kept first, as S2's slippage cannot be judged by the client), both legs below → `liquidatable`, else `expired`; before the deadline unchanged; only which existing text shows (the health banner, the Positions page's tooltip; the status filter reads the indexer's status and is unaffected) (§3.8) | `positions.test.tsx` (each boundary: one second before, at and after the deadline; at the line, the live leg only, no CR; no route; armed; the banner's and the tooltip's text) |


*The pass's review (2026-10-11).* Three reviewers (claim, taps, words) read the pass; the fixes
below. Gates on the fixed tree: biome, typecheck, `vitest run` (76 files, 751 passed, 1 opt-in file
skipped), `pnpm build` (no `<style>` tag or `style` attribute in `dist/index.html`); the default E2E
95 passed (69 desktop, 26 phone; the reference screenshots refreshed); the responsive gate 96 passed
at all eight viewports (63–65 states each) — against the pass's capture: 0 document overflow, 0
elements off screen, axe 0, 0 targets under 24 px, 0 text under 12 px, 0 tap covers with 18,631
controls judged (11,190 before: the dialogs and the controls partly inside a clipping box now
count, and the stepped captures count a control once per scroll step — `phone-land` and `phone-xs`,
but by the bare `(orientation: landscape)` query also 16, 14 and 1 states of `laptop-s`, `laptop`
and `desktop`; the next review counts each control once and compares the landscape-phone query
itself); the sideways scrollers
unchanged. The review's constructed cover (an `::after` grown over the dialog's foot at `laptop`)
fails the gate ("tx-cancel under tx-retry (10/25), tx-retry under tx-sign (10/25)").

| Item | Change | Tests |
|---|---|---|
| The search for syncs and the price crank | the search's key held the market's realised-rent index (`acc_r`), which a crank's accrual raises while a vest runs, so every crank searched again, the Claim fell back to its in-flight state (disabled, busy) and the superseded searches queued behind the two slots, never cancelled. The key now holds only what changes the pick and a crank leaves alone (the claimant's lender fields, the pot, the share epoch, the total shares; a vest's release moves `acc` and `acc_r` together, so no lender's surplus moves; the lenders' SOL, moved by sweeps, is not read by a sync's gain); a search runs again every 60 s while shown (rent paid in raises `acc_r` alone); while a new search runs for the same market and owner the last result stays (a placeholder from the cache: `useQueries` gives a new key a new observer, so `keepPreviousData` would not carry it; not fresh, so the review searches again); react-query's `signal` reaches the queue (a dropped query leaves it without running) and the search (it stops before its next indexer page; the 20 s timeout stops it too) (§3.6) | `syncSearch.test.ts` (dropped searches leave the queue, the latest runs first; a running one stops on a drop and on the timeout), `LendPanel.test.tsx` (six `acc`/`acc_r`-only reads: no second search, the button never busy; a pool change searches again with the last label kept) |
| The armed state past the deadline | the record and `exitState`'s comment said `liquidate` and `expire` refuse while the position is armed; they refuse (`UseSolSettle`) only while the live buyback check fails, and `s_armed` is the persisted S1 / S2 clock — a keeper may still close an armed position whose live buyback is allowed again. Reworded; `armed` stays first by choice (S2's live slippage cannot be judged by the client, an S1-only proxy could name an exit the program refuses, and "Settlement in SOL being considered" stays true while the clock runs) (§3.8) | — (wording) |
| The tap-cover check in dialogs | the clip rule walked DOM ancestors, and the review `<dialog>` sits in the DOM inside the trade panel or sheet (overflow hidden / auto) while drawn in the top layer, so at 768 px and wider every dialog control was skipped (157 skipped in a full run). Now the clip box follows the containing-block chain up to the top layer (`:modal`, `:popover-open`), and only the points inside it are judged instead of skipping the control (§10.5) | the responsive gate (the review's constructed `::after` cover over the dialog's foot now fails, as its probe showed) |
| Measured in a layout the device never shows | unrolling made `phone-land` portrait on any page taller than 844 px (its landscape rules off: the sheet's static form foot, the compact intro) and `phone-xs` taller than 700 px (the short-phone rules off). Where the unroll would change those media queries the page is not unrolled: the PNG is a full-page capture at the real size (drawn with the fine pointer: the capture drops the touch emulation) plus `-scroll-N` slices of the device's view, the tap covers are measured at the real size in half-screen scroll steps (the document, then each box that scrolls on its own), axe runs at the real size (§10.5) | the responsive gate |
| The bars' exemption | a bar was exempt by its class alone; now only while it is over the page (fixed, or sticky and stuck away from its place in the flow), so a form foot in the flow is judged; `.modal-head` and `.menu-head`, which never exempted a point, left the list (the focus walk's too; its list has no current case) (§7.6, §10.5) | the responsive gate, E2E `focus`, `mobile-focus` |
| The Short size field at 1,024 px | beside Slippage, the unit and Max, the size box cut the first digit of "250000" (since before the round); in the desktop side panel (from 1,024 px) Slippage now goes under Size. Layout only | the responsive gate (`short-preview` at `laptop-s`, `laptop`, `desktop`) |
| Records | the Claim's two-line condition in §3.6 (when the picked syncs can add 10,000 lamports or more, not "while there is a shortfall"); §7.6's busy submit without a reason; `claim.label.full`'s note (also while the search runs with something claimable now; copy.ts, owner-copy.ts §6, §4.3 regenerated, the 143 strings unchanged); `lend.css`'s comment on the two-line Claim | `copy.test.ts` (§4.3 against the generated block) |

*The fix round's review (2026-10-11).* Two reviewers (search, gate) read the fixes above; the
changes below. Gates on the fixed tree: biome, typecheck, `vitest run` (78 files, 760 passed, 1
opt-in file skipped), `pnpm build` (no `<style>` tag or `style` attribute in `dist/index.html`);
the default E2E 95 passed (69 desktop, 26 phone; the reference screenshots refreshed); the
responsive gate 96 passed at all eight viewports (63–65 states each): 0 document overflow, 0
elements off screen, axe 0, 0 targets under 24 px, 0 text under 12 px, 0 tap covers with 11,470
controls judged — now each control once per state (18,631 before, which counted a control once per
scroll step: laptops and desktop unroll again, and the stepped states count each control once).
The reviewers' two passing probes now fail the gate: the review dialog's "Simulated result" pulled
over the acknowledgement at `laptop` ("disclosure-ack-borrower.liquidation under … «Simulated
result» (25/25)"), and the Close slippage field moved onto Close position at `phone-land` ("close-submit
under close-slippage-input (25/25)"). The stepped Manage sheet's first full run found a real cover
(below).

| Item | Change | Tests |
|---|---|---|
| A timed-out search | the 20 s timeout dropped the syncs a slow indexer's first pages had found and cached "no syncs" as fresh, so the Claim read "Nothing to claim yet" and a click within 15 s built the claim without syncs. `gatherPotSyncs` now reports its pick and whether a failed read or an abort cut it short (`SearchProgress`); the timeout keeps that pick, and a search cut short is shown but never fresh, so the review searches again (§3.6) | `syncSearch.hook.test.tsx` (the timeout keeps 15,000 lamports, not fresh; a cut search not fresh, a whole one fresh), `build.test.ts` (the progress per page, cut by a failed page and by an abort), `syncSearch.test.ts` |
| The indexer down or slow | each background search's indexer page went through the indexer queries' retries (≈ 15 s before an unreachable indexer counted as none), two at a time. The background search asks for each page once (a failure cuts it short, so the review searches again with the retries), and the Lending page runs none while it knows the indexer is down (§3.6, §3.8) | `syncSearch.hook.test.tsx` (`retry: false` for the button's search only), `LenderDashboardPage.test.tsx` (indexer down: no search, the reason at once) |
| The refresh's load | every shown search ran again every 60 s, all of a page's rows together. Now only with nothing claimable now (where it decides whether the Claim opens), every 45–75 s, fixed per market (§3.6) | `syncSearch.hook.test.tsx` (no refresh with something claimable; the market's period) |
| Spent syncs carried over | after the user's own claim the last result (whose syncs that claim ran) stayed while the new search ran: the two-line label for syncs gone. The placeholder now comes only from a result for the same own position (§3.6) | `syncSearch.hook.test.tsx` (the pot moved: carried over; the user's `claimed` moved: in flight) |
| The 0-pay hold's two reads | `holdSign` compared the vault's lamports from a read before and from the simulation, two RPC calls that may land on different slots. It now holds on the rise of the position's `claimed` (cumulative SOL paid, which only the owner's claim or withdrawal moves; `claimPaidFrom`), the vault's delta only when the claim closes the position | `plans.test.ts` (a vault that seems to pay 50,000 lamports while `claimed` stays: held) |
| The dialog's body and the sheets | a dialog's tap covers were measured once with its body at the top, and the sheets captured as the viewport (the Manage sheet, the trade sheet) once, so a cover below the fold passed. A dialog or overlay state is now measured with each box that scrolls in it stepped at the real size, and an open sheet is also sliced there (§10.5) | the responsive gate (the two probes above) |
| The bare orientation query | `(orientation: landscape)` alone stopped laptop and desktop pages taller than wide from unrolling (16, 14 and 1 states); the check now compares the landscape-phone rules' own query (§10.5) | the responsive gate (`laptop-s`, `laptop`, `desktop`: no `-scroll-N` slices) |
| The slices | a slice scrolled 75 % of the viewport, more than shows between the bars at `phone-xs` and `phone-land` (rows fell between slices), and in a sheet scrolled the inert page. A slice now scrolls 75 % of what shows between the bars, and an open sheet's body (§10.5) | the responsive gate (`phone-xs` `market-overview`'s Tier row; the sheets' lower parts pictured) |
| The Manage sheet's head on a landscape phone | the stepped gate found the ✕ under Add margin at `phone-land`: the side sheet's form foot is in the flow there but kept the sticky foot's `z-index: 1`, which a flex item honours without a position, so at the sheet's end the Add margin foot painted over the sticky head's title and ✕ and took its taps. The foot in the flow resets its z-index (as does the trade panel's foot with the wallet chooser open) (§7.6) | `stacking.test.ts`, the responsive gate |

*The verification after it (2026-10-11).* The three earlier probes, re-run on the fixed gate: the
review dialog's Sign pulled over Refresh at `laptop` fails ("tx-retry «Refresh» under tx-sign «Sign
in wallet» (20/25)"), and the Short submit pulled over the landscape trade sheet at `phone-land`
fails ("short-submit-reason … under short-submit (10/25)"); the same submit pulled 70 px over the
side trade panel at `laptop-s` passed once laptops unrolled again — the unrolled page was measured
once, the panel's own scroll at its top and its stuck form foot exempt, so the panel's end was never
judged. The unrolled page, and a page that fits its screen, now step each box that scrolls its own
content, as dialogs and sheets already did (§10.5); the probe fails ("disclosure-borrower.residuals
summary «Known limits…» under short-submit (15/25)"). The fix round's four test files are formatted
as Biome wants (`pnpm --filter web lint`, as CI runs it, failed on them). Gates: biome, typecheck,
`vitest run` (78 files, 760 passed, 1 opt-in file skipped); the responsive gate 96 passed at all
eight viewports (63–65 states each): 0 document overflow, 0 elements off screen, axe 0, 0 targets
under 24 px, 0 text under 12 px, 0 tap covers with 11,520 controls judged (11,470 before; the side
trade panel's lower controls at `laptop-s`, `laptop` and `desktop`).

*CI on the merged head (2026-10-11).* CI's lint (run 38123721574, on `04c5236`) failed on 7 errors
the local gates had passed: under a path with a `.cache` segment, where the work clones live, Biome
2.5.14 does not apply its `react` and `test` domains, so `noArrayIndexKey` and
`useJsxKeyInIterable` never ran there, and the M10-RESP batch had dropped the suppressions of
`DataTable`'s placeholder rows and `Kv`'s rows. `7b2121d`: `biome.json` enables both domains; the
suppressions are back with their reasons (and on `KvLines`, How it works' blocks and `keepIds`'
parts: fixed lists in a fixed order), and the Position panel's Q owed value has its key; no
rendering or text change. CI on `7b2121d`: run 38125079869, green (known-issues M10-CI-LINT).

### M10-RESP changes (v1.3 → v1.4)

The owner's request of 2026-10-09 — "optimize the site for phone and make it look good and
intuitive both on pc and phone" — built as the M10-RESP spec: a foundation (shell, tokens, shared
components, mobile wallet entry, router sheet keys, the phone E2E project and the responsive gate),
then three page groups in parallel (MARKETS, TRADE, LEND), then an integration. The dark terminal,
every `data-testid`, every flow (simulate → review → sign) and the disclosure texts are unchanged.
**Approved as built by the owner on 2026-10-10 (thirteenth round)**, with the page groups' earlier
deviations (the stat-strip order, Review hidden on a venue refusal, TRADE keeping Status under a
single-status filter, LEND's Sync at priority 2, cards from 960 px, no Max chip and Total value at the
indexer's EMA price, the modal wallet menu on a desktop).

| Item | Change | Tests |
|---|---|---|
| Scope (owner, 2026-10-09) | the "laptop-only / ≥ 1,024 px" non-goal and the 1,024 px minimum dropped: 320 px and up (§1, §7.3, §7.7) | `responsive.spec` (gate, eight viewports) |
| Foundation | document scroll, the phone shell (tab bar, More, status strip, search overlay), tokens and the closed breakpoint set, `DataTable` priorities / cards / sort bar, sheets (`useSheetMode`, `?sheet=`), `FormFoot`, `ActionBar`, `ConnectCta`, `Reveal`, the review's D4 fixes, open-in-wallet links (§3.1, §5.4, §7.4, §7.6) | `breakpoints.test.ts`, `responsive.test.tsx`, `deepLinks.test.ts`, `wallet.test.tsx`, `StatusBar.test.ts`, `TxReviewModal.test.tsx`, `router.test.tsx`; E2E `mobile-shell` |
| MARKETS | markets cards and filters toggle, the market page's header / stats / action bar / trade sheet, every raw table a `DataTable`, create market in one column with Review in a form foot (D14), status and security at every width (§3.3–§3.5, §3.9) | `markets/responsive.test.tsx`, `CreateMarketPage.test.tsx`; E2E `mobile-markets` |
| TRADE | the short panel's form foot and reason, position cards and totals, the position page's hero line, Manage sheet and action bar, D8 (only ShortPosition accounts read as positions), toasts with links (§3.7, §3.8) | `ShortPanel.test.tsx`, `positions.test.tsx`, `positionAccount.test.ts`; E2E `mobile-trade` |
| LEND | the lend forms' feet and reasons, withdraw chips and the ≈ line, lender KPIs, Sync cell, row links into the trade sheet, Claim all in the action bar (§3.6, §3.8) | `LendPanel.test.tsx`, `LenderDashboardPage.test.tsx`; E2E `mobile-lend` |
| Integration | the groups' shared requests: priority-3 cells stay hidden on cards; 44 px text fields and action-bar buttons under a coarse pointer; the dialog / sheet close 44 px (a small button's min-height won before); `FormFoot` above the fixed bars in the page; an open sheet's scroll padding; a phone's toast over an open sheet in the sheet head's title strip; `Checkbox id` / `Disclosure ackId` (TRADE's DOM-set id removed); `.target-link` shared (the review's confirmed and explorer links ≥ 24 px); the lender table framed like the positions table; More marked current on /status and /security; the market page's header and banners open the market column so the trade panel starts at the top (its sticky submit clears the status bar on the first screen, §3.4); Manage's preview labels keep their line; the capture opens sheets as a user does, restores touch emulation after full-page shots and runs axe on the unrolled page; `summary.ts <before> <after>` (§7.6, §10.5) | `router.test.tsx`; E2E `responsive` (gate), `screens`, the phone project |
| Fix round (desktop, phone, a11y reviews) | desktop: the stat strip never leaves an empty cell (4 + 3 / one row); the market Positions tab shows borrower, margin and entry from 560 px (age from 960); numeric columns share the spare width (D15's `width: 1%` dropped) with 16 px before each from 1,280 px; no 1 px document scroll (the status bar's line a shadow); symmetric gutters in the trade and Manage panels; "Cancelled" as a wallet cancel's stage; the tier's info mark an SVG; shortened ids never split, the review's numbers take only their width and its dates may break between day and time (the account label keeps the room); Overview in two columns with Fees under both; position facts in column groups; lender row actions one button style, the reason after them; install links as a list; a visible disclosure chevron; the `/` chip gone while the search is in use; one explorer link in a confirmed review. Phone: Done on a confirmed review closes the sheet (`TxDoneContext`); the wallet popup has a title and ✕ and is history-backed; the sort direction names its order; the toast strip has no second ✕ and keeps an error's reason; a toast's 6 s count only in view; a title alone takes a card's first row. Accessibility: focus returns to the opener when a popup unmounts (or to `#main` without one), from the status list on Escape; a card table's header shows while focused; More's `aria-current` (§3.1, §3.4, §5.3, §7.3, §7.6) | `responsive.test.tsx`, `TxReviewModal.test.tsx`, `wallet.test.tsx`, `router.test.tsx`; E2E `keyboard`, `mobile-shell`, `mobile-markets`, `mobile-trade`, `mobile-lend`, `responsive` (a short page does not scroll) |
| Wrap-up (final re-review) | every way out of a confirmed review in a sheet (Done, ✕, Escape) closes the sheet, never one closed before confirmation; the tablet stat strip 4 + 3 (the desktop rule) instead of 3 + 3 + 1; the Overview's two columns share the card's width (no 420 px cap), and so do the Coverage tab's (the same grid); a dialog whose opener is disabled or gone on close (the lend and short submits after a confirmed send, on a desktop) returns focus to an open sheet's title or `#main`, never `<body>` (§3.4, §5.3, §7.6, §8) | `TxReviewModal.test.tsx`, `Modal.test.tsx`; E2E `mobile-lend` (Escape on the confirmed deposit), `lend-deposit` (focus after Done), `responsive` |
| Owner calls, not built (decided 2026-10-10, thirteenth round: O-1 … O-5 built, §13 "Thirteenth round"; O-6 after the audit, off by default — *lifted 2026-10-13, §13 "Mobile wallets"*) | O-1 deposit reorder, O-2 coverage de-duplication, O-3 intro / glossary, O-4 desktop 12 px floor or density toggle, O-5 Claim wording, O-6 Mobile Wallet Adapter | — |

### M10 changes (v1.2 → v1.3)

What M10 changed in the app and this contract (each item names its section; the decisions are the
owner's of 2026-10-02 … 2026-10-08):

| Item | Change | Tests |
|---|---|---|
| FE-15 (WASM facade) | the TypeScript math ports removed; the facade exports the open guards, the pool-selection comparisons, `mintCheck` and `previewWithdrawal`; the withdraw panel shows the lock a withdrawal leaves (§3.5, §3.6, §6.7, §13) | `src/m10-fe15/parity.test.ts`, `previews.test.ts` |
| FE-18 (validator E2E) | the post-wipe bundle on the mock venue, the claim-time `sync_lender`, the venue cashback claims on their own; two more seeded markets and six wallets (§10.2, §13) | E2E `lend-post-wipe`, `lend-claim-sync`, `lend-sweep-cashback` |
| T-MEMO, TMEMO-9 | every Token-2022 withdrawal passes spl-memo v3; the validator runs mainnet's Token-2022 and checks both SPL programs' hashes; FE-9 met on the mainnet build (§3.6, §5.7, §10.2, §10.3) | E2E `lend-withdraw-memo` (the race lands), `lend-cpi-guard` |
| Q3 (security contact) | `/security` (`SecurityPolicyPage`) and RFC 9116 `/.well-known/security.txt` with build, unit, E2E and deploy checks; the status bar links the page (§3.1, §3.2, §9.3, §11) | `securityTxt.test.ts`, `securityPolicy.test.ts`, E2E `frame`, `screens` (`60-security`) |
| SQ-G8 | error 6105 `AuthoritiesNotDistinct` mapped (action `admin`) (§5.9) | `errors.test.ts` |
| Late-depositor shortfall (owner, 2026-10-03; program batch 3 in the web only) | the deposit shows the pool's unpaid rent and this deposit's part of it; `lender.unpaidRent` (§3.6, §4.2, §11; FE-27) | `previews.test.ts`, `LendPanel.test.tsx`, E2E `lend-deposit` |
| Extras denylist hardening (program batch 3) | a `DeniedAccount` set falls back to the next set like `BadExtras` / `BadRemainingAccounts`; error 6003's message names the new cases (§5.1) | `plans.test.ts` |
| PUMP-2026-10-1 (program batch 4) | the PumpSwap reader and venue state read the pool's fee buckets (`protocol_fees` @271, `creator_fees` @279): `payableQuote = vault − buckets` (none above the vault: not executable), `r = min(payable, effective)`, `PoolView.virtualPart = max(VQR + buckets, 0)` for the tier inputs (§6.7, the program's `PoolVenue::virtual_part`) | `readers.test.ts`, `state.test.ts` |
| PUMP-2026-10-2 (owner, 2026-10-03: disclose) | `TxPlan.refineError`; an open's `ProceedsBelowMin` on a pump account its first trade grows gets a specific message (§5.9) | `growth.test.tsx` |
| KI-65 sub-unit conversions (owner, 2026-10-07, seventh round) | the review of the user's own deposit, withdraw, claim, cover or sync shows `lender.subUnitConversion` with the vest's end when its conversion can take a sub-one-share position to 0 shares while the pool's income vests (the worst case, its floor through the records: other lenders' conversions can land first; KI-66 review); `lender.residuals` gains the approved KI-65 paragraph (§3.6, §4.2) | `previews.test.ts`, `plans.test.ts`, `LendPanel.test.tsx` (the claim's review), `disclosures.test.ts` |
| KI66-R4, KI-67 (owner, 2026-10-08, eighth round; program KI-66 full / KI-67) | the claim prepends the unsigned `sync_lender` it needs — current lenders and absent lenders behind a rebase record alike (the pot-only touch and its forward chain) — judged with the WASM `previewTouchPot` on the records from each one's `fwdEpoch` / `epoch` (one read), ≤ 4, read-only records, fitted to 1,232 bytes, a failing one dropped after a simulation; `lender.potSyncs`, `lender.potSyncsDeferred`, `lender.claimCloses`; a closed position's row and claim; the previews map `fwd*`; a chained owner's transactions carry ≤ 7 records (§3.6, §4.2, §10.2) | `quote.test.ts`, `plans.test.ts`, `build.test.ts`, `previews.test.ts` (vectors: `touchPot`, `claimOutcome` against `pool_model`), `LendPanel.test.tsx`, E2E `lend-claim-pot-chain`, `lend-post-wipe` |
| FE-30, 3007 (owner, 2026-10-08, ninth round) | the claim's pot-sync candidates: the indexer's lenders behind the share epoch first (`epoch=behind`), then the current ones, 50 a page, paged further while the best ≤ 4 do not cover the shortfall, ≤ 4 pages, every row read whatever its epoch, ranked by `previewTouchPot`'s gain (`gatherPotSyncs`; the filter and the bound, the builder's choices, confirmed by the owner on 2026-10-09, §13); the 4-sync cap, the 1,232-byte trim, the simulation drop and the claim without the indexer unchanged; short.win's 3007 on the `lender` account rebuilds like 3012; the four lender texts approved (§3.6, §4.2, §5.9, §13) | `build.test.ts` (behind first past 60 current lenders, with and without the indexer's filter; an indexer behind and ahead of the RPC's share epoch; an oversized page; paging to the bound; a failed page), `quote.test.ts`, `errors.test.ts`; indexer `api_contract`, `api_replayed` |
| Open items | FE-9 met; FE-15 closed; FE-18 partly closed (composite pump / PumpSwap bundle M11); FE-22 → M11; FE-24 → M11 deployment (now also security.txt); FE-21 and FE-23 still the owner's; new FE-27 … FE-29, and FE-30 with KI66-R4 (§13; closed in the ninth round) | — |

### Review fixes (v1.1 → v1.2, the M9 security and UX review)

Every finding was reproduced first (the review's reproducers; they now fail against the fixed
tree) and has a regression test (`src/m9-review/*.test.ts(x)`, `src/features/markets/m9-review.test.tsx`,
the E2E specs named):

| Finding | Fix | Regression |
|---|---|---|
| M9-SEC-1 sending-only wallets unchecked; "expired" from the app's blockhash | post hoc `checkLanded`; `unknown-outcome` instead of `expired`; menu and review labels (§5.4) | `tx.test.ts`, `render.test.tsx`, `wallet.test.tsx` |
| M9-SEC-2 positions priced on the indexer's market | the chain's mint of the position's own market; mismatch flagged (§3.8) | `positions.test.tsx` |
| M9-SEC-3 market header identity and decimals from the indexer | route mint, chain decimals / venue / tier / flags / creator; a detail for another mint ignored (§9.5) | `markets/m9-review.test.tsx` |
| M9-UX-1 Retry re-simulated a stale plan; "the app adds it" never kept | `TxPlan.rebuild`, `retryMode`, accurate copy (§5.3, §5.9) | `ux.test.tsx`, `render.test.tsx`, E2E `lend-withdraw-memo` race |
| M9-UX-2 lender residuals missing | three `lender.residuals` paragraphs (§4.2) | `ux.test.tsx`, `disclosures.test.ts` |
| M9-SEC-4 changed blockhash unchecked; endless confirming; FE-2 fee not shown | lifetime in `assertSameIntent`, `confirmationLifetime`, hard timeout, `tx-final-fee` (§5.5, §5.8) | `tx.test.ts`, E2E `tx-errors` |
| M9-SEC-5 untrusted symbols in titles and labels | normalisation at the API boundary (and in the indexer), `<bdi>`, the mint next to every symbol, duplicate marks (§9.5) | `positions.test.tsx`, indexer `refresher` test |
| M9-SEC-6 frameable on a header-less host; public source maps | runtime frame guard, preview headers, deploy check, hidden maps (§2.2, §9.3) | `render.test.tsx`, E2E `frame.spec.ts` |
| M9-SEC-7 allowlist per program | per (program, instruction) (§5.8) | `tx.test.ts` |
| M9-SEC-8 same-name wallets indistinguishable; signer not shown | identity keys, duplicate badge, silent reconnect undone, `tx-wallet` (§5.4) | `wallet.test.tsx`, `render.test.tsx`, E2E `tx-errors` |
| M9-UX-3 governable numbers hard-coded; `tx.simulation` missing | placeholders from config / locked terms; `tx.simulation` in every review (§4.1, §4.2) | `positions.test.tsx`, `render.test.tsx`, `disclosures.test.ts` |
| M9-UX-4 "Liquidatable" on the spot leg alone / at the line | `riskLevel` on both legs, strict, one function with the exit state (§3.8) | `positions.test.tsx`, `components.test.tsx` |
| M9-UX-5 lender previews on the device clock | `Clock` in the lend read; `chainNowSeconds` (§6.5) | `ux.test.tsx`, `chain.test.ts` |
| M9-UX-6 stale data silent in the money views | `slot-age-badge`, Submit blocked when stale, freshness badges (§6.6) | `render.test.tsx` |
| M9-UX-7 RPC errors read as "you rejected" | only the wallet call's errors (§5.4, §5.9) | `tx.test.ts` |
| M9-UX-8 prices without a visible unit | `Price` shows SOL; "Buyback cost (SOL)" | `render.test.tsx` |

### Integration changes (v1.0 → v1.1)

The tracks' shared-file requests, applied by the integrator, and the integration's own fixes:

1. `src/tx/errors.ts`: the failing program is the **first** "Program … failed" log line (Token-2022
   42 / 36 inside short.win's CPI map to `token`); `IntegrityError` maps to `integrity` (§5.9).
2. `src/api`: `MarketsParams.cursor`, passed by `indexerQueries.markets`; MARKETS' paging uses it.
3. `src/rpc/queries.ts`: the shared chain hooks (from MARKETS); `src/rpc/pda.ts`: the event
   authority (three copies removed); the pump / PumpSwap discriminators and event authorities of
   the sweep bundle live once in `venues/constants.ts`.
4. `src/shell/freshness.tsx`: `useNow`, `useIndexerFreshness`, `FreshnessBadge` (from MARKETS),
   used by the status bar; LEND's clock on `useNow`.
5. `BREAK_EVEN_DAYS` in `src/config/disclosures.ts` (MARKETS and LEND had a copy each).
6. `src/venues/types.ts`: `ExtrasSelection`, `ExtrasChoice`, `TradeContext.extras` and
   `extrasCandidates` (SHORT returned a subtype).
7. `DataTable`: `rowAttrs` and control-aware row activation; SHORT's own `PositionsTable` removed.
8. `TxReviewModal`: the plan's instructions (`src/tx/describe.ts`, `tx-instruction-<i>`).
9. Toasts bottom-left; route-level code splitting (`lazyRouteComponent`).
10. The market page's Positions tab: CR now and risk (SHORT's `useMarketPositionsLive`).
11. The visual pass (§7.3, §10.2 `screens`): stat strip, compact size cap, sticky trade panel,
    table padding at 1,024 px, `.case-keep`, the token label without a repeated mint, "No reading
    yet" for an uncranked oracle, SPEC §5.3's sentence once, the Positions tab's note padding.
12. Lint: 26 `useJsxKeyInIterable` errors (MARKETS, LEND; the rule only fires in a git checkout).
13. E2E: the review's instruction list asserted in `lend-cpi-guard`; `screens.spec.ts`; a reused
    build is checked against its sources; `fund.ts`, `start.ts --env`, `dev:e2e`; CI runs every
    spec.
