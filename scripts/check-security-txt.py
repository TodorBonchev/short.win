#!/usr/bin/env python3
"""Check the security.txt V1 blob of a release program ELF (ARCHITECTURE §8.5, M10 SECTXT-1).

Both deployed programs embed `solana_security_txt::security_txt!` (neodyme-labs/solana-security-txt
=1.1.3; `programs/{shortwin,complete_curve}/src/lib.rs`), which Solana Explorer and
`query-security-txt` read. This mirrors the crate's own parser (`find_and_parse`, parser.rs) and the
Explorer's: the first BEGIN marker, the first END marker after it, NUL-separated key/value pairs, the
four required keys, no unknown or duplicate key, typed contacts. It also requires exactly one BEGIN
marker (the crate's README: "the contract should not include the security.txt markers anywhere
else").

    python3 scripts/check-security-txt.py target/deploy/shortwin.so "short.win"
    python3 scripts/check-security-txt.py --forbid-placeholders target/deploy/complete_curve.so \
        "short.win complete_curve"

The owner fields hold `OWNER-FILL` placeholders until the public identity is decided (M10 owner
question Q3): without `--forbid-placeholders` they are reported, not refused, so every build stays
checkable. `--forbid-placeholders` is the release gate (the audit freeze and the mainnet deploy).

Exit status: 0 valid, 1 invalid, 2 usage.
"""
import argparse
import sys

BEGIN = b"=======BEGIN SECURITY.TXT V1=======\0"
END = b"=======END SECURITY.TXT V1=======\0"
REQUIRED = ("name", "project_url", "contacts", "policy")
OPTIONAL = ("preferred_languages", "encryption", "source_code", "source_release",
            "source_revision", "auditors", "acknowledgements", "expiry")
CONTACT_TYPES = ("email", "discord", "telegram", "twitter", "link", "other")
PLACEHOLDER = "OWNER-FILL"


def problems_of(elf: bytes, name: str | None, forbid_placeholders: bool):
    """The parsed fields and the list of problems (empty: valid)."""
    n = elf.count(BEGIN)
    if n != 1:
        return {}, [f"{n} security.txt BEGIN markers (want exactly 1)"]
    start = elf.index(BEGIN) + len(BEGIN)
    end = elf.find(END, start)
    if end < 0:
        return {}, ["security.txt END marker not found"]
    parts = elf[start:end].split(b"\0")
    if parts[-1] != b"":
        return {}, ["security.txt does not end with a NUL before the END marker"]
    parts = parts[:-1]
    if len(parts) % 2:
        return {}, ["odd number of security.txt parts"]
    fields, problems = {}, []
    for k, v in zip(parts[0::2], parts[1::2]):
        try:
            key, val = k.decode("utf-8"), v.decode("utf-8")
        except UnicodeDecodeError:
            problems.append(f"field {k!r} is not UTF-8")
            continue
        if key in fields:
            problems.append(f"duplicate field {key}")
        elif key not in REQUIRED + OPTIONAL:
            problems.append(f"unknown field {key}")
        fields[key] = val
    for key in REQUIRED:
        if not fields.get(key):
            problems.append(f"missing field {key}")
    for c in fields["contacts"].split(",") if fields.get("contacts") else ():
        kind, sep, info = c.strip().partition(":")
        if not sep or kind.lower() not in CONTACT_TYPES or not info:
            problems.append(f"invalid contact {c!r}")
    if name is not None and fields.get("name") != name:
        problems.append(f"name {fields.get('name')!r}, expected {name!r}")
    held = sorted(k for k, v in fields.items() if PLACEHOLDER in v)
    if held and forbid_placeholders:
        problems.append(f"{PLACEHOLDER} placeholders in {', '.join(held)} (the release gate)")
    return fields, problems


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    ap.add_argument("elf", help="the program ELF (target/deploy/<program>.so)")
    ap.add_argument("name", nargs="?", help="the expected `name` field")
    ap.add_argument("--forbid-placeholders", action="store_true",
                    help=f"refuse any {PLACEHOLDER} value (the release gate)")
    args = ap.parse_args()
    try:
        elf = open(args.elf, "rb").read()
    except OSError as e:
        print(f"::error::{args.elf}: {e}")
        return 2
    fields, problems = problems_of(elf, args.name, args.forbid_placeholders)
    for p in problems:
        print(f"::error::{args.elf}: {p}")
    if problems:
        return 1
    for k, v in fields.items():
        print(f"  {k}: {v}")
    held = sorted(k for k, v in fields.items() if PLACEHOLDER in v)
    if held:
        print(f"::notice::{args.elf}: {PLACEHOLDER} placeholders in {', '.join(held)} "
              f"(refused by --forbid-placeholders)")
    print(f"ok: {args.elf} carries a valid security.txt")
    return 0


if __name__ == "__main__":
    sys.exit(main())
