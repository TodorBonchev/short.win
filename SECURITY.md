# Security policy

> **Draft (M10).** Each `OWNER-FILL` marker is a value the owner has not decided yet (M10
> owner question Q3: domain, security contact, policy, source visibility). This file, the web
> app's `/.well-known/security.txt` (RFC 9116) and the `security.txt` that both programs
> embed must carry the same values. The release gate refuses any placeholder
> (`scripts/check-security-txt.py --forbid-placeholders`; TODO(WP-6)).

## Scope

| Component | Identifier |
|---|---|
| short.win program (Solana mainnet) | `5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK` |
| `complete_curve` helper program (Solana mainnet) | `4B5mGykX9EWwaYdbTvsp8qXCUTbc7caQourz16Gat6LJ` |
| Web app | `https://OWNER-FILL-domain` |
| Keeper, indexer and API operated by short.win | OWNER-FILL (in scope or not) |

From the audit release on, both programs embed a `security.txt` (TODO(WP-6): not in the
programs yet). To read it: `query-security-txt <program id>`.

Not in scope:
- the third-party programs short.win calls: pump.fun, PumpSwap, pump-fees, Raydium, SPL
  Token / Token-2022, Pyth and Squads. Report issues in them to their maintainers;
- the TEST-ONLY mock venue under `tests/mock_cpamm`, which is never deployed;
- the issues already listed, with their bounds, in
  [`docs/audit/known-issues.md`](docs/audit/known-issues.md), unless you show that a stated
  bound is wrong or can be exceeded.

## Reporting a vulnerability

- **Contact:** `mailto:OWNER-FILL-security-mailbox`. A second channel (a bug-bounty platform
  link) is OWNER-FILL.
- **Encryption:** OWNER-FILL (a PGP key or the platform's channel).
- **Language:** English.
- **Do not** open a public issue, pull request or discussion for a vulnerability.

Please include:
- the affected program or component, and the commit or deployed program hash;
- the impact: which funds, or which liveness property (exits, withdrawals, claims), and how
  much;
- a reproducer. A LiteSVM test against `tests/litesvm` is ideal (see `docs/AUDIT.md` §9 for
  the build);
- whether the issue is being exploited, as far as you know.

## What happens next

- **Acknowledgement and triage times:** OWNER-FILL.
- **Fix:** a fix that needs a program upgrade goes through the Governance multisig's
  timelock (`docs/design/ARCHITECTURE.md` §3.1, §8.5). Before the upgrade, the Emergency
  multisig can pause new opens and disable a venue for opens. No admin key can move user
  funds or pause exits.
- **Disclosure:** coordinated. We publish an advisory after the fix is deployed, crediting
  the reporter unless they ask otherwise. The timeline is OWNER-FILL.
- **Rewards:** OWNER-FILL. Either the bug-bounty programme and its terms, or "no bounty yet".
- **Safe harbour:** OWNER-FILL.

## Audits

Audit reports are listed at OWNER-FILL (the `auditors` link of the on-chain `security.txt`).
The audit package for reviewers is [`docs/AUDIT.md`](docs/AUDIT.md).
