# Capped mainnet release preparation

The proposed program is **1.1.1**, ID
`BCPiRMTMoLNY5EHm2RYxoxJChgyrttkFX2Qe2Rnbe3jh`. The repository prepares a
public review packet; it does not sign, deploy, initialize or activate it.
`execute` and `readyForActivation` stay false, including after public roles are assigned.

## Exact artifact and preparation

Select the preserved ELF with SHA-256
`86147af04bda2c1fbab92ad670dc8d7cc272162724787dac666344bf24b547f2`.
It is the exact duration-tested artifact and the primary program in the
[dated Jupiter review](../config/jupiter-reviewed-deployment.json). The separate
root rebuild has a different full ELF hash; identical executable instructions do
not transfer the duration result to that whole file. The preparer refuses a
replacement ELF rather than silently changing the selected artifact.

From a clean reviewed checkout, use the actual preserved artifact path:

```sh
python3 config/verify-production-proposal.py
python3 -m unittest discover -s config -p 'test_*.py' -v
mkdir -p target
python3 scripts/prepare-mainnet-release.py \
  --program /absolute/path/to/preserved/shortwin_simple.so \
  --source-ref "$(git rev-parse HEAD)" \
  --output target/mainnet-release
```

The output directory must be new. `release.json` binds the exact copied ELF,
public roles, primary policy/service/operator proposals, retained router review,
offline validation result and active source hashes. No role keyfile is read or
included. The preparer verifies the retained Jupiter/source/model bindings using
the real proposal verifier. It does not independently verify the separately
retained 24-hour campaign receipt; retain and check that receipt and its artifact
hashes during release review. The successful sequential campaign and saved-route
qualification do not establish arbitrary multi-loan interleaving, live service
coverage or an external audit.

After assigning only public mainnet identities, this additional preparation gate
must pass:

```sh
python3 config/verify-production-proposal.py --require-assigned-roles
```

`prepare-mainnet-release.py --require-assigned-roles` applies the same gate when
creating a fresh packet. It is expected to fail while any public identity is null.
Funding, alerts and commissioning remain separate gates even after it passes.
The authoritative [release specification](../config/mainnet-release.proposed.json)
keeps all activation gates pending and opens paused; setup-rent and market-fee
amounts require a later concrete initialization review.

## Selected bounds and residuals

| Bound | Selected initial policy |
| --- | --- |
| Booked notional per position | 0.1–1 SOL |
| Booked notional per market | 5 SOL |
| Wallet concentration | 20% of market cap: 1 SOL per wallet per market |
| Utilization at opening | 60% |
| Term and grace | 24 hours each |
| Initial service coverage | One exact `(market, policy, quantity)` slot |
| Operator payer budgets | 25,000 lamports per transaction; 100,000 per pass; 1,000,000 daily publication fee reservation |
| New references / keeper route setup | Four lifetime allocations per preserved operator database / zero native setup allowance |

Deposits, permissionless market count and aggregate protocol exposure are
**uncapped**. Wallet splitting bypasses wallet concentration. Booked opening
notional does not cap future replacement cost, and later withdrawals can change
utilization. This release does not promise a global loss limit or whole-token
recovery. At three times opening bid and a 5% spread, the retained 0.1 SOL example
needs 0.315 SOL while its forced-buy budget is 0.248 SOL; default distributes
0.24865 SOL to lenders, 78.93% of that tested replacement cost. See the full
[policy economics](../config/README.md).

Any positive margin addition clears the current pre-maturity liquidation arm
under the selected policy, so repeated small additions can delay confirmation;
fixed maturity/default deadlines do not move. Withdrawals need available tokens;
default can lose token principal and distribute SOL compensation. The trusted
reporter and governance remain authorities.

The operator scans all program accounts and fails closed above the configured
2,048-account ceiling (hard maximum 10,000), before default dispatch. Uncapped
market/receipt/reference growth therefore requires account-count headroom alerts
and a reviewed capacity plan. Other keepers can invoke route-independent default,
but that does not make this operator's scan unlimited. One exact-q service slot
is distinct from reference-rent allocation allowance and program account capacity.

## Ordered release gates

1. Freeze the reviewed commit and selected ELF. Recheck retained qualification and
   campaign hashes; resolve the conditional upstream loader-extension availability
   lead against the actual loader policy. Keep the reviewed Jupiter pin strict.
2. Assign distinct controlled roles, confirm private custody and backups outside
   Git, verify canonical mainnet genesis and actual loader/upgrade authority, and
   identify the authorized initializer. Never reuse deterministic demo identities.
3. Re-observe canonical Jupiter bytes/slot/allocation before the launch packet.
   Changed upstream bytes or reviewed source require renewed compatibility review.
4. Obtain current deployment/buffer/account rent and fee quotes, identify the
   deployment payer, and retain actual deployment and operating funding receipts.
   Review explicit initialization rent/creation-fee ceilings and custody transfer.
5. Commission the free provider/RPC account, one exact quantity, non-overlapping
   bounded passes, current exact clock handling, publication budgets and preserved
   history/operator/identity/lock stores. Measure worst-case route/history timing.
6. Deliver and acknowledge stale-feed, heartbeat, queue-full, low-balance,
   unresolved-signature and account-count-headroom alerts. Rehearse consistent
   backup/restore and refusal after missing/mismatched storage identities.
7. Review the exact paused initialization/simulation packet and later canary
   lifecycle plan with all authorities, fees and limits frozen. Perform any
   deployment or financial transaction only after explicit approval of that packet.
8. Read back deployed ELF, loader authority, Config, immutable policy, router pin
   and role/genesis bindings. Complete the approved limited canary lifecycle and
   confirm operator/reference coverage before separately reviewing opening activation.

## Stop and recovery

On artifact/pin/identity mismatch, stale service, missing alerts, unresolved
submission or insufficient capacity/funding, keep new openings paused and stop
admission. Preserve pending signatures, reservations, raw evidence and the complete
storage identity set. Pause prices only through the reviewed emergency/governance
process; direct repayment and deterministic default remain price-independent.
Reconcile unknown signatures before retrying. Do not reset stores or recreate
identities to clear a barrier, fee allowance or reference allocation limit.

An on-chain upgrade cannot undo prior transactions. Prepare any rollback as a new
reviewed upgrade with an explicitly selected artifact, simulation and authority;
keep openings paused while recovering. Do not close a program or move pooled
assets as an automatic rollback action. A funded deployment, recovery or unpause
requires a concrete separately approved transaction packet.
