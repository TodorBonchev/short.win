# Devnet 1.0.3 deployment and market readiness (2026-10-22)

**The program-only devnet deployment is complete; full devnet market readiness is not established.** The initial approved upload stopped after creating a funded buffer and writing 14 chunks. The owner separately approved a same-buffer resume, which finalized at slot **513919880** in [transaction `2twpM5s…GhVm4j`](https://explorer.solana.com/tx/2twpM5sUnQeAJLVTeKo9gHeDPfpU3iNitPZNg9UVm7At6zMyQCn6DATh9azQRk9rd1DzCi315W8HKZLbDpGhVm4j?cluster=devnet). Program `5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK` is executable with the exact canonical ELF hash `d6f4e092dd15639c502535d68bcdb6c428c423b66d49d08f4f89e9409af538bc`, temporary upgrade authority `3JfrUHa1RihL7rgJDWSmdV9DNGXkwKAUAcRDBh75cS14`, and no GlobalConfig, VenueGuard or OracleCache yet. The [deployment receipt](DEVNET_1_0_3_DEPLOYMENT_2026_10_22.md) records the on-chain checks. The owner separately authorized fresh devnet CLI wallets outside the repository; their creation did not change the compiled program address.

The inspected integrated source is `b31c02a091008da18c39d7836c8e0e9d6d018973`. Existing technical release evidence is attributed in [the October 22 release record](../audit/release-checks-1.0.3-2026-10-22.md); its dated Jupiter drift was later [dispositioned](../audit/probes/release-wrap-2026-10-22/jupiter-mainnet-pin-2026-10-22.md). The subsequent [local service staging](devnet-local-service-staging-2026-10-22.md) and [fresh market feasibility review](devnet-market-feasibility-2026-10-22.md) record the current operating progress and unresolved devnet governance/Pump compatibility gates. A devnet rehearsal or prerelease tag does not certify a stable release.

## Public devnet observations

The **predeployment dependency survey** used bounded read-only requests to `https://api.devnet.solana.com`, with confirmed commitment, no credential-bearing endpoint and no arbitrary wallet lookup. Genesis was `EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG`; the account metadata batch was at slot **513,721,817**, API `4.4.0-beta.1`. All public addresses were validated, venue ProgramData derivations were checked against source constants, and executable Program pointers matched the queried ProgramData. Only dependency metadata prefixes were fetched; their executable hashes and signed compatibility were not established. The short.win program's complete ELF was separately verified after deployment.

| Dependency | Actual devnet observation | Gate |
|---|---|---|
| short.win `5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK` | Program, ProgramData and initial config/guard/oracle PDAs absent **at survey slot 513721817** | Program and ProgramData deployed at slot 513919880; config/guard/oracle still absent. Deployment needed the matching program signer, separately from the payer wallet. |
| Pump | Expected source ID executable; ProgramData 1,996,805 B, deploy slot 510,657,843 | A candidate real devnet venue; its executable and selected market still need scoped compatibility evidence. |
| PumpSwap | Expected source ID executable; ProgramData 1,234,749 B, deploy slot 508,659,436 | Same conditional scope as Pump. |
| Pump-fees | Expected source ID executable; ProgramData 920,053 B, deploy slot 508,659,783 | Expected-owner global and fee accounts exist; executable/interface compatibility remains unverified. |
| Canonical Raydium v4 / CPMM IDs | System-owned, nonexecutable accounts | Current release cannot trade against them on devnet. |
| Official devnet Raydium IDs | `DRaya7Kj3aMWQSy19kSjvmuwq9docCHofyP9kanQGaav` and `DRaycpLY18LhpbydsBWbVJtxpNv9oXPgjRSfpF2bWpYb` are executable | Using these requires a separately reviewed code/client/keeper/browser build profile. No constants were switched. |
| Canonical Jupiter ID | System-owned, nonexecutable account | No executable at that identity here; do not activate Jupiter scope. |
| Squads v4 `SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf` | Executable, but ProgramData is mutable, slot 446,638,068, authority `HM5y4mz3Bt9JY9mr1hkyhnvqxSH4H2u2451j7Hc2dtvK` | Current `shortwin-gov verify` must fail its immutability requirement. Do not weaken the check or describe the production governance topology as verified on devnet. |
| Squads ProgramConfig | Expected owner/discriminator; creation fee 0; treasury `HM5y4mz3Bt9JY9mr1hkyhnvqxSH4H2u2451j7Hc2dtvK` | Creation metadata exists; the mutable-program trust difference remains. |
| SPL Token, Token-2022, ATA, Memo and Pyth receiver | Expected IDs executable; WSOL mints have expected owners | Presence is established, runtime compatibility is separate. |

Pump, PumpSwap and pump-fees share observed devnet upgrade authority `GUYCUEpxSkm1ccDo3LSKMR4Xpe5bgMnEZaFygWUSEVH3`. Pump/PumpSwap ProgramData are below the 10 MiB cap used in the mainnet availability reasoning. Their existing slot-only treatment can consequently latch opens after a pure extension on devnet. No guard mask, fingerprint rule or mainnet identity was changed to accommodate this difference.

The configured Pyth account `7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE` was receiver-owned, 134 B, fully verified and carried the expected SOL/USD feed. At coherent Pyth/Clock context **513,722,069**, its age was **134 s**, exceeding compiled `oracle_max_age_s = 120`. Its positive price and default confidence test passed. That single dated sample would refuse a crank as stale; it does not establish a permanent outage. Require a fresh observation at smoke time, without weakening the parameter.

`shortwin-gov init-config` refuses an explicitly configured slot that differs from an existing live ProgramData. A missing ProgramData with an explicit slot only warns and uses that value; the on-chain initialization itself does not fetch venues or Pyth. Successful initialization therefore cannot be used as evidence that unavailable venues work. No fake slots or stale mainnet slots were supplied as reviewed devnet values.

## Artifact and version gates

The earlier canonical release ELF is **1,644,024 B**, SHA-256 `d6f4e092dd15639c502535d68bcdb6c428c423b66d49d08f4f89e9409af538bc`, trimmed executable hash `f3d8dd7a5c62385abc6306e2da395ff666a1f69234ea50f2e7eb0523ab686250`, SBPF v3. It was found in the existing primary checkout; it is not automatically a fresh integrated-source build. The article wording edit adds a net source line in `add_margin.rs`; [the build rule](../DEVELOPMENT.md#the-build-rule) explains that error-origin line changes can affect ELF bytes.

Root subsequently produced an intermediate candidate of the same **1,644,024 B**, SHA-256 `d8d08784b3b3081d5fe1cf7aaff319e7e9e063733c308bd2e9e7ed8981bee182`. Its one changed diagnostic immediate, source line 72 to 73, gave it a different identity. **That intermediate artifact is superseded and is not selected for deployment.**

The first official-verifier attempt stopped before building because this session lacked Docker socket access. That failure remains in its [dated record](../audit/probes/release-wrap-2026-10-22/verifier/readiness.md). **Docker access is now repaired:** the owner authorized adding `todor_dev` to the Docker group and starting `docker.service`; both succeeded through the OS authentication dialog. A fresh group shell can reach Docker Engine 29.9.1. Existing login processes need `newgrp docker` or a new login to acquire the group. Socket permissions and boot enablement were not changed. See the [access receipt](../audit/probes/release-wrap-2026-10-22/verifier/docker-access.json).

Commit `7b11cc7f85503474c6b87e0ee8b2fedd51bbc249` reformats the corrected comment into its original number of lines, preserving the corrected rent description. **Exact-source native and real pinned-image builds both passed and reproduce the canonical release `d6f4…538bc`, including its trimmed hash and 1,644,024-byte length.** The test-venue and mock builds also match their canonical bytes. Git object comparisons establish unchanged fuzzer/harness, fixtures, runtime math, lockfile and toolchain inputs. The owner's completed ≥24-hour fuzz record therefore applies to these same runtime inputs, retaining its original source, runner, dates and coverage limits. No new fuzz run is claimed; the interrupted task remains stopped. See [build and input-equivalence evidence](../audit/probes/release-wrap-2026-10-22/byte-rebuild/SUMMARY.md).

The earlier fresh final-byte fuzz attempt was interrupted after the program worker received a provider safety block; its runner/supervisor were stopped and its additive launch review was interrupted. No qualifying 24-hour result is claimed for that interrupted attempt, and the blocked task was not restarted or reassigned. The [root stop receipt](../audit/probes/release-wrap-2026-10-22/fuzz-provider-stop.json) preserves that distinction.

Artifact preparation checklist:

1. **Done:** isolated builds of exact `7b11cc7`, package 1.0.3, Agave 4.1.2/platform-tools v1.57/SBPF v3. Native Anchor 1.2.0 used `--locked`; Docker's post-build lockfile and tracked tree are unchanged. Installed Anchor's source-revision provenance is unproved; the separately pinned Docker reproduction establishes executable identity.
2. **Done:** solana-verify 0.5.2 and the pinned image reproduce the exact canonical release, with full and trimmed hashes retained. Both build checkouts were isolated from primary target/key files.
3. **Done with original attribution:** release security.txt/A49 and IDL/client equality passed on the integration; the final comment formatting changes no interface and returns the already checked canonical bytes. The `test-venue` ELF and `mock_cpamm` remain local test artifacts. A bare `anchor deploy` would also deploy the mock; deploy only the selected shortwin artifact.
4. **Done for canonical runtime evidence:** the completed frozen-runtime fuzz applies through demonstrated artifact/source-input equivalence. The services and browser retain their separate CI/focused evidence. Program-only deployment is recorded below; full audit completion and operating readiness remain separate requirements.

Draft 1.0.3 is unreleased in the inspected records. Fixing that draft does not itself require 1.0.4. If 1.0.3 has shipped elsewhere, or a new patch candidate is deliberately chosen, use 1.0.4 with accurate evidence attribution. A possible prerelease is `v1.0.3-devnet.1` or `v1.0.4-devnet.1`; it is separate from stable release approval. The ELF embeds `source_release = v + CARGO_PKG_VERSION`, so a version change alters artifact bytes and must align with generated metadata and its claimed tag. No version or tag was changed by this preparation.

## Funding estimate and bounded faucet options

**Historical funding observation:** at 2026-10-22 07:20:41 UTC,
confirmed devnet slot 513,807,141, the payer held **25 SOL**. This exceeds the
20-SOL reserve target below. The initial zero balance and failed faucet probe
remain historical observations, not current blockers. The same predeployment
read-only check found the canonical program absent. See the
[funding receipt](../audit/probes/release-wrap-2026-10-22/devnet/funding-2026-10-22.json).
After the loader-only deployment, the payer held **16.638061120 SOL**. Funding
and deployment do not close the governance, venue or operator gates.

Read-only devnet rent queries returned:

| Account | Data B | Rent lamports |
|---|---:|---:|
| Program | 36 | 833,120 |
| ProgramData for the earlier canonical ELF, with no spare max length | 1,644,069 | 8,352,520,760 |
| Buffer for that ELF | 1,644,061 | 8,352,480,120 |
| GlobalConfig | 1,448 | 8,006,080 |
| VenueGuard | 304 | 2,194,560 |
| Each of four VenueExtras | 6,512 | 33,731,200 |
| OracleCache | 45 | 878,840 |
| Market | 3,104 | 16,418,560 |
| MarketVault | 2,200 | 11,826,240 |
| Typical SPL token account | 165 | 1,488,440 |

Initialization account rents total **146,004,280 lamports (0.14600428 SOL)**. Before deployment, a conservative reserve covering Program, ProgramData and a complete buffer concurrently was **16.705834 SOL** for the canonical ELF. This was a planning reserve, not a measured CLI minimum. Later initialization and markets must separately include governance/proposal rents, the tool-required fee-vault prefund of 890,880 lamports, the 5,000,000-lamport market creation fee, seed liquidity/margin and operating reserve. The initial upgrade authority pays initialization rents in the existing ops path. No payer balance was inferred from wallet creation. The devnet payer and now-deployed temporary upgrade authority is `3JfrUHa1RihL7rgJDWSmdV9DNGXkwKAUAcRDBh75cS14`. It initially held 0 SOL; one 2-SOL CLI airdrop attempt failed with a faucet/rate-limit error and was not repeatedly retried. The subsequently funded **25-SOL** balance covered the approved program upload while preserving the selected reserve. Actual deployment cost and remaining balance appear in the deployment receipt.

The failed initial faucet request is retained as historical evidence. The owner has since funded the payer with 25 SOL, and no further funding request is currently needed. The original [funding guide](https://solana.com/developers/cookbook/development/airdrops-and-faucets) and [Foundation faucet](https://faucet.solana.com/) links remain available for later owner-managed test funding. No automated faucet retry loop or alternate-wallet rotation was run.

## Review packet and allowed rehearsal scope

The concrete signing packet must identify cluster/genesis, exact RPC and artifact hash/size, fixed program ID, matching program signer public key, payer and initial upgrade-authority public keys, chosen max length, fee/rent estimate and a spending ceiling. The root operator has now verified the existing canonical program signer public key equals `5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK`; its key material was not read by this review. This closes the earlier public signer-identity question. The new payer wallet is a separate signer. No program address was replaced.

**Historical concrete packet:** the [deployment packet](../audit/probes/release-wrap-2026-10-22/deployment-packet/deployment-packet.md)
binds canonical artifact `d6f4…538bc`, max length 1,644,024, the existing payer/upgrade
authority and actual upload buffer `2JGjQoiGyDLTsqy9186Ct7kN4zdG2ULb8nARjN7ddgcL`.
Unsigned buffer preparation passed at slot 513,816,000 (`err=null`, 2,820 CU,
10,000-lamport fee). At slot 513,816,163 the payer still held 25 SOL and all three
deployment accounts were absent. Expected net debit for one upload pass is
**8.361938880 SOL**; a conservative funding/fee envelope is **16.714459640 SOL**.
The stock CLI does not enforce a total-spend cap; the operator checks the 20-SOL
ceiling and 5-SOL reserve before an approved invocation. The CLI later deployed
the exact selected artifact after separately approved upload attempts.

**Historical first approved attempt, 09:02–09:04 UTC:** the one authorized CLI invocation
returned `Data writes to account failed: Custom error: Max retries exceeded`
after about 88 seconds. It created the approved buffer and confirmed 14 of
1,713 upload writes. The program and ProgramData were still absent then. Read-only RPC
at slot 513,833,429 found the buffer owned by the upgradeable loader, controlled
by the selected payer, funded with 8.352520760 SOL and containing the 14 matching
chunks. The payer held 16.647399240 SOL; 80,000 lamports in fees had been spent.
All 15 recorded buffer transactions succeeded on chain. The failure occurred
in the client upload/retry path, without a reported on-chain program error. The
[attempt and public-state receipts](../audit/probes/release-wrap-2026-10-22/deployment-packet/failed-attempt-retained-files.json)
retain the evidence. At that point no second upload, buffer close,
initialization or market transaction had been sent, and a new concrete review
and approval were required for recovery. Those occurred before the successful
resume.

**Historical recovery proposal:** keep the same funded buffer, artifact and signer identities.
Agave 4.1.2 reads the buffer and writes only chunks that differ; 1,699 of 1,713
chunks remain. An unsigned simulation of the first missing write at offset
13,440 passed at slot 513,834,223 (`err=null`, 2,670 CU, 5,000-lamport fee).
Estimated incremental cost for one successful resume pass is **0.009338120 SOL**,
including remaining writes, final fee and Program rent. A proposed **0.05-SOL
additional spending ceiling** allows up to five signing cycles while preserving
the 5-SOL reserve. The [routing research](../audit/probes/release-wrap-2026-10-22/deployment-packet/upload-routing-research.md)
recommends omitting `--use-rpc` to try the CLI's QUIC TPU route and setting
`--max-sign-attempts 5`. Public RPC throttling is a strong explanation for the
first failure, not a proven cause. The successful resume does not prove the
precise route used for every write.

**Approved resume completed:** the owner explicitly approved the same-buffer
retry within the additional 0.05-SOL ceiling. It finalized in
[`2twpM5s…GhVm4j`](https://explorer.solana.com/tx/2twpM5sUnQeAJLVTeKo9gHeDPfpU3iNitPZNg9UVm7At6zMyQCn6DATh9azQRk9rd1DzCi315W8HKZLbDpGhVm4j?cluster=devnet)
at slot **513919880**. On-chain ProgramData contains the exact full ELF hash
`d6f4e092dd15639c502535d68bcdb6c428c423b66d49d08f4f89e9409af538bc`;
the temporary upgrade authority is the selected payer and the upload buffer was
consumed. The additional debit was 0.009338120 SOL, and the full debit from the
original 25-SOL balance was 8.361938880 SOL, leaving **16.638061120 SOL**.
The [deployment receipt](DEVNET_1_0_3_DEPLOYMENT_2026_10_22.md) and
[retained evidence](../audit/probes/release-wrap-2026-10-22/deployment-packet/README.md)
give the finalized signature and exact account checks. No initialization,
governance, lending or market transaction is claimed.

The [owner-delegated decision](devnet-decisions-2026-10-22.md) selected the first phase below, retaining version 1.0.3 and upgradeability, with a 20-SOL deployment ceiling and at least 5 SOL reserved. Its concrete transaction reviews and owner approvals preceded both signed upload attempts. Later initialization and markets require their own preparation.

Phases:

- **Deploy-only rehearsal — complete:** shortwin was deployed and its loader, ProgramData, authority and executable hash were verified. This proves loader deployment, not initialized governance or functional markets.
- **Disabled-venue initialization rehearsal:** additionally needs an explicit public authority layout and a disposition for mutable devnet Squads. Existing immutability checks remain intact and cannot be recorded as passing. A config-only success cannot establish trade readiness.
- **Real Pump/PumpSwap smoke:** additionally requires scoped devnet executable/interface compatibility, a supported real devnet mint/curve/canonical pool, fresh oracle, reviewed guard slots/extras, adequate real reserves/burned LP, operator funding and a capital ceiling. Then exercise normal observations, lend, open/add-margin/close and withdraw, retaining signatures and balance/account checks. No real test market has yet been selected.
- **All-venue local or fork testing:** existing reviewed venue fixtures and TEST-ONLY mock can support local rehearsal. Existing Playwright setup starts and seeds a local validator with synthetic wallets and fixture ELFs/accounts; do not point it at devnet or relabel it as live devnet evidence.

## Frontend and operator configuration

A separate public frontend build must explicitly set `VITE_CLUSTER=devnet`, all four `VITE_RPC_URL`, `VITE_RPC_WS_URL`, `VITE_INDEXER_URL`, `VITE_INDEXER_WS_URL`, and explorer `https://explorer.solana.com/tx/{sig}?cluster=devnet`. Public RPC values for this preparation are `https://api.devnet.solana.com` and `wss://api.devnet.solana.com`. Browser program constants remain source-pinned. Leave `VITE_ENABLE_TEST_VENUE=false`; the parser rejects that feature off localnet. A bare build defaults to localnet.

Keeper public configuration must name the exact covered market set, protocol ALT if needed, daily budget, priority ceilings, `dry_run=false` for actual coverage, and one designated pauser using a separate coverage authority. Fund zero-reward exit operation and retain top-up/alert ownership. Example files are templates, not running evidence.

Use source-matched indexer/database setup, migrate, backfill, then one live ingest writer and its readers. Record compatible running artifact identities and devnet RPC/WS/first-deployment boundary before any market smoke. Only documented startup compatibility is in this preparation; the previously stopped public API/WebSocket server review and fixtures remain stopped.

The approved loader-only rehearsal checked the deployed hash, Program/ProgramData and exact upgrade authority. In later initialized/market phases, additionally check 304-byte VenueGuard/full digest fields, config/fee destination, pending proposals, market/vault accounting and actual keeper/reader/UI behavior. Keep the stable release gate pending until its independently documented requirements pass.

## Retained evidence

Timestamped raw responses and validated summaries are retained outside the source checkout under `release-wrap-2026-10-22/release_devnet_plan/`: `devnet-rpc-raw.json`, `devnet-rpc-summary.json`, `devnet-cost-oracle-raw.json`, `devnet-cost-oracle-summary.json`, the bounded lookup scripts, and `gates.json`. The lookup scripts contain only public addresses and read-only JSON-RPC methods. No private keys, .env files or credentials were read or copied into this evidence.

Primary address references: [Pump](https://github.com/pump-fun/pump-public-docs/blob/main/docs/PUMP_PROGRAM_README.md), [PumpSwap](https://github.com/pump-fun/pump-public-docs/blob/main/docs/PUMP_SWAP_README.md), [Raydium](https://github.com/raydium-io/raydium-docs-v1/blob/main/reference/program-addresses.mdx), [Squads devnet quickstart](https://docs.squads.so/main/development/introduction/quickstart), [Pyth push feeds](https://docs.pyth.network/price-feeds/core/push-feeds/solana). Published availability does not replace the observed target-cluster gates above.
