# Supported-path validation evidence

Source candidate: `e04a6776f1c2f5e02e581d76dd1255c416270275`.
The original [execution metadata](validation-evidence.json) records input/source and
raw-log hashes, counts and cache/wrapper limits. Its source paths are the actual local
execution paths; these archives preserve the same log bytes for repository review.
[Archive index](archive-index.json) records compressed and uncompressed hashes.

Logs are gzip-compressed without changing their content, including ANSI sequences and
line endings. To check a raw log against the original metadata, for example:

```sh
gzip -dc m5-gates.log.gz | sha256sum
gzip -dc m6-gates.log.gz | sha256sum
```

The event-tail discovery failure is retained as characterization history; the later
compatibility log records the corrected, explicit prefix/tail comparison. No failed
assertion is counted as a successful run. New and historical behavior profiles remain
distinguished from full ELF identity. Tests use historical state snapshots and the
specified new upstream programs, not a coherent live mainnet bank or verified upstream
source. No budget/result thresholds were increased. Executions overlap and are not
unique-test totals. See the [scoped review](../../../live-preflight-1.0.3-2026-10-21.md).
