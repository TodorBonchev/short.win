# Concrete devnet command packet proposal

Status: proposal only; no deployment, initialization, simulation or signing run by this reviewer. Parent root owns fresh-artifact validation, funding and the final user signing review. Repo-native explanation: `docs/deployment/DEVNET_1_0_3_READINESS_2026_10_22.md` in the integrated release checkout.

## Program-only transaction summary

- Cluster: devnet; RPC `https://api.devnet.solana.com`, WS `wss://api.devnet.solana.com`; observed genesis `EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG`.
- Program: `5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK`; ProgramData `5UUu4yttKrtDNegGbUgZBxubJPaoPrsuiybeCJ8BnqcF`; both observed absent.
- Release ELF: root reports 1,644,024 B, SHA-256 `d8d08784b3b3081d5fe1cf7aaff319e7e9e063733c308bd2e9e7ed8981bee182`. Root must fill its exact immutable artifact path and trimmed executable hash after verifier/gate completion. The primary checkout's older d6f4… artifact is not this ELF.
- Program signer: owner-held existing canonical signer, root-verified public identity above. This reviewer did not open its file.
- Fee payer and proposed initial upgrade authority: `3JfrUHa1RihL7rgJDWSmdV9DNGXkwKAUAcRDBh75cS14`, root-created CLI wallet outside repo.
- Max program length: explicitly 1,644,024 B, no spare capacity. Any future larger upgrade needs a separately reviewed extension and extra rent; do not silently double allocation.
- Rent: Program 833,120; PD 8,352,520,760; buffer 8,352,480,120 lamports. Conservative concurrent total 16.705834 SOL plus network fees; 20 devnet SOL is a sensible funding target. Actual CLI peak/fee result must be reviewed before signing. Root reports initial balance 0 and a single failed 2-SOL faucet request.
- Current approval scope to request: first loader deployment only, preserving upgradeability and setting the stated initial authority. Initialization, governance, markets and public hosting remain separate.

Proposed CLI shape, after root replaces the artifact placeholder and all required gates pass:

```sh
NO_DNA=1 solana program deploy '<EXACT_FRESH_VERIFIED_RELEASE_ELF>' \
  --url https://api.devnet.solana.com \
  --ws wss://api.devnet.solana.com \
  --commitment confirmed \
  --program-id /home/todor_dev/Projects/short.win/target/deploy/shortwin-keypair.json \
  --fee-payer /home/todor_dev/.config/shortwin/devnet/release-testing-2026-10-22.json \
  --upgrade-authority /home/todor_dev/.config/shortwin/devnet/release-testing-2026-10-22.json \
  --max-len 1644024 --max-sign-attempts 1 --no-auto-extend \
  --with-compute-unit-price 0 --use-rpc --output json
```

This is a deployment command, not a dry run. Do not execute it before explicit approval of the completed summary and any actual spending ceiling. It must not use `--final`, `--skip-preflight`, `--skip-feature-verify`, a bare `anchor deploy` or the test-venue/mock ELF. Do not proceed on 0 balance. Bound any failure handling and review its buffer state before choosing cleanup; do not generate or repeatedly retry new program signers.

Read-only post-deploy verification must inspect actual program/ProgramData ownership, size, authority and deployed trimmed executable hash against the freshly verified artifact. Record the actual deploy/write transaction signatures and public account addresses.

## Initialized/full-market smoke is a separate blocked packet

The public authority/governance member/create-key layout and actual test mint/market are not yet supplied. Devnet SQDS4 is mutable, so existing immutable-governance verification fails. Canonical Raydium/Jupiter IDs are nonexecutables. Pump/PumpSwap executables have unreviewed devnet identities and are below the mainnet 10 MiB cap assumption; Pyth's sampled age134s exceeded the default120s. No `init-config` command is executable as a full-readiness gate from these facts.

Preserve existing immutability, masks, IDs and release/test separation. A limited initialization rehearsal needs an explicit trust/scope disposition; real Pump tests need compatibility evidence and a real supported mint/pool, a fresh oracle and a reviewed capital ceiling. Full all-venue testing stays in the labelled local/fork fixture profile unless a separate devnet adaptation is explicitly designed and reviewed.

## Public frontend/operator config proposal

Do not write .env files or copy credential endpoints into evidence. Build-time public settings are:

```text
VITE_CLUSTER=devnet
VITE_RPC_URL=https://api.devnet.solana.com
VITE_RPC_WS_URL=wss://api.devnet.solana.com
VITE_INDEXER_URL=<actual operator-owned devnet reader endpoint>
VITE_INDEXER_WS_URL=<matching devnet reader WebSocket endpoint>
VITE_EXPLORER_TX_URL=https://explorer.solana.com/tx/{sig}?cluster=devnet
VITE_ENABLE_TEST_VENUE=false
```

The two indexer endpoints must be real and tied to reviewed running artifacts. Current program/venue constants are unchanged. An optional Anchor `[programs.devnet] shortwin = "5yvy…VpKK"` stanza can document the existing identity, but cannot make dependencies compatible; explicit cluster arguments avoid changing the provider default. No Anchor/gov/code config was edited here.

Keeper needs an exact covered-market list, public budget/priority config, funded payer, separate coverage authority, one designated pauser, metrics/alerts and zero-reward exit backstop. Stage compatible readers, then reviewed initialization/keeper, then small normal market smoke. The prior stopped API/WebSocket server review remains stopped.

## Faucet handling

Funding is root-owned. One CLI airdrop probe has failed; no retry storm. Official source: https://solana.com/developers/cookbook/development/airdrops-and-faucets . The Foundation faucet page https://faucet.solana.com/ advertises at most two web requests per8h and GitHub higher-limit access; its agent guidance directs programmatic methods. A human may fund this public address using approved access or an existing developer balance. No third-party faucet credential or mining tool is needed for this plan. Funding does not authorize program deployment.
