# Devnet upload recovery review — 2026-10-22

The approved single attempt stopped during upload after 87.99 seconds with `Max retries exceeded`. **shortwin is not deployed. No second attempt or reclamation has been made or approved.** The original approval covered one upload attempt; a second invocation needs fresh explicit confirmation under [solana-dev W009](/home/todor_dev/.codex/skills/solana-dev/SKILL.md): “Never sign or send transactions without explicit user approval.”

The confirmed public read at **09:06:51 UTC, slot 513,833,813**, and root's subsequent finalized read at **09:08:24 UTC, slot 513,834,200**, agree on the following state. Devnet genesis remains `EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG`.

| Item | Observed value |
|---|---|
| Program | `5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK` — absent |
| ProgramData | `5UUu4yttKrtDNegGbUgZBxubJPaoPrsuiybeCJ8BnqcF` — absent |
| Existing buffer | `2JGjQoiGyDLTsqy9186Ct7kN4zdG2ULb8nARjN7ddgcL` |
| Payer and buffer authority | `3JfrUHa1RihL7rgJDWSmdV9DNGXkwKAUAcRDBh75cS14` |
| Payer balance | **16.647399240 SOL** |
| Buffer balance | **8.352520760 SOL** |
| Buffer owner / state | Upgradeable loader; mutable Buffer; nonexecutable |
| Buffer size | 1,644,061 bytes, including 37-byte metadata |
| Complete 960-byte upload chunks | **14 of 1,713** |
| Remaining writes | **1,699**; first missing chunk 14 at byte offset 13,440 |

The 8.352520760 SOL remains in the controlled buffer and fully covers the currently quoted ProgramData rent. Root independently confirmed **15 successful transaction signatures and 80,000 lamports (0.000080 SOL) in fees**. That also matches the payer decrease after subtracting the buffer balance. Public bytes showed no unexpected nonzero mismatches; this measures upload progress against the accepted artifact, not artifact qualification. The full buffer response stays external. Its compact [verified summary](recovery-public-summary.json) and the [recovery packet](recovery.json) retain receipt SHA-256 `feab5e5b99ad31d704e157fa09aeeaae606d369acbc75310871086ab546d130b`.

**Resume behavior.** CLI 4.1.2 reuses an explicitly supplied existing buffer after checking its loader owner, Buffer state, authority and capacity. It skips buffer creation/funding, compares the accepted file with existing chunks and writes only differences. Use the same accepted artifact, program signer, payer/authority and buffer keypair. These behaviors are in the [pinned CLI source](https://github.com/anza-xyz/agave/blob/v4.1.2/cli/src/program.rs#L1472-L1521) and [deployment path](https://github.com/anza-xyz/agave/blob/v4.1.2/cli/src/program.rs#L2451-L2501).

The cause is **strongly suspected, not proven**, to involve public RPC capacity. The original `--use-rpc` write path schedules submissions about 10 ms apart, roughly 100 per second, while official devnet documentation lists **40 requests per ten seconds per RPC method**. The recorded error establishes that allowed signing cycles ended with uploads unconfirmed; it does not prove throttling or exclude delivery/confirmation problems. See the [pinned sender](https://github.com/anza-xyz/agave/blob/v4.1.2/client/src/send_and_confirm_transactions_in_parallel.rs) and [published endpoint limits](https://solana.com/docs/references/clusters#devnet-rate-limits).

**Recommended proposal for fresh approval:** one invocation reusing this buffer, **omit `--use-rpc`**, and change **`--max-sign-attempts` from `1` to `5`**. Keep priority price `0`, max length 1,644,024, no automatic extension, retained upgradeability and all preflight/feature/fee checks. The exact [review-only command](recovery-command.review-only.txt) has been validated using `--help` only. Default QUIC TPU delivery from this host is unproven; RPC/WS control and confirmation remain, and the sender may fall back to RPC. This is a delivery proposal with a bounded cost estimate, not a successful route test.

At **09:08:30 UTC, slot 513,834,223**, an unsigned simulation of the first missing 960-byte write passed: **`err=null`, 2,670 CU, fee 5,000 lamports**. It used the actual buffer and payer with a zero signature placeholder, `sigVerify=false` and blockhash replacement. The [simulation summary](recovery-simulation-summary.json) retains its exact scope and fresh fee quotes. A successful complete final deployment cannot yet be simulated against the incomplete buffer; the CLI will simulate it after all required writes land.

| Additional cost from the current funded-buffer state | Devnet SOL |
|---|---:|
| One remaining upload pass plus final transaction fees | 0.008505000 |
| New Program account rent | 0.000833120 |
| Expected additional debit for one successful pass | **0.009338120** |
| Conservative fees for five complete sets of 1,699 writes plus one final transaction | 0.042485000 |
| Five-cycle envelope including Program rent | **0.043318120** |
| Payer balance after that envelope | **16.604081120** |
| Proposed additional operator ceiling | **0.050000000** |

The five-cycle envelope intentionally charges every remaining write in every signing cycle, although the CLI normally retries only unconfirmed writes. Current quotes are 5,000 lamports per write and 10,000 for the final transaction; priority price is zero. Buffer rent is already funded. The proposal preserves the original **20-SOL aggregate ceiling** and **5-SOL minimum reserve**. Including the first attempt, the five-cycle envelope totals **8.395918880 SOL** of payer debit.

**The CLI has no native aggregate spend cap.** The 0.05-SOL additional limit is a proposed operator ceiling based on unchanged current quotes, valid existing buffer funding, at most 1,699 remaining chunks, at most five write signing cycles, and one final deploy. Refresh state and costs just before any approved execution and stop on an unexpected result. No further invocation, top-up, authority change or reclamation is included.

The alternatives are to leave the controlled funded buffer in place, or, if the owner abandons deployment, prepare a separately approved buffer-close/refund transaction. Neither requires action during this read-only review. The safe next step is to present the concrete same-buffer proposal, its route uncertainty, unsigned result and costs for a **new explicit transaction approval**.

Retain only the compact text/JSON/script whitelist in `recovery.json`. Keep the full `recovery-state-raw.json` buffer response, ELF and every keypair file outside Git. No private key contents, signatures, sending, retry, reclaim, project edits, Git writes, services, security review or stopped-task work were part of this recovery review.
