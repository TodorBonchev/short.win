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
