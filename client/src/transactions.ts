import {
  appendTransactionMessageInstructions,
  compileTransaction,
  compressTransactionMessageUsingAddressLookupTables,
  createNoopSigner,
  createSolanaRpc,
  createTransactionMessage,
  getBase64EncodedWireTransaction,
  getBase64Decoder,
  getBase64Encoder,
  getSignatureFromTransaction,
  getTransactionDecoder,
  getTransactionEncoder,
  getTransactionSize,
  setTransactionMessageComputeUnitLimit,
  setTransactionMessageComputeUnitPrice,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  type Address,
  type AddressesByLookupTableAddress,
  type Instruction,
  type Signature,
  type TransactionMessageBytesBase64,
} from "@solana/kit";
import { sendRpc } from "./rpc.ts";
const json = (value: unknown) =>
  JSON.stringify(value, (_key, v) =>
    typeof v === "bigint" ? v.toString() : v,
  );
export type PreparedTransaction = {
  bytes: Uint8Array;
  base64: string;
  lastValidBlockHeight: bigint;
  units: bigint;
  feeLamports: bigint;
  logs: readonly string[];
};
export async function prepareTransaction(
  url: string,
  payer: Address,
  instructions: readonly Instruction[],
  tables: AddressesByLookupTableAddress = {},
  microLamports = 0n,
): Promise<PreparedTransaction> {
  if (microLamports < 0n || microLamports > 1_000_000n)
    throw new Error("Priority fee is outside the client cap");
  const rpc = createSolanaRpc(url);
  const { value: lifetime } = await sendRpc(
    rpc.getLatestBlockhash({ commitment: "confirmed" }),
  );
  // Keep one lifetime across the probe, final fee quote and wallet review. A
  // refreshed blockhash would produce a different message that needs new review.
  const build = (limit: number) => {
    const m0 = setTransactionMessageFeePayerSigner(
      createNoopSigner(payer),
      createTransactionMessage({ version: 0 }),
    );
    const m1 = setTransactionMessageLifetimeUsingBlockhash(lifetime, m0);
    const m2 = setTransactionMessageComputeUnitLimit(limit, m1);
    const m3 = setTransactionMessageComputeUnitPrice(microLamports, m2);
    const m4 = appendTransactionMessageInstructions([...instructions], m3);
    return compileTransaction(
      compressTransactionMessageUsingAddressLookupTables(m4, tables),
    );
  };
  const probe = build(1_400_000);
  if (getTransactionSize(probe) > 1232)
    throw new Error(
      "Transaction exceeds 1232 bytes; supply verified on-chain address lookup tables",
    );
  const { value: simulation } = await sendRpc(
    rpc.simulateTransaction(getBase64EncodedWireTransaction(probe), {
      encoding: "base64",
      sigVerify: false,
      commitment: "confirmed",
    }),
  );
  if (simulation.err !== null)
    throw new Error(
      "Simulation failed: " +
        json(simulation.err) +
        "\n" +
        (simulation.logs ?? []).join("\n"),
    );
  const units = simulation.unitsConsumed ?? 400_000n;
  const limit = Math.min(
    1_400_000,
    Math.max(20_000, Number((units * 115n + 99n) / 100n) + 10_000),
  );
  const tx = build(limit);
  const bytes = Uint8Array.from(getTransactionEncoder().encode(tx));
  const { value: fee } = await sendRpc(
    rpc.getFeeForMessage(
      getBase64Decoder().decode(
        tx.messageBytes,
      ) as TransactionMessageBytesBase64,
      { commitment: "confirmed" },
    ),
  );
  if (fee === null)
    throw new Error("Fee quote unavailable or blockhash expired");
  return {
    bytes,
    base64: getBase64Decoder().decode(bytes),
    lastValidBlockHeight: lifetime.lastValidBlockHeight,
    units,
    feeLamports: fee,
    logs: simulation.logs ?? [],
  };
}
export function assertSameMessage(
  expected: Uint8Array,
  signed: Uint8Array,
): void {
  const decoder = getTransactionDecoder();
  const a = decoder.decode(expected),
    b = decoder.decode(signed);
  if (
    a.messageBytes.length !== b.messageBytes.length ||
    a.messageBytes.some((x, i) => x !== b.messageBytes[i])
  )
    throw new Error(
      "Wallet returned a changed transaction; prepare it again for review",
    );
}

/** A signed transaction was handed to RPC, but the client cannot prove its final outcome. */
export class OutcomeUnknownError extends Error {
  readonly signature: Signature;
  override readonly cause: unknown;

  constructor(signature: Signature, reason: string, cause?: unknown) {
    super(`${reason} Signature: ${signature}. Reconcile it before signing another transaction.`);
    this.name = "OutcomeUnknownError";
    this.signature = signature;
    this.cause = cause;
  }
}

/** RPC proved that the signed transaction failed on chain. A fresh transaction is safe. */
export class TransactionFailedError extends Error {
  readonly signature: Signature;

  constructor(signature: Signature, error: unknown) {
    super(`Transaction failed ${signature}: ${json(error)}`);
    this.name = "TransactionFailedError";
    this.signature = signature;
  }
}

/** Derive the canonical signature locally; never depend on a send response to recover it. */
export function signedTransactionSignature(signed: Uint8Array): Signature {
  return getSignatureFromTransaction(getTransactionDecoder().decode(signed));
}

export async function submitSigned(
  url: string,
  prepared: PreparedTransaction,
  signed: Uint8Array,
  beforeBroadcast?: (signature: Signature) => void,
): Promise<Signature> {
  if (signed.length > 1232)
    throw new Error("Signed transaction exceeds packet limit");
  assertSameMessage(prepared.bytes, signed);
  const transaction = getTransactionDecoder().decode(signed);
  const signature = getSignatureFromTransaction(transaction);
  const rpc = createSolanaRpc(url);
  if (
    (await sendRpc(rpc.getBlockHeight({ commitment: "confirmed" }))) >
    prepared.lastValidBlockHeight
  )
    throw new Error("Reviewed transaction expired before send");
  beforeBroadcast?.(signature);
  let returned: Signature;
  try {
    // Even a transport deadline here cannot prove the node did not accept it.
    // Keep the locally derived signature for reconciliation instead of retrying.
    returned = await sendRpc(
      rpc.sendTransaction(getBase64EncodedWireTransaction(transaction), {
        encoding: "base64",
        skipPreflight: false,
        maxRetries: 2n,
      }),
    );
  } catch (cause) {
    throw new OutcomeUnknownError(
      signature,
      "Submission failed after broadcast began; the transaction may still land.",
      cause,
    );
  }
  if (returned !== signature)
    throw new OutcomeUnknownError(
      signature,
      `RPC returned a different signature (${returned}); the signed transaction outcome is unknown.`,
    );
  for (let i = 0; i < 60; i++) {
    let status;
    try {
      ({ value: [status] } = await sendRpc(rpc.getSignatureStatuses([signature])));
    } catch (cause) {
      throw new OutcomeUnknownError(
        signature,
        "Status lookup failed after submission; the transaction may still land.",
        cause,
      );
    }
    if (status?.err)
      throw new TransactionFailedError(signature, status.err);
    if (
      status?.confirmationStatus === "confirmed" ||
      status?.confirmationStatus === "finalized"
    )
      return signature;
    await new Promise((resolve) => setTimeout(resolve, 1000));
  }
  throw new OutcomeUnknownError(
    signature,
    "No confirmation arrived within the time limit; the transaction may still land.",
  );
}
/** Read lookup table contents from RPC, never trust the API-supplied address list. */
export async function readLookupTables(
  url: string,
  keys: readonly Address[],
): Promise<AddressesByLookupTableAddress> {
  if (keys.length > 8) throw new Error("Too many lookup tables");
  const result: Record<string, Address[]> = {};
  const rpc = createSolanaRpc(url);
  for (const key of keys) {
    const { value } = await sendRpc(
      rpc.getAccountInfo(key, { encoding: "base64", commitment: "confirmed" }),
    );
    if (!value || value.owner !== "AddressLookupTab1e1111111111111111111111111")
      throw new Error("Bad lookup table owner");
    const bytes = getBase64Encoder().encode(value.data[0]);
    if (
      bytes.length < 56 ||
      (bytes.length - 56) % 32 !== 0 ||
      new DataView(bytes.buffer, bytes.byteOffset).getUint32(0, true) !== 1
    )
      throw new Error("Bad lookup table data");
    // Active only; deactivated tables are not accepted for new reviewed transactions.
    if (
      new DataView(bytes.buffer, bytes.byteOffset).getBigUint64(4, true) !==
      0xffffffffffffffffn
    )
      throw new Error("Lookup table is deactivated");
    const { getAddressDecoder } = await import("@solana/kit");
    const decode = getAddressDecoder();
    result[key] = [];
    for (let i = 56; i < bytes.length; i += 32)
      result[key]!.push(decode.decode(bytes.slice(i, i + 32)));
  }
  return result as AddressesByLookupTableAddress;
}
