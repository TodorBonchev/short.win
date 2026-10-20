# Full-digest candidate CI evidence — 2026-10-21

The final full CI is [39509811932](https://github.com/TodorBonchev/short.win/actions/runs/39509811932)
on exact test source `ff85a1b0c8452cb9fad32767f7083d22377e90a3`. Its final result and
new-run artifact comparison remain pending at this snapshot. Consult the
[readiness record](../../release-readiness-1.0.3-2026-10-21.md) for subsequent status.

- [Superseded run outcomes](superseded-runs.json) retain job IDs, conclusions and counts.
  Both runs ended cancelled; neither supplies a completed full-CI gate.
- [Superseded e04 verifier comparison](superseded-e04-verifier-comparison.json) and
  [Anchor comparison](superseded-e04-anchor-comparison.json) record actual byte checks
  against the frozen final fuzz inputs. They do not credit the cancelled CI as green.
- [Independent e04-to-ff85 source equivalence](source-equivalence-e04-ff85.json) confirms
  the only tracked change is the strengthened standalone math regression. All 2,064
  other entries, 14 checked production/fuzzer/fixture/config scopes, six recorded build
  inputs and four actual frozen artifacts match. This does not prove CI/fuzz completion.

The accurately labelled AI reviews and owner decisions are separate from execution
results. No deployed program, official firm report or completed 24-hour run is claimed.
