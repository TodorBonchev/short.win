# Candidate provider trust and settings worksheet

The owner authorized signed Jupiter-derived feeds and concrete review before
deployment. This worksheet records provider screening, historical candidate
identities and exact open configuration work. It approves no provider, queue, authority, quorum or financial
setting. Mainnet remains postponed; new quote-risk markets remain disabled.

## Screening result: Switchboard excluded from deployment selection

Switchboard's own [September18 announcement](https://switchboardxyz.medium.com/important-announcement-54dd6dbe9379)
states that it is shutting down and links the
[Foundation notice](https://x.com/SwitchboardFDN/status/2101088966640611478).
Retain the SDK/identity work as historical candidate research; do not continue
qualifying Switchboard for deployment on the basis of accessible code/accounts.
This does not establish that every independent node stopped or that the SDK
findings caused the shutdown. No official resumption evidence has been established.
The detailed September25 support deadline remains attributed to secondary coverage;
it was not independently read from the inaccessible X article here.

The selected signed-feed architecture remains provider-neutral. A maintained
provider must prove the fixed-q reporting contract. The
[prepared RedStone capability request](PROVIDER_CAPABILITY_REQUEST.md) is unsent;
its public Solana/custom-integration documentation does not establish our exact
report capability. A protocol-operated attester network is another trust model,
requiring explicit owner review. Neither alternative is silently selected.

## Observed Switchboard identities retained as research

Public read-only finalized RPC observations on 2026-10-27 used context slots
462019610 and462019879,09:27:47.860205Z–09:28:45.000177Z. These are candidate
observations, not fixture promotion, source-to-ELF verification or a live feed proof.

| Role | Observed address / identity |
|---|---|
| SDK 0.13.0 mainnet on-demand program | SBondMDrcV3K4kxZR1HNVT7osZxAHVHgYXL5Ze1oMUv |
| On-demand ProgramData | EW3AZYxoQrYWBvTmrCmiDpm6oBCNpAqN2kekDjuYFmsB |
| On-demand deploy slot / trimmed ELF SHA256 | 420822344 / ab59c70a8a17a224342eb23ee26985acc088fde26f80d40b6b07ddd1223503df |
| On-demand upgrade authority | DREcTwxxuehtUVgnGwDpTcKT9zTK5uDmdLbnRY7cijEx |
| SDK 0.13.0 default mainnet queue | A43DyUGA7s8eXPxqEjJY6EBu1KKbNgfxF8h17VAHn13w |
| Queue owner / authority | on-demand program above / DREcTwxxuehtUVgnGwDpTcKT9zTK5uDmdLbnRY7cijEx |
| SDK 0.13.0 quote storage program | orac1eFjzWL5R3RbbdMV68K9H6TaCVVcL6LjvQQWAbz |
| Quote ProgramData | 61qUnCDSzWvuhXNLK4GfECJTEr4WzZYwFfn8oeW5y833 |
| Quote deploy slot / trimmed ELF SHA256 | 438467619 / f0eb4451b84c47b712be2bbd855f86d24f90c8f2ebcd4fb89709903e92a5d59c |
| Quote upgrade authority | 31Sof5r1xi7dfcaz4x9Kuwm8J9ueAdDduMcme59sP8gc |

The observed queue has 6280bytes, matching the pinned SDK layout. Its30 signing-key
slots contain 12 distinct nonzero keys. This does not prove 12 active, independent
operators, available quorum or source independence. The same address is named as
queue authority and on-demand upgrade authority; its governance/control mechanism
has not been qualified. The quote program has a different named upgrade authority.
Program hashes identify bytes; they do not establish reviewed source provenance.

The quote storage program is a separate authority/trust surface. Reading its owned
canonical account would trust its update verification; it cannot silently bypass
the [candidate verifier's qualification blockers](../../audit/jupiter-oracle-sdk-review-2026-10-27.md).
Its actual managed-update/consumer path still needs a source/deployment review and SBF proof.

## Required concrete activation packet

| Setting | Recommended constraint / evidence to present before approval |
|---|---|
| Provider and transport | Identify repaired verifier or separately reviewed safe consumer, exact source/ELF pins and regression/SBF receipts. Current SDK verifier stays quarantined. |
| Queue, authorities and rotation | Pin exact reviewed program/queue/owner and control mechanisms; document who can rotate signing/enclave keys or upgrade code. Review changes before rearming. |
| Quorum and independence | Count distinct authorized signing keys; satisfy configured and signed minimums. Demonstrate active availability and operator/source independence. The7/5 design example is not a selected setting. |
| Feed IDs and job policy | Supply immutable canonical jobs, strict schema, exact mint/program/decimals/raw q mapping, source/search/fee/taker policy and feed IDs. Credentials-only request overrides. No generic mutable-mint/amount feed. |
| Amount and economic references | Implement exact-q candidate search plus separately reviewed lower/central reference methodology. Demonstrate actual net output and coverage of every outstanding quantity. No finite forced-action band from an upper-only quote. |
| Source freshness and coherent epochs | Measure actual source observation age, not signature/blockhash-fetch age. Specify slot/time limits, vector skew, complete-epoch cadence, historical warmup, missed-update behavior and anti-backfill rules. Numeric values await measured valid updates. |
| Caps, tiers, fee rise and settlements | Present quote-native capacity/premium policy, booked-notional definition and locked opening terms; retain the 25 SOL baseline plus inventory/utilization/mint protections. Specify all-in S2 replacement, S3/challenge/recovery and Pump-graduation continuity. These remain unapproved financial settings. |
| Circuit breaker and exits | Treat stale, invalid, unknown, disputed or unreviewed-upgrade evidence as unavailable. Disable opens/forced quote decisions; retain adding margin and explicitly implemented safe token repayment/borrower-authorized bounded exits. Test the outage paths. |
| Execution resources | Present successful pinned-Jupiter SBF routes, adversarial rollback tests and measured CU/account/data/rent bounds. Current header/format tests are not routed execution. |
| Fees and operating budget | Obtain actual quote/update transaction costs, API rate/cost policy, chosen-market quantity/feed count, cadence and monthly budget. Demonstrate funded independent keeper monitoring/top-ups. No paid feed or funding evidence exists yet. |
| Release identity | Full regression/CI/independent integration review, exact reproducible final bytes/fixtures, fresh qualifying fuzz and actual operator artifacts. No borrowed 1.0.3 duration or final tag. |

This packet can be reviewed once its pending evidence is concrete. The architecture
choice alone accepts neither arbitrary oracle settings nor unauthenticated API data.
The [producer contract](QUOTE_PRODUCER_CONTRACT.md) and
[implementation milestones](IMPLEMENTATION_PLAN.md) define the remaining engineering work.
