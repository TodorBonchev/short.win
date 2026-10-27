# Owner decision: postpone mainnet for Jupiter redesign — 2026-10-27

The owner explicitly answered the release-scope question:

> Postpone the first mainnet launch and redesign for broad Jupiter support now

This supersedes the immediate first-mainnet timing of the earlier October 27
[release decision](owner-mainnet-decision-1.0.3-2026-10-27.md). The desired design
retains direct Pump bonding-curve support and broadens other execution through
Jupiter, including eligible assets routed through supported DEXes. The existing
per-market borrowing cap and mint/lender protections remain the baseline; no
new global cap or universal token/exit guarantee is asserted.

The owner then asked whether Jupiter quotes can supply margin/liquidation
pricing. That is a question, not acceptance of a new oracle trust model. The
[design proposal](../design/JUPITER_REDESIGN.md) separates execution quotes from
authenticated risk observations and records the outstanding choice. No keeper-
provided JSON is authorized to become an unchecked on-chain risk price.

The source branch is provisionally named `codex/jupiter-redesign-1.1`; this is a
development name, not an approved final version or tag. The current 1.0.3 work
remains preserved as baseline evidence. Its old completed fuzz duration cannot
qualify the new program, and the unstarted Token-2022-only long campaign remains
on hold while the new program scope is determined. Continue retaining actual
CI/fork/benchmark results without calling the new design qualified.

The intended initial-mainnet sequence remains **before an external audit and
before a funded bounty**, funded later from earned protocol fees as previously
recorded. This timing change introduces no audit-first requirement, changes no
fee allocation and claims no external-firm certification. Final runtime tests,
reproducible bytes, independent review, fresh qualifying fuzz, current upstream
compatibility and real funded/monitored operation are still required.

No program rewrite, source tag, deployment, financial transaction, oracle feed
creation, new signer authority or long fuzz run is claimed by this record.
