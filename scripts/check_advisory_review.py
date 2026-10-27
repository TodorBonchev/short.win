#!/usr/bin/env python3
"""Fail CI on expired dependency review or changed exception metadata."""

from datetime import date, datetime, timezone
from pathlib import Path
import tomllib

ROOT = Path(__file__).resolve().parent.parent
REVIEW_DUE = date(2026, 11, 27)
REVIEW = "dependency-advisories-2026-10-27.md"
SCOPED = {
    "deny.toml": {"RUSTSEC-2026-0385", "RUSTSEC-2026-0387"},
    "services/deny.toml": {
        "RUSTSEC-2026-0387", "RUSTSEC-2026-0391", "RUSTSEC-2026-0398",
        "RUSTSEC-2026-0403", "RUSTSEC-2026-0410",
        "RUSTSEC-2026-0395", "RUSTSEC-2026-0396", "RUSTSEC-2026-0397",
        "RUSTSEC-2026-0399", "RUSTSEC-2026-0400", "RUSTSEC-2026-0401",
        "RUSTSEC-2026-0402", "RUSTSEC-2026-0404", "RUSTSEC-2026-0405",
        "RUSTSEC-2026-0406",
    },
}


def check(today=None):
    today = today or datetime.now(timezone.utc).date()
    active = []
    for policy, ids in SCOPED.items():
        entries = tomllib.loads((ROOT / policy).read_text())["advisories"]["ignore"]
        reasons = {entry["id"]: entry.get("reason", "") for entry in entries}
        for advisory in sorted(ids.intersection(reasons)):
            active.append((policy, advisory))
            reason = reasons[advisory]
            if REVIEW not in reason or str(REVIEW_DUE) not in reason:
                raise SystemExit(f"{policy}: missing dated nonapplicability reason for {advisory}")
        if policy.startswith("services/") and "RUSTSEC-2026-0385" in reasons:
            raise SystemExit("Services ctutils requires its patched version, not the root unkeyed-operation exception.")
    if active and today >= REVIEW_DUE:
        raise SystemExit(f"Dependency review expired {REVIEW_DUE} UTC: remove exceptions or retain a fresh scoped review ({REVIEW}).")
    if not active:
        return "No dated dependency exceptions remain."
    return f"Scoped dependency review current; expires {REVIEW_DUE} UTC. Caller/feature/target changes still require review."


if __name__ == "__main__":
    print(check())
