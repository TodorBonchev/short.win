/**
 * Node-side chain helpers for the E2E bootstrap, the seed and the specs: the Kit RPC clients of the
 * E2E validator, a `send` that signs with Kit signers, account and token readers, and the PDAs the
 * seed needs that `@shortwin/client` does not export. TEST-ONLY.
 */
import {
    type Address,
    addSignersToTransactionMessage,
    appendTransactionMessageInstructions,
    type Base64EncodedDataResponse,
    createSolanaRpc,
    createSolanaRpcSubscriptions,
    createTransactionMessage,
    getAddressEncoder,
    getBase64Encoder,
    getProgramDerivedAddress,
    getSignatureFromTransaction,
    type Instruction,
    type KeyPairSigner,
    pipe,
    type Signature,
    sendAndConfirmTransactionFactory,
    setTransactionMessageComputeUnitLimit,
    setTransactionMessageFeePayerSigner,
    setTransactionMessageLifetimeUsingBlockhash,
    signTransactionMessageWithSigners,
    type TransactionSigner,
} from '@solana/kit';
import { URLS } from '../env.ts';

export const PROGRAMS = {
    shortwin: '5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK' as Address,
    /**
     * The retired `complete_curve` helper (pump-final D2): not deployed on the validator; only the
     * `full` profile's indexer config still names it, until the indexer drops its ingestion (P6).
     */
    completeCurve: '4B5mGykX9EWwaYdbTvsp8qXCUTbc7caQourz16Gat6LJ' as Address,
    mock: 'BPAZFPkmVtYEWEZHCzFUns7oybh1VWfAvmu3ofM2mQi7' as Address,
    system: '11111111111111111111111111111111' as Address,
    token: 'TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA' as Address,
    token2022: 'TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb' as Address,
    ata: 'ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL' as Address,
    memo: 'MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr' as Address,
    computeBudget: 'ComputeBudget111111111111111111111111111111' as Address,
    upgradeableLoader: 'BPFLoaderUpgradeab1e11111111111111111111111' as Address,
    pump: '6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P' as Address,
    pumpAmm: 'pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA' as Address,
    pumpFees: 'pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ' as Address,
    raydiumV4: '675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8' as Address,
    raydiumCpmm: 'CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C' as Address,
    jupiter: 'JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4' as Address,
} as const;

export const WSOL_MINT = 'So11111111111111111111111111111111111111112' as Address;
export const CLOCK_SYSVAR = 'SysvarC1ock11111111111111111111111111111111' as Address;
export const LAMPORTS_PER_SOL = 1_000_000_000n;

export type Rpc = ReturnType<typeof createSolanaRpc>;

let rpcSingleton: Rpc | null = null;
let subsSingleton: ReturnType<typeof createSolanaRpcSubscriptions> | null = null;

export function rpc(): Rpc {
    rpcSingleton ??= createSolanaRpc(URLS.rpc);
    return rpcSingleton;
}

export function rpcSubscriptions(): ReturnType<typeof createSolanaRpcSubscriptions> {
    subsSingleton ??= createSolanaRpcSubscriptions(URLS.ws);
    return subsSingleton;
}

export interface SendOptions {
    /** Extra signers that sign hand-built instructions (whose metas carry only roles). */
    readonly signers?: readonly TransactionSigner[];
    /** A `SetComputeUnitLimit` (omitted by default: 200k per instruction, and 45 bytes saved). */
    readonly computeUnits?: number;
}

/**
 * Sends `instructions` with `payer` as fee payer, at `confirmed`, and returns the signature.
 * Codama builders embed their signers in the account metas; `signers` adds the others.
 */
export async function send(
    payer: KeyPairSigner,
    instructions: readonly Instruction[],
    opts: SendOptions = {},
): Promise<Signature> {
    const { value: blockhash } = await rpc().getLatestBlockhash({ commitment: 'confirmed' }).send();
    const message = pipe(
        createTransactionMessage({ version: 0 }),
        (m) => setTransactionMessageFeePayerSigner(payer, m),
        (m) => setTransactionMessageLifetimeUsingBlockhash(blockhash, m),
        (m) => (opts.computeUnits ? setTransactionMessageComputeUnitLimit(opts.computeUnits, m) : m),
        (m) => appendTransactionMessageInstructions([...instructions], m),
        (m) => addSignersToTransactionMessage([...(opts.signers ?? [])], m),
    );
    const signed = await signTransactionMessageWithSigners(message);
    const signature = getSignatureFromTransaction(signed);
    const sendAndConfirm = sendAndConfirmTransactionFactory({
        rpc: rpc(),
        rpcSubscriptions: rpcSubscriptions(),
    } as Parameters<typeof sendAndConfirmTransactionFactory>[0]);
    try {
        await sendAndConfirm(signed as Parameters<typeof sendAndConfirm>[0], { commitment: 'confirmed' });
    } catch (e) {
        const logs =
            (e as { context?: { logs?: string[] }; cause?: { context?: { logs?: string[] } } })?.context?.logs ??
            (e as { cause?: { context?: { logs?: string[] } } })?.cause?.context?.logs ??
            [];
        const err = new Error(`transaction failed: ${(e as Error).message}\n${logs.join('\n')}`);
        (err as Error & { cause?: unknown }).cause = e;
        throw err;
    }
    return signature;
}

const b64 = getBase64Encoder();

export interface RawAccount {
    readonly address: Address;
    readonly lamports: bigint;
    readonly owner: Address;
    readonly data: Uint8Array;
    readonly executable: boolean;
}

/** One account at `confirmed`, or `null`. */
export async function getAccount(address: Address): Promise<RawAccount | null> {
    const { value } = await rpc().getAccountInfo(address, { encoding: 'base64', commitment: 'confirmed' }).send();
    if (!value) return null;
    return {
        address,
        lamports: BigInt(value.lamports),
        owner: value.owner,
        data: new Uint8Array(b64.encode((value.data as Base64EncodedDataResponse)[0])),
        executable: value.executable,
    };
}

/** Several accounts at `confirmed` (≤ 100 per call). */
export async function getAccounts(addresses: readonly Address[]): Promise<(RawAccount | null)[]> {
    const out: (RawAccount | null)[] = [];
    for (let i = 0; i < addresses.length; i += 100) {
        const chunk = addresses.slice(i, i + 100);
        const { value } = await rpc()
            .getMultipleAccounts(chunk, { encoding: 'base64', commitment: 'confirmed' })
            .send();
        value.forEach((v, j) => {
            const address = chunk[j] as Address;
            out.push(
                v
                    ? {
                          address,
                          lamports: BigInt(v.lamports),
                          owner: v.owner,
                          data: new Uint8Array(b64.encode((v.data as Base64EncodedDataResponse)[0])),
                          executable: v.executable,
                      }
                    : null,
            );
        });
    }
    return out;
}

export async function getBalance(address: Address): Promise<bigint> {
    const { value } = await rpc().getBalance(address, { commitment: 'confirmed' }).send();
    return BigInt(value);
}

/** A token account's `amount` (offset 64 of the base layout), or `null` if it does not exist. */
export async function getTokenAmount(address: Address): Promise<bigint | null> {
    const a = await getAccount(address);
    if (!a || a.data.length < 72) return null;
    return new DataView(a.data.buffer, a.data.byteOffset).getBigUint64(64, true);
}

/** The validator's `Clock` (unix timestamp and slot). */
export async function clock(): Promise<{ slot: bigint; unixTimestamp: bigint }> {
    const a = await getAccount(CLOCK_SYSVAR);
    if (!a) throw new Error('no Clock sysvar');
    const v = new DataView(a.data.buffer, a.data.byteOffset);
    return { slot: v.getBigUint64(0, true), unixTimestamp: v.getBigInt64(32, true) };
}

/** Waits until the validator's clock reaches `unixTimestamp`. */
export async function waitForClock(unixTimestamp: bigint, timeoutMs = 180_000): Promise<void> {
    const deadline = Date.now() + timeoutMs;
    for (;;) {
        const c = await clock();
        if (c.unixTimestamp >= unixTimestamp) return;
        if (Date.now() > deadline) throw new Error(`validator clock ${c.unixTimestamp} never reached ${unixTimestamp}`);
        await sleep(1_000);
    }
}

export function sleep(ms: number): Promise<void> {
    return new Promise((r) => setTimeout(r, ms));
}

const enc = getAddressEncoder();
const text = new TextEncoder();

/** `PDA(program, seeds)`, seeds given as strings, addresses or raw bytes. */
export async function pda(program: Address, seeds: readonly (string | Address | Uint8Array)[]): Promise<Address> {
    const parts = seeds.map((s) =>
        s instanceof Uint8Array ? s : isBase58Address(s) ? enc.encode(s as Address) : text.encode(s),
    );
    const [a] = await getProgramDerivedAddress({ programAddress: program, seeds: parts });
    return a;
}

function isBase58Address(s: string): boolean {
    return s.length >= 32 && s.length <= 44 && /^[1-9A-HJ-NP-Za-km-z]+$/.test(s);
}

/** `ATA(owner, mint, tokenProgram)`. */
export async function ata(owner: Address, mint: Address, tokenProgram: Address): Promise<Address> {
    const [a] = await getProgramDerivedAddress({
        programAddress: PROGRAMS.ata,
        seeds: [enc.encode(owner), enc.encode(tokenProgram), enc.encode(mint)],
    });
    return a;
}

/** short.win's event-CPI authority `["__event_authority"]`. */
export function eventAuthority(): Promise<Address> {
    return pda(PROGRAMS.shortwin, ['__event_authority']);
}

/** The ProgramData account of an upgradeable program. */
export function programDataOf(program: Address): Promise<Address> {
    return pda(PROGRAMS.upgradeableLoader, [program]);
}

/** The deploy slot recorded in a ProgramData header (`u32 tag ‖ u64 slot ‖ option<authority>`). */
export async function programDataSlot(program: Address): Promise<bigint | null> {
    const a = await getAccount(await programDataOf(program));
    if (!a || a.data.length < 12) return null;
    return new DataView(a.data.buffer, a.data.byteOffset).getBigUint64(4, true);
}

/** Per-market PDAs short.win derives (programs/shortwin/src/state). */
export async function marketKeys(mint: Address, tokenProgram: Address) {
    const market = await pda(PROGRAMS.shortwin, ['market', mint]);
    const trader = await pda(PROGRAMS.shortwin, ['trader', market]);
    return {
        mint,
        tokenProgram,
        market,
        marketVault: await pda(PROGRAMS.shortwin, ['market_vault', market]),
        poolVault: await pda(PROGRAMS.shortwin, ['pool_vault', market]),
        trader,
        traderWsol: await pda(PROGRAMS.shortwin, ['trader_wsol', market]),
        traderBaseAta: await ata(trader, mint, tokenProgram),
        traderWsolAta: await ata(trader, WSOL_MINT, PROGRAMS.token),
    };
}

export type MarketKeys = Awaited<ReturnType<typeof marketKeys>>;

/** pump's `["bonding-curve", mint]` and the canonical PumpSwap pool (create_market's detection set). */
export async function detectionSet(mint: Address): Promise<{ bondingCurve: Address; canonicalPool: Address }> {
    const bondingCurve = await pda(PROGRAMS.pump, ['bonding-curve', mint]);
    const poolAuthority = await pda(PROGRAMS.pump, ['pool-authority', mint]);
    const canonicalPool = await pda(PROGRAMS.pumpAmm, ['pool', new Uint8Array([0, 0]), poolAuthority, mint, WSOL_MINT]);
    return { bondingCurve, canonicalPool };
}
