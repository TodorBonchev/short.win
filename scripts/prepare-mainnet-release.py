#!/usr/bin/env python3
"""Freeze a public, non-executable mainnet review packet. No RPC or signing."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
SPEC = "config/mainnet-release.proposed.json"
MAINNET_GENESIS = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d"
PROGRAM = "BCPiRMTMoLNY5EHm2RYxoxJChgyrttkFX2Qe2Rnbe3jh"
QUALIFIED_PROGRAM_SHA256 = "86147af04bda2c1fbab92ad670dc8d7cc272162724787dac666344bf24b547f2"
MAX_PROGRAM_BYTES = 4 * 1024 * 1024
INPUTS = {
    "reviewManifest": "config/jupiter-reviewed-deployment.json",
    "policy": "config/production-policy.proposed.json",
    "service": "config/production-service.proposed.json",
    "operatorBounds": "config/operator-bounds.proposed.json",
    "publicRoles": "config/production-roles.template.json",
}
ACTIVATION_GATES = {
    "public-role-assignment-and-custody", "canonical-mainnet-loader-and-initializer-authority",
    "current-jupiter-bytes-and-compatibility-recheck", "selected-program-and-duration-receipt-custody",
    "deployment-rent-quote-and-payer-funding", "reporter-and-keeper-funded-budgets",
    "delivered-alerts-and-account-count-headroom", "measured-one-quantity-provider-and-operator-coverage",
    "consistent-storage-backup-and-recovery", "paused-initialization-and-reviewed-canary-lifecycle",
    "owner-reviewed-final-activation-packet",
}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def validate_spec(spec: dict, inputs: dict[str, dict]) -> None:
    require(spec.get("schemaVersion") == 1, "release schema")
    require(spec.get("execute") is False and spec.get("readyForActivation") is False,
            "release preparation cannot enable execution or activation")
    require(spec.get("network") == "mainnet-beta" and spec.get("expectedGenesisHash") == MAINNET_GENESIS,
            "canonical mainnet genesis is required")
    require(spec.get("programId") == PROGRAM and spec.get("programSha256") == QUALIFIED_PROGRAM_SHA256,
            "release must select the exact primary qualified program")
    for key, path in INPUTS.items():
        require(spec.get(key) == path, "release input path differs: " + key)
    reviewed = inputs["reviewManifest"]
    require(reviewed.get("execute") is False and reviewed.get("ownProgram") == {
        "programId": PROGRAM, "elfSha256": QUALIFIED_PROGRAM_SHA256},
        "review manifest does not identify the exact primary qualified program")
    roles = inputs["publicRoles"]
    require(roles.get("programId") == PROGRAM and roles.get("execute") is False,
            "release public role identity/execution differs")
    require(roles.get("genesisHash") in (None, MAINNET_GENESIS), "role genesis differs from mainnet")
    require(roles.get("jupiterReview") == reviewed.get("jupiterReview"), "role/router review differs")
    intent = spec.get("initializationIntent", {})
    require(intent.get("opensPaused") is True and intent.get("policyVersion") == 1,
            "initialization must keep openings paused with first immutable policy")
    # A rent allowance and creation fee require explicit later review; preparation does not invent them.
    require(intent.get("marketCreationFeeLamports") is None and intent.get("maxSetupRentLamports") is None,
            "initialization costs remain unassigned in the proposed release")
    bounds = spec.get("releaseBounds", {})
    p = inputs["policy"]
    bound_policy = {
        "minPositionNotionalLamports": p["minNotionalLamports"],
        "maxPositionNotionalLamports": p["positionNotionalCapLamports"],
        "maxMarketBookedNotionalLamports": p["marketNotionalCapLamports"],
        "walletMarketShareBps": p["walletNotionalCapBps"],
        "openingUtilizationBps": p["utilizationCapBps"],
        "termSeconds": p["termSeconds"], "graceSeconds": p["defaultGraceSeconds"],
        "exactQuantityHistorySlots": inputs["service"]["max_jobs"],
        "globalExposureCapLamports": None, "depositCapRawTokens": None, "marketCountCap": None,
    }
    require(bounds == bound_policy, "release bounds differ from primary policy/service")
    require(bounds == {
        "minPositionNotionalLamports": 100_000_000, "maxPositionNotionalLamports": 1_000_000_000,
        "maxMarketBookedNotionalLamports": 5_000_000_000, "walletMarketShareBps": 2000,
        "openingUtilizationBps": 6000, "termSeconds": 86400, "graceSeconds": 86400,
        "exactQuantityHistorySlots": 1, "globalExposureCapLamports": None,
        "depositCapRawTokens": None, "marketCountCap": None,
    }, "selected capped release changed; a new explicit release review is required")
    require(inputs["operatorBounds"].get("execute") is False, "operator execution remains disabled")
    gates = spec.get("activationGates")
    require(isinstance(gates, list) and len(gates) == len(ACTIVATION_GATES), "activation gates are incomplete")
    require(all(isinstance(g, dict) and isinstance(g.get("id"), str) and g["id"]
                and g.get("status") == "pending" for g in gates),
            "preparation cannot mark commissioning gates complete")
    require({g["id"] for g in gates} == ACTIVATION_GATES, "activation gate identities differ")


def read_program(path: Path) -> bytes:
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    try:
        before = os.fstat(fd)
        require(stat.S_ISREG(before.st_mode) and 0 < before.st_size <= MAX_PROGRAM_BYTES,
                "program must be a bounded regular ELF file")
        chunks = []
        remaining = before.st_size
        while remaining:
            chunk = os.read(fd, min(65536, remaining))
            require(bool(chunk), "program changed during read")
            chunks.append(chunk)
            remaining -= len(chunk)
        after = os.fstat(fd)
        require((before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns, before.st_ctime_ns)
                == (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns),
                "program changed during read")
    finally:
        os.close(fd)
    data = b"".join(chunks)
    require(data[:4] == b"\x7fELF" and sha256(data) == QUALIFIED_PROGRAM_SHA256,
            "program differs from the exact primary qualified ELF; a rebuild is not substituted")
    return data


def source_hashes() -> dict[str, str]:
    roots = ["programs/shortwin-simple/src", "crates/pooled-core/src", "crates/jupiter-route/src",
             "client/src", "client/bin", "client/idl", "services/src", "scripts", "config",
             ".github/workflows", "web/src", "tests/runtime"]
    suffixes = {".rs", ".ts", ".mts", ".py", ".json", ".sh", ".mjs", ".yml", ".yaml", ".toml", ".css"}
    names = {"Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "mise.toml", "package.json",
             "pnpm-lock.yaml", "pnpm-workspace.yaml", "client/package.json", "client/tsconfig.json",
             "web/package.json", "web/tsconfig.json", "web/vite.config.ts"}
    for root in roots:
        for file in (ROOT / root).rglob("*"):
            if any(part in {"target", "node_modules", "__pycache__"} for part in file.parts):
                continue
            if file.is_file() and file.suffix in suffixes:
                names.add(file.relative_to(ROOT).as_posix())
    for folder in ["programs/shortwin-simple", "crates/pooled-core", "crates/jupiter-route"]:
        names.add(folder + "/Cargo.toml")
    require(all(not (ROOT / name).is_symlink() for name in names), "source symlink is not a frozen input")
    return {name: sha256((ROOT / name).read_bytes()) for name in sorted(names)}


def proposal_validation(require_assigned: bool) -> dict:
    command = [sys.executable, str(ROOT / "config/verify-production-proposal.py")]
    if require_assigned:
        command.append("--require-assigned-roles")
    result = subprocess.run(command, capture_output=True, text=True, timeout=60,
                            env={"PYTHONDONTWRITEBYTECODE": "1"}, check=False)
    require(result.returncode == 0, "production proposal validation failed: " + result.stderr.strip()[:4096])
    require(len(result.stdout) <= 2 * 1024 * 1024, "oversized proposal validation report")
    report = json.loads(result.stdout)
    require(report.get("status") == "proposed_configuration_verified_offline_not_activated",
            "unexpected proposal validation status")
    return report


def public_roles(roles: dict) -> dict:
    return {key: roles[key] for key in [
        "schemaVersion", "assignmentStatus", "network", "programId", "genesisHash", "rolePublicKeys",
        "jupiterReview", "execute"]}


def write_json(path: Path, value: object) -> None:
    with path.open("x", encoding="utf-8") as stream:
        json.dump(value, stream, indent=2, sort_keys=True)
        stream.write("\n")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--program", type=Path, required=True, help="Exact preserved primary qualified ELF")
    parser.add_argument("--source-ref", required=True, help="Reviewed full 40-hex Git commit; source files are also hashed")
    parser.add_argument("--output", type=Path, required=True, help="Fresh directory; its parent must already exist")
    parser.add_argument("--require-assigned-roles", action="store_true")
    args = parser.parse_args()
    require(re.fullmatch(r"[0-9a-f]{40}", args.source_ref) is not None, "source ref must be full lowercase 40-hex commit")
    require(not args.output.exists() and not args.output.is_symlink(), "release output must be a fresh directory")
    raw_inputs = {path: (ROOT / path).read_bytes() for path in [SPEC, *INPUTS.values()]}
    spec = json.loads(raw_inputs[SPEC])
    inputs = {key: json.loads(raw_inputs[path]) for key, path in INPUTS.items()}
    validate_spec(spec, inputs)
    program = read_program(args.program)
    hashes = source_hashes()
    require(all(hashes[path] == sha256(raw) for path, raw in raw_inputs.items()),
            "release inputs changed during preparation")
    validation = proposal_validation(args.require_assigned_roles)
    require(source_hashes() == hashes, "source changed during proposal validation")
    # All checks precede output creation. This packet contains public material only.
    args.output.mkdir(mode=0o700)
    files = {
        "release-spec.json": spec, "policy.json": inputs["policy"], "service.json": inputs["service"],
        "operator-bounds.json": inputs["operatorBounds"], "roles.public.json": public_roles(inputs["publicRoles"]),
        "reviewed-jupiter.json": inputs["reviewManifest"], "proposal-validation.json": validation,
    }
    for name, value in files.items():
        write_json(args.output / name, value)
    with (args.output / "shortwin_simple.so").open("xb") as stream:
        stream.write(program)
    payloads = {name: sha256((args.output / name).read_bytes())
                for name in sorted([*files, "shortwin_simple.so"])}
    packet = {
        "schemaVersion": 1, "status": "prepared-for-review-not-activated", "execute": False,
        "readyForActivation": False, "sourceRef": args.source_ref, "sourceFilesSha256": hashes,
        "network": "mainnet-beta", "expectedGenesisHash": MAINNET_GENESIS,
        "programId": PROGRAM, "programSha256": sha256(program), "programBytes": len(program),
        "filesSha256": payloads, "initializationIntent": spec["initializationIntent"],
        "releaseBounds": spec["releaseBounds"], "activationGates": spec["activationGates"],
        "publicIdentityAssignmentComplete": validation["role_setup"]["complete_public_identity_assignment"],
        "scope": "Offline proposal and retained compatibility integrity only. This preparer does not independently verify the separately retained duration campaign, current deployment state, custody, funding, service commissioning or deployment approval.",
    }
    write_json(args.output / "release.json", packet)
    print(json.dumps({"packet": str(args.output / "release.json"), "programSha256": sha256(program),
                      "execute": False, "readyForActivation": False}))


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError, TypeError, OSError, subprocess.TimeoutExpired) as error:
        raise SystemExit(f"mainnet preparation failed: {error}") from None
