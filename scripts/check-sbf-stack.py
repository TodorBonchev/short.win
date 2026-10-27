#!/usr/bin/env python3
"""Fail release qualification on SBF frame diagnostics, even when build exits zero."""
import re
import sys
from pathlib import Path

FRAME_LIMIT = 4096
BAD_DIAGNOSTIC = re.compile(
    r"overflows the maximum allowed frame space|"
    r"stack offset of .+ exceeded max offset|"
    r"stack frame (?:size )?.*(?:exceeds|exceeded|larger than)",
    re.IGNORECASE,
)
ESTIMATED_FRAME = re.compile(r"Estimated function frame size:\s*(\d+)\s*bytes", re.IGNORECASE)


def violations(text):
    result = []
    for number, line in enumerate(text.splitlines(), 1):
        estimate = ESTIMATED_FRAME.search(line)
        if BAD_DIAGNOSTIC.search(line) or (estimate and int(estimate[1]) > FRAME_LIMIT):
            result.append((number, line))
    return result


def main(argv=None):
    paths = (sys.argv[1:] if argv is None else argv)
    if not paths:
        print("usage: check-sbf-stack.py <complete-build-log> [...]", file=sys.stderr)
        return 2
    failed = False
    for name in paths:
        path = Path(name)
        try:
            data = path.read_text(errors="replace")
        except OSError:
            print(f"missing/unreadable SBF build log: {path}", file=sys.stderr)
            return 2
        if not data.strip():
            print(f"empty SBF build log: {path}", file=sys.stderr)
            return 2
        for number, line in violations(data):
            failed = True
            print(f"{path}:{number}: rejected SBF stack diagnostic: {line[:1000]}", file=sys.stderr)
    if failed:
        return 1
    print("ok: no SBF frame/stack overflow diagnostics in supplied build logs")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
