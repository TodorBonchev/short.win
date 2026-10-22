# Program-only devnet deployment packet — 2026-10-22

> Historical pre-approval state. The deployment later succeeded; see the
> [finalized outcome and receipts](README.md).

Concrete preparation is complete. The accepted artifact and real upload buffer signer are bound below. **No transaction has been signed or sent. Explicit user approval of this transaction summary is the remaining step.**

The selected operation is one shortwin-only, uninitialized, upgradeable loader deployment rehearsal. Version 1.0.3 remains unreleased; this operation creates no stable release tag. Governance initialization and functional markets are outside this operation.

| Item | Selected value |
|---|---|
| Cluster | Solana devnet |
| RPC / WebSocket | `https://api.devnet.solana.com` / `wss://api.devnet.solana.com` |
| Expected genesis | `EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG` |
| Commitment | `confirmed` |
| Program / matching program signer public key | `5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK` |
| ProgramData | `5UUu4yttKrtDNegGbUgZBxubJPaoPrsuiybeCJ8BnqcF` |
| Loader | `BPFLoaderUpgradeab1e11111111111111111111111` |
| Fee payer, temporary upgrade authority and buffer authority | `3JfrUHa1RihL7rgJDWSmdV9DNGXkwKAUAcRDBh75cS14` |
| Upload buffer | `2JGjQoiGyDLTsqy9186Ct7kN4zdG2ULb8nARjN7ddgcL` |
| ELF / maximum program length | 1,644,024 bytes / 1,644,024 bytes; automatic extension disabled |
| Upgradeability | Retained, with the selected temporary upgrade authority |
| Artifact SHA-256 | `d6f4e092dd15639c502535d68bcdb6c428c423b66d49d08f4f89e9409af538bc` |
| Exact source revision | `7b11cc7f85503474c6b87e0ee8b2fedd51bbc249` |
| Reviewed ceiling / minimum reserve | 20 devnet SOL / 5 devnet SOL |

Root reports acceptance of the host and Docker release rebuilds and test/mock variant evidence. This operational task checked only the accepted ELF's size and full-file hash, then retained an identical external deployment snapshot, `shortwin-d6f4e092.so`, outside Git. The [artifact identity receipt](artifact-identity.json) identifies the accepted source file. Artifact qualification was performed separately by root.

The upload keypair was generated under the owner's existing authorization at `/home/todor_dev/.config/shortwin/devnet/release-buffer-2026-10-22.json`, without overwriting an existing key. Its directory is mode 0700 and file mode 0600. Secret and seed output was discarded; only `solana-keygen pubkey` output is retained in the [public identity receipt](buffer-public-identity.json). The payer and canonical program signer keypair contents were not inspected by this task.

**Current public state and costs.** At 07:56:38 UTC, confirmed slot **513,816,163**, genesis matched, the payer held **25 SOL**, and Program, ProgramData and upload buffer accounts were absent. See the [final public receipt](final-public-state-raw.json). Rent was refreshed at the unsigned simulation's confirmed context.

| Cost | Lamports | Devnet SOL |
|---|---:|---:|
| Program account, 36 bytes | 833,120 | 0.000833120 |
| ProgramData, 1,644,069 bytes | 8,352,520,760 | 8.352520760 |
| Buffer minimum rent, 1,644,061 bytes | 8,352,480,120 | 8.352480120 |
| Selected CLI's initial buffer funding | 8,352,520,760 | 8.352520760 |
| At most one upload pass, including initial/final fees | 8,585,000 | 0.008585000 |
| Expected net debit with that pass | 8,361,938,880 | **8.361938880** |
| Expected payer balance afterwards | 16,638,061,120 | **16.638061120** |
| Conservative two-ProgramData-rent envelope plus those fees | 16,714,459,640 | **16.714459640** |
| Balance after that conservative envelope | 8,285,540,360 | **8.285540360** |

The buffer's funds support the final ProgramData account; the conservative envelope deliberately reserves both amounts at once. These amounts are current quotes and planning bounds. A failed upload may leave rent locked in its buffer.

The stock CLI has **no hard aggregate spend-cap or reserved-balance flag**. The 20-SOL ceiling is an operator limit. The selected command constrains exposure with exact max length, no automatic extension, zero priority price and one write signing cycle. Current quotes leave substantial room below the ceiling. No manual second attempt, top-up or extra transaction is covered; refresh costs and state before execution and stop on an unexpected result. Do not describe the CLI as automatically reserving 5 SOL.

**Unsigned simulation.** The actual selected payer and buffer public key were used in an unsigned `simulateTransaction` request at slot **513,816,000**, with two zero signature placeholders, `sigVerify=false`, `replaceRecentBlockhash=true`, and confirmed commitment. Buffer creation and initialization passed: **`err=null`, 2,820 CU, 10,000 lamports**. The account is allocated to the upgradeable loader with the selected buffer authority. See [summary](unsigned-simulation-summary.json), [raw request/result](unsigned-simulation-raw.json), and [bounded read-only script](unsigned-buffer-simulation.py).

The CLI-shaped fee messages returned 10,000 lamports for initial preparation, 5,000 for a write and 10,000 for final deployment. A 960-byte maximum chunk gives at most 1,713 upload writes; all-zero chunks may reduce the actual count. These quotes include zero priority price.

A populated buffer does not yet exist on chain, so **the complete final loader deployment has not been simulated**. A remote simulation cannot carry a 1.64-MB upload in one transaction. With explicit compute price `0`, the selected CLI simulates initial preparation, the first write, and final deployment before signing each stage, while ordinary send preflight remains enabled. The retained [review-only command](deploy-command.review-only.txt) is not executed. It includes no preflight, fee-check or feature-check bypass. Agave deployment behavior was checked against the [pinned CLI source](https://github.com/anza-xyz/agave/blob/v4.1.2/cli/src/program.rs) and [compute-budget helper](https://github.com/anza-xyz/agave/blob/v4.1.2/cli/src/compute_budget.rs); the [write sender](https://github.com/anza-xyz/agave/blob/v4.1.2/client/src/send_and_confirm_transactions_in_parallel.rs) limits the selected write path to one signing cycle. The [RPC documentation](https://solana.com/docs/rpc/http/simulatetransaction) permits unsigned simulation when signature verification is disabled. There is no CLI deploy dry-run or deploy sign-only option.

After approval, refresh genesis, account absence, balance, artifact hash/size, rent and fees immediately before executing the retained command. Stop if the conservative current envelope exceeds 20 SOL or leaves less than 5 SOL. Keep the stage simulations and preflight enabled. Retain actual signatures and the payer's balance change.

After deployment, verify that the Program account is executable, owned by the selected loader, and points to the expected ProgramData. Verify ProgramData's exact 1,644,069-byte length, deployed slot, selected upgrade authority and full ELF SHA-256 over the 1,644,024 bytes after its 45-byte metadata prefix. Record buffer closure or any remaining balance without initiating an extra cleanup transaction. Confirm the payer retains at least 5 SOL.

The [machine-readable packet](deployment-packet.json) retains all paths and conditions. No repo file, Git ref, service, deployment, initialization or governance state was changed by this task, and no stopped work was resumed.

**Why a final confirmation remains.** The applied [solana-dev skill](/home/todor_dev/.codex/skills/solana-dev/SKILL.md) explicitly says, under Transaction review (W009): “Never sign or send transactions without explicit user approval.” It requires displaying the transaction summary and waiting for confirmation, and says: “Simulate before sending.” The scoped unsigned result above is ready to surface with the concrete summary. Root should request that final transaction approval; this document makes no signature request itself.
