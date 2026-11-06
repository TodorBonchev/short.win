import test from "node:test";
import assert from "node:assert/strict";
import {
  address,
  blockhash,
  compileTransaction,
  createNoopSigner,
  createTransactionMessage,
  getTransactionDecoder,
  getTransactionEncoder,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
} from "@solana/kit";
import { installLocalDemoWallet, isLoopbackUrl, LOCAL_DEMO_ADDRESS } from "../src/local-demo-wallet.ts";
import { connectWallet, signTransaction, type Wallet } from "../src/wallet.ts";
const GENESIS = "EJw55eCjS1AV1VPCMrUpzhZjoqVPkpAiPeSCQectXYor";
function fixture() {
  const target = new EventTarget();
  let rpc = "http://127.0.0.1:8899", service = "http://127.0.0.1:8787";
  let wallet: Wallet | undefined;
  const options = { enabled: true, hostname: () => "localhost", rpcUrl: () => rpc, serviceUrl: () => service, target };
  return {
    target, options,
    setRpc: (value: string) => { rpc = value; },
    setService: (value: string) => { service = value; },
    wallet() {
      target.dispatchEvent(new CustomEvent("wallet-standard:app-ready", {
        detail: { register: (value: Wallet) => { wallet = value; } },
      }));
      assert.ok(wallet);
      return wallet;
    },
  };
}
function wire(payer = LOCAL_DEMO_ADDRESS) {
  return Uint8Array.from(getTransactionEncoder().encode(compileTransaction(
    setTransactionMessageLifetimeUsingBlockhash({
      blockhash: blockhash("11111111111111111111111111111111"), lastValidBlockHeight: 100n,
    }, setTransactionMessageFeePayerSigner(createNoopSigner(address(payer)), createTransactionMessage({ version: 0 }))),
  )));
}
function localFetch(options: { genesis?: string; valid?: boolean; onValid?: () => void } = {}) {
  const original = globalThis.fetch;
  const methods: string[] = [];
  globalThis.fetch = async (input, init) => {
    assert.ok(init?.signal instanceof AbortSignal, "every local handshake request has a deadline signal");
    assert.equal(init.signal.aborted, false);
    const request = new Request(input, init);
    assert.equal(new URL(request.url).hostname, "127.0.0.1", "all requests stay on loopback");
    assert.equal(request.redirect, "error", "redirects cannot escape to an external server");
    assert.equal(request.credentials, "omit");
    if (request.url === "http://127.0.0.1:8787/demo/info")
      return Response.json({ genesis: GENESIS, mode: "local-fork", testFunds: true, syntheticHistory: true });
    assert.equal(request.url, "http://127.0.0.1:8899/");
    const body = await request.json() as { method: string; id: unknown };
    methods.push(body.method);
    assert.ok(["getGenesisHash", "isBlockhashValid"].includes(body.method), "wallet never sends transactions");
    if (body.method === "isBlockhashValid") options.onValid?.();
    return Response.json({ jsonrpc: "2.0", id: body.id, result: body.method === "getGenesisHash" ? options.genesis ?? GENESIS : { context: { slot: 1 }, value: options.valid ?? true } });
  };
  return { methods, restore: () => { globalThis.fetch = original; } };
}

test("local demo wallet requires an explicit flag and loopback page, RPC, and service", () => {
  for (const value of ["https://rpc.example", "http://localhost.evil.test", "http://user:password@localhost:8899", "file:///tmp/rpc", "not a URL"])
    assert.equal(isLoopbackUrl(value), false, value);
  for (const value of ["http://localhost:8899", "http://127.0.0.1:8899"])
    assert.equal(isLoopbackUrl(value), true);
  for (const override of [
    { enabled: false }, { hostname: () => "example.com" },
    { rpcUrl: () => "https://api.mainnet-beta.solana.com" },
    { serviceUrl: () => "https://service.example" },
  ]) {
    const f = fixture();
    assert.equal(installLocalDemoWallet({ ...f.options, ...override }), false);
  }
});

test("local demo wallet registers through Wallet Standard and signs the original message with the public test identity", async () => {
  const f = fixture(), network = localFetch();
  try {
    assert.equal(installLocalDemoWallet(f.options), true);
    const wallet = f.wallet();
    assert.equal(wallet.name, "Local demo wallet");
    assert.deepEqual(Object.keys(wallet.features).sort(), ["solana:signTransaction", "standard:connect", "standard:events"]);
    assert.deepEqual(wallet.accounts, []);
    const connected = await connectWallet(wallet);
    assert.equal(connected.address, LOCAL_DEMO_ADDRESS);
    assert.deepEqual(connected.account.chains, ["solana:devnet"]);
    const original = wire(), signed = await signTransaction(connected, original, "solana:devnet");
    const decoded = getTransactionDecoder().decode(signed);
    assert.deepEqual(decoded.messageBytes, getTransactionDecoder().decode(original).messageBytes);
    const signature = decoded.signatures[connected.address];
    assert.ok(signature);
    const publicKey = await crypto.subtle.importKey("raw", Uint8Array.from(connected.account.publicKey), "Ed25519", false, ["verify"]);
    assert.equal(await crypto.subtle.verify("Ed25519", publicKey, Uint8Array.from(signature), Uint8Array.from(decoded.messageBytes)), true);
    assert.deepEqual(network.methods, ["getGenesisHash", "getGenesisHash", "isBlockhashValid"]);
  } finally { network.restore(); }
});

test("local demo connection refuses a genesis mismatch", async () => {
  const f = fixture(), network = localFetch({ genesis: "11111111111111111111111111111111" });
  try {
    installLocalDemoWallet(f.options);
    await assert.rejects(connectWallet(f.wallet()), /genesis does not match/);
  } finally { network.restore(); }
});

test("local demo wallet refuses expired blockhashes and a different fee payer", async () => {
  const f = fixture(), network = localFetch({ valid: false });
  try {
    installLocalDemoWallet(f.options);
    const connected = await connectWallet(f.wallet());
    await assert.rejects(signTransaction(connected, wire(), "solana:devnet"), /blockhash is not valid/);
    await assert.rejects(signTransaction(connected, wire("11111111111111111111111111111111"), "solana:devnet"), /fee payer/);
  } finally { network.restore(); }
});

test("local demo wallet rejects endpoint edits before and during signing", async () => {
  const f = fixture(), network = localFetch({ onValid: () => { f.setService("https://service.example"); } });
  try {
    installLocalDemoWallet(f.options);
    const connected = await connectWallet(f.wallet());
    await assert.rejects(signTransaction(connected, wire(), "solana:devnet"), /loopback/);
    f.setService("http://127.0.0.1:8787");
    f.setRpc("https://api.mainnet-beta.solana.com");
    const requests = network.methods.length;
    await assert.rejects(signTransaction(connected, wire(), "solana:devnet"), /loopback/);
    assert.equal(network.methods.length, requests, "changed external endpoints are never contacted");
  } finally { network.restore(); }
});
