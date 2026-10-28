import test from "node:test";
import assert from "node:assert/strict";
import {
  compileTransaction,
  createNoopSigner,
  createTransactionMessage,
  getTransactionEncoder,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  type Blockhash,
  address,
} from "@solana/kit";
import { submitSigned } from "@shortwin/simple-client";
import { IntentRevision } from "../src/intent.ts";
import {
  connectWallet,
  watchWallet,
  assertConnection,
  signTransaction,
  type Wallet,
  type WalletAccount,
} from "../src/wallet.ts";
const account: WalletAccount = {
  address: "11111111111111111111111111111111",
  publicKey: new Uint8Array(32),
  chains: ["solana:devnet"],
  features: ["solana:signTransaction"],
};
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => (resolve = r));
  return { promise, resolve };
}
function mockWallet(events = true) {
  let accounts: readonly WalletAccount[] = [account];
  let listener: (value: unknown) => void = () => {};
  let unsubscribed = 0;
  const response = deferred<{ signedTransaction: Uint8Array }[]>();
  const wallet: Wallet = {
    name: "Mock",
    get accounts() {
      return accounts;
    },
    features: {
      "standard:connect": { connect: async () => ({ accounts }) },
      "solana:signTransaction": { signTransaction: () => response.promise },
      ...(events
        ? {
            "standard:events": {
              on: (_event: string, cb: (value: unknown) => void) => {
                listener = cb;
                return () => {
                  listener = () => {};
                  unsubscribed++;
                };
              },
            },
          }
        : {}),
    },
  };
  return {
    wallet,
    response,
    change(next: readonly WalletAccount[]) {
      accounts = next;
      listener({ accounts: next });
    },
    featureChange() {
      listener({ features: wallet.features });
    },
    get unsubscribed() {
      return unsubscribed;
    },
  };
}
test("editing or cancelling while route preparation is pending revokes later review", async () => {
  for (const action of ["edit", "cancel"]) {
    const intent = new IntentRevision(),
      quote = deferred<string>();
    const expected = intent.capture();
    const prepared = intent.guard(expected, quote.promise);
    intent.invalidate();
    quote.resolve("old amount");
    await assert.rejects(prepared, /changed/, action);
    const current = intent.capture();
    assert.doesNotThrow(() => intent.assert(current));
    assert.throws(() => intent.assert(expected), /changed/);
  }
});
test("wallet account or feature changes while signing prevent broadcast and clean old subscriptions", async () => {
  for (const mode of ["account", "features", "cancel"]) {
    const mock = mockWallet();
    const connected = await connectWallet(mock.wallet);
    const intent = new IntentRevision();
    const off = watchWallet(connected, () => intent.invalidate());
    const expected = intent.capture();
    let broadcasts = 0;
    const signing = (async () => {
      const signed = await intent.guard(
        expected,
        signTransaction(connected, new Uint8Array([1]), "solana:devnet"),
      );
      broadcasts++;
      return signed;
    })();
    if (mode === "account") mock.change([]);
    else if (mode === "features") mock.featureChange();
    else intent.invalidate();
    mock.response.resolve([{ signedTransaction: new Uint8Array([2]) }]);
    await assert.rejects(signing, /changed|disconnected/);
    assert.equal(broadcasts, 0);
    off();
    assert.equal(mock.unsubscribed, 1);
    const revision = intent.capture();
    mock.featureChange();
    assert.equal(intent.capture(), revision, "old subscription removed");
  }
});
test("wallets without change events are rechecked after their async reply, including chain support", async () => {
  for (const next of [
    [],
    [{ ...account, chains: ["solana:mainnet"] }],
    [{ ...account, publicKey: new Uint8Array(32).fill(1) }],
  ]) {
    const mock = mockWallet(false);
    const connected = await connectWallet(mock.wallet);
    assertConnection(connected, "solana:devnet");
    const signing = signTransaction(
      connected,
      new Uint8Array([1]),
      "solana:devnet",
    );
    mock.change(next);
    mock.response.resolve([{ signedTransaction: new Uint8Array([2]) }]);
    await assert.rejects(signing, /changed|supported/);
  }
});

test("cancel during the final blockheight check prevents even the broadcast RPC call", async () => {
  const owner = address("11111111111111111111111111111111");
  const message = setTransactionMessageLifetimeUsingBlockhash(
    {
      blockhash: "11111111111111111111111111111111" as Blockhash,
      lastValidBlockHeight: 100n,
    },
    setTransactionMessageFeePayerSigner(
      createNoopSigner(owner),
      createTransactionMessage({ version: 0 }),
    ),
  );
  const bytes = Uint8Array.from(
    getTransactionEncoder().encode(compileTransaction(message)),
  );
  const signed = bytes.slice();
  signed.fill(1, 1, 65);
  const prepared = {
    bytes,
    base64: "",
    lastValidBlockHeight: 100n,
    units: 1n,
    feeLamports: 1n,
    logs: [],
  };
  const intent = new IntentRevision(),
    revision = intent.capture(),
    entered = deferred<void>(),
    release = deferred<void>();
  let broadcasts = 0;
  const original = globalThis.fetch;
  globalThis.fetch = async (input, init) => {
    const request = new Request(input, init);
    const body = (await request.json()) as { method: string; id: unknown };
    if (body.method === "sendTransaction") {
      broadcasts++;
      throw new Error("must not broadcast");
    }
    assert.equal(body.method, "getBlockHeight");
    entered.resolve();
    await release.promise;
    return new Response(
      JSON.stringify({ jsonrpc: "2.0", id: body.id, result: 1 }),
      { headers: { "content-type": "application/json" } },
    );
  };
  try {
    const submission = submitSigned(
      "https://rpc.invalid.test",
      prepared,
      signed,
      () => intent.assert(revision),
    );
    await entered.promise;
    intent.invalidate();
    release.resolve();
    await assert.rejects(submission, /changed/);
    assert.equal(broadcasts, 0);
  } finally {
    globalThis.fetch = original;
  }
});
