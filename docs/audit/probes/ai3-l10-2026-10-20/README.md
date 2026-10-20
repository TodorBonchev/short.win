# AI3-L10: repeated-donation deposit hold

This is executed characterization evidence on source `007915da9d9c7a764783683f38bd3f4b961e6398`
and the frozen FR01 release ELF. It is not a permanent G-full regression or a change
to program policy. Apply [probe.patch](probe.patch) to the
[original signed-transaction test](../../../../tests/litesvm/tests/ai2_halted_s3_vest_it.rs)
in a disposable checkout using [the reproduction command](PROVENANCE.txt).
The tracked test was restored byte for byte after the probe.

The [passing log](run.log) records 31 donation-and-sweep transactions donating 78 lamports
and paying 155,000 fee lamports. They extended the S3 deposit hold by 219,140 seconds
(about 60.87 hours) across 30 successive prior deadlines. Every attempted new deposit
at those deadlines failed `PoolDrained`; that separate depositor paid 150,000 fee
lamports. Once only one old vesting lamport remained, a new one-lamport donation
extended the deadline by 10,799 seconds in each of 20 measured cycles.

After donations stopped, release completed. The original lender received
11,711,842,891 gross lamports versus 11,711,842,813 in the control, exactly the
additional 78 donations. A new lender could then deposit and received zero historical
SOL. These are finite fixture results, not universal cost/duration bounds, a live
fee-market estimate or a net-profit claim.

The small donation and sweep execute atomically through LiteSVM with all global
invariants asserted afterward. The test harness's prefix-as-separate-transaction check
would reject the transient non-rent-exempt trader after the transfer alone; runtime
rent checks for the complete signed bundle remain enabled. This distinction is
documented in [PROVENANCE.txt](PROVENANCE.txt).

[SHA256SUMS](SHA256SUMS) hashes the exact patch, log, provenance and ELF hash record.
The [owner decision](../../owner-decisions-1.0.3-2026-10-21.md) acknowledges the
renewable availability exposure while preserving historical-income isolation.
