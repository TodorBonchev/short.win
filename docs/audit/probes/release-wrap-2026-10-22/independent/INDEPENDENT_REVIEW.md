# Independent release-preparation review, 2026-10-22

Status: accepted for root first documentation/evidence commit after the final freeze notification. The Explorer patch is accepted at `bf166672efee578b6a2a1835e0dd53729c548671`. No outstanding correction request blocks this narrowly scoped preparation commit. This is not stable-release, deployment or audit-completion approval. No source edit, signing, deployment, faucet, Git commit or broad security-audit claim was performed by this reviewer.

## Scope and disposition

Reviewed `/home/todor_dev/Projects/short.win-release-devnet`, initial merge `b31c02a091008da18c39d7836c8e0e9d6d018973`, the two-file Explorer configuration patch (subsequently committed by root at `bf166672efee578b6a2a1835e0dd53729c548671`), and the release wrap-up/devnet documentation and their TODO/AUDIT/index pointers. No applicable ancestor or targeted subtree AGENTS.md was found.

The Explorer patch is approved in this narrow scope. Devnet receives `?cluster=devnet` by default. Explicit templates keep precedence; mainnet/localnet defaults retain their prior behavior. Existing env.ts transaction/account consumers preserve the cluster query. A separately delegated independent focused review ran `pnpm --filter web test src/config/parseConfig.test.ts`: exit0, one file/seven tests passed. No prohibited paths or tests were invoked.

## Independent consistency checks

- All local Markdown target paths in the five touched release documentation files exist. The AUDIT section12 anchor exists. This is local-target verification, not a live external-link availability claim.
- Complete-file hashes independently matched prior canonical `d6f4e092dd15639c502535d68bcdb6c428c423b66d49d08f4f89e9409af538bc` and fresh `d8d08784b3b3081d5fe1cf7aaff319e7e9e063733c308bd2e9e7ed8981bee182`; each is1644024 bytes. Exactly one complete-file byte differs at offset330428:72 to73. Existing program evidence attributes that to the Anchor ZeroAmount diagnostic line; the review does not convert prior fuzz into evidence for the new hash.
- Existing services20-entry and devnet13-entry SHA256SUMS manifests all matched at review time.
- The devnet rent summary independently sums to16.705834SOL for Program+ProgramData+buffer concurrently and0.14600428SOL for initial config/guard/four extras/oracle rents. These are planning rents, not a measured deployment CLI minimum.
- Proposed deployment flags exist in installed Solana4.1.2 `solana program deploy --help`. The proposal remains a signing command with an unfilled verified-artifact placeholder and must not be executed by this review.
- The new source/runtime boundaries are explicit: article harness is outside runtime workspaces; local frontend build used localnet settings; no real target-cluster runtime operation was established; prior CI/fuzz remain attributed to their original source/artifacts.
- Keeping unreleased1.0.3 is supported by local manifest/tag inventory and retained release history. No localv1.0.3 tag exists; remote inventory/on-chain release history was not freshly established by this reviewer.

## Private-material boundary

Tracked credential-like filename inventory found only `apps/web/.env.example`; it contains documented public loopback/template settings. No tracked keypair/PEM/private-key filename was found. The new release documents and inspected source change contain public addresses and file paths, with no PEM or64-byte keypair-like array. An existing `https://user:secret@rpc.example` fixture is unchanged negative-test input for URL credential rejection, not a real credential. This is a focused addition/content check, not an exhaustive historical secret audit.

Metadata-only stat confirms `/home/todor_dev/.config/shortwin/devnet` mode0700 and external keyfile mode0600. No key contents were read. Public payer is `3JfrUHa1RihL7rgJDWSmdV9DNGXkwKAUAcRDBh75cS14`; the canonical program signer public identity is a distinct root-attributed input.

## Requested corrections resolved

Root corrected the devnet locked build/IDL commands, TODO wallet-path link and completed/failed faucet-probe tense. The final first freeze includes an exact retained official-Docker verifier blocker; its denied socket and password-required sudo outputs match the wording, and no verifier build is claimed. The proposal uses the full canonical address in the actual deployment command; the separate abbreviated optional Anchor stanza is explicitly an example. No further correction blocks this preparation commit.

## Remaining gates and exclusions

The scoped code change can be integrated. Stable release/tag remains blocked on selected-final-source/full gates, pinned/verifiable artifact reproduction, a new qualifying final-byte24h fuzz run, and the retained mainnet upstream/operator/approval gates. Devnet market readiness remains blocked on funding, immutable-governance disposition, target-cluster venue compatibility, fresh oracle, real market/authority inputs and funded compatible running operators. Program-only loader rehearsal is a separate potential review packet and proves no initialized-market behavior.

The broader audit checkpoint remains incomplete:36 hypotheses,13 independent decisions (six rejected,seven needing validation),23 without independent disposition; no confirmed new finding. No safe/closed/whole-audit-complete disposition is given. Permanently excluded public indexer API/WebSocket server and provider-stopped notes/fixtures and nonmonotone settlement FV-corner work were not read, tested, reconstructed, retried or reassigned. Only an ordinary focused browser configuration review was delegated.

## Final freeze verification

After root final-freeze notification, all63 retention-manifest entries passed retained-hash/size and decompressed-original-hash/size checks. Current original evidence files also match the manifest. Copied artifacts contain no PEM private-key,64-byte keypair-like array or credential-bearing URL markers. All104 local Markdown target paths across the five overview/readiness files and retained-evidence README exist; relevant AUDIT/DEVELOPMENT anchors were checked. `git diff --check` passed and runtime source paths still equal the reviewed `bf166672` source. Exact file fingerprints, manifest identity and checks are saved in `final-review-checks.json`.

Retained manifest SHA-256: `20786cf8ce98d9f42731fbadb0bb9a9e33108fc2ac3c0b4d72daf52b764fd4d4`. Review snapshot is limited to this first documentation/evidence freeze. A later additive new-fuzz launch receipt is outside this final snapshot and must retain exact current artifact/harness/fixtures, actual launch status and eventual completion separately. Qualification running is not a completed24h result.
