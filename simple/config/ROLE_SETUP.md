# Production role setup

This is a preparation guide for untagged version **1.1.0**. The generated public
program ID is `BCPiRMTMoLNY5EHm2RYxoxJChgyrttkFX2Qe2Rnbe3jh`; its private
deployment key remains outside the repository. Governance, emergency, reporter,
fee recipient and keeper roles remain unassigned. No operating wallet is created,
funded or submitted by these files. Put only public keys in
`production-roles.template.json`. A completed manifest still does not enable
execution or authorize deployment.

| Identity | Proposed custody / purpose |
| --- | --- |
| Program ID | Generated public ID `BCPiRMTMoLNY5EHm2RYxoxJChgyrttkFX2Qe2Rnbe3jh` is the active build/qualification identity for untagged 1.1.0. Preserve its private key outside Git and confirm custody before deployment. The old local placeholder cannot be promoted. |
| Governance / upgrade authority | Offline controlled signer; review the actual canonical loader authority used for initialization. Keep separate from the reporter and emergency role. |
| Emergency | Separate controlled signer for the program's pause powers. |
| Fee recipient | Dedicated public treasury address; no lender funds or collateral counted as protocol revenue. |
| Reporter | Separate VPS operating key signing authenticated references, with explicit funding and publication limits. |
| Keeper | Separate VPS operating key paying transaction fees and temporary deposits; initial native setup allowance is zero. |
| Deployment payer | Record separately in the deployment packet, with the actual program/buffer/loader rent quote. Operating-wallet balances do not cover that deposit. |

If using local Solana keyfiles, prepare them on the machine that will hold each
role, with private backups under the owner's control. For an operating key, run
these commands **locally yourself**, outside the repository. They are examples,
not an instruction to generate offline governance keys on the VPS:

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
receipts. Run the proposal verifier with `--require-assigned-roles`; incomplete
or placeholder identities must fail. Prepare and simulate the exact initialization
packet for owner review after freezing these identities; do not infer approval
to sign it from a completed JSON file.

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
policy, not evidence that execution is ready. The combined qualification campaign
is stopped and its unresolved work remains open; no new 24-hour run or stable tag
is implied by this setup guide.
