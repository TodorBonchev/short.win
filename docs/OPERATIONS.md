# Operating contract

Run all commands from the repository root. The software version is **1.1.1** and the program ID is `BCPiRMTMoLNY5EHm2RYxoxJChgyrttkFX2Qe2Rnbe3jh`. The private deployment key stays outside Git. Mainnet deployment, role assignment and funded operation remain to be commissioned.

## Build and verification

Use the pinned host and JS toolchains, then:

```sh
python3 scripts/materialize-jupiter-probe.py
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
set -o pipefail
mkdir -p target/deploy target/idl
cargo build-sbf --manifest-path programs/shortwin-simple/Cargo.toml --tools-version v1.57 --arch v3 --sbf-out-dir target/deploy -- --locked 2>&1 | tee target/sbf-build.log
python3 scripts/check-sbf-stack.py target/sbf-build.log
cargo test --workspace --locked
python3 -m unittest discover -s services/tests -v
python3 -m unittest discover -s config -p 'test_*.py' -v
pnpm install --frozen-lockfile
pnpm check
pnpm build
```

Capture the SBF build output to `target/sbf-build.log` when running the stack diagnostic gate. CI does this with `tee` and shell pipe failure propagation. Generate IDL with `anchor idl build -p shortwin_simple -o target/idl/shortwin_simple.json`, compare it with `client/idl/shortwin_simple.json`, then run `pnpm --dir client generate`. Verify source, IDL and generated client together. Amounts are integer token units and lamports.

[Runtime qualification](../tests/runtime/README.md) uses actual SBF, captured Jupiter routes and seeded scenario targets. A successful test or software tag alone does not establish deployed operation. Freeze source, program, runner, configuration and corpus identity for each duration campaign, preserve its start/end and exit receipts, and verify result hashes before declaring a pass.

## Roles and operating inputs

Review [the policy and payer proposals](../config/README.md) and [role setup](../config/ROLE_SETUP.md). Keep governance, emergency and reporter identities separate. Confirm private key custody, actual canonical ProgramData upgrade authority, cluster genesis, fee recipient and keeper identities. Only public identities belong in the role manifest.

The initializer must be authorized by the actual canonical ProgramData upgrade authority. Verify Jupiter's reviewed canonical loader tuple, deployment slot, ProgramData length and ELF digest before activation. Upstream bytes changing requires renewed compatibility review.

The immutable financial policy and service model must match byte for byte, including history epochs, timing, freshness, spread and `price_policy_hash`. The proposal verifier checks these bindings; example settings and incomplete role templates remain non-executable review inputs.

Retain actual artifact digests, payer funding, account deposits, transaction budgets, backups and measured provider capacity. Test alerts for heartbeat loss, stale references, queue saturation, low balances, unresolved publication outcomes and RPC/provider failure. Keep automatic funding disabled until a concrete operating plan authorizes it.

## Operator and browser

The trusted history service uses Python standard library and SQLite. The Node-only operator is a bounded one-pass runner:

```sh
pnpm --dir client operator --help
pnpm --dir client server --help
```

Both use `SOLANA_RPC_URL`; Jupiter requests use the optional private `JUPITER_API_KEY`. The operator defaults to dry-run. Actual submission requires explicit execute mode, matching role keyfiles, simulation and reviewed limits. The HTTP bridge does not sign. Do not forward Jupiter setup, cleanup or tip instructions around the protocol handler.

The bridge defaults to `http://127.0.0.1:8787`; Vite development uses `http://127.0.0.1:5173`. Hosted operation requires HTTPS and the exact configured frontend origin, with provider/RPC credentials kept server-side. The wallet interface requires review after simulation. These development defaults do not prove hosted availability.

## Publication and scheduling

Private exact-q observations remain off chain while an unarmed loan is healthy. A public warm request records a publication intent that expires after `epoch_seconds * (history_epochs + 2) + reference_max_age_seconds`. A usable confirmed reference satisfies it. Otherwise a finite candidate may be published once, with fulfillment stored only after confirmation. Publication also supports arming/confirming adverse prices, clearing a recovered arm and maturity buyback. Unknown observations confer no eligibility.

A newer healthy or Unknown private observation vetoes this operator's use of an older adverse on-chain snapshot. Each priced action rereads the confirmed reference, loan and immutable policy. Local candidates never stand in for confirmed publication. Default takes priority when its boundary has arrived and needs no price, mint or route.

A complete finalized same-bank account/clock snapshot, current queue revision and shared exclusive publisher/pass lock govern retirement. Active debt, pending publication, a finite reference fresh in either clock or an unresolved warm intent protects the slot. Re-admission advances the generation and starts cold history. Capacity exhaustion refuses admission; it does not evict protected work. Retiring a monitoring slot does not close a reference account or clear a financial reservation.

Do not overlap operator passes. A timer must let the preceding pass finish. Quote capacity is shared between history and route construction. The Python `capacity` command reports spacing bounds, not measured availability. Commission the actual Free-key/RPC account and one exact quantity before expanding coverage.

## Persistent storage and recovery

The history UUID, operator-store UUID, program ID and normalized database paths are durably bound in both stores and the separate `<history>.publisher-identity.sqlite` anchor. The `<history>.publisher-lock.sqlite` companion remains exclusively locked throughout a pass.

After any history UUID is recorded, missing/replaced files, corrupt identities or mismatched bindings fail the entire pass, including defaults, until reviewed recovery. Do not recreate stores to clear barriers or budgets. Only a fresh first boot with no history UUID ever recorded permits default-only work before later initial binding under the same operator store and companion anchor; it cannot publish, retire history or execute priced actions.

Stop all cooperating processes and exclude overlapping passes/writers before backup. Preserve a consistent set of history, operator, identity and lock databases, including applicable SQLite WAL/SHM/journal files, raw history, generations, publication barriers and fulfillment records. Restore the complete reviewed set.

The initial operator proposal reserves at most four distinct new reference accounts and at most 1,000,000 lamports of publication fees per UTC calendar day. Reservations survive restarts, retirement and re-admission. Unknown submission outcomes retain their allowance. These local budgets require persistent storage; they are not protocol-wide caps or permission for automatic top-ups. Dry-run projects work without consuming new durable allowance.

## Routing and native setup

The quote profile requests ordinary RouteV2, transaction version 0, routing-account budget 16 with one 32-account fallback, zero route fees and `excludeDexes=Scorch`. It bounds returned quote metas to 40 and instruction bytes to 768; the program permits at most 48 metas. These filters do not establish a 1232-byte transaction fit. Resolve actual lookup tables and simulate the complete transaction.

Each sampled ask is an actually quoted budget that guarantees the whole debt within the spread bound. It is not the cheapest ask or a monotonicity claim. The fallback can require six HTTP starts for one history observation, and an execution search has a separate bounded request budget. Measure combined latency on the actual shared provider account.

The actor-signed native route setup allowance is separate from economic trade amounts and is capped at 5,000,000 lamports. The actor advances it plus two scratch-account deposits within the signed temporary-funding cap. Unused setup and scratch deposits return to that actor. Actual setup spend cannot enter opening notional, collateral, fees or keeper reward bases. The allowance bounds outflow, not the purpose of every opaque route recipient.

Setup can recur for each loan and route family. The initial keeper allowance is zero. A positive allowance requires an explicit payer subsidy and net-cost decision; it cannot charge lenders or collateral. Account setup and transaction costs can exceed a small loan's keeper reward. The upfront fee does not establish self-funded operation.
