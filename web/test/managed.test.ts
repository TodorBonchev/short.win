import { test } from "node:test";
import assert from "node:assert/strict";
import { boundBuyInput, managedCall } from "../src/managed.ts";
test("managed service preserves exact quantities and uses fixed endpoints", async () => {
  let body = "";
  let endpoint = "";
  const fake = (async (input: RequestInfo | URL, init?: RequestInit) => {
    endpoint = String(input);
    body = String(init?.body);
    return new Response(JSON.stringify({ status: "Warming" }), { status: 202 });
  }) as typeof fetch;
  const result = await managedCall(
    "http://127.0.0.1:8787",
    "/v1/requests",
    { quantity: 9007199254740993n },
    fake,
  );
  assert.equal(result.status, "Warming");
  assert.equal(JSON.parse(body).quantity, "9007199254740993");
  assert.equal(endpoint, "http://127.0.0.1:8787/v1/requests");
});
test("managed service rejects insecure hosted URLs and reports capacity refusals", async () => {
  await assert.rejects(
    managedCall("http://example.com", "/v1/requests", {}),
    /HTTPS/,
  );
  const fake = (async () =>
    new Response(JSON.stringify({ error: "History queue full" }), {
      status: 503,
    })) as typeof fetch;
  await assert.rejects(
    managedCall("https://prices.example.com", "/v1/requests", {}, fake),
    /History queue full/,
  );
});

test("managed buy input can shrink but cannot exceed or reinterpret the signed user budget", () => {
  assert.equal(
    boundBuyInput("9007199254740993", 9007199254740994n),
    9007199254740993n,
  );
  assert.equal(boundBuyInput("17", 17n), 17n);
  for (const value of [
    "18",
    "0",
    "-1",
    "+1",
    "1e1",
    "01",
    "1.5",
    "18446744073709551616",
    17,
  ])
    assert.throws(() => boundBuyInput(value, 17n));
});
