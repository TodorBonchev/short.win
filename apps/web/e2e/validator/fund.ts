/**
 * Funds a browser wallet on the E2E validator for manual work with the app (docs/DEVELOPMENT.md
 * "Run the app"): SOL by airdrop, and tokens of every seeded market's mint into the wallet's ATAs,
 * minted with the seed's generated mint authority. Throwaway localnet only.
 *
 *   node e2e/validator/fund.ts <wallet address> [--sol 25] [--tokens 1000000]
 */
import { type Address, address, airdropFactory, isAddress, lamports } from '@solana/kit';
import { getCreateAssociatedTokenIdempotentInstructionAsync, getMintToInstruction } from '@solana-program/token';
import { getMintToInstruction as getMintToInstruction2022 } from '@solana-program/token-2022';
import { ata, getBalance, LAMPORTS_PER_SOL, PROGRAMS, rpc, rpcSubscriptions, send } from './chain.ts';
import { signerFor } from './keys.ts';
import { loadState } from './state.ts';

function arg(name: string, dflt: bigint): bigint {
    const i = process.argv.indexOf(name);
    const v = i >= 0 ? process.argv[i + 1] : undefined;
    if (v === undefined) return dflt;
    if (!/^\d+$/.test(v)) throw new Error(`${name} takes a whole number, got ${v}`);
    return BigInt(v);
}

const target = process.argv[2];
if (!target || !isAddress(target)) {
    console.error('usage: node e2e/validator/fund.ts <wallet address> [--sol 25] [--tokens 1000000]');
    process.exit(2);
}
const owner: Address = address(target);
const sol = arg('--sol', 25n);
const tokens = arg('--tokens', 1_000_000n);

try {
    const state = loadState();
    const airdrop = airdropFactory({ rpc: rpc(), rpcSubscriptions: rpcSubscriptions() } as Parameters<
        typeof airdropFactory
    >[0]);
    if (sol > 0n) {
        await airdrop({ recipientAddress: owner, lamports: lamports(sol * LAMPORTS_PER_SOL), commitment: 'confirmed' });
    }
    const payer = await signerFor('payer');
    const mintAuthority = await signerFor('mintAuthority');
    for (const m of Object.values(state.markets)) {
        const raw = tokens * 10n ** BigInt(m.decimals);
        const create = await getCreateAssociatedTokenIdempotentInstructionAsync({
            payer,
            owner,
            mint: m.mint,
            tokenProgram: m.tokenProgram,
        });
        const token = await ata(owner, m.mint, m.tokenProgram);
        const mintTo =
            m.tokenProgram === PROGRAMS.token2022
                ? getMintToInstruction2022({ mint: m.mint, token, mintAuthority, amount: raw })
                : getMintToInstruction({ mint: m.mint, token, mintAuthority, amount: raw });
        await send(payer, [create, mintTo]);
        console.log(`[fund] ${tokens} ${m.symbol} (${m.mint}) -> ${token}`);
    }
    console.log(`[fund] ${owner}: ${Number(await getBalance(owner)) / 1e9} SOL`);
    process.exit(0);
} catch (e) {
    console.error(e);
    process.exit(1);
}
