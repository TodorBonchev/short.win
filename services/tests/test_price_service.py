"""Offline behavioral qualification; no provider account, wallet or mainnet send."""
from dataclasses import replace
import importlib.util
import json
from pathlib import Path
import sys
import subprocess
import tempfile
import unittest
from unittest.mock import patch

SOURCE = Path(__file__).resolve().parents[1] / "src" / "shortwin_price_service.py"
SPEC = importlib.util.spec_from_file_location("shortwin_price_service", SOURCE)
service = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = service
SPEC.loader.exec_module(service)

MARKET = "kXYTnbz7MJ3g1d1cKX5wfZLy8KmN7Cctx4oePy5ndB2"
POLICY = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
MINT = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"


def settings(**updates):
    # Synthetic functional-test settings, not deployment recommendations.
    return replace(service.Settings(60, 2, 5, 30, 100, 50, 1000, 10000, 12, 2, 2, 1), **updates)


def job(q=100, active=False):
    return service.Job(MARKET, POLICY, MINT, q, active)


class FakeTime:
    def __init__(self):
        self.now = 1000.0

    def clock(self):
        return self.now

    def sleep(self, seconds):
        self.now += seconds


def client(configuration, provider):
    timer = FakeTime()
    return service.Jupiter(configuration, MARKET, "unused", provider,
                           clock=timer.clock, sleeper=timer.sleep)


class Provider:
    def __init__(self):
        self.calls = []

    def __call__(self, params):
        self.calls.append(params)
        amount = int(params["amount"])
        output = amount * 2 if params["inputMint"] == service.WSOL else amount // 2
        slip = int(params["slippageBps"])
        data = (service.ROUTE_V2 + amount.to_bytes(8, "little") + output.to_bytes(8, "little")
                + slip.to_bytes(2, "little") + bytes(4) + (1).to_bytes(4, "little") + bytes(5))
        return {"inputMint": params["inputMint"], "outputMint": params["outputMint"],
                "inAmount": str(amount), "outAmount": str(output),
                "otherAmountThreshold": str(max(1, (output*(10000-slip)+9999)//10000)), "swapMode": "ExactIn",
                "slippageBps": int(params["slippageBps"]), "routePlan": [{"fixture": True}],
                "swapInstruction": {"programId": service.JUPITER_PROGRAM,
                                    "accounts": [{"pubkey": MARKET, "isSigner": True, "isWritable": False}],
                                    "data": service.base64.b64encode(data).decode()}}


class PriceServiceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.path = Path(self.temp.name) / "history.sqlite3"
        self.history = service.History(self.path, settings())
        self.history.enqueue(job(), 600)

    def tearDown(self):
        self.history.close()
        self.temp.cleanup()

    def complete(self, skip=None, unknown=None):
        for timestamp in range(600, 721, 5):
            if timestamp == skip:
                continue
            bid, ask = (0, 0) if timestamp == unknown else (105 + timestamp % 6, 115)
            self.history.record(job(), timestamp, timestamp, bid, ask, {"fixture": timestamp})

    def test_exact_in_buy_search_requires_guaranteed_full_quantity(self):
        provider = Provider()
        jupiter = client(settings(), provider)
        quote = jupiter.buy(job(), 20)
        self.assertGreaterEqual(quote.guaranteed_output, 100)
        self.assertEqual(quote.amount, 50)
        self.assertLessEqual(len(provider.calls), settings().max_ask_requests)
        self.assertTrue(all("swapMode" not in call for call in provider.calls))
        self.assertTrue(all(call["wrapAndUnwrapSol"] == "false" for call in provider.calls))
        with self.assertRaises(service.Unavailable):
            client(settings(max_buy_lamports=40), Provider()).buy(job(), 20)
        with self.assertRaises(service.Unavailable):
            jupiter.buy(job(), 20, maximum=40)
        jupiter.clock = iter([0, 31]).__next__
        with patch.object(jupiter, "quote", return_value=service.Quote(100, 200, 199, "", {})):
            with self.assertRaises(service.Unavailable):
                jupiter.buy(job(), 100)

    def test_buy_keeps_a_successful_quote_when_a_smaller_refinement_has_no_route(self):
        provider = Provider()
        def transport(params):
            if int(params["amount"]) < 100:
                provider.calls.append(params)
                raise service.RouteUnavailable("smaller budget has no route")
            return provider(params)
        quote = client(settings(), transport).buy(job(), 100)
        self.assertEqual(quote.amount, 100)
        self.assertGreaterEqual(quote.guaranteed_output, job().quantity)
        self.assertEqual(quote.raw_hash, service.digest(quote.raw))
        self.assertEqual([p["amount"] for p in provider.calls], ["100", "50", "50"])

    def test_buy_does_not_hide_initial_route_failure_or_refinement_outage(self):
        def missing(_params):
            raise service.RouteUnavailable("no initial candidate")
        with self.assertRaisesRegex(service.RouteUnavailable, "no initial candidate"):
            client(settings(), missing).buy(job(), 100)

        provider = Provider()
        def outage(params):
            if int(params["amount"]) < 100:
                raise service.Unavailable("provider outage")
            return provider(params)
        with self.assertRaisesRegex(service.Unavailable, "provider outage"):
            client(settings(), outage).buy(job(), 100)

    def test_buy_does_not_return_a_quote_that_aged_during_failed_refinement(self):
        timer = FakeTime()
        def transport(params):
            if int(params["amount"]) < 100:
                timer.now += 31
                raise service.RouteUnavailable("smaller budget has no route")
            return Provider()(params)
        jupiter = service.Jupiter(settings(), MARKET, "unused", transport,
                                  clock=timer.clock, sleeper=timer.sleep)
        with self.assertRaisesRegex(service.Unavailable, "outlived reference freshness"):
            jupiter.buy(job(), 100)

    def test_observation_reuses_actual_ask_and_caps_growing_budget_to_live_spread(self):
        provider = Provider()
        jupiter = client(settings(), provider)
        bid, ask, evidence = jupiter.observe(job(), 51)
        self.assertEqual((bid, ask), (50, 51))
        self.assertEqual(len(provider.calls), 2)
        self.assertEqual(evidence["buy"]["response"]["inAmount"], "51")
        provider.calls.clear()
        self.assertEqual(jupiter.observe(job(), 999)[1], 55)
        self.assertEqual(len(provider.calls), 2)
        provider.calls.clear()
        self.assertGreaterEqual(jupiter.observe(job())[1], 50)
        self.assertLessEqual(len(provider.calls), 3)
        with self.assertRaises(service.Unavailable):
            client(settings(max_buy_lamports=49), Provider()).observe(job())

    def test_bounded_account_fallback_supports_larger_ordinary_routes_without_retrying_outages(self):
        provider=Provider()
        def transport(params):
            if params["maxAccounts"] == "16":
                provider.calls.append(params)
                raise service.RouteUnavailable("fixture no route")
            return provider(params)
        quote=client(settings(),transport).quote(MINT,service.WSOL,100)
        self.assertEqual(quote.amount,100)
        self.assertEqual([p["maxAccounts"] for p in provider.calls],["16","32"])
        def outage(params):
            provider.calls.append(params)
            raise service.Unavailable("fixture rate limit")
        provider.calls.clear()
        with self.assertRaises(service.Unavailable):
            client(settings(),outage).quote(MINT,service.WSOL,100)
        self.assertEqual(len(provider.calls),1)

    def test_shared_or_mismatched_scalar_header_is_not_a_finite_quote(self):
        for offset, value in [(0, 209), (8, 1), (16, 1), (24, 51), (26, 1), (28, 1), (30, 9)]:
            def transport(params, offset=offset, value=value):
                raw = Provider()(params)
                data = bytearray(service.base64.b64decode(raw["swapInstruction"]["data"]))
                data[offset] = value
                raw["swapInstruction"]["data"] = service.base64.b64encode(data).decode()
                return raw
            with self.assertRaises(service.Unavailable, msg=str(offset)):
                client(settings(), transport).quote(MINT, service.WSOL, 100)

    def test_publication_requests_are_explicit_and_loan_sync_does_not_refresh_them(self):
        self.assertEqual(self.history.publication_requested_at(job()), 600)
        self.history.sync_loans([job()], 700)
        self.assertEqual(self.history.publication_requested_at(job()), 600)
        self.history.enqueue(job(), 800)
        self.assertEqual(self.history.publication_requested_at(job()), 800)

    def test_quote_domain_and_threshold_are_validated_without_float_amounts(self):
        for field, value in [("swapMode", "ExactOut"), ("inAmount", "99"),
                             ("otherAmountThreshold", "1000000"), ("outAmount", 1.5),
                             ("inputMint", POLICY)]:
            def transport(params, field=field, value=value):
                raw = Provider()(params)
                raw[field] = value
                return raw
            with self.assertRaises(service.Unavailable, msg=field):
                client(settings(), transport).quote(MINT, service.WSOL, 100)
        with self.assertRaises(ValueError):
            service.Job(MARKET, POLICY, MINT, 1.5)
        with self.assertRaises(ValueError):
            service.Job("2"*32, POLICY, MINT, 1)

    def test_completed_exact_quantity_history_survives_restart(self):
        self.complete()
        result = self.history.reference(job(), 4, 720, 720)
        self.assertEqual(result["status"], "Finite")
        payload = result["args"]
        self.assertEqual((payload["sequence"], payload["quantity"]), ("5", "100"))
        self.assertEqual((payload["history_start_ts"], payload["history_end_ts"], payload["completed_epochs"]), (600, 719, 2))
        self.assertEqual(int(payload["history_ask_upper_max"]), 115)
        self.assertEqual(len(payload["evidence_hash"]), 64)
        self.history.close()
        self.history = service.History(self.path, settings())
        self.assertEqual(self.history.reference(job(), 4, 720, 720), result)
        self.assertEqual(self.history.reference(job(101), 0, 720, 720)["status"], "Unknown")

    def test_gap_unknown_stale_and_future_never_become_finite(self):
        self.complete(skip=650)
        self.assertEqual(self.history.reference(job(), 0, 720, 720)["status"], "Unknown")
        self.history.db.execute("DELETE FROM observations")
        self.history.db.commit()
        self.complete(unknown=650)
        self.assertEqual(self.history.reference(job(), 0, 720, 720)["status"], "Unknown")
        self.history.db.execute("DELETE FROM observations")
        self.history.db.commit()
        self.complete()
        for slot, timestamp in [(821, 720), (720, 751), (719, 720), (720, 719)]:
            self.assertEqual(self.history.reference(job(), 0, slot, timestamp)["status"], "Unknown")

    def test_queue_is_bounded_and_no_entry_is_evicted_without_reconciliation(self):
        self.assertTrue(self.history.enqueue(job(101), 600))
        self.assertFalse(self.history.enqueue(job(102), 600))
        self.assertFalse(self.history.enqueue(job(103, True), 600))
        self.assertTrue(self.history.enqueue(job(101, True), 600))
        selected = self.history.plan(600)
        self.assertEqual(len(selected), 2)
        self.assertTrue(selected[0].active_loan)
        self.assertEqual(selected[0].quantity, 101)
        self.assertTrue(self.history.enqueue(job(101, False), 600))
        self.assertTrue(self.history.plan(600)[0].active_loan)

    def test_model_change_requires_fresh_history_and_u64_quantities_remain_exact(self):
        with self.assertRaises(ValueError):
            service.History(self.path, settings(slippage_bps=51))
        maximum = job(service.U64_MAX, True)
        self.assertTrue(self.history.enqueue(maximum, 600))
        self.history.record(maximum, 600, 600, 1, 1)
        payload = self.history.reference(maximum, 0, 600, 600)["args"]
        self.assertEqual(payload["quantity"], str(service.U64_MAX))

    def test_tick_preserves_explicit_unknown_on_provider_failure(self):
        class Missing:
            def observe(self, *_):
                raise service.Unavailable("fixture missing route")
        result = service.tick(self.history, Missing(), lambda: (600, 600), 600, wall_clock=lambda: 600)
        self.assertFalse(result[0]["finite"])
        payload = self.history.reference(job(), 0, 600, 600)["args"]
        self.assertFalse(payload["finite"])
        self.assertEqual(payload["evidence_hash"], "00"*32)

    def test_invalid_initial_clock_cannot_advance_history_or_scheduling(self):
        class Quotes:
            calls = 0
            def observe(self, *_):
                self.calls += 1
                return 100, 105, {"fixture": True}
        quotes = Quotes()
        before = list(self.history.db.iterdump())
        for invalid in [(600, 1_000_000), (600, 569), (0, 600), (600, -1)]:
            with self.subTest(clock=invalid):
                with self.assertRaises(service.Unavailable):
                    service.tick(self.history, quotes, lambda: invalid, 600, wall_clock=lambda: 600)
                self.assertEqual(list(self.history.db.iterdump()), before)
                self.assertEqual(quotes.calls, 0)
        # A healthy next tick remains due and can write the first observation.
        result = service.tick(self.history, quotes, lambda: (600, 600), 600, wall_clock=lambda: 600)
        self.assertTrue(result[0]["finite"])
        self.assertEqual(quotes.calls, 1)
        self.assertEqual(self.history.db.execute("SELECT ts,slot FROM observations").fetchall(), [(600, "600")])

    def test_invalid_final_clock_records_unknown_only_at_validated_initial_time(self):
        class Quotes:
            def observe(self, *_):
                return 100, 105, {"fixture": True}
        for index, invalid in enumerate([(601, 1_000_000), (0, 601), (599, 601), (601, 599)]):
            with self.subTest(clock=invalid):
                history = service.History(Path(self.temp.name) / f"final-clock-{index}.sqlite3", settings())
                try:
                    history.enqueue(job(), 600)
                    clock = iter([(600, 600), invalid]).__next__
                    result = service.tick(history, Quotes(), clock, 600, wall_clock=lambda: 600)
                    self.assertFalse(result[0]["finite"])
                    self.assertEqual(history.db.execute("SELECT ts,slot,finite FROM observations").fetchall(), [(600, "600", 0)])
                    self.assertEqual(history.db.execute("SELECT due FROM jobs").fetchone()[0], 602)
                    recovered = service.tick(history, Quotes(), lambda: (602, 602), 602, wall_clock=lambda: 602)
                    self.assertTrue(recovered[0]["finite"])
                finally:
                    history.close()

    def test_clock_that_becomes_stale_during_observation_is_not_finite(self):
        class Quotes:
            def observe(self, *_):
                return 100, 105, {"fixture": True}
        result = service.tick(self.history, Quotes(), lambda: (600, 600), 600,
                              wall_clock=iter([600, 631]).__next__)
        self.assertFalse(result[0]["finite"])
        self.assertEqual(self.history.db.execute("SELECT ts,slot,finite FROM observations").fetchall(), [(600, "600", 0)])

    def test_loan_sync_can_only_add_priority_and_rolls_back_capacity_failure(self):
        self.history.enqueue(job(101, True), 600)
        self.history.sync_loans([job(100)], 600)
        selected = self.history.plan(600)
        self.assertEqual([(item.quantity, item.active_loan) for item in selected], [(100, True), (101, True)])
        before = selected
        different_mint = service.Job(MARKET, POLICY, MARKET, 101, True)
        with self.assertRaises(ValueError):
            self.history.sync_loans([job(102), different_mint], 600)
        self.assertEqual(self.history.plan(600), before)
        with self.assertRaises(ValueError):
            self.history.record(job(999), 600, 600, 1, 1)

    def test_policy_hash_export_and_stdin_cli_match_immutable_terms(self):
        s = settings()
        terms = {"historyEpochSeconds": s.epoch_seconds, "minimumHistoryEpochs": s.history_epochs,
                 "referenceMaxAgeSeconds": s.reference_max_age_seconds,
                 "referenceMaxAgeSlots": str(s.reference_max_age_slots),
                 "maxSpreadBps": s.max_spread_bps, "pricePolicyHash": list(bytes.fromhex(s.model_hash))}
        service.verify_policy(s, {"terms": terms})
        with self.assertRaises(ValueError):
            service.verify_policy(s, {"terms": dict(terms, minimumHistoryEpochs=3)})
        with self.assertRaises(ValueError):
            service.verify_policy(s, {"terms": dict(terms, pricePolicyHash="00"*32)})
        setting_file = Path(self.temp.name) / "settings.json"
        setting_file.write_text(json.dumps(service.asdict(s)))
        response = subprocess.run([sys.executable, str(SOURCE), "--settings", str(setting_file),
                                   "check-policy", "-"], input=json.dumps({"terms": terms}),
                                  capture_output=True, text=True, check=True)
        self.assertEqual(json.loads(response.stdout), {"valid": True, "model_hash": s.model_hash})

    def test_nonadvancing_history_write_rolls_back_without_replacing_evidence(self):
        self.history.record(job(), 600, 600, 100, 105, {"first": True})
        before = self.history.db.execute("SELECT * FROM observations").fetchall()
        for timestamp, slot in [(599, 601), (600, 601), (601, 600)]:
            with self.assertRaises(ValueError):
                self.history.record(job(), timestamp, slot, 1, 1, {"bad": True})
            self.assertEqual(self.history.db.execute("SELECT * FROM observations").fetchall(), before)

    def test_shared_database_provider_spacing_and_capacity_are_explicit(self):
        timer = FakeTime()
        other = service.History(self.path, settings())
        try:
            self.history.rate_gate(timer.clock, timer.sleep)
            other.rate_gate(timer.clock, timer.sleep)
            self.assertEqual(timer.now, 1001.0)
            self.history.rate_gate(timer.clock, timer.sleep)
            self.assertEqual(timer.now, 1002.0)
        finally:
            other.close()
        report = service.capacity_report(settings())
        self.assertFalse(report["spacing_fits_target"])
        self.assertFalse(report["ready_for_mainnet"])
        self.assertEqual(report["measured_latency_and_sustained_coverage"], "pending")

    def test_keyless_http_omits_api_header_and_keeps_exact_input_contract(self):
        captured = []

        class Response:
            def __init__(self, body):
                self.body = body

            def __enter__(self):
                return self

            def __exit__(self, *_):
                return None

            def read(self, _):
                return self.body

        def respond(request, timeout):
            captured.append(request)
            params = {name: values[0] for name, values in service.urllib.parse.parse_qs(
                service.urllib.parse.urlsplit(request.full_url).query).items()}
            return Response(service.canonical(Provider()(params)))

        timer = FakeTime()
        jupiter = service.Jupiter(settings(request_interval_ms=2000), MARKET, "",
                                  clock=timer.clock, sleeper=timer.sleep)
        with patch.object(service.urllib.request, "urlopen", side_effect=respond):
            quote = jupiter.quote(MINT, service.WSOL, 100)
        self.assertEqual(quote.amount, 100)
        self.assertFalse(captured[0].has_header("X-api-key"))

    def test_malformed_rpc_json_and_clock_results_are_unavailable(self):
        class Response:
            def __init__(self, value):
                self.body = service.canonical(value)
            def __enter__(self):
                return self
            def __exit__(self, *_):
                return None
            def read(self, _limit):
                return self.body
        clock = service.RpcClock("https://rpc.invalid")
        for reply in [None, [], 7, "not an RPC envelope", {},
                      {"error": {"code": -1}}, {"result": None},
                      {"result": True}, {"result": "not a clock"}]:
            with self.subTest(reply=reply):
                with patch.object(service.urllib.request, "urlopen", return_value=Response(reply)):
                    with self.assertRaises(service.Unavailable):
                        clock.read()
        with patch.object(service.urllib.request, "urlopen", side_effect=[Response({"result": 600}), Response({"result": 601})]):
            self.assertEqual(clock.read(), (600, 601))

    def test_invalid_rpc_configuration_does_not_echo_its_url(self):
        clock = service.RpcClock("invalid-rpc-url?dummy-private-marker")
        with patch.object(service.urllib.request, "urlopen") as transport:
            with self.assertRaises(service.Unavailable) as error:
                clock.read()
        self.assertNotIn("dummy-private-marker", str(error.exception))
        transport.assert_not_called()

    def test_interrupted_http_reads_are_sanitized_without_route_retry(self):
        class Response:
            def __init__(self, failure):
                self.failure = failure
            def __enter__(self):
                return self
            def __exit__(self, *_):
                return None
            def read(self, _limit):
                raise self.failure

        for endpoint in ("Jupiter", "RPC"):
            for failure in (service.http.client.IncompleteRead(b"dummy-private-marker", 1),
                            service.http.client.HTTPException("dummy-private-marker")):
                with self.subTest(endpoint=endpoint, failure=type(failure).__name__):
                    timer = FakeTime()
                    jupiter = service.Jupiter(settings(), MARKET, "", clock=timer.clock, sleeper=timer.sleep)
                    read = (lambda: jupiter.quote(MINT, service.WSOL, 100)) if endpoint == "Jupiter" else service.RpcClock("https://rpc.invalid").read
                    with patch.object(service.urllib.request, "urlopen", return_value=Response(failure)) as transport:
                        with self.assertRaises(service.Unavailable) as error:
                            read()
                    self.assertIs(type(error.exception), service.Unavailable)
                    self.assertNotIn("dummy-private-marker", str(error.exception))
                    transport.assert_called_once()

    def test_http_error_status_does_not_depend_on_readable_diagnostic_body(self):
        class BrokenBody:
            def __init__(self):
                self.reads = 0
            def read(self, _limit=None):
                self.reads += 1
                raise service.http.client.IncompleteRead(b"dummy-private-marker", 1)
            def close(self):
                pass

        for endpoint in ("Jupiter", "RPC"):
            for status in (400, 401, 429, 503):
                with self.subTest(endpoint=endpoint, status=status):
                    fallback = endpoint == "Jupiter" and status == 400
                    bodies = [BrokenBody() for _ in range(2 if fallback else 1)]
                    errors = [service.urllib.error.HTTPError("https://provider.invalid/?dummy-private-marker",
                              status, "dummy-private-marker", {}, body) for body in bodies]
                    timer = FakeTime()
                    jupiter = service.Jupiter(settings(), MARKET, "", clock=timer.clock, sleeper=timer.sleep)
                    read = (lambda: jupiter.quote(MINT, service.WSOL, 100)) if endpoint == "Jupiter" else service.RpcClock("https://rpc.invalid").read
                    with patch.object(service.urllib.request, "urlopen", side_effect=errors) as transport:
                        with self.assertRaises(service.Unavailable) as error:
                            read()
                    self.assertIs(type(error.exception), service.RouteUnavailable if fallback else service.Unavailable)
                    self.assertNotIn("dummy-private-marker", str(error.exception))
                    self.assertEqual(transport.call_count, len(bodies))
                    self.assertTrue(all(body.reads == 0 for body in bodies))

    def test_routing_budget_is_distinct_from_total_envelope_and_pinned_in_model(self):
        provider = Provider()
        client(settings(), provider).quote(MINT, service.WSOL, 100)
        self.assertEqual(provider.calls[0]["maxAccounts"], "16")

        def oversized(params):
            raw = Provider()(params)
            raw["swapInstruction"]["accounts"] *= 49
            return raw

        with self.assertRaises(service.Unavailable):
            client(settings(), oversized).quote(MINT, service.WSOL, 100)
        def excluded(params):
            raw = Provider()(params)
            raw["routePlan"] = [{"swapInfo": {"label": "Scorch"}}]
            return raw
        with self.assertRaises(service.Unavailable):
            client(settings(), excluded).quote(MINT, service.WSOL, 100)
        self.assertEqual(provider.calls[0]["excludeDexes"], "Scorch")
        original = settings().model_hash
        with patch.object(service, "REQUEST_ROUTE_ACCOUNT_BUDGET", 31):
            self.assertNotEqual(settings().model_hash, original)
        with patch.object(service, "EXCLUDED_DEXES", ""):
            self.assertNotEqual(settings().model_hash, original)
        with patch.object(service, "MAX_ROUTE_ACCOUNTS", 47):
            self.assertNotEqual(settings().model_hash, original)
        with patch.object(service, "MAX_ROUTE_DATA_BYTES", 767):
            self.assertNotEqual(settings().model_hash, original)


if __name__ == "__main__":
    unittest.main()
