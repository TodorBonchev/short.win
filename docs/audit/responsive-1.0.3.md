# Draft 1.0.3 responsive browser gate

The opt-in `@responsive` suite passed **96/96** on the final web source at
`9b910602ed076a6d4359c496960b3ffc0e7c6af8` (2026-10-18, 21.8 minutes).
It captured 514 page states across eight viewports. Every viewport reported zero capture
errors, document or main overflow, offscreen controls, controls below 24 px, tap covers
and axe violations. The raw local `metrics.json` has SHA-256
`a4301c3ef14f4f605756a771021fca8d6e0645c1249ca10eb6f51a34ca2dd2a0`;
the ignored local HTML report and metrics are preserved under
`apps/web/.e2e/archive/170be5a-responsive-green/`; screenshots remain under
`apps/web/.e2e/responsive/`.

| Viewport | Captures | Document/main overflow | Offscreen | Below 24 px | Tap covers | Axe violations |
|---|---:|---:|---:|---:|---:|---:|
| phone-xs | 65 | 0 | 0 | 0 | 0 | 0 |
| phone-s | 65 | 0 | 0 | 0 | 0 | 0 |
| phone | 65 | 0 | 0 | 0 | 0 | 0 |
| phone-land | 65 | 0 | 0 | 0 | 0 | 0 |
| tablet | 65 | 0 | 0 | 0 | 0 | 0 |
| laptop-s | 63 | 0 | 0 | 0 | 0 | 0 |
| laptop | 63 | 0 | 0 | 0 | 0 | 0 |
| desktop | 63 | 0 | 0 | 0 | 0 | 0 |

The preceding run failed two cases: a stale-price mobile trade sheet covered its Close
control, and a long RPC slot widened the 768 px tablet shell. The final suite waits for
the stale-price state, asserts one Connect wallet action and an unobstructed Close tap,
and stresses the tablet strip with four-digit and mainnet-sized RPC slots. It also checks
keyboard access to the last status link. Both cases passed in the complete rerun.

The suite is opt-in by design. To reproduce the gate with the repository's browser test
fixture, use `SHORTWIN_E2E_RESPONSIVE=1 SHORTWIN_RESPONSIVE_GATE=1 pnpm --filter web e2e --grep @responsive`
and inspect `apps/web/.e2e/responsive/metrics.json` with
`node apps/web/e2e/responsive/summary.ts apps/web/.e2e/responsive`.
