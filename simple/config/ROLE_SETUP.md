# Production role setup

This is a preparation guide. Production identities are intentionally unassigned;
no wallet is created, funded or submitted by these files. Put only public keys
in `production-roles.template.json`. A completed manifest still does not enable
execution or authorize deployment.

| Identity | Proposed custody / purpose |
| --- | --- |
| Program ID | Dedicated program key; freeze its public ID before final ELF, IDL and qualification. The local placeholder is rejected by the proposal verifier. |
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
Select governance/emergency/treasury and program custody separately, then record
only their public identities.

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
