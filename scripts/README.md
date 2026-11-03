# Repository tools

Run from the repository root. Capture/discovery commands read public network data;
materialization, replay and release preparation use saved inputs. None of these
tools deploys the mainnet program. Generated files belong under ignored `target/`.

| Tool | Maintained purpose |
| --- | --- |
| `capture-jupiter-deployment.py` | Save canonical finalized Program, ProgramData, Clock and raw RPC evidence. |
| `capture-current-route-matrix.py` | Capture each recipe in a dated route matrix, with explicit resume support. |
| `qualify-current-routes.py` | Collect a route's raw provider/RPC evidence, accounts and actual lookup tables. |
| `discover-pump-candidates.py` | Discover physically active Pump curve/AMM inventory for the Token-2022 fixture requirement. |
| `archive-route-qualification.py` | Retain route evidence with deterministic compression and file hashes. |
| `materialize-jupiter-probe.py` | Reconstruct the hash-pinned historical witness used by runtime tests. |
| `materialize-current-route-candidate.py` | Reconstruct one captured route candidate and bind decoded accounts to raw evidence. |
| `materialize-reviewed-route-candidate.py` | Bind a candidate to a dated reviewed router; historical overlays require an explicit flag. |
| `qualify-current-route-suite.py` | Replay the historical regression matrix with actual SBF. |
| `qualify-reviewed-route-suite.py` | Replay the fresh dated matrix, refusals, packet and conservation gates for explicit own ELFs. |
| `check-sbf-stack.py` | Reject stack warnings and build failures from the captured SBF build log. |
| `build-public-web.mjs` | Build and inspect the recorded public showcase before copying verified media. |
| `prepare-mainnet-release.py` | Freeze the exact duration-qualified ELF and public operating proposals into a non-executable packet. |
| `local-fork/` | [Resume/reset and run the disclosed local historical demo](local-fork/README.md). |

Historical snapshots, negative cases, the generated SDK and public parser variants
remain active regression or public API inputs. The `simple` program/package names
are current protocol identities; the former `simple/` directory has been removed.

See the [runtime guide](../tests/runtime/README.md) for replay commands and evidence
limits, and the [mainnet guide](../docs/MAINNET_RELEASE.md) for release gates.
