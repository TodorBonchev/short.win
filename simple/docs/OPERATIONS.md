# New pooled-version operating contract

Use the `simple/` workspace and its generated client only. Legacy keeper, indexer projections, frontend and governance builders use different accounts and must not operate this new program.

## Local verification

```sh
python3 scripts/materialize-jupiter-probe.py
cd simple
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build-sbf --manifest-path programs/shortwin-simple/Cargo.toml --tools-version v1.57 --arch v3 --sbf-out-dir target/deploy -- --locked
cargo test --workspace --locked
python3 -m unittest discover -s services/tests -v
pnpm install --frozen-lockfile
pnpm check
pnpm build
```

Generate IDL with `anchor idl build -p shortwin_simple -o target/idl/shortwin_simple.json`, then copy it to `client/idl/shortwin_simple.json` and run `pnpm --dir client generate`. Verify source/IDL/client together; generated wire contract tests compare exact Rust bytes, including values above JavaScript's safe integer range. Use raw integer token units and lamports throughout.

## Operator integration still to commission

The initial service uses Python stdlib/SQLite and Node 24, avoiding the old Postgres, WASM math and per-venue services. It fits a small development VPS; real sizing depends on active distinct quantities and Jupiter/RPC budgets. No VPS or provider account is provisioned by this implementation, and no paid service is assumed.

1. Select the new program key and governance, emergency, reporter, fee-recipient and keeper roles. The initializer must be authorized by the actual canonical ProgramData upgrade authority. Keep reporter and governance roles distinct. Review the exact Jupiter loader instance/digest before activation.
2. Select and review the immutable financial policy and history settings together. The service model hash and policy price_policy_hash must match byte for byte; epoch counts, timing, freshness and spread settings must also match. Example/test settings are not a signed production configuration.
3. Feed a complete authoritative active-loan snapshot to the service queue, retain exact-q warm-up requests, and prioritize active debt. Capacity exhaustion or provider outage is unavailable coverage, not evidence of a healthy price. Observe all required history epochs before publishing a finite reference for a new quantity.
4. Connect the generated publish/arm/force/default instructions to a bounded, funded runner. Publication uses current on-chain sequence+reporter epoch, actual RPC clock, retained local history evidence and simulation. Keeper action planning must reread the loan and immutable policy, choose the deadline/default path first, and submit a fresh simulated route for forced buyback. Do not forward Jupiter setup, cleanup or tip instructions around the protocol handler. The Node-only operator implements this bounded loop; dry-run is the default and explicit execute mode requires role keyfiles. Its funded automatic operation still needs to be commissioned and tested end to end.
5. Record the actual running client/indexer/reporter/keeper artifact digests, RPC genesis hash and program identity. Fund rent, reporter updates, keeper temporary rent, priority fees and retries with explicit budgets. Add independent heartbeat, stale-reference, queue saturation, payer-low, pending-expiry and RPC/provider outage alerts and test their delivery. Code tests do not prove these operating conditions.
6. Test additional current Jupiter route families, transaction lookup-table/size constraints, current external program baselines, provider API behavior and the complete automatic onboarding cycle on the exact candidate. Finalize and freeze runtime/fixtures; run fresh whole-system invariant fuzz for 24 hours with readable summary, zero anomalies/failures and retained hashes. The bounded pure-ledger model exercise is not that run.
7. Present the reviewed candidate commit, reproducible program hash, concrete parameter file, cost/coverage plan and operating receipts for the owner's stable-tag/deployment decision. Mainnet before external audit/bounty remains the owner's policy, with the limitations and trust terms disclosed. Never mark missing operating evidence passed.

The minimal web UI defaults to a local RPC and supports wallet review after simulation. It is a development interface, not proof of hosted mainnet availability. Do not point the legacy web release at the new program.

## CLI components

Run the trusted service from the repository checkout so its fixed script path and model settings remain attributable. `pnpm --dir client operator --help` lists the one-pass runner, and `pnpm --dir client server --help` lists the optional loopback HTTP bridge. Both need `SOLANA_RPC_URL`; the bridge does not sign. The operator defaults to dry-run and requires an explicit execute flag and matching role keyfiles for actual submission. Do not pass seed phrases, API keys or wallet bytes in command arguments or commit them.

The browser's service URL defaults to `http://127.0.0.1:8787`; Vite's development origin is `http://127.0.0.1:5173`. For a hosted deployment use HTTPS and the configured exact frontend origin, keep RPC/provider credentials server-side, and put any process supervisor/rotation/alert configuration under the operator's concrete deployment review. Do not start overlapping operator passes. An inactive one-pass timer must allow each prior pass to finish.

The Python `capacity` command reports a spacing-only request-budget bound and explicitly leaves measured coverage pending. Free API access can support only a small number of distinct quantities at a time. The example uses one queued exact quantity and conservative spacing; keyless access requires slower spacing. Increasing queue length alone does not increase quote capacity. Public requests are rate/size bounded, active debt is prioritized, and missing coverage yields Unknown. Automatic admission means no manual per-token approval, not unlimited instant quote availability.

## New fuzz harness

`cargo run -p shortwin-simple-runtime-tests --example program_fuzz -- --help` describes the actual-SBF runner. Use `--cases 32` for a bounded smoke. The mutually exclusive duration mode is available for a separately approved/frozen campaign; do not infer 24-hour qualification from a short smoke or from the pure-core model runner.

The harness freezes its native executable, new program, source and fixture bytes and records actual scenario/operation counts, wall time, readable summary, exact log hashes, and completion data. Review [the harness documentation](../tests/runtime/README.md) for its limits and replay contract. A long qualifying record must pass duration, zero anomalies/failures/setup/invariant failures, required real scenario coverage, unchanged frozen inputs and summary/log checks. Finish in-flight work after the deadline, and preserve partial/error evidence instead of silently restarting.
