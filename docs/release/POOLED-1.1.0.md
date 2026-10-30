# short.win pooled v1.1.0 software release

The owner authorized the **v1.1.0 software tag** and consolidation into sole branch
`main` on 30 October 2026. The active implementation is `simple/`; the repository's
legacy program, services and dated release records remain historical. This source
release is prepared for operational commissioning. It does not authorize mainnet
deployment, initialization, governance/financial transactions or unrestricted use.

## Qualified software identity

- Program ID: `BCPiRMTMoLNY5EHm2RYxoxJChgyrttkFX2Qe2Rnbe3jh`.
- Program ELF SHA256: `d4868d29c14f4a04abe1cdead451f197b2102286fec10abe34161d387f66c510`.
- IDL SHA256: `52bdccd8a9f29739885e95a4c4595e70067edf3572e9d60c23aa69741b6abc15`.
- Frozen financial source: `0f4871c7f90a4f1c4ddbc6ca14e9cc7821e4e76d`.
- [Qualified runtime CI 40801471141](https://github.com/TodorBonchev/short.win/actions/runs/40801471141),
  [frozen-source CI 40840794732](https://github.com/TodorBonchev/short.win/actions/runs/40840794732)
  and [completion-record CI 41026519920](https://github.com/TodorBonchev/short.win/actions/runs/41026519920)
  passed. The final tag annotation must also identify the completed CI for its exact
  consolidated commit and independently checked ELF/IDL bytes.

The [24-hour financial campaign](../audit/simple-financial-campaign-2026-10-29.md)
completed 124,584 cases, all eight scenario families, in 86,400.558399631 runner
seconds and 86,407.00566560202 supervisor seconds. Exit was zero, with zero failures,
anomalies, setup or invariant failures and matching frozen/final hashes. Exact
[final evidence](../audit/simple-financial-campaign-evidence-2026-10-29/final/verification.json)
and the independent AI review are retained. Its scope is historical CPMM actual
SBF, synthetic clocks/history/funding and one loan per fresh VM. Six positive/two
expected-negative current routes and 42 service, six configuration and 39 client/
operator tests remain separate bounded evidence. No 24-hour current Pump, queue,
real-provider-history, arbitrary multi-loan or universal Jupiter coverage is claimed.

Only documentation, evidence, CI branch routing and historical merge ancestry
change during this release consolidation. Program, client, policy, service and
fixture bytes remain qualified; no duration transfers to changed runtime inputs.
The internal route-helper crate/schema versions are not software release numbers.

## Branch consolidation

[The branch decision record](branch-consolidation-1.1.0-2026-10-30.json) records all
original refs, the verified private local Git bundle and independent dispositions.
Most branch work was already included. Four unique legacy commits are preserved
as historical merge parents: the older cap patch is already covered by pooled
notional logic; legacy no-route settlement and PumpSwap-cache patches do not fit
this independent ledger/operator. Applicable dependency updates are already in
current lockfiles. Preserving history does not apply or qualify those legacy fixes
for pooled code. In particular, current-holder default allocation remains an
economic property, not fixed original-lender cohort protection.

After checking the consolidated commit, fast-forward `main`, retain all branch
history, verify the tag and remove the redundant branch refs. Keep existing tags.
Old worktree directories and untracked files remain in place at detached commits;
the primary checkout's protected `fork_seed.rs` is preserved. Automatic pooled CI
follows `main`; the legacy CI/probe workflows remain available by manual dispatch only.

## Mainnet commissioning and deployment plan

1. Back up and verify custody of the private program key outside Git. Assign and
   review public upgrade/governance, emergency, reporter, treasury and keeper roles,
   actual RPC genesis and canonical Jupiter loader/ProgramData identities. The
   committed templates remain non-executable and unassigned until reviewed.
2. Commission the actual indexer, history/reporter, operator/keeper and client/web
   artifacts. Record their digests, measure free Jupiter-key/RPC capacity, fund
   bounded payer rent/fees/retries/reserves and verify fresh history, queue admission/
   reuse, outage/restart and database backup/restore. Code tests do not prove funding
   or live capacity. Deliver test alerts for outages, stale references, queue limits,
   low funds and pending work before activation.
3. Recheck upstream program identities and current routes at deployment. Changed
   upstream bytes require their compatibility review; changed pump-fees code needs
   renewed authority/account analysis. Changed pooled program/service/policy/fixture
   inputs need appropriate fresh qualification, including a new qualifying duration
   campaign where the frozen program/fuzzer baseline changes.
4. Present exact tag/commit, ELF/IDL hashes, actual public configuration, measured
   coverage, funded payer budgets and delivered operating receipts for the owner's
   separate deployment/activation decision. Then prepare and simulate the bounded
   deployment/initialization plan; no transaction is sent by tagging this release.
5. After explicit deployment approval, deploy the canonical bytes, independently
   verify chain configuration and running services, and enable only the reviewed
   operating policy. Never repoint `v1.1.0`; later source fixes require a new version
   and appropriate evidence.

The owner's policy remains an initial borrowing-capped mainnet before an external
audit and funded bug bounty, with fees intended to accumulate those budgets.
Deposits and market count remain uncapped; these are not TVL/global exposure caps.
Trusted in-house history and the disclosed economic/default risks remain. Neither
an official external audit nor funded bounty, funded operation or mainnet launch
is claimed by this software tag.
