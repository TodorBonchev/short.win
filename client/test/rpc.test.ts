import test from "node:test";
import assert from "node:assert/strict";
import {
  address,
  compileTransaction,
  createNoopSigner,
  createTransactionMessage,
  getTransactionEncoder,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  type Blockhash,
} from "@solana/kit";
import { RpcTimeoutError, sendRpc } from "../src/rpc.ts";
import { OutcomeUnknownError, signedTransactionSignature, submitSigned } from "../src/transactions.ts";

test("RPC deadline cancels a stalled request and releases its caller", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let signal: AbortSignal | undefined;
  const request = sendRpc({
    send: (options) => {
      signal = options?.abortSignal;
      return new Promise<never>(() => {});
    },
  });
  const rejected = assert.rejects(request, RpcTimeoutError);
  t.mock.timers.tick(15_000);
  await rejected;
  assert.equal(signal?.aborted, true);
});

test("completed RPC requests clear their deadline and retain ordinary errors", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let signal: AbortSignal | undefined;
  assert.equal(await sendRpc({ send: async (options) => { signal = options?.abortSignal; return 7; } }), 7);
  t.mock.timers.tick(15_000);
  assert.equal(signal?.aborted, false);
  const error = new Error("RPC unavailable");
  await assert.rejects(sendRpc({ send: async () => { throw error; } }), (received) => received === error);
});

test("a deadline after broadcast preserves the signature for reconciliation", async (t) => {
  // This transport regression handles every request with the fetch stub below;
  // the dummy signature is never sent to an RPC service.
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const message = setTransactionMessageLifetimeUsingBlockhash(
    { blockhash: "11111111111111111111111111111111" as Blockhash, lastValidBlockHeight: 100n },
    setTransactionMessageFeePayerSigner(createNoopSigner(address("11111111111111111111111111111111")), createTransactionMessage({ version: 0 })),
  );
  const bytes = Uint8Array.from(getTransactionEncoder().encode(compileTransaction(message)));
  const signed = bytes.slice();
  signed.fill(1, 1, 65);
  const prepared = { bytes, base64: "", lastValidBlockHeight: 100n, units: 1n, feeLamports: 1n, logs: [] };
  let entered!: () => void;
  const broadcast = new Promise<void>((resolve) => { entered = resolve; });
  const original = globalThis.fetch;
  globalThis.fetch = async (input, init) => {
    const request = new Request(input, init);
    const body = await request.json() as { method: string; id: unknown };
    if (body.method === "getBlockHeight")
      return new Response(JSON.stringify({ jsonrpc: "2.0", id: body.id, result: 1 }), { headers: { "content-type": "application/json" } });
    assert.equal(body.method, "sendTransaction");
    entered();
    return new Promise<never>((_resolve, reject) => {
      request.signal.addEventListener("abort", () => reject(request.signal.reason), { once: true });
    });
  };
  try {
    const submission = submitSigned("https://rpc.invalid.test", prepared, signed);
    await broadcast;
    const rejected = assert.rejects(submission, (error) => {
      assert(error instanceof OutcomeUnknownError);
      assert.equal(error.signature, signedTransactionSignature(signed));
      assert(error.cause instanceof RpcTimeoutError);
      return true;
    });
    t.mock.timers.tick(15_000);
    await rejected;
  } finally {
    globalThis.fetch = original;
  }
});
