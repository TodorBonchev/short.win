# Devnet 1.0.3 preparation (2026-10-22)

**A program deployment rehearsal is being prepared. Full devnet market readiness is not established.** No deployment, initialization, governance transaction or market transaction was executed by this review. The owner separately authorized creating a fresh devnet CLI wallet outside the repository; the wallet is not authority to change the compiled program address or to deploy before the concrete transaction review.

The inspected integrated source is `b31c02a091008da18c39d7836c8e0e9d6d018973`. Existing technical release evidence is attributed in [the October 22 release record](../audit/release-checks-1.0.3-2026-10-22.md). That record's mainnet Jupiter drift and final mainnet operating gates remain separate from the actual devnet observations below. A devnet rehearsal or prerelease tag does not certify a stable release.

## Public devnet observations

Bounded read-only requests used only `https://api.devnet.solana.com`, with confirmed commitment, no credential-bearing endpoint and no arbitrary wallet lookup. Genesis was `EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG`; the account metadata batch was at slot **513,721,817**, API `4.4.0-beta.1`. All public addresses were validated, venue ProgramData derivations were checked against source constants, and executable Program pointers matched the queried ProgramData. Only metadata prefixes were fetched; executable hashes and signed compatibility were not established.

| Dependency | Actual devnet observation | Gate |
|---|---|---|
| short.win `5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK` | Program, ProgramData and initial config/guard/oracle PDAs absent | Initial deployment needs the matching program signer, separately from the new payer wallet. |
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

Root subsequently produced a fresh candidate of the same **1,644,024 B**, SHA-256 `d8d08784b3b3081d5fe1cf7aaff319e7e9e063733c308bd2e9e7ed8981bee182`. Root attributes the byte change to one diagnostic immediate, source line 72 to 73. This is a new artifact identity; its verifier comparison and relevant new gates remain required. The rent estimates below apply unchanged because the size is unchanged.

The actual official-verifier attempt stopped before building: the session cannot access the Docker socket, and noninteractive sudo requires a password. No daemon, permission or tool configuration was changed. Its [retained record](../audit/probes/release-wrap-2026-10-22/verifier/readiness.md) distinguishes the local candidate from an independently reproduced pinned-image build.

Before deployment, produce and retain:

1. A clean exact revision and package identity, built with Anchor 1.2.0, Agave 4.1.2, platform-tools v1.57, SBPF v3 and locked dependencies. In an isolated build target use the CI split: `anchor build --tools-version v1.57 --arch v3 --ignore-keys --no-idl -- -- --locked`, followed by `anchor idl build -p shortwin -o <TARGET>/idl/shortwin.json -t <TARGET>/types/shortwin.ts -- --locked`. Do not generate/synchronize program keys as a side effect.
2. A byte-identical solana-verify 0.5.2 build using the pinned image and [verifiable build procedure](../DEVELOPMENT.md#the-build-rule). Record full file hash, trimmed executable hash, size and exact source ref. Use a dedicated cache/output for concurrent checkouts.
3. Passing release security.txt/A49 checks and actual IDL/client equality. The `test-venue` ELF and `mock_cpamm` are test artifacts and must not be deployed. A bare `anchor deploy` would also deploy the mock; deploy only the reviewed shortwin artifact.
4. Appropriately attributed gates for relevant changes. Prior frozen-runtime fuzz and earlier CI do not silently become evidence for newly compiled bytes.

Draft 1.0.3 is unreleased in the inspected records. Fixing that draft does not itself require 1.0.4. If 1.0.3 has shipped elsewhere, or a new patch candidate is deliberately chosen, use 1.0.4 with accurate evidence attribution. A possible prerelease is `v1.0.3-devnet.1` or `v1.0.4-devnet.1`; it is separate from stable release approval. The ELF embeds `source_release = v + CARGO_PKG_VERSION`, so a version change alters artifact bytes and must align with generated metadata and its claimed tag. No version or tag was changed by this preparation.

## Funding estimate and bounded faucet options

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

Initialization account rents total **146,004,280 lamports (0.14600428 SOL)**. A conservative reserve covering Program, ProgramData and a complete buffer concurrently is **16.705834 SOL** for the earlier ELF. This is a planning reserve, not a measured CLI minimum. Recalculate size-dependent rent after the exact candidate is built, and include write/deploy network fees, governance/proposal rents, the tool-required fee-vault prefund of 890,880 lamports, the 5,000,000-lamport market creation fee, seed liquidity/margin and operating reserve. The initial upgrade authority pays initialization rents in the existing ops path. No payer balance is inferred from wallet creation. Root reports the new devnet payer/proposed initial upgrade authority is `3JfrUHa1RihL7rgJDWSmdV9DNGXkwKAUAcRDBh75cS14`, initially 0 SOL. One 2-SOL CLI airdrop attempt failed with a faucet/rate-limit error; it was not repeatedly retried. A **20 devnet SOL** funding target covers the conservative program/buffer reserve, initialization rents and a working margin; it is a planning target, not an exact fee quote or a claim that initialization/markets are ready. An approximately 10-SOL balance may cover the final deployed rent, but is below the conservative simultaneous-buffer reserve and must not be promised sufficient without the CLI's exact funding calculation.

A fresh wallet may not obtain the required balance in one faucet request. The authorized bounded CLI airdrop probe for **2 devnet SOL** has already failed, as recorded above. The next funding step is owner-provided test SOL, not another automated request loop. On rate limiting or failure, stop rather than run repeated requests, rotate endpoints or create extra wallets. The official [funding guide](https://solana.com/developers/cookbook/development/airdrops-and-faucets) lists other sources. The [Foundation faucet](https://faucet.solana.com/) currently advertises at most two web requests per eight hours and a GitHub-based higher limit; its agent guidance points to programmatic methods. A human can fund the supplied public address or use their authorized faucet access. A developer-controlled existing devnet balance is another option. No faucet request or transfer was made by this review, and no mining tool was installed or run.

## Review packet and allowed rehearsal scope

The concrete signing packet must identify cluster/genesis, exact RPC and artifact hash/size, fixed program ID, matching program signer public key, payer and initial upgrade-authority public keys, chosen max length, fee/rent estimate and a spending ceiling. The root operator has now verified the existing canonical program signer public key equals `5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK`; its key material was not read by this review. This closes the earlier public signer-identity question. The new payer wallet is a separate signer. No program address was replaced.

The current honest options are:

- **Deploy-only rehearsal:** after artifact, signer, funding and transaction approval, deploy only shortwin and verify its loader, ProgramData, authority and executable hash. This proves loader deployment, not initialized governance or functional markets.
- **Disabled-venue initialization rehearsal:** additionally needs an explicit public authority layout and a disposition for mutable devnet Squads. Existing immutability checks remain intact and cannot be recorded as passing. A config-only success cannot establish trade readiness.
- **Real Pump/PumpSwap smoke:** additionally requires scoped devnet executable/interface compatibility, a supported real devnet mint/curve/canonical pool, fresh oracle, reviewed guard slots/extras, adequate real reserves/burned LP, operator funding and a capital ceiling. Then exercise normal observations, lend, open/add-margin/close and withdraw, retaining signatures and balance/account checks. No real test market has yet been selected.
- **All-venue local or fork testing:** existing reviewed venue fixtures and TEST-ONLY mock can support local rehearsal. Existing Playwright setup starts and seeds a local validator with synthetic wallets and fixture ELFs/accounts; do not point it at devnet or relabel it as live devnet evidence.

## Frontend and operator configuration

A separate public frontend build must explicitly set `VITE_CLUSTER=devnet`, all four `VITE_RPC_URL`, `VITE_RPC_WS_URL`, `VITE_INDEXER_URL`, `VITE_INDEXER_WS_URL`, and explorer `https://explorer.solana.com/tx/{sig}?cluster=devnet`. Public RPC values for this preparation are `https://api.devnet.solana.com` and `wss://api.devnet.solana.com`. Browser program constants remain source-pinned. Leave `VITE_ENABLE_TEST_VENUE=false`; the parser rejects that feature off localnet. A bare build defaults to localnet.

Keeper public configuration must name the exact covered market set, protocol ALT if needed, daily budget, priority ceilings, `dry_run=false` for actual coverage, and one designated pauser using a separate coverage authority. Fund zero-reward exit operation and retain top-up/alert ownership. Example files are templates, not running evidence.

Use source-matched indexer/database setup, migrate, backfill, then one live ingest writer and its readers. Record compatible running artifact identities and devnet RPC/WS/first-deployment boundary before any market smoke. Only documented startup compatibility is in this preparation; the previously stopped public API/WebSocket server review and fixtures remain stopped.

After any approved rehearsal, recheck deployed hash, exact authorities, 304-byte VenueGuard/full digest fields, config/fee destination, pending proposals, market/vault accounting and actual keeper/reader/UI behavior. Keep the stable release gate pending until its independently documented requirements pass.

## Retained evidence

Timestamped raw responses and validated summaries are retained outside the source checkout under `release-wrap-2026-10-22/release_devnet_plan/`: `devnet-rpc-raw.json`, `devnet-rpc-summary.json`, `devnet-cost-oracle-raw.json`, `devnet-cost-oracle-summary.json`, the bounded lookup scripts, and `gates.json`. The lookup scripts contain only public addresses and read-only JSON-RPC methods. No private keys, .env files or credentials were read or copied into this evidence.

Primary address references: [Pump](https://github.com/pump-fun/pump-public-docs/blob/main/docs/PUMP_PROGRAM_README.md), [PumpSwap](https://github.com/pump-fun/pump-public-docs/blob/main/docs/PUMP_SWAP_README.md), [Raydium](https://github.com/raydium-io/raydium-docs-v1/blob/main/reference/program-addresses.mdx), [Squads devnet quickstart](https://docs.squads.so/main/development/introduction/quickstart), [Pyth push feeds](https://docs.pyth.network/price-feeds/core/push-feeds/solana). Published availability does not replace the observed target-cluster gates above.
