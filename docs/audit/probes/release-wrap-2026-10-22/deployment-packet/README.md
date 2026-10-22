# 1.0.3 program-only devnet deployment evidence — 2026-10-22

**Outcome: deployed and independently verified.** The approved ShortWin ELF is
executable at [`5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK`](https://explorer.solana.com/address/5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK?cluster=devnet).
The final [transaction](https://explorer.solana.com/tx/2twpM5sUnQeAJLVTeKo9gHeDPfpU3iNitPZNg9UVm7At6zMyQCn6DATh9azQRk9rd1DzCi315W8HKZLbDpGhVm4j?cluster=devnet)
is finalized without error at slot `513919880`. The complete on-chain ELF is
1,644,024 bytes and has SHA-256
`d6f4e092dd15639c502535d68bcdb6c428c423b66d49d08f4f89e9409af538bc`.
The [deployment record](../../../../deployment/DEVNET_1_0_3_DEPLOYMENT_2026_10_22.md)
contains the balances, authority and remaining prerequisites.

The owner approved an initial upload. It stopped with `Max retries exceeded`
after creating a funded buffer and landing 14 write chunks. The owner then
approved a single same-buffer continuation, which succeeded. The original
[pre-approval packet](deployment-packet.md), [failed attempt](deployment-execution-result.json)
and [recovery proposal](recovery-packet.md) are retained as historical
point-in-time records; their statements that the program was absent or the
continuation was pending are superseded by the finalized receipts below.

| Stage | Retained evidence |
|---|---|
| Fresh state before continuation | [Preflight](approved-resume-preflight.json) |
| Unsigned first-missing-write simulation | [Summary](approved-resume-simulation-summary.json), [request](approved-resume-unsigned-simulation-request.json), [response](approved-resume-unsigned-simulation-raw.json), [context request](approved-resume-simulation-context-request.json), [context response](approved-resume-simulation-context-raw.json) |
| Approved CLI invocation | [Start and arguments](approved-resume-execution-start.json), [CLI output](approved-resume-execution.log), [exit result](approved-resume-execution-result.json) |
| Root post-deployment read | [Program, ProgramData, hash, accounts and payer](post-deploy-onchain-verification.json) |
| Finalized transaction | [Signature status and fee](finalized-deployment-signature.json) |
| Independent finalized read | [Fresh public RPC verification](independent-deploy-verification.json) |
| Receipt integrity | [File sizes and SHA-256 hashes](deployment-result-retained-files.json) |

Both on-chain reads match the accepted full ELF hash and the temporary upgrade
authority. The consumed upload buffer and the selected GlobalConfig,
VenueGuard and OracleCache accounts are absent. This was a program-only loader
deployment; the program has not been initialized and no market transaction has
been tested. The ELF snapshot, full account responses and all signing keys
remain outside Git. No raw secret bytes are retained in this packet.
