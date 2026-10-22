# 1.0.3 devnet program deployment — 2026-10-22

**The program-only devnet rehearsal is complete.** The program is executable at
[`5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK`](https://explorer.solana.com/address/5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK?cluster=devnet).
It remains uninitialized and upgradeable. This records a loader deployment,
not a working lending market or approval of a stable 1.0.3 tag.

| Item | Verified result |
|---|---|
| Cluster/genesis | Solana devnet, `EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG` |
| Final transaction | [`2twpM5sUnQeAJLVTeKo9gHeDPfpU3iNitPZNg9UVm7At6zMyQCn6DATh9azQRk9rd1DzCi315W8HKZLbDpGhVm4j`](https://explorer.solana.com/tx/2twpM5sUnQeAJLVTeKo9gHeDPfpU3iNitPZNg9UVm7At6zMyQCn6DATh9azQRk9rd1DzCi315W8HKZLbDpGhVm4j?cluster=devnet), finalized, no transaction error |
| Deployment slot | `513919880` |
| ProgramData | `5UUu4yttKrtDNegGbUgZBxubJPaoPrsuiybeCJ8BnqcF`, 1,644,069 bytes |
| Loader | Upgradeable BPF loader; Program executable and points to the expected ProgramData |
| Upgrade authority | `3JfrUHa1RihL7rgJDWSmdV9DNGXkwKAUAcRDBh75cS14`, the temporary devnet payer |
| On-chain ELF | 1,644,024 bytes; full SHA-256 `d6f4e092dd15639c502535d68bcdb6c428c423b66d49d08f4f89e9409af538bc` |
| Trimmed executable hash | `f3d8dd7a5c62385abc6306e2da395ff666a1f69234ea50f2e7eb0523ab686250` |
| Built source | `7b11cc7f85503474c6b87e0ee8b2fedd51bbc249`; its native and pinned-Docker builds exactly matched the canonical artifact |
| Upload buffer | Consumed; no remaining buffer account at the selected address |
| Initialization | GlobalConfig, VenueGuard and OracleCache PDAs absent |

The owner approved one initial attempt. It created the buffer and landed 14
upload writes, then the CLI returned `Max retries exceeded`. The owner then
approved one same-buffer resume. That invocation used the existing funded buffer,
the CLI's default validator transport, five possible signing cycles, zero
priority price, exact `--max-len 1644024` and `--no-auto-extend`. The CLI exited
zero in 37.38 seconds and returned the final signature above. The final
transaction is finalized at the deployment slot. The previous failure and the
successful continuation are both retained in the [attempt evidence](../audit/probes/release-wrap-2026-10-22/deployment-packet/README.md).

The payer started with 25 SOL and finished with **16.638061120 SOL**. The total
debit was **8.361938880 SOL**, consisting of 8.353353880 SOL of Program and
ProgramData rent and 0.008585000 SOL of transaction fees across both attempts.
The successful resume alone debited **0.009338120 SOL**, exactly the quoted
one-pass amount, within the separately approved 0.05-SOL additional ceiling.
The final balance exceeds the selected 5-SOL reserve. No buffer rent remains
stranded after deployment.

The 1.0.3 package/version stays unreleased as a stable tag. A separate phase
must establish governance compatibility and initialize configuration before
any market test. Devnet Pump/PumpSwap compatibility, a real supported market,
fresh oracle, guard/extras, funded keeper, indexer/readers and client operation
remain unverified. Mainnet Jupiter drift and the incomplete broader audit are
also independent release gates; this devnet deployment does not close them.
