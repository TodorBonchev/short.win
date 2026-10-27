# TEST-ONLY isolated scratch SBF execution probe

This standalone crate is excluded from the production Rust workspace and Anchor program list.
Never deploy it. Its one ELF is loaded under two synthetic test IDs: an executor and a
hostile target. Both execute as SBF; no native processor substitutes for the target.
Caller-submitted numerical bounds are deliberately test inputs, not risk authorization.

The first gate proves a BuyAtLeast mechanic using ordinary SPL Token, fresh executor-derived
165-byte scratch accounts and a protected synthetic pool. A program-owned test escrow funds
exact declared inventory plus account rent. Scratch donations are normalized into escrow
before the spend baseline. Only an isolated, read-only PDA authority signs the target CPI;
payer, escrow and final pool credit account never enter that CPI. The shared isolation module
checks actual runtime privilege unions, critical keys, foreign signers, strict token state
and measured scratch deltas. The local probe additionally enforces full net pool credit,
surplus donation, complete scratch closure/rent refunds and actual escrow debit equal to
measured spend. Lower input consumption with full debt delivery is valid and refunded.

Failures after target Token CPIs or after final refunds must roll back every protected account;
transaction payer fees are excluded because a failed landed transaction may still charge fees.
Cases include underdelivery, spending beyond the bounded scratch balance, output delegation or
owner changes, early close/native theft/reentry attempts, critical account aliases, writable
runtime authority/mint unions, foreign signers and dirty/substituted scratch accounts. Some
attempts fail inside the target/runtime, others at executor postconditions; only actual logs
identify which mechanism rejected a case. This is not a blanket proof of all reentrancy paths.

```sh
scripts/build-jupiter-exec-probe.sh
cargo test -p shortwin-litesvm-tests --test jupiter_exec_probe \
  --features jupiter-exec-probe --locked -- --nocapture
```

The artifact defaults to `target/jupiter-exec-probe/deploy/shortwin_jupiter_exec_probe.so`;
`SHORTWIN_JUPITER_EXEC_PROBE_SO` can name that independently retained artifact. The build script
pins platform-tools v1.57 and SBPF v3 and records exact source/artifact hashes, including a dirty
working-tree indicator. `target/deploy/shortwin.so` and the primary checkout are untouched.
The standalone lockfile does not update the root, services or pnpm dependency baselines.

The harness target requires the explicit `jupiter-exec-probe` feature. Default CI does not
execute or credit these tests. The dedicated `jupiter-exec-probe` workflow builds the artifact,
checks test discovery and runs the focused actual SBF tests; retained logs/identity establish
what actually ran. An available build recipe or native unit-test pass alone is not SBF proof.

## Separate pinned Jupiter gate

`scripts/materialize-jupiter-probe.py` reconstructs the candidate-only Program/ProgramData
and ELF from the retained fixed-hash RPC bytes. It makes no network request. The separate
actual-route test loads original loader metadata, current pinned Jupiter code and unchanged
frozen Raydium CPMM state. It constructs exactly one ordinary V2 ExactIn RaydiumCP step, binds
its fixed roles, executes it through the same isolated SBF escrow/scratch/net-credit/refund
path, and requires both Jupiter and CPMM invocation logs. A real routed minimum-output
refusal must occur after a successful inner DEX swap and roll all DEX/protected accounts back.
The bank slot is explicit and later than the pinned deploys; historical pool time/state is
preserved and is not represented as a live routing snapshot. Existing SPL Token uses the
LiteSVM runtime's bundled implementation; no claim that every nested token instruction uses
an independently reproduced Token ELF is made.

External DEX vaults can retain older native reserve values or lamport donations. The probe
validates their identity/base state separately; it never rewrites their frozen bytes or applies
those old values to fresh protocol scratch. Fresh scratch/pool credit/all-in debit checks remain
strict. The route is hardcoded, not a complete arbitrary Swap-enum decoder or production route
admission policy. No Jupiter API, live quote, reporter transaction, pricing policy, general
mainnet route or production handler is qualified here. Shared accounts, ExactOut, venue-side rent, arbitrary setup/cleanup, hosted
orders, partial debt and Token-2022 scratch extensions are outside this first probe scope.
