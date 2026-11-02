import {
  assertIsTransactionWithinSizeLimit,
  createKeyPairSignerFromPrivateKeyBytes,
  getBase58Encoder,
  getCompiledTransactionMessageDecoder,
  getTransactionDecoder,
  getTransactionEncoder,
  getTransactionLifetimeConstraintFromCompiledTransactionMessage,
  type KeyPairSigner,
} from "@solana/kit";
import type { Wallet, WalletAccount } from "./wallet.ts";

/** This seed is public test data. It must never hold funds on a public cluster. */
export const LOCAL_DEMO_ADDRESS = "3wpYnGqceZ8DzN3guiTd9rrYkWTwTHCChBSuo6cvkXTG";
const CHAIN = "solana:devnet";
type Options = {
  enabled: boolean;
  hostname: () => string;
  rpcUrl: () => string;
  serviceUrl: () => string;
  target?: EventTarget;
};
type Settings = { rpc: string; service: string };
type RegistrationApi = { register: (...wallets: Wallet[]) => unknown };
type SignInput = { account: WalletAccount; chain?: string; transaction: Uint8Array };

export function isLoopbackUrl(value: string): boolean {
  try {
    const url = new URL(value);
    return (url.protocol === "http:" || url.protocol === "https:") &&
      (url.hostname === "localhost" || url.hostname === "127.0.0.1") &&
      !url.username && !url.password;
  } catch {
    return false;
  }
}
function settings(options: Options): Settings {
  if (!options.enabled || !["localhost", "127.0.0.1"].includes(options.hostname()))
    throw new Error("The local demo wallet is available only on a local demo page");
  const rpc = options.rpcUrl().trim(), service = options.serviceUrl().trim();
  if (!isLoopbackUrl(rpc) || !isLoopbackUrl(service))
    throw new Error("The local demo wallet requires loopback RPC and service URLs");
  return { rpc, service };
}
function assertUnchanged(options: Options, expected: Settings) {
  const current = settings(options);
  if (current.rpc !== expected.rpc || current.service !== expected.service)
    throw new Error("Local demo connection settings changed; reconnect the wallet");
}
async function localRpc(url: string, method: "getGenesisHash" | "isBlockhashValid", params: unknown[] = []): Promise<unknown> {
  if (!isLoopbackUrl(url)) throw new Error("Local demo RPC must use loopback");
  const response = await fetch(url, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: 1, method, params }),
    credentials: "omit",
    redirect: "error",
  });
  if (!response.ok) throw new Error("Local demo RPC is unavailable");
  const result: unknown = await response.json();
  if (!result || typeof result !== "object" || !("result" in result) || "error" in result)
    throw new Error("Local demo RPC returned an invalid response");
  return result.result;
}
async function verifyLocalChain(current: Settings): Promise<string> {
  const response = await fetch(new URL("/demo/info", current.service), {
    credentials: "omit", redirect: "error", cache: "no-store",
  });
  if (!response.ok) throw new Error("The local demo service is unavailable");
  const info: unknown = await response.json();
  if (!info || typeof info !== "object" || !("genesis" in info) || typeof info.genesis !== "string" ||
    !("mode" in info) || info.mode !== "local-fork" || !("testFunds" in info) || info.testFunds !== true)
    throw new Error("The service has not identified a local fork with test funds");
  if (getBase58Encoder().encode(info.genesis).length !== 32)
    throw new Error("The local demo genesis hash is invalid");
  const genesis = await localRpc(current.rpc, "getGenesisHash");
  if (genesis !== info.genesis)
    throw new Error("Local RPC genesis does not match the local demo service");
  return info.genesis;
}

/** Register a signing-only Wallet Standard provider. It has no transaction-send feature. */
export function installLocalDemoWallet(options: Options): boolean {
  try { settings(options); } catch { return false; }
  const target = options.target ?? window;
  const listeners = new Set<(properties: { accounts: readonly WalletAccount[] }) => void>();
  let account: WalletAccount | undefined;
  let connected: (Settings & { genesis: string }) | undefined;
  let signer: KeyPairSigner | undefined;
  const wallet: Wallet & { version: string; chains: readonly string[]; icon: string } = {
    version: "1.0.0",
    name: "Local demo wallet",
    icon: "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='64' height='64'%3E%3Crect width='64' height='64' rx='12' fill='%23c5f777'/%3E%3Cpath d='M19 43 44 18M22 18h22v22' fill='none' stroke='%2317240e' stroke-width='6'/%3E%3C/svg%3E",
    chains: [CHAIN],
    get accounts() { return account ? [account] : []; },
    features: {
      "standard:connect": {
        version: "1.0.0",
        connect: async () => {
          const current = settings(options);
          const genesis = await verifyLocalChain(current);
          assertUnchanged(options, current);
          signer ??= await createKeyPairSignerFromPrivateKeyBytes(new Uint8Array(32).fill(100));
          assertUnchanged(options, current);
          if (signer.address !== LOCAL_DEMO_ADDRESS)
            throw new Error("The public test wallet address did not match");
          connected = { ...current, genesis };
          const nextAccount: WalletAccount = {
            address: signer.address,
            publicKey: Uint8Array.from(getBase58Encoder().encode(signer.address)),
            chains: [CHAIN],
            features: ["solana:signTransaction"],
          };
          account = nextAccount;
          for (const listener of listeners) listener({ accounts: [nextAccount] });
          return { accounts: [nextAccount] };
        },
      },
      "standard:events": {
        version: "1.0.0",
        on: (event: string, listener: (properties: { accounts: readonly WalletAccount[] }) => void) => {
          if (event === "change") listeners.add(listener);
          return () => { listeners.delete(listener); };
        },
      },
      "solana:signTransaction": {
        version: "1.0.0",
        supportedTransactionVersions: ["legacy", 0],
        signTransaction: async (...inputs: SignInput[]) => {
          const active = connected, activeAccount = account, activeSigner = signer;
          if (!active || !activeAccount || !activeSigner)
            throw new Error("Connect the local demo wallet first");
          const output: { signedTransaction: Uint8Array }[] = [];
          for (const input of inputs) {
            assertUnchanged(options, active);
            if (input.account !== activeAccount || (input.chain !== undefined && input.chain !== CHAIN))
              throw new Error("The local demo wallet supports only its local test account and Devnet signing chain");
            if (!(input.transaction instanceof Uint8Array))
              throw new Error("No transaction bytes were provided");
            const transaction = getTransactionDecoder().decode(input.transaction.slice());
            assertIsTransactionWithinSizeLimit(transaction);
            const message = getCompiledTransactionMessageDecoder().decode(transaction.messageBytes);
            if (message.staticAccounts[0] !== activeSigner.address)
              throw new Error("The local demo wallet must be the transaction fee payer");
            const lifetimeConstraint = await getTransactionLifetimeConstraintFromCompiledTransactionMessage(message);
            if (!("blockhash" in lifetimeConstraint))
              throw new Error("The local demo wallet requires a recent local blockhash");
            const genesis = await verifyLocalChain(active);
            if (genesis !== active.genesis)
              throw new Error("The local fork changed; reconnect and prepare a new transaction");
            const valid = await localRpc(active.rpc, "isBlockhashValid", [lifetimeConstraint.blockhash, { commitment: "confirmed" }]);
            if (!valid || typeof valid !== "object" || !("value" in valid) || valid.value !== true)
              throw new Error("The transaction blockhash is not valid on the local fork; prepare again");
            assertUnchanged(options, active);
            if (connected !== active || account !== activeAccount)
              throw new Error("The local demo account changed; reconnect");
            const [signatures] = await activeSigner.signTransactions([{ ...transaction, lifetimeConstraint }]);
            assertUnchanged(options, active);
            if (connected !== active || account !== activeAccount)
              throw new Error("The local demo account changed while signing");
            output.push({ signedTransaction: Uint8Array.from(getTransactionEncoder().encode({
              ...transaction,
              signatures: { ...transaction.signatures, ...signatures },
            })) });
          }
          return output;
        },
      },
    },
  };
  const register = (api: RegistrationApi) => { api.register(wallet); };
  target.addEventListener("wallet-standard:app-ready", ((event: CustomEvent<RegistrationApi>) => register(event.detail)) as EventListener);
  target.dispatchEvent(new CustomEvent("wallet-standard:register-wallet", { detail: register }));
  return true;
}
