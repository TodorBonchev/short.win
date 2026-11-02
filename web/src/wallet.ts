import { address, type Address } from "@solana/kit";
export type WalletAccount = {
  address: string;
  publicKey: Uint8Array;
  chains: readonly string[];
  features: readonly string[];
};
type Feature = Record<string, unknown>;
export type Wallet = {
  name: string;
  accounts: readonly WalletAccount[];
  features: Record<string, Feature>;
};
export type Connection = {
  wallet: Wallet;
  account: WalletAccount;
  address: Address;
};
export const wallets: Wallet[] = [];
const listeners = new Set<() => void>();
function register(...values: Wallet[]) {
  for (const w of values) {
    if (
      !wallets.includes(w) &&
      w.features["standard:connect"] &&
      w.features["solana:signTransaction"]
    )
      wallets.push(w);
  }
  for (const f of listeners) f();
  return () => {
    for (const w of values) {
      const i = wallets.indexOf(w);
      if (i >= 0) wallets.splice(i, 1);
    }
    for (const f of listeners) f();
  };
}
const api = Object.freeze({ register, get: () => wallets });
export function discoverWallets(changed: () => void): void {
  listeners.add(changed);
  window.addEventListener("wallet-standard:register-wallet", ((
    e: CustomEvent<(walletApi: typeof api) => void>,
  ) => e.detail(api)) as EventListener);
  window.dispatchEvent(
    new CustomEvent("wallet-standard:app-ready", { detail: api }),
  );
  changed();
}
export async function connectWallet(wallet: Wallet): Promise<Connection> {
  const connect = wallet.features["standard:connect"]?.connect;
  if (typeof connect !== "function") throw new Error("Wallet cannot connect");
  const result = await (
    connect as () => Promise<{
      accounts: readonly WalletAccount[];
    }>
  ).call(wallet.features["standard:connect"]);
  const account = result.accounts.find((a) =>
    a.features.includes("solana:signTransaction"),
  );
  if (!account)
    throw new Error("No Solana transaction-signing account was provided");
  return { wallet, account, address: address(account.address) };
}
/** A Wallet Standard change revokes the current connection, even if the old address remains authorized. */
export function watchWallet(
  connection: Connection,
  changed: () => void,
): () => void {
  const on = connection.wallet.features["standard:events"]?.on;
  if (typeof on !== "function") return () => {};
  const off = (
    on as (event: string, listener: (properties: unknown) => void) => unknown
  ).call(connection.wallet.features["standard:events"], "change", changed);
  return typeof off === "function" ? (off as () => void) : () => {};
}
export function assertConnection(connection: Connection, chain?: string): void {
  const account = connection.wallet.accounts.find(
    (a) => a.address === connection.address,
  );
  if (
    !account ||
    account.publicKey.length !== connection.account.publicKey.length ||
    account.publicKey.some((b, i) => b !== connection.account.publicKey[i]) ||
    !account.features.includes("solana:signTransaction") ||
    typeof connection.wallet.features["solana:signTransaction"]
      ?.signTransaction !== "function"
  )
    throw new Error(
      "Wallet account changed or disconnected; reconnect the wallet",
    );
  if (
    chain &&
    (!connection.account.chains.includes(chain) ||
      !account.chains.includes(chain))
  )
    throw new Error("Select a chain supported by the connected wallet account");
}
export async function signTransaction(
  connection: Connection,
  bytes: Uint8Array,
  chain: string,
): Promise<Uint8Array> {
  assertConnection(connection, chain);
  const sign =
    connection.wallet.features["solana:signTransaction"]?.signTransaction;
  if (typeof sign !== "function")
    throw new Error("Wallet cannot sign transactions");
  const result = await (
    sign as (...args: unknown[]) => Promise<
      {
        signedTransaction: Uint8Array;
      }[]
    >
  ).call(connection.wallet.features["solana:signTransaction"], {
    account: connection.account,
    chain,
    transaction: bytes,
  });
  assertConnection(connection, chain);
  const signed = result[0]?.signedTransaction;
  if (!(signed instanceof Uint8Array))
    throw new Error("Wallet returned no signed transaction");
  return signed;
}
