# Production role setup

This is a preparation guide for release candidate **1.1.1**. The generated public
program ID is `BCPiRMTMoLNY5EHm2RYxoxJChgyrttkFX2Qe2Rnbe3jh`; its private
deployment key remains outside the repository. Five distinct governance, emergency,
reporter, fee recipient and keeper public addresses were generated locally on
2026-11-03 at the owner's request and recorded for review in
`production-roles.template.json`, with the canonical expected mainnet genesis.
Private keyfiles remain outside Git. Owner custody review, offline backups,
operating installation and funding remain pending; no deployment or transaction
is implied. The public-assignment gate passes, while execution stays disabled.

| Identity | Proposed custody / purpose |
| --- | --- |
| Program ID | Generated public ID `BCPiRMTMoLNY5EHm2RYxoxJChgyrttkFX2Qe2Rnbe3jh` is the active build/qualification identity for release candidate 1.1.1. Preserve its private key outside Git and confirm custody before deployment. |
| Governance / upgrade authority | Offline controlled signer; review the actual canonical loader authority used for initialization. Keep separate from the reporter and emergency role. |
| Emergency | Separate controlled signer for the program's pause powers. |
| Fee recipient | Dedicated public treasury address; no lender funds or collateral counted as protocol revenue. |
| Reporter | Separate VPS operating key signing authenticated references, with explicit funding and publication limits. |
| Keeper | Separate VPS operating key paying transaction fees and temporary deposits; initial native setup allowance is zero. |
| Deployment payer | Record separately in the deployment packet, with the actual program/buffer/loader rent quote. Operating-wallet balances do not cover that deposit. |

Review custody of the generated keyfiles and protect private backups under the
owner's control before relying on these identities. Future replacement operating
keys can be prepared on the machine that will hold them. These commands are
examples for a separately reviewed replacement, outside the repository; do not
overwrite the current keyfiles or silently replace the public assignments, and
do not generate offline governance keys on the VPS:

```sh
install -d -m 700 /data/shortwin/keys
umask 077
solana-keygen new --silent --outfile /data/shortwin/keys/reporter.json
solana-keygen pubkey /data/shortwin/keys/reporter.json
solana-keygen new --silent --outfile /data/shortwin/keys/keeper.json
solana-keygen pubkey /data/shortwin/keys/keeper.json
```

The verified CLI's `--silent` option suppresses display of the seed phrase. Do
not overwrite an existing keyfile. Complete any passphrase prompt on that local
machine; never send wallet bytes, seed phrases, passwords or API keys in chat or
commit them. Back up the actual protected keyfiles before relying on them.
Select governance/emergency/treasury custody separately and confirm custody of
the existing generated program key; record only public identities. Do not
regenerate the program key to work around a missing file or qualification gate.

Create the **free** Jupiter API key in the owner's account and set
`JUPITER_API_KEY` and `SOLANA_RPC_URL` in a private operator environment, outside
Git and browser configuration. No key is requested in chat. The API budget is
shared; route construction and history use the same SQLite request gate.

Before activation, verify actual mainnet genesis, the canonical program/loader
upgrade authority and the current reviewed Jupiter tuple against retained RPC
receipts. The [2026-11-03 review](jupiter-reviewed-deployment.json) records Jupiter
deployment slot **462802085**, ProgramData length **2892269** and trimmed ELF
SHA-256 `f09c1c20bc5734da6ba759acbb276c36a0b3e82306a60ff4b94341364aa0b895`.
Fresh saved-route actual-SBF qualification passed all 18 expected outcomes for
two separately identified own ELFs, including the exact previously duration-tested
ELF. The post-qualification finalized RPC recheck at
**2026-11-03 09:19:31 Europe/Sofia** observed the same tuple and program bytes.
This dated compatibility gate is complete for those captures; it does not assign
roles, establish funded commissioning, extend the prior duration evidence's scope
or provide external audit approval. A changed upstream deployment requires fresh
compatibility review.

Run the proposal verifier with `--require-assigned-roles`; the generated public
assignments pass this identity check, while incomplete or placeholder identities
must fail. This does not verify private custody, backups, funding or current loader
authority. Prepare and simulate the exact initialization packet for owner review
after freezing these identities; do not infer approval to sign it from a completed
JSON file.

Retain funding receipts for the reporter and keeper, low-balance thresholds,
maximum fee/route setup/temporary advances, delivered alerts and process artifact
hashes. The suggested 0.1 SOL per operating payer is a commissioning budget,
not a statement that funds exist or a deployment-rent estimate. Keep automatic
top-ups disabled until the concrete operating plan is reviewed.

Before any operator pass, retain and verify the complete storage identity set:
the v3 history UUID and immutable operator binding, the operator-store UUID and
program/path binding, and the separate `<history>.publisher-identity.sqlite`
companion anchor. The `<history>.publisher-lock.sqlite` companion must stay
exclusively locked throughout the pass. Use the exact normalized absolute paths
expected by both processes. Once a history UUID is recorded, missing/replaced or
corrupt storage and mismatched bindings intentionally fail the whole pass,
including defaults, until reviewed recovery; never reinitialize to clear them.

Defaults require no price configuration, provider/API, finite observation or
unpaused price service. Only fresh first boot with no history UUID ever recorded
permits default-only work before later initial history binding under the same
operator store and companion anchor. This does not authorize deleting recorded
history or any price-dependent action. For backups, stop all cooperating
processes with no overlapping pass/writer and preserve a consistent set of
history, operator, companion identity and lock databases with applicable SQLite
WAL/SHM/journal files and pending publication/fulfillment records.

A complete finalized same-bank snapshot, current revision and exclusive publisher
lock are required to retire an inactive exact-q history slot. Active debt, pending
publications and finite references still fresh in either clock protect that slot.
Re-admission uses a new generation and cold history. Neither retirement nor a
process restart resets lifetime reference-creation reservations or daily publication
fee budgets; preserving these financial bounds is part of commissioning.

The owner selected borrowing-capped mainnet before the external audit and bug
bounty, with deposits and market count uncapped. This is a disclosed release
policy, not evidence that execution is ready. Fresh evidence must bind the exact operating inputs. This setup guide does not start a campaign or authorize deployment.
