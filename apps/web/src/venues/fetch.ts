/** Batched account reads for the venue layer: `getMultipleAccounts` at `confirmed` as `RawAccount`s. */
import { type Address, getBase64Encoder } from '@solana/kit';
import type { AppRpc } from '../rpc/client.ts';
import type { RawAccount } from './readers.ts';

const b64 = getBase64Encoder();

/** Reads `keys` (100 per call) with the context slot of the last call. */
export async function fetchRaw(
    rpc: AppRpc,
    keys: readonly Address[],
    signal?: AbortSignal,
): Promise<{ slot: bigint; accounts: Map<Address, RawAccount | null> }> {
    const out = new Map<Address, RawAccount | null>();
    let slot = 0n;
    for (let i = 0; i < keys.length; i += 100) {
        const chunk = keys.slice(i, i + 100);
        const res = await rpc
            .getMultipleAccounts(chunk, { encoding: 'base64', commitment: 'confirmed' })
            .send(signal ? { abortSignal: signal } : undefined);
        slot = res.context.slot;
        res.value.forEach((a, j) => {
            const k = chunk[j] as Address;
            out.set(
                k,
                a
                    ? {
                          address: k,
                          owner: a.owner,
                          lamports: BigInt(a.lamports),
                          executable: a.executable,
                          data: new Uint8Array(b64.encode(a.data[0])),
                      }
                    : null,
            );
        });
    }
    return { slot, accounts: out };
}

/** `Clock`: `slot` @0, `unix_timestamp` @32. */
export const CLOCK_SYSVAR = 'SysvarC1ock11111111111111111111111111111111' as Address;
