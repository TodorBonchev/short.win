/** Any user edit or cancellation revokes in-flight preparation and wallet replies. */
export class IntentRevision {
  #value = 0;
  capture(): number {
    return this.#value;
  }
  invalidate(): void {
    this.#value += 1;
  }
  assert(expected: number): void {
    if (expected !== this.#value)
      throw new Error(
        "Inputs or wallet changed; prepare the transaction again",
      );
  }
  async guard<T>(expected: number, work: Promise<T>): Promise<T> {
    const result = await work;
    this.assert(expected);
    return result;
  }
}

export type SignatureReconciliation =
  | "failed"
  | "confirmed"
  | "expired-absent"
  | "pending";

/**
 * Decide whether an earlier submission is resolved strongly enough to prepare
 * another transaction. Expiry proves absence only while the RPC still has no
 * status; any observed non-error status can continue toward confirmation.
 */
export function reconcileSignatureStatus(
  known: {
    readonly err: unknown;
    readonly confirmationStatus?: string | null;
  } | null,
  blockhashExpired: boolean,
): SignatureReconciliation {
  if (known?.err != null) return "failed";
  if (
    known?.confirmationStatus === "confirmed" ||
    known?.confirmationStatus === "finalized"
  )
    return "confirmed";
  if (known === null && blockhashExpired) return "expired-absent";
  return "pending";
}
