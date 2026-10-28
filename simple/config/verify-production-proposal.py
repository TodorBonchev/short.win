#!/usr/bin/env python3
"""Check proposed JSON/model bindings and emit integer economic/capacity scenarios.

No RPC, quotes, wallets, transaction submission or deployment. This checks a
proposal; it does not certify token recovery, oracle honesty or funded operation.
"""
from __future__ import annotations
import argparse
import importlib.util
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("proposal_price_service", ROOT / "services/src/shortwin_price_service.py")
service = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = service
spec.loader.exec_module(service)
U64 = (1 << 64) - 1
U128 = (1 << 128) - 1
SOL = 1_000_000_000


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def multiply(a: int, b: int) -> int:
    require(0 <= a <= U64 and 0 <= b <= U64, "u64 input overflow")
    require(a * b <= U128, "u128 product overflow")
    return a * b


def bps(value: int, rate: int, up: bool = False) -> int:
    product = multiply(value, rate)
    require(product + (9999 if up else 0) <= U128, "u128 round overflow")
    result = (product + (9999 if up else 0)) // 10000
    require(result <= U64, "u64 result overflow")
    return result


def verify_terms(p: dict) -> None:
    required = {
        "termSeconds", "defaultGraceSeconds", "termFeeBps", "protocolShareBps", "crOpenBps", "crLiqBps",
        "minNotionalLamports", "positionNotionalCapLamports", "marketNotionalCapLamports", "utilizationCapBps",
        "walletNotionalCapBps", "keeperFixedLamports", "keeperBps", "keeperCapLamports", "buybackPremiumBps",
        "referenceMaxAgeSeconds", "referenceMaxAgeSlots", "historyEpochSeconds", "minimumHistoryEpochs",
        "maxSpreadBps", "crConfirmationDelaySeconds", "crConfirmationDelaySlots", "pricePolicyHash"}
    require(set(p) == required, "FinancialPolicy field set differs")
    for key in required - {"pricePolicyHash"}:
        require(type(p[key]) is int and 0 <= p[key] <= U64, f"invalid integer {key}")
    require(0 < p["termSeconds"] <= 259200 and p["defaultGraceSeconds"] == 86400, "term/grace")
    require(0 <= p["termFeeBps"] <= 10000 and 0 <= p["protocolShareBps"] <= 10000, "fee shares")
    require(10000 < p["crLiqBps"] < p["crOpenBps"] <= 1000000, "collateral ratios")
    require(0 < p["minNotionalLamports"] <= p["positionNotionalCapLamports"] <= p["marketNotionalCapLamports"] <= 25 * SOL, "caps")
    require(0 < p["utilizationCapBps"] < 10000 and 0 < p["walletNotionalCapBps"] <= 10000, "utilization/wallet")
    require(all(p[key] <= 10000 for key in ["keeperBps", "buybackPremiumBps", "maxSpreadBps"]), "bps limit")
    require(0 < p["referenceMaxAgeSeconds"] <= 60 and 0 < p["referenceMaxAgeSlots"] <= 1000, "freshness")
    require(0 < p["historyEpochSeconds"] <= 300 and 2 <= p["minimumHistoryEpochs"] <= 8, "history")
    require(0 < p["crConfirmationDelaySeconds"] <= 60 and 0 < p["crConfirmationDelaySlots"] <= 150, "confirmation")
    require(isinstance(p["pricePolicyHash"], str) and len(bytes.fromhex(p["pricePolicyHash"])) == 32 and int(p["pricePolicyHash"], 16) > 0, "model hash")
    require(p["keeperFixedLamports"] <= p["keeperCapLamports"] < bps(p["minNotionalLamports"], p["crOpenBps"], True), "keeper reserve consumes minimum collateral")
    # Opening arithmetic at the largest admitted N must fit the program's u64.
    bps(p["positionNotionalCapLamports"], p["crOpenBps"], True)


def keeper(basis: int, available: int, p: dict) -> int:
    return min(min(U64, p["keeperFixedLamports"] + bps(basis, p["keeperBps"])),
               p["keeperCapLamports"], available)


def opening(sale: int, live: int, history: int, p: dict) -> dict:
    n = max(sale, live, history)
    collateral = bps(n, p["crOpenBps"], True)
    fee = bps(n, p["termFeeBps"], True)
    protocol = bps(fee, p["protocolShareBps"])
    require(collateral >= sale and fee >= protocol, "opening arithmetic")
    return {"sale_lamports": sale, "live_ask_upper_lamports": live, "history_ask_upper_max_lamports": history,
            "notional_lamports": n, "borrower_margin_lamports": collateral - sale, "collateral_lamports": collateral,
            "upfront_fee_lamports": fee, "lender_fee_lamports": fee - protocol, "protocol_fee_lamports": protocol,
            "borrower_opening_cash_excluding_account_rent_and_network_fee_lamports": collateral - sale + fee}


def finance_report(p: dict) -> dict:
    sizes = [p["minNotionalLamports"], 250000000, p["positionNotionalCapLamports"]]
    opens = [opening(n, n, n, p) for n in sizes]
    min_n = sizes[0]
    v = opens[0]["collateral_lamports"]
    scenarios = []
    for factor in [10000, 12500, 12501, 15000, 20000, 25000, 30000]:
        bid = bps(min_n, factor)
        ask = bps(bid, 10000 + p["maxSpreadBps"], True)
        reward = keeper(ask, max(0, v - ask), p)
        cap = min(bps(ask, 10000 + p["buybackPremiumBps"]), max(0, v - p["keeperCapLamports"]))
        default_fee = keeper(v, v, p)
        recovered = v - default_fee
        scenarios.append({"bid_factor_bps_of_opening_notional": factor,
                          "bid_lower_lamports": bid, "ask_upper_at_maximum_spread_lamports": ask,
                          "bid_health_unhealthy_strict_boundary": multiply(v, 10000) < multiply(bid, p["crLiqBps"]),
                          "illustrative_forced_budget_lamports": cap,
                          "this_tested_ask_fits_budget": ask <= cap,
                          "maturity_keeper_reward_if_this_ask_spent_lamports": reward if ask <= cap else None,
                          "default_lender_sol_lamports": recovered,
                          "default_borrower_economic_sol_lamports": 0,
                          "default_sol_as_bps_of_current_tested_ask": multiply(recovered, 10000) // ask})
    # These exact values guard policy drift and make the economic recommendations reviewable.
    require(opens[0]["collateral_lamports"] == 250000000, "review minimum collateral after parameter change")
    require(opens[0]["protocol_fee_lamports"] == 200000 and opens[0]["lender_fee_lamports"] == 800000, "review fee economics")
    require(not scenarios[1]["bid_health_unhealthy_strict_boundary"] and scenarios[2]["bid_health_unhealthy_strict_boundary"], "strict CR boundary")
    require(not scenarios[-1]["this_tested_ask_fits_budget"], "stress must expose unfillable full-debt route")
    rounding = opening(100000000, 100000001, 99999999, p)
    return {"opening_examples": opens,
            "rounding_example": rounding,
            "wallet_booked_notional_cap_lamports_per_market": bps(p["marketNotionalCapLamports"], p["walletNotionalCapBps"]),
            "minimum_size_positions_at_market_booked_cap": p["marketNotionalCapLamports"] // min_n,
            "maximum_size_positions_at_market_booked_cap": p["marketNotionalCapLamports"] // p["positionNotionalCapLamports"],
            "example_1000_token_cash_opening_borrow_limit_raw_tokens": bps(1000, p["utilizationCapBps"]),
            "bid_trigger_factor_bps_of_opening_notional_at_equal_sale_ask": p["crOpenBps"] * 10000 // p["crLiqBps"],
            "minimum_notional_stress_examples": scenarios,
            "small_loan_maturity_keeper_reward_at_initial_notional_lamports": keeper(min_n, v-min_n, p),
            "small_loan_default_keeper_reward_lamports": keeper(v, v, p),
            "example_three_transactions_at_25000_lamports_each": 75000,
            "minimum_fee_protocol_revenue_covers_25000_lamport_transactions_before_other_costs": opens[0]["protocol_fee_lamports"] // 25000,
            "conditional_bounds": ["The 25% bid-rise trigger assumes unchanged collateral and initial bid equals N; max historical bid must also qualify and confirmation must complete.",
              "Finite ask upper is one successful exact-input guaranteed-output quote, not fair value, global cheapest price, monotonicity, or a future executable guarantee.",
              "Default distributes SOL, not a guarantee of recovering the lent token quantity. No economic payout goes to the borrower after grace.",
              "Fee examples omit network fees and refundable account/scratch deposits. No accrued fee or rate refund exists.",
              "Utilization and booked SOL caps are checked on opening; withdrawals, current prices, sybil wallets and uncapped market creation defeat any claimed global or permanent cash reserve."]}


def profile_report(name: str, p: dict, settings: object) -> dict:
    identities = settings.max_jobs
    normal_calls = 2 * identities
    http_per_probe = 2
    maximum_observation_calls = min(1 + settings.max_ask_requests, 3) * http_per_probe * identities
    normal_ms = normal_calls * settings.request_interval_ms
    target_ms = max(1, settings.max_sample_gap_seconds // 2) * 1000
    rounds_per_day = 86400 // max(1, settings.max_sample_gap_seconds // 2)
    return {"profile": name, "model_hash": settings.model_hash,
            "configured_exact_quantity_slots": identities, "maximum_http_starts_per_quote_probe": http_per_probe,
            "minimum_completed_history_seconds": settings.history_epochs * settings.epoch_seconds,
            "warmup_worst_case_without_boundary_latency_seconds": (settings.history_epochs + 1) * settings.epoch_seconds,
            "normal_two_successful_gets_per_identity_assumption": True,
            "normal_spacing_only_round_ms": normal_ms,
            "target_round_ms": target_ms,
            "normal_latency_and_rpc_headroom_ms": target_ms - normal_ms,
            "all_identity_maximum_observation_spacing_only_round_ms": maximum_observation_calls * settings.request_interval_ms,
            "one_execution_buy_search_additional_maximum_spacing_ms": settings.max_ask_requests * http_per_probe * settings.request_interval_ms,
            "all_maximum_observations_plus_one_execution_buy_search_spacing_ms": (maximum_observation_calls + settings.max_ask_requests * http_per_probe) * settings.request_interval_ms,
            "one_fallback_plus_other_normal_spacing_only_round_ms": (min(1 + settings.max_ask_requests, 3) * http_per_probe + 2 * (identities - 1)) * settings.request_interval_ms,
            "existing_service_worst_case_capacity_report": service.capacity_report(settings),
            "hypothetical_continuous_publication_1_signature_base_fee_5000_lamports_per_day": rounds_per_day * identities * 5000,
            "hypothetical_continuous_publication_at_25000_lamports_per_tx_per_day": rounds_per_day * identities * 25000,
            "on_demand_two_publications_at_25000_lamports_per_loan": 50000,
            "minimum_loan_protocol_revenue_lamports": bps(bps(p["minNotionalLamports"], p["termFeeBps"], True), p["protocolShareBps"]),
            "measured_live_coverage": "pending", "ready_for_activation": False}


def public_key(value: object, name: str) -> None:
    alphabet = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
    require(isinstance(value, str) and 32 <= len(value) <= 44, f"unassigned/invalid public key: {name}")
    require(all(c in alphabet for c in value), f"invalid base58 public key: {name}")
    number = 0
    for char in value: number = number * 58 + alphabet.index(char)
    length = (number.bit_length() + 7) // 8 + len(value) - len(value.lstrip("1"))
    require(length == 32 and number > 0, f"public key must be nonzero and 32 bytes: {name}")


def verify_roles(roles: dict, require_assigned: bool = False) -> dict:
    require(roles.get("schemaVersion") == 1 and roles.get("network") == "mainnet-beta", "role manifest schema/network")
    require(roles.get("assignmentStatus") in ["unassigned", "assigned-for-review", "owner-reviewed"], "role assignment status")
    require(roles.get("execute") is False, "review manifest cannot enable execution")
    keys = roles.get("rolePublicKeys")
    require(isinstance(keys, dict) and set(keys) == {"governance", "emergency", "reporter", "feeRecipient", "keeper"}, "role field set")
    review = roles.get("jupiterReview", {})
    require(review.get("programId") == service.JUPITER_PROGRAM, "Jupiter canonical identity")
    require(review.get("programData") == "4Ec7ZxZS6Sbdg5UGSLHbAnM7GQHp2eFd4KYWRexAipQT", "Jupiter canonical ProgramData")
    missing = []
    for name, value in {"programId": roles.get("programId"), "genesisHash": roles.get("genesisHash"), **keys}.items():
        if value is None: missing.append(name)
        else: public_key(value, name)
    require(roles.get("programId") != "kXYTnbz7MJ3g1d1cKX5wfZLy8KmN7Cctx4oePy5ndB2", "local placeholder program cannot be silently promoted")
    for name in ["deploySlot", "programDataLength", "elfSha256"]:
        if review.get(name) is None: missing.append("jupiterReview." + name)
    if review.get("deploySlot") is not None:
        require(type(review["deploySlot"]) is int and review["deploySlot"] > 0, "Jupiter deploy slot")
    if review.get("programDataLength") is not None:
        require(type(review["programDataLength"]) is int and review["programDataLength"] >= 45, "Jupiter ProgramData length")
    if review.get("elfSha256") is not None:
        value = review["elfSha256"]
        require(isinstance(value, str) and len(bytes.fromhex(value)) == 32 and int(value, 16) > 0, "Jupiter ELF digest")
    if keys.get("reporter") is not None:
        require(keys["reporter"] != keys.get("governance") and keys["reporter"] != keys.get("emergency"), "reporter must differ from governance/emergency")
    if require_assigned:
        require(not missing, "production identities are unassigned: " + ", ".join(missing))
        require(roles.get("assignmentStatus") != "unassigned", "identity assignment not marked for review")
    return {"complete_public_identity_assignment": not missing,
            "missing_public_identity_fields": missing, "execution_enabled": False,
            "note": "Complete assignments still require deployment/loader review and commissioned operating evidence; this tool never signs or submits."}


def captured_reference_rent(policy: dict) -> dict:
    folder = ROOT.parent / "docs/audit/simple-current-route-evidence-2026-10-28"
    raw_path = folder / "reference-rent205.json"
    receipt_path = folder / "reference-rent205.receipt.json"
    if not raw_path.exists() or not receipt_path.exists():
        return {"status": "dated_rpc_evidence_not_present"}
    raw = raw_path.read_bytes()
    receipt = json.loads(receipt_path.read_text())
    fixed_hash = "e3533cddf22cdac9e1b41a7fdedf4d0ed7d82734fbb42c4d0ef0c188a16e2499"
    require(hashlib.sha256(raw).hexdigest() == fixed_hash == receipt["sha256"], "captured reference-deposit receipt hash")
    require(receipt["request"]["method"] == "getMinimumBalanceForRentExemption" and receipt["request"]["params"][0] == 205, "reference-deposit request")
    deposit = json.loads(raw)["result"]
    require(type(deposit) is int and deposit > 0, "captured reference deposit")
    revenue = bps(bps(policy["minNotionalLamports"], policy["termFeeBps"], True), policy["protocolShareBps"])
    return {"status": "dated_read_only_rpc_observation", "allocation_bytes": 205,
            "deposit_lamports": deposit, "observed_utc": receipt["finished_at"],
            "response_sha256": fixed_hash,
            "minimum_size_loan_protocol_fee_shares_to_cover_deposit_before_other_costs": (deposit+revenue-1)//revenue,
            "note": "Storage deposit is locked capital, not a funded-operation receipt or a current price guarantee; no reference close instruction exists."}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write-report", type=Path)
    parser.add_argument("--require-assigned-roles", action="store_true")
    args = parser.parse_args()
    roles = verify_roles(json.loads((ROOT / "config/production-roles.template.json").read_text()), args.require_assigned_roles)
    operator = json.loads((ROOT / "config/operator-bounds.proposed.json").read_text())
    require(operator.get("execute") is False and operator.get("overlappingPassesAllowed") is False, "proposed runner must not execute/overlap")
    setup = operator.get("maxRouteSetupLamports")
    require(type(setup) is int and 0 <= setup <= 5_000_000, "operator native setup bound")
    require(operator["suggestedInitialLamportsPerOperatingPayer"] >= operator["minBalanceLamports"] + max(operator["maxTemporaryLamports"], operator["maxReferenceRentLamports"]) + operator["maxFeeLamports"], "suggested payer balance cannot meet residual/peak/fee bounds")
    profiles = []
    primary = None
    for prefix in ["production", "keyless"]:
        p = json.loads((ROOT / f"config/{prefix}-policy.proposed.json").read_text())
        settings = service.Settings(**json.loads((ROOT / f"config/{prefix}-service.proposed.json").read_text()))
        verify_terms(p)
        service.verify_policy(settings, p)
        profiles.append(profile_report(prefix, p, settings))
        if prefix == "production": primary = p
    result = {"status": "proposed_configuration_verified_offline_not_activated", "financial_source_sha256": {str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest() for path in [ROOT / "programs/shortwin-simple/src/state.rs", ROOT / "programs/shortwin-simple/src/finance.rs", ROOT / "programs/shortwin-simple/src/open.rs", ROOT / "programs/shortwin-simple/src/exits.rs", ROOT / "programs/shortwin-simple/src/route.rs", ROOT / "crates/jupiter-route/src/isolation.rs"]}, "price_service_model_version": service.MODEL_VERSION, "financial_model": finance_report(primary), "profiles": profiles, "captured_reference_deposit": captured_reference_rent(primary), "role_setup": roles, "operator_bounds": operator}
    output = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.write_report: args.write_report.write_text(output)
    else: print(output, end="")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError, TypeError) as error:
        raise SystemExit(f"production proposal check failed: {error}") from None
