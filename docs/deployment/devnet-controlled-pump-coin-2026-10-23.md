# Controlled Pump coin for the first devnet market smoke — 2026-10-23

This is a **devnet-only, no-value test coin**, not a public investment asset. Its purpose is to give the test lender a real SOL-quoted Pump curve token for ShortWin's first market and for an account-complete venue CPI smoke. The mint keypair is in a mode-`0600` file outside Git. No creation or buy transaction was sent when this packet was prepared.

## Pinned packet and controls

The [CreateV2 script](../../apps/web/scripts/devnet-create-pump-coin.ts) uses the 16 fixed accounts and discriminator from [Pump's published coin-creation guide](https://github.com/pump-fun/pump-public-docs/blob/639b87fb45c3adeffa6f7c4a1bf0cb85e392bf22/docs/instructions/COIN_CREATION.md). It creates a Token-2022, SOL-quoted coin with `mayhem=false`; the three optional trailing flags are omitted, which decodes as cashback off, creator fee override zero and holder rewards off. The [BuyV3 script](../../apps/web/scripts/devnet-buy-pump-coin.ts) uses [Pump's published 17-account v3 trade interface](https://github.com/pump-fun/pump-public-docs/blob/639b87fb45c3adeffa6f7c4a1bf0cb85e392bf22/docs/instructions/TRADE_V3.md), with a Token-2022 buyer ATA created idempotently in the same transaction. Both refer to upstream IDL SHA-256 `7616681ea557975f9f623ac211080f22a93060781eaf4765b0a488d980997207`.

Both scripts fix the public devnet RPC and genesis hash, reject a changed Pump ProgramData deploy slot, check account owners and local key-file mode, print the transaction roles and caps, and run unsigned `simulateTransaction` before any signing. They sign and send only with an explicit `--send` flag. CreateV2 limits payer debit to at most 0.3 devnet SOL by default; BuyV3 requires an explicit trade cap at or below 0.4 SOL and an overall payer-debit cap at or below 0.5 SOL. A failed or missing simulation debit refuses signing. The buy also refuses an unexpected creator, a completed/mayhem/cashback/non-SOL curve, or a purchase of at least one-quarter of the remaining real tokens.

The metadata lives in [the versioned public JSON](../static/shortwin-devnet-smoke-token-2026-10-23.json). The CreateV2 script's default URI is its `raw.githubusercontent.com` URL on `main`; `--send` refuses until that URL returns matching name/symbol and a devnet-only description. Publish that JSON to `main` **before** creating the coin. The browser app does not need a new asset.

## Prepared addresses and unsigned simulation

| Role | Address |
|---|---|
| Deployment payer / coin creator | `3JfrUHa1RihL7rgJDWSmdV9DNGXkwKAUAcRDBh75cS14` |
| New test mint | `GCLN3PspeYrV3U9BZeGbmn7VVf7kabW23h2gBHvoKRXD` |
| New Pump bonding curve | `JDWyEkPqDxBPPJ7wKY6kNSpx7nhAz2GS9ifVXjfBdDtQ` |
| Later inventory buyer / lender | `APCRy7175QKaNK7YM8QcvxTFd7npQQPjwfKpnzqRdfN2` |

The current devnet Pump `CreateV2` **unsigned** simulation returned no error at genesis `EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG`: 102,127 CU, 896 transaction bytes, and **6,009,480 lamports** simulated payer debit. The configured maximum is 300,000,000 lamports. The simulation supplied an absent mint and curve, and the current Pump ProgramData slot was `510657843`. No key signed the simulation. This proves only coin creation under current devnet state. BuyV3 requires the new curve to exist, so it cannot be simulated until CreateV2 confirms.

Run the same pre-send CreateV2 check from `apps/web`:

```sh
NO_DNA=1 node scripts/devnet-create-pump-coin.ts \
  --payer-keypair /home/todor_dev/.config/shortwin/devnet/release-testing-2026-10-22.json \
  --mint-keypair /home/todor_dev/.local/state/shortwin-devnet-1.0.3/secrets/pump-smoke-mint-2026-10-23.json
```

After the metadata URL is live and the packet is reviewed, adding `--send` sends that same scoped create transaction. It must not be rerun after the mint exists. Once the separate lender wallet is funded, run the bounded inventory buy **without** `--send` first:

```sh
NO_DNA=1 node scripts/devnet-buy-pump-coin.ts \
  --buyer-keypair /home/todor_dev/.local/state/shortwin-devnet-1.0.3/secrets/lender-smoke.json \
  --mint GCLN3PspeYrV3U9BZeGbmn7VVf7kabW23h2gBHvoKRXD \
  --expected-creator 3JfrUHa1RihL7rgJDWSmdV9DNGXkwKAUAcRDBh75cS14 \
  --token-amount-raw 10000000000000 \
  --max-sol-cost-lamports 400000000 \
  --max-payer-debit-lamports 500000000
```

Ten million whole tokens at six decimals is a **target**, not a quote. If the live simulation exceeds the cap or fails, reduce the token amount and re-simulate; do not lift the caps to make a failing transaction pass. Only a successful reviewed BuyV3 simulation should be rerun with `--send`. Then inspect the lender's Token-2022 ATA balance and the live curve reserves before the ShortWin market packet. This coin and one buy still do not establish ShortWin's CPI compatibility, keeper coverage or an open/close smoke.
