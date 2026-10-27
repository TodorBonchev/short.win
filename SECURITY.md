# Security policy

This file, the web app's policy page (`https://short.win/security`), its
`/.well-known/security.txt` (RFC 9116) and the `security.txt` that the program embeds carry
the same values (owner decision of 2026-10-02, M10 Q3). The release gate refuses any
placeholder in the program's copy (`scripts/check-security-txt.py --forbid-placeholders`, CI's
`anchor-build` job).

## Scope

| Component | Identifier |
|---|---|
| short.win program (Solana mainnet) | `5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK` |
| Web app | `https://short.win` |
| Keeper, indexer and API operated by short.win | in scope: reports are welcome and handled like the others |

The program embeds a `security.txt` (`solana-security-txt` 1.1.3). To read it:
`query-security-txt <program id>`. Its `source_release` is the program's package version
(`v1.0.0` at the audit freeze). The `complete_curve` helper of the audited 1.0.0 scope was never
deployed and is retired with pump's v3 exits (design pump-final §11, owner decision D2 of
2026-10-13).

Not in scope:
- the third-party programs short.win calls: pump.fun, PumpSwap, pump-fees, Raydium, SPL
  Token / Token-2022, Pyth and Squads. Report issues in them to their maintainers;
- the TEST-ONLY mock venue under `tests/mock_cpamm`, which is never deployed;
- the issues already listed, with their bounds, in
  [`docs/audit/known-issues.md`](docs/audit/known-issues.md), unless you show that a stated
  bound is wrong or can be exceeded.

## Reporting a vulnerability

- **Contact:** `mailto:security@short.win`. The policy page `https://short.win/security`
  repeats this policy.
- **Encryption:** none published yet. If your report needs an encrypted channel, say so in a
  first short message without the details and we will arrange one.
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

- **Acknowledgement and triage:** best effort. We acknowledge a report as soon as we can and
  keep the reporter informed of the triage and the fix.
- **Fix:** a fix that needs a program upgrade goes through the Governance multisig's
  timelock (`docs/design/ARCHITECTURE.md` §3.1, §8.5). Before the upgrade, the Emergency
  multisig can pause new opens and disable a venue for opens. No admin key can move user
  funds or pause exits.
- **Disclosure:** coordinated, on a best-effort timeline agreed with the reporter. We publish
  an advisory after the fix is deployed, crediting the reporter unless they ask otherwise.
- **Rewards:** no funded bug bounty and no guaranteed reward. Under the owner's
  2026-10-27 decision, the capped mainnet phase comes first; protocol fees are intended
  to accumulate funding for a later bounty and external audit. Publish the funded
  bounty's scope, eligibility and reward terms before representing it as active.
- **Safe harbour:** we will not pursue or support legal action against research done in good
  faith under this policy: research that avoids privacy violations, the loss or destruction of
  data and the interruption or degradation of the service; that never moves or puts at risk
  funds that are not your own (reproduce on a local validator or a LiteSVM fork, never against
  other users on mainnet); that does not exploit an issue beyond what is needed to show it;
  and that reports the issue to us promptly and keeps it confidential until a fix is deployed.

## Audits

No external security audit has been completed or published. The repository includes
AI-assisted reviews and validation records, with their scope and authorship limits
disclosed; these do not constitute an external firm's audit.

**Launch policy (owner decision, 2026-10-27): capped mainnet before an external audit
and a funded bug bounty**, using protocol fees to accumulate funding for both. This
supersedes the 2026-10-02 Q2 requirement to audit before mainnet. The
[1.0.3 mainnet plan](docs/release/MAINNET-1.0.3.md) records required exposure controls
and release/operating checks. This decision alone does not establish deployment,
effective deployment configuration, funding, an audit booking or a bounty reward
commitment. The owner selected the existing `market_max_notional` default of
**25 SOL of aggregate opening notional per market**. Deposits and market count are
uncapped; this is not a TVL, borrower-margin, current-marked-exposure or protocol-wide
cap. No external audit or funded bounty is required before this initial phase.

Reports will be listed on `https://short.win/security` once published. Add the
program's `auditors` field only when an actual auditor and applicable report can be
identified. The review package remains [`docs/AUDIT.md`](docs/AUDIT.md).
