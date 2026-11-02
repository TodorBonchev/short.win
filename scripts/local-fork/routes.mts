/** Historical Jupiter quotes by unsigned simulation on the local validator.
 *
 * The temporary accounts, their signatures and the public fixture funding exist
 * only inside simulateTransaction. This module never signs or sends a transaction.
 */
import { createRequire } from 'node:module';
import type { BuildResponse, RouteBinding } from '../../client/src/route.ts';

// Types are taken through the client too, so pnpm resolves Kit's own dependency
// graph before following its declarations. No repository-root dependency is needed.
type Address = RouteBinding['executionAuthority'];
type PrepareTransaction = typeof import('../../client/src/transactions.ts').prepareTransaction;
type Instruction = Parameters<PrepareTransaction>[2][number];
type AddressesByLookupTableAddress = NonNullable<Parameters<PrepareTransaction>[3]>;

// Resolve the active client dependency tree rather than an archived SDK.
const require = createRequire(new URL('../../client/package.json', import.meta.url));
const kit = require('@solana/kit');
const {
  AccountRole, address, appendTransactionMessageInstructions, compileTransaction,
  compressTransactionMessageUsingAddressLookupTables, createNoopSigner,
  createSolanaRpc, createTransactionMessage, getAddressEncoder,
  getBase64EncodedWireTransaction, getTransactionSize,
  setTransactionMessageComputeUnitLimit, setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
} = kit;

export const RPC_URL = 'http://127.0.0.1:8899';
const endpoint = new URL(RPC_URL);
if (endpoint.protocol !== 'http:' || endpoint.hostname !== '127.0.0.1'
    || endpoint.port !== '8899' || endpoint.pathname !== '/') {
  throw new Error('Historical quotes require http://127.0.0.1:8899');
}
const rpc = createSolanaRpc(RPC_URL);
const keyBytes = getAddressEncoder();
const U64_MAX = 0xffffffffffffffffn;
const TOKEN_ACCOUNT_SIZE = 165;
const SLIPPAGE_BPS = 50;

const SYSTEM = address('11111111111111111111111111111111');
const TOKEN = address('TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA');
const WSOL = address('So11111111111111111111111111111111111111112');
const BASE = address('Dz9mQ9NzkBcCsuGPFJ3r1bS4wgqKMHBPiVuniW8Mbonk');
const JUPITER = address('JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4');
const EVENT_AUTHORITY = address('D8cy77BBepLMngZx6ZukaTff5hCt1HrWyKk3Hnd9oitf');
const CPMM = address('CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C');
const POOL_AUTHORITY = address('GpMZbSM2GgvTKHJirzeGfMFoaZ8UR2X7F4v8vHTvxFbL');
const CONFIG = address('D4FPEruKEHrG5TenZ2mpDGEfu1iUvTiqBxvpU8HLBvC2');
const POOL = address('Q2sPHPdUWFMg7M7wwrQKLrn619cAucfRsmhVJffodSp');
const WSOL_VAULT = address('HgNPDD8bpbSrGyHegiCT5xrYxHTfwLfZydwGkjNCJRKA');
const BASE_VAULT = address('9xsCiNwYQXM3ZeHFSVj9JQdP1vREJREpN23f6wvxA1ty');
const OBSERVATION = address('4UdSz2kMddtX4woMmdgkWg75fdBP8FgYwqfkh4ri7mnD');
const PAYER = address('AKnL4NNf3DGWZJS6cPknBuEGnVsV4A4m5tgebLHaRSZ9');
const QUOTE_OWNER = address('GmaDrppBC7P5ARKV8g3djiwP89vz1jLK23V2GBjuAEGB');
const QUOTE_VAULT = address('4Yk9HoDSfJv9QcmJbLcXdWVgS7nfvdUqiVcvbSu8VBru');

/** Public fixed keys suitable for the local route's address lookup table. */
export const fixedRouteAccounts: readonly Address[] = Object.freeze([
  BASE, WSOL, TOKEN, JUPITER, EVENT_AUTHORITY, CPMM, POOL_AUTHORITY,
  CONFIG, POOL, WSOL_VAULT, BASE_VAULT, OBSERVATION, SYSTEM, QUOTE_VAULT,
]);

type Side = 'sell' | 'buy';
type RouteAccount = BuildResponse['swapInstruction']['accounts'][number];

function amount(input: bigint): void {
  if (typeof input !== 'bigint' || input <= 0n || input > U64_MAX) {
    throw new Error('Jupiter input must be a positive u64 bigint');
  }
}

function endpoints(side: Side, authority: Address, base: Address, wsol: Address) {
  if (side !== 'sell' && side !== 'buy') throw new Error('Unsupported Jupiter side');
  [authority, base, wsol].forEach(address);
  if (new Set([authority, base, wsol]).size !== 3
      || [authority, base, wsol].some(key => fixedRouteAccounts.includes(key)
        || key === PAYER || key === QUOTE_OWNER)) {
    throw new Error('Jupiter scratch addresses must be distinct from fixed fixture accounts');
  }
  return side === 'buy'
    ? { source: wsol, destination: base, inputMint: WSOL, outputMint: BASE,
      inputVault: WSOL_VAULT, outputVault: BASE_VAULT }
    : { source: base, destination: wsol, inputMint: BASE, outputMint: WSOL,
      inputVault: BASE_VAULT, outputVault: WSOL_VAULT };
}

/** Exact account ordering in tests/runtime/src/jupiter.rs::HistoricalRoute. */
function routeAccounts(side: Side, authority: Address, base: Address, wsol: Address): RouteAccount[] {
  const e = endpoints(side, authority, base, wsol);
  const ro = (pubkey: Address): RouteAccount => ({ pubkey, isWritable: false, isSigner: pubkey === authority });
  const rw = (pubkey: Address): RouteAccount => ({ pubkey, isWritable: true, isSigner: false });
  return [
    ro(authority), rw(e.source), rw(e.destination), ro(e.inputMint), ro(e.outputMint),
    ro(TOKEN), ro(TOKEN), ro(JUPITER), ro(EVENT_AUTHORITY), ro(JUPITER),
    ro(CPMM), ro(authority), ro(POOL_AUTHORITY), ro(CONFIG), rw(POOL),
    rw(e.source), rw(e.destination), rw(e.inputVault), rw(e.outputVault),
    ro(TOKEN), ro(TOKEN), ro(e.inputMint), ro(e.outputMint), rw(OBSERVATION),
  ];
}

function routeData(input: bigint, quotedOutput: bigint, slippageBps: number): Uint8Array {
  const data = new Uint8Array(39);
  data.set([187, 100, 250, 204, 49, 196, 175, 20]);
  const view = new DataView(data.buffer);
  view.setBigUint64(8, input, true);
  view.setBigUint64(16, quotedOutput, true);
  view.setUint16(24, slippageBps, true);
  // Platform fee and positive slippage fee are both zero.
  view.setUint32(30, 1, true);
  data.set([46, 0x10, 0x27, 0, 1], 34);
  return data;
}

function createAccount(account: Address, lamports: bigint): Instruction {
  if (lamports > U64_MAX) throw new Error('Scratch funding exceeds u64');
  const data = new Uint8Array(52);
  const view = new DataView(data.buffer);
  view.setUint32(0, 0, true); // SystemInstruction::CreateAccount
  view.setBigUint64(4, lamports, true);
  view.setBigUint64(12, BigInt(TOKEN_ACCOUNT_SIZE), true);
  data.set(keyBytes.encode(TOKEN), 20);
  return {
    programAddress: SYSTEM,
    accounts: [
      { address: PAYER, role: AccountRole.WRITABLE_SIGNER, ...{ signer: createNoopSigner(PAYER) } },
      { address: account, role: AccountRole.WRITABLE_SIGNER, ...{ signer: createNoopSigner(account) } },
    ],
    data,
  };
}

function initializeAccount(account: Address, mint: Address, owner: Address): Instruction {
  const data = new Uint8Array(33);
  data[0] = 18; // SPL Token InitializeAccount3; owner lives in instruction data.
  data.set(keyBytes.encode(owner), 1);
  return {
    programAddress: TOKEN,
    accounts: [{ address: account, role: AccountRole.WRITABLE }, { address: mint, role: AccountRole.READONLY }],
    data,
  };
}

function seedBase(destination: Address, input: bigint): Instruction {
  const data = new Uint8Array(10);
  data[0] = 12; // SPL Token TransferChecked
  new DataView(data.buffer).setBigUint64(1, input, true);
  data[9] = 6;
  return {
    programAddress: TOKEN,
    accounts: [
      { address: QUOTE_VAULT, role: AccountRole.WRITABLE },
      { address: BASE, role: AccountRole.READONLY },
      { address: destination, role: AccountRole.WRITABLE },
      { address: QUOTE_OWNER, role: AccountRole.READONLY_SIGNER, ...{ signer: createNoopSigner(QUOTE_OWNER) } },
    ],
    data,
  };
}

function tokenAmount(
  account: { owner: Address; data: readonly [string, 'base64']; executable: boolean } | null,
  mint: Address,
  owner: Address,
): bigint {
  if (account === null || account.owner !== TOKEN || account.executable
      || !Array.isArray(account.data) || account.data[1] !== 'base64') {
    throw new Error('Local simulation returned an invalid SPL token account');
  }
  const data = Buffer.from(account.data[0], 'base64');
  if (data.length !== TOKEN_ACCOUNT_SIZE || data[108] !== 1
      || !data.subarray(0, 32).equals(Buffer.from(keyBytes.encode(mint)))
      || !data.subarray(32, 64).equals(Buffer.from(keyBytes.encode(owner)))) {
    throw new Error('Local simulation token account does not match its bound mint and authority');
  }
  return data.readBigUInt64LE(64);
}

/** Actual destination delta from an unsigned, non-committing Jupiter simulation.
 * All scratch accounts must be absent in the validator before simulation.
 */
export async function quote(
  side: Side,
  input: bigint,
  executionAuthority: Address,
  executionBase: Address,
  executionWsol: Address,
  lookupTables: AddressesByLookupTableAddress = {},
): Promise<bigint> {
  amount(input);
  const e = endpoints(side, executionAuthority, executionBase, executionWsol);
  const [rent, { value: lifetime }, { value: accounts }] = await Promise.all([
    rpc.getMinimumBalanceForRentExemption(BigInt(TOKEN_ACCOUNT_SIZE), { commitment: 'confirmed' }).send(),
    rpc.getLatestBlockhash({ commitment: 'confirmed' }).send(),
    rpc.getMultipleAccounts([executionBase, executionWsol, QUOTE_VAULT], { encoding: 'base64', commitment: 'confirmed' }).send(),
  ]);
  if (accounts[0] !== null || accounts[1] !== null) {
    throw new Error('Quote scratch token accounts already exist; use fresh execution addresses');
  }
  if (side === 'sell' && input > tokenAmount(accounts[2], BASE, QUOTE_OWNER)) {
    throw new Error('Jupiter sell input exceeds the local quote-vault balance');
  }
  const instructions: Instruction[] = [
    createAccount(executionBase, rent),
    initializeAccount(executionBase, BASE, executionAuthority),
    createAccount(executionWsol, rent + (side === 'buy' ? input : 0n)),
    initializeAccount(executionWsol, WSOL, executionAuthority),
  ];
  if (side === 'sell') instructions.push(seedBase(executionBase, input));
  instructions.push({
    programAddress: JUPITER,
    accounts: routeAccounts(side, executionAuthority, executionBase, executionWsol).map(a => ({
      address: address(a.pubkey),
      role: a.isSigner ? AccountRole.READONLY_SIGNER : a.isWritable ? AccountRole.WRITABLE : AccountRole.READONLY,
      ...(a.isSigner ? { signer: createNoopSigner(executionAuthority) } : {}),
    })),
    // Probe with a minimum of one raw output unit and no slippage. The result,
    // rather than a constant-product formula, supplies the later quote header.
    data: routeData(input, 1n, 0),
  });
  const m0 = setTransactionMessageFeePayerSigner(createNoopSigner(PAYER), createTransactionMessage({ version: 0 }));
  const m1 = setTransactionMessageLifetimeUsingBlockhash(lifetime, m0);
  const m2 = setTransactionMessageComputeUnitLimit(1_400_000, m1);
  const m3 = appendTransactionMessageInstructions(instructions, m2);
  const transaction = compileTransaction(compressTransactionMessageUsingAddressLookupTables(m3, lookupTables));
  if (getTransactionSize(transaction) > 1232) {
    throw new Error('Local quote exceeds 1232 bytes; provide the local fixture address lookup table');
  }
  const { value: simulation } = await rpc.simulateTransaction(getBase64EncodedWireTransaction(transaction), {
    encoding: 'base64', sigVerify: false, commitment: 'confirmed',
    accounts: { addresses: [e.destination], encoding: 'base64' },
  }).send();
  if (simulation.err !== null) {
    throw new Error('Local Jupiter quote simulation failed: '
      + JSON.stringify(simulation.err, (_key, value) => typeof value === 'bigint' ? value.toString() : value)
      + '\n' + (simulation.logs ?? []).join('\n'));
  }
  // Initialization made the destination balance zero in this simulation.
  const before = 0n;
  const after = tokenAmount(simulation.accounts?.[0] ?? null, e.outputMint, executionAuthority);
  if (after <= before) throw new Error('Local Jupiter route produced no output');
  return after - before;
}

/** Jupiter-shaped build response containing only the bound RouteV2 swap. */
export async function build(
  side: Side,
  input: bigint,
  executionAuthority: Address,
  executionBase: Address,
  executionWsol: Address,
  lookupTables: AddressesByLookupTableAddress = {},
): Promise<BuildResponse> {
  const e = endpoints(side, executionAuthority, executionBase, executionWsol);
  const output = await quote(side, input, executionAuthority, executionBase, executionWsol, lookupTables);
  return {
    inputMint: e.inputMint, outputMint: e.outputMint,
    inAmount: input.toString(), outAmount: output.toString(),
    otherAmountThreshold: ((output * 9950n + 9999n) / 10000n).toString(),
    swapMode: 'ExactIn', slippageBps: SLIPPAGE_BPS,
    routePlan: [{ swapInfo: { label: 'RaydiumCPMM' } }],
    swapInstruction: {
      programId: JUPITER,
      accounts: routeAccounts(side, executionAuthority, executionBase, executionWsol),
      data: Buffer.from(routeData(input, output, SLIPPAGE_BPS)).toString('base64'),
    },
    addressesByLookupTableAddress: lookupTables,
  };
}
