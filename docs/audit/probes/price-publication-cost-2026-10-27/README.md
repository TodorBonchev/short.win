# Price publication packet and base-fee illustration — 2026-10-27

This target-only experiment uses the current generated `getPublishPriceReportInstruction` builder and Solana Kit 8.3.0, running under Node v24.21.0. It compiles and serializes unsigned v0 transactions. It does not generate keys, sign, call RPC, simulate, submit transactions, initialize accounts or execute the program. Addresses, report values, blockhash, epochs and compute settings are synthetic. All transaction signatures remain null and the wire bytes contain zero signature placeholders.

The assumed two reports share a market/configuration and use distinct, already-initialized exact-q report accounts. Their publisher is also the fee payer and sole transaction signer. Four baseline cases have no address lookup table. A fifth case uses an in-memory, **non-existent synthetic ALT** containing configuration, market and both report addresses; the signer, short.win program ID and ComputeBudget program ID remain static. No ALT account was fetched, created or changed. No case uses a nonce, signature-verification precompile or account-creation instruction. Each current Finite publication payload is **457 bytes**.

| Finite reports | ComputeBudget limit + price instructions | Serialized v0 packet | Remaining against 1,232 bytes | Fits |
| --- | --- | ---: | ---: | --- |
| 1 | None | 729 bytes | 503 bytes | Yes |
| 1 | Both | 781 bytes | 451 bytes | Yes |
| 2 | None | 1,226 bytes | 6 bytes | Yes |
| 2 | Both | 1,278 bytes | −46 bytes | No |
| 2 + synthetic four-address ALT | Both | 1,188 bytes | 44 bytes | Yes, structurally |

ComputeBudget values of 200,000 CU and 1,000 micro-lamports/CU are illustrative fixed-width encodings. Their values are neither approved production settings nor measured sufficient limits. This work does not estimate execution CU or qualify publication landing. The normal CU limit and price instructions add 52 bytes here, including their program key. The second report itself adds 497 bytes. Two reports are consequently the largest batch fitting this current no-ALT layout, and its six-byte headroom cannot accommodate the normal pair of ComputeBudget instructions. Extra signers, keys or instructions can also invalidate that fit. The synthetic ALT variant demonstrates a packet-size path at **1,188 bytes**, retaining both normal CU instructions and 44 bytes of headroom. Its two writable and two readonly lookup indices load four accounts; three keys remain static. This does not establish that an ALT exists, nor approve its creation, funding, authority, extension, activation, deactivation, closure or lifecycle. Those and any different payload/layout need separate design and qualification. Three Finite payloads already total 1,371 data bytes before any transaction overhead, so three cannot fit this v0 packet limit even with an ALT.

## Exact 30-day base-fee lower bounds

Solana's official documentation currently states a **5,000-lamport base fee per transaction signature**, separately from optional priority fees. Transactions that execute unsuccessfully still pay fees. The transaction documentation gives a **1,232-byte limit for legacy/v0 packets**. These statements were checked on 2026-10-27 against [Solana fee structure](https://solana.com/docs/core/fees/fee-structure) and [Solana transactions](https://solana.com/docs/core/transactions). The experiment remains v0; it does not rely on the newer v1 format.

Thirty consecutive 24-hour days contain 2,592,000 seconds. A steady two-second schedule has 1,296,000 publication updates per feed, or 2,592,000 report instructions across two feeds.

| Schedule | Transactions / 30 days | Transaction signatures | Base fees in lamports | Base fees in SOL |
| --- | ---: | ---: | ---: | ---: |
| Two separate one-report transactions every two seconds | 2,592,000 | 2,592,000 | 12,960,000,000 | **12.96 SOL** |
| One two-report batch every two seconds, if qualified | 1,296,000 | 1,296,000 | 6,480,000,000 | **6.48 SOL** |

These are base-fee lower bounds for the stated steady schedule, assuming one signature per transaction. They exclude priority fees, failed/new fee-paying attempts, initialization and account rent, additional signers or verification precompiles, other keeper/indexer actions, RPC/API credits and price-service/hosting charges. They do not estimate SOL/USD prices. The conditional 6.48-SOL case is not an approved batching plan: the current no-ALT packet fails with the normal ComputeBudget pair; the synthetic ALT shows structural fit but no usable ALT/lifecycle, observation cadence, CU consumption, execution, confirmation, source-window freshness or same-epoch landing was tested. Both reports must also be ready together for a valid batch. Atomic batching couples their success/failure. The arithmetic assumes a 100% publication duty cycle for the full 30 days. A lower duty cycle must be costed from its actual transaction count and reviewed against required fresh-history warm-up, outstanding-debt coverage and exit availability; it is not assumed to preserve the same safety or availability.

## Effect on the pending proposal

**The proposal and final-settings documentation need an explicit SOL operating-cost and cadence caveat.** The previously approved approximately $245.20/month hosting/API baseline excludes SOL transaction fees. It therefore does not approve the additional 12.96-SOL/month separate-publication base-fee floor, or the conditional 6.48-SOL batching floor. A funded payer, actual CU/priority-fee/landing measurements, batching design and contingency headroom must be reviewed before approving the production cadence. This experiment selects no economic default, changes no pending owner decision and authorizes no deployment.

## Reproduction and retained evidence

From the repository root, use the existing Linux Node runtime:

```sh
/root/.local/share/mise/installs/node/24.21.0/bin/node target/price-publication-cost-2026-10-27/measure.mts
python3 target/price-publication-cost-2026-10-27/verify_wire.py
```

`results.json` records the measured source head, generated-source/lockfile hashes, synthetic assumptions, packet lengths/hashes, exact integer fee arithmetic and official-source URLs. Five `.unsigned.bin` files retain exact wire bytes. `verify_wire.py` independently parses the shortvec/v0 wire format without Kit and verifies the counts, finite discriminator, null-signature placeholders, lookup indices, packet hashes and fee arithmetic; its result is `wire-check.json`. `SHA256SUMS` covers the retained files except itself.
