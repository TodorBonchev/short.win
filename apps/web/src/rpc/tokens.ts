/**
 * The connected owner's token accounts of a mint (FRONTEND.md §5.7): source candidates for a
 * deposit (CPI Guard detected per account) and destination candidates for a withdrawal (memo
 * requirement detected per account). ATA first.
 */
import { type Address, getBase64Encoder } from '@solana/kit';
import { parseTokenAccount, type TokenAccountInfo } from '../tx/tokenAccount.ts';
import type { AppRpc } from './client.ts';

export async function readOwnerTokenAccounts(
    rpc: AppRpc,
    owner: Address,
    mint: Address,
    ata: Address | null,
): Promise<TokenAccountInfo[]> {
    const { value } = await rpc
        .getTokenAccountsByOwner(owner, { mint }, { encoding: 'base64', commitment: 'confirmed' })
        .send();
    const b64 = getBase64Encoder();
    const out: TokenAccountInfo[] = [];
    for (const { pubkey, account } of value) {
        const info = parseTokenAccount(pubkey, account.owner, new Uint8Array(b64.encode(account.data[0])));
        if (info) out.push({ ...info, mint, owner });
    }
    return out.sort((a, b) => (a.address === ata ? -1 : b.address === ata ? 1 : Number(b.amount - a.amount)));
}
