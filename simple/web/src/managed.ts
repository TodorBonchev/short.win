/** Browser transport to the configured in-house service; credentials stay server-side. */
export async function managedCall(
  base: string,
  path: "/v1/route" | "/v1/requests",
  payload: unknown,
  send: typeof fetch = fetch,
): Promise<Record<string, unknown>> {
  const root = new URL(base);
  if (
    root.protocol !== "https:" &&
    !(
      root.protocol === "http:" &&
      ["127.0.0.1", "localhost", "[::1]"].includes(root.hostname)
    )
  )
    throw new Error("Use HTTPS for a hosted price service");
  const url = new URL(path, root);
  const response = await send(url, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(payload, (_k, v) =>
      typeof v === "bigint" ? v.toString() : v,
    ),
    signal: AbortSignal.timeout(45000),
  });
  const data = (await response.json()) as Record<string, unknown>;
  if (!response.ok)
    throw new Error(
      typeof data.error === "string"
        ? data.error
        : "In-house service unavailable",
    );
  return data;
}

/** Buy routes may use less than the user's budget, but never silently raise it. */
export function boundBuyInput(value: unknown, budget: bigint): bigint {
  if (typeof value !== "string" || !/^(0|[1-9][0-9]*)$/.test(value))
    throw new Error("Invalid service buyback input");
  const actual = BigInt(value);
  if (actual <= 0n || actual > 0xffffffffffffffffn || actual > budget)
    throw new Error("Service route exceeds your buyback input budget");
  return actual;
}
