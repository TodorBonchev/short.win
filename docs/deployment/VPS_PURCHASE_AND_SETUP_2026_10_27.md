# Initial mainnet infrastructure purchase and setup proposal

Prepared 2026-10-27 (Europe/Kyiv). The owner requested a purchase/setup proposal;
the owner subsequently approved the initial 245.20 USD/month baseline budget
(before tax, overages and SOL fees). No service has been ordered, paid for or
provisioned. DigitalOcean sign-in/account verification is pending in the opened
in-app browser; the owner was asked to complete it directly. This is for the selected
in-house price worker, keeper, indexer and private PostgreSQL stack. Software
integration and final qualification remain open. The [operations runbook](MAINNET_OPERATIONS.md)
remains the commissioning contract; neither this bill nor a running VM proves readiness.

## Proposed order

| Item | Proposed selection | Monthly USD baseline |
|---|---|---:|
| Compute | DigitalOcean Frankfurt `fra1`, regular General Purpose, 4 dedicated vCPU / 16 GiB, Ubuntu 24.04 LTS x86-64, 50 GiB boot disk | 126.00 |
| Database storage | 150 GiB attached block volume in the same region, giving 200 GiB nominal total with boot disk | 15.00 |
| Boot-image recovery | Weekly Droplet backups, 20% of compute price | 25.20 |
| Off-host database/evidence backup | Private Spaces bucket, proposed separate region, base 250 GiB storage / 1 TiB outbound allowance | 5.00 |
| Jupiter | Developer, 10 requests/sec, 25M monthly credits | 25.00 |
| Solana RPC | Helius Developer, 50 RPC requests/sec, 5 sends/sec, 10M monthly credits | 49.00 |
| **Baseline total** | Before tax, overages and network transaction fees | **245.20** |

The plan specifications/prices were checked against primary sources:
[Droplets](https://www.digitalocean.com/pricing/droplets),
[block volumes](https://docs.digitalocean.com/products/volumes/details/pricing/),
[backups](https://www.digitalocean.com/pricing/backups),
[Spaces](https://www.digitalocean.com/pricing/spaces-object-storage),
[Jupiter](https://developers.jup.ag/pricing), and
[Helius](https://www.helius.dev/pricing).
Confirm availability, exact checkout totals and monthly billing before purchase.
This selects the regular monthly-capped compute plan, not the distinct v5 hourly
product. No commitment to a larger plan or API overage is included.

Boot backups **do not include attached volumes**. The database needs its own
consistent encrypted backup and a verified restore; a Droplet backup alone cannot
recover this proposed database placement. See the provider's
[backup limitations](https://docs.digitalocean.com/products/backups/details/limits/).
Spaces is off-host storage in this proposal, not an independent provider or high
availability replica. Keep a separately controlled recovery copy and recovery keys.

Jupiter Developer is a starting qualification budget. Reserve request capacity for
exits; do not fill all 10 RPS with periodic prices. Measure endpoint credit costs,
monthly use, 429s and worst-case bursts. If measured work needs Jupiter Launch
(100 USD/month, 50 RPS, 100M credits), the total becomes 320.20 USD/month before
other changes. No upgrade is ordered by this proposal. RPC request limits likewise
do not guarantee that 10M credits cover full-time backfills/simulations. Measure
actual call mix, including reads, simulations, retries and confirmations.

The total excludes domain/website costs, any paid independent alert/fallback RPC
service, backup/egress overages and SOL transaction/deployment/rent budgets.
The current proposed two-feed, two-second publication schedule was separately
measured in the [publication-cost record](../audit/probes/price-publication-cost-2026-10-27/README.md):
continuous 30-day base signature fees alone would be **12.96 SOL** with separate
updates, or **6.48 SOL** with a qualified two-report batch. The no-ALT batch with
normal compute-budget instructions is 1,278 bytes and exceeds the 1,232-byte v0
limit. A synthetic ALT reduces it to 1,188 bytes, proving only structural fit.
No actual ALT, batching/landing cadence, priority-fee budget or additional SOL
spending is approved by the hosting/API budget. Review these costs, the publication
duty cycle and payer funding before selecting production timing. Determine
those from final artifacts, measured operation and the owner's actual accounts;
do not substitute a guessed SOL balance or spot fiat conversion for funding evidence.

## Setup order after purchase approval

1. Create the VPS under the owner's account with MFA and an SSH public key. Record
   its public identity, exact plan, region, OS and volume identifier. Use a regular
   administrative account with sudo and tested recovery access; apply SSH changes
   only after the second session works. No password or private key goes into Git/chat.
2. Mount the explicitly identified data volume by stable ID. Do not format a device
   until its identity and empty/new status are verified. Put database data/backups
   on the intended filesystem and verify free space, permissions and reboot mounts.
3. Build reviewed pinned artifacts on CI/build infrastructure, not on the serving
   keeper host. Verify source, binaries, IDL, images and SHA256 receipts before install.
4. Run private PostgreSQL, the existing indexer roles, keeper and the separately
   supervised price worker. The price worker/update instruction is still under
   development; no fictional CLI command or working live reporter is implied here.
   Public exposure should be limited to required HTTPS; keep database/metrics private
   and restrict administrative SSH. Separate scoped reporter/execution credentials;
   keep governance, emergency and deployment signers off this host.
5. Configure bounded API/RPC concurrency, response sizes, deadlines and retention.
   Reserve execution capacity, cap background backfill and test restart/backoff.
   Keep covered markets explicit; a small-host estimate is not unlimited coverage.
6. Configure encrypted database/evidence backups with separate credentials, retention
   and an alert for backup failure. Restore into a clean isolated database and compare
   the replay checkpoint/data before counting this gate complete.
7. Prove alerts are delivered outside the host: stale reports/history, keeper coverage,
   failed exits/default candidates, payer low balance/top-up failure, API/RPC outage,
   process failure, disk growth and backup failure. An installed monitor or successful
   code test does not establish delivery. Use the eventual agreed channels; none is
   contacted or subscribed by this proposal.
8. Run combined-load and failure/recovery drills using final artifacts and declared
   market/request load. Record CPU/RSS/swap, database growth/IO, keeper latency,
   queue age, report freshness, p95/p99 response times, 429s and actual monthly credits.
   Fix any operating-bound failure before enabling a market. One host remains a
   single administrative/availability failure domain and no independent oracle quorum.
9. Supply actual governance/emergency/reporter public identities, covered markets,
   funded payers/top-up policy and exact reviewed risk settings. Prepare/simulate the
   final deployment/configuration packet and obtain the final concrete owner review.
   Tagging/deployment/financial transactions are separate from this setup proposal.

## Evidence still required

All resource/account/public-key identities, purchase receipts, tested backup restore,
delivered alerts, funded payer balances, operating drills and final price-worker /
keeper / indexer artifact identities are pending. The owner chose to receive this
proposal rather than provide existing services. Prior devnet receipts and the old
program's fuzz duration do not fill these mainnet or redesign gates.
