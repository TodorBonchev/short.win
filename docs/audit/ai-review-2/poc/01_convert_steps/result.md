# PoC 01_convert_steps

- Key: `shortwin-math|convert-steps|lock-rounds-down-at-conversion`
- Label: **UNVERIFIED**
- Note: No test ran: a safety classifier stopped the PoC agent before it wrote the test file, so cargo found no test target.

## Result of the PoC run

```text
No output. The test file was not created and no cargo command was run. A safety classifier stopped the agent's response while it was still reading the harness (settle_it.rs, lender_it/common.rs), before it wrote the test.
```

## Faithfulness (the PoC agent's account)

No test exists, so faithfulness was not assessed. Nothing was written under the scratch workspace or the audited tree. The finding keeps its gated confidence (90) and needs a PoC in a later run or a manual check.

## Adversarial review

Verdict: kept. The test file <checkout>/tests/litesvm/tests/poc_01_convert_steps.rs does not exist, there is no shortwin-math variant either, and my re-run of the same command stops before it compiles anything because cargo finds no such test target. So no test was built or run, and nothing confirms or refutes the claim. Reading the code (pool.rs:1243-1248) shows the mismatch the finding describes: convert_shares can round shares up by one, while `l.lock = scale(l.lock)?` floors the lock. But reading code is not a PoC, so the label stays UNVERIFIED.

```text
$ ls .../tests/litesvm/tests/poc_01_convert_steps.rs
ls: cannot access '.../tests/litesvm/tests/poc_01_convert_steps.rs': No such file or directory
$ cd .../poc/repo && TMPDIR=... slot cargo test -p shortwin-litesvm-tests --test poc_01_convert_steps -- --nocapture
error: no test target named `poc_01_convert_steps` in `shortwin-litesvm-tests` package
help: available test targets:
    add_margin_it
    attacks_exit_it
    ... (no poc_* target listed)
EXIT=101
```
