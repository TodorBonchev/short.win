# Independent AI review of the route and operating changes

This is a separate-agent engineering review, not a report from an external
audit firm. It reviews the working changes to the new pooled program, isolated
Jupiter CPI surface, metadata-only mint admission, service/client route profile,
publication budgets and archived current-route replay suite. Exact reviewed file
hashes are recorded in `independent-review-files.json`; subsequent changes need
an appropriate follow-up. This review is additional evidence, not a replacement
for final CI, current-byte route replays, sustained operation or frozen fuzz.

## Financial boundary

The writable-authority integration is limited to the derived, System-owned,
empty loan-local execution PDA. The strict isolation function remains strict by
default. Other signers, protocol-owned/forbidden financial accounts, writable
mint/program identities and inconsistent duplicate privileges remain rejected.
Writable authority prefunding is normalized into the loan and recorded as
recovered prefunding, separately from trade proceeds. The PDA must retain its
owner, empty data and exact post-normalization lamport baseline after both the
Jupiter CPI and scratch-account closure.

The existing independent balance checks still require exact pool token debit on
sale, all debt tokens newly credited on buyback, complete scratch closure,
unchanged vault metadata/rent and the exact loan-lamport delta attributable to
measured WSOL flow and recovered prefunding. Allowing a writable authority does
not itself permit a route to charge protocol-owned financial accounts or retain
SOL on that authority. Failing any postcondition rolls back the instruction.
No blocking accounting regression was identified in this bounded static review.
Actual final-byte replay evidence is required in addition to these source
checks; earlier read-only or differently hashed executions do not suffice.

The follow-up native setup escrow was reviewed separately. Its signed route
allowance is capped at five million lamports. The opening borrower or exiting
actor first advances both scratch rent and that allowance; only actual net
setup consumption is charged to that actor, while unused setup and scratch rent
are returned. `settle_setup` requires the derived authority to remain empty and
System-owned, measures unused funds against its normalized baseline, refunds
them and restores that baseline. Exact loan-lamport equations include measured
setup consumption separately from WSOL trade flow. Thus setup cost does not
reduce lender principal, reported sale proceeds or the borrower's economic
collateral entitlement. The operator must reserve this potential permanent cost
as well as temporary peak funding. Setup may recur per loan and venue, and a
forced keeper's setup cost may exceed its reward; it is not assumed reimbursed
by protocol fees.

## Mint admission

The base mint is decoded with the pinned SPL decoder, remains initialized and
has no freeze authority. Plain SPL mints retain their exact allocation rule.
Token-2022 admission is capped in size and permits only MetadataPointer and
TokenMetadata. The entire bounded TLV area is checked, duplicates and unknown or
behavior-changing extensions fail, and nonzero bytes cannot be hidden behind an
empty TLV. Embedded metadata uses a fully consumed bounded Borsh/UTF-8 shape and
must name the actual mint. Mutable metadata authorities and display strings do
not supply protocol identity, balances or prices. This extension set does not
require extending the plain 165-byte protocol token accounts.

## Oracle, client and operating limits

The service model hash covers the deterministic 16/32 request fallback, Scorch
exclusion, ordinary zero-fee V2 header, stricter 40-meta service limit and retained
bounded ask rule. It checks exact quantities, domains, scalar headers, threshold
rounding and route limits before recording a finite price. Provider transport,
authentication and rate-limit failures do not multiply into fallback requests.
An actual successful buy must guarantee all q tokens within the live spread cap;
interpolation never fabricates an executable ask. Since settings cap spread at
100%, an observation starts at least at bid and needs at most one growth probe
to the cap: six HTTP calls including both account budgets is a valid worst-case
spacing bound. It remains a lower bound on operational capacity, not measured
provider latency or guaranteed continuous coverage.

The review requested explicit SDK validation that returned raw and header
slippage match the requested value and that returned labels honor the Scorch
exclusion. Both checks were added and the revised source was reviewed. The SDK
also caps the signed setup allowance and explicitly marks the derived authority
writable when funded. The operator reserves worst-case setup consumption as
potential permanent spend as well as checking the temporary peak advance.

Reference account allocations and UTC-day publication fees are reserved
persistently before submission, including unknown outcomes. Dry runs project
costs without treating simulated publication as confirmed. Healthy private
history does not trigger constant paid publication; finite snapshots are sent
when an opening request or a pending loan action needs them. Financial actions
use a reread confirmed reference and must agree with the fresh private decision;
deadline default remains independent of a price publication. These limits depend
on the documented single supervisor and preserved operator database. They do
not prove an actual funded payer or working alert destination. The proposed
keeper setup allowance remains zero until its funding and subsidy terms are
reviewed. A positive-allowance replay is not approval to spend keeper funds.

## Replay and evidence integrity

The suite materializes accounts from exact timestamped raw RPC replies and
checks decoded bytes against their manifests. Positive cases require the newly
built ELF hash, successful execution and actual retained-lookup-table packet fit.
Scorch negative cases require the specific downstream error and trace, not any
failure exit. A 48-meta envelope is not a claim that the full outer transaction
fits 1,232 bytes; the oversized mixed RAY witness remains a negative observation.
All eight archived full snapshots reconstructed independently, and every stored
and decompressed raw hash in the capture index was verified.

The final suite was independently checked against its exact report and log
hashes: program `8c4ad9af77afa32ae09ae48f5fdcfce868bd3759c96affac975130c63781a00d`,
IDL `9b262cb8ddfe2d55ada06c31aa11049044515c9a6b85324b6f2167b17c61cc26`,
native runner `051866471c8c59a11a2e5c9d762b78adda3d6d35759749c1039b7ea7dd05ff69`.
All six positive cases passed with actual lookup tables and packet fit; both
specific Scorch failures matched their pinned negative expectations. The final
functional spread cap is 500 bps, while other functional policy terms and H2
history remain explicitly synthetic. Pump opening setup was actor-paid; all
three same-loan whole-q buybacks passed with zero setup cost. Four setup funding
rejections, post-CPI rollback and native/token conservation checks were retained.
No additional blocking issue was found in this bounded review.

No official-firm attribution, universal token/venue coverage, live transaction,
deployment, funded operation or 24-hour qualification follows from this review.
