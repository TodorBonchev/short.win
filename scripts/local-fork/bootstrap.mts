#!/usr/bin/env node
/** Local historical-fork demo only. Every signer below is a PUBLIC test identity.
 * No keypair files, public RPC endpoints, airdrops, or financial-program edits.
 * Run: node scripts/local-fork/bootstrap.mts [--verify-only]
 */
import { createRequire } from 'node:module';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import assert from 'node:assert/strict';
import * as client from '../../client/src/index.ts';

const require = createRequire(new URL('../../client/package.json', import.meta.url));
const k = require('@solana/kit');
const OUT = new URL('../../target/local-fork/', import.meta.url);
const RECEIPT = new URL('bootstrap.json', OUT);
const RPC = process.env.SHORTWIN_LOCAL_RPC ?? 'http://127.0.0.1:8899';
const VERIFY_ONLY = process.argv.slice(2).includes('--verify-only');
assert(process.argv.slice(2).every(argument => argument === '--verify-only'), 'Only --verify-only is supported');
const LOADER = k.address('BPFLoaderUpgradeab1e11111111111111111111111');
const ALT_PROGRAM = k.address('AddressLookupTab1e1111111111111111111111111');
const CLOCK = k.address('SysvarC1ock11111111111111111111111111111111');
const MINT = k.address('Dz9mQ9NzkBcCsuGPFJ3r1bS4wgqKMHBPiVuniW8Mbonk');
const JUPITER_ELF = '5e31d320f5d88ae93c7983ce82a1497067abd86ff540b35073abaeeb25ef397b';
const JUPITER_DATA_HASH = '21e28c59e3b0a5536e48d31a1ca7346514a2bc61d70dc1388c920d7e3be35c1b';
const INITIAL_CASH = 1_000_000_000_000n;
const ROLE_BALANCE = 100_000_000_000n;
const RENT_CAP = 100_000_000n;
const stringify = (value: unknown) => JSON.stringify(value, (_key, v) => typeof v === 'bigint' ? v.toString() : v instanceof Uint8Array ? [...v] : v, 2);
const hash = (data: Uint8Array) => createHash('sha256').update(data).digest('hex');
const bytes = (data: string) => Uint8Array.from(Buffer.from(data, 'base64'));
const rawKey = (key: string) => k.getAddressEncoder().encode(k.address(key));
const concat = (...parts: Uint8Array[]) => Uint8Array.from(Buffer.concat(parts));
const u32 = (n: number) => { const b = Buffer.alloc(4); b.writeUInt32LE(n); return Uint8Array.from(b); };
const u64 = (n: bigint) => { const b = Buffer.alloc(8); b.writeBigUInt64LE(n); return Uint8Array.from(b); };
const terms = {
  termSeconds: 3600, defaultGraceSeconds: 86400, termFeeBps: 100, protocolShareBps: 1000,
  crOpenBps: 20000, crLiqBps: 15000, minNotionalLamports: 1000n,
  positionNotionalCapLamports: 1_000_000_000n, marketNotionalCapLamports: 25_000_000_000n,
  utilizationCapBps: 8000, walletNotionalCapBps: 10000, keeperFixedLamports: 1000n,
  keeperBps: 100, keeperCapLamports: 1_000_000n, buybackPremiumBps: 500,
  referenceMaxAgeSeconds: 60, referenceMaxAgeSlots: 1000n, historyEpochSeconds: 60,
  minimumHistoryEpochs: 2, maxSpreadBps: 1000, crConfirmationDelaySeconds: 5,
  crConfirmationDelaySlots: 5n, pricePolicyHash: new Uint8Array(32).fill(42),
};

// A literal loopback address prevents DNS, redirects, and accidental remote writes.
const endpoint = new URL(RPC);
assert(endpoint.protocol === 'http:' && endpoint.hostname === '127.0.0.1' && endpoint.port === '8899' && endpoint.pathname === '/' && !endpoint.search && !endpoint.username && !endpoint.password, 'Only http://127.0.0.1:8899 is allowed');
// The startup helper verifies a newly generated local genesis and its fixture
// accounts before writing this receipt. Resuming a ledger keeps that same pin.
const runtime = JSON.parse(await readFile(new URL('runtime-verification.json', OUT), 'utf8'));
assert.equal(runtime.health, 'ok', 'Run the verified local validator startup helper first');
assert.equal(typeof runtime.genesis_hash, 'string', 'Runtime receipt has no genesis pin');
const GENESIS: string = runtime.genesis_hash;
assert.equal(k.getBase58Encoder().encode(GENESIS).length, 32, 'Invalid runtime genesis pin');
assert(![
  '5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d',
  'EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG',
  '4uhcVJyU9pJkvQyS88uRDiswHXSCkY3zQawwpjk2NsNY',
].includes(GENESIS), 'Public cluster genesis is forbidden');
assert(Array.isArray(runtime.verified_accounts), 'Runtime receipt is missing verified fixtures');
assert.match(runtime.own_elf_sha256, /^[a-f0-9]{64}$/, 'Runtime receipt is missing the verified own ELF hash');
const fixture = JSON.parse(await readFile(new URL('fixtures.receipt.json', OUT), 'utf8'));
const rpc = k.createSolanaRpc(RPC);
assert.equal(await rpc.getHealth().send(), 'ok', 'Local validator is unhealthy');
assert.equal(await rpc.getGenesisHash().send(), GENESIS, 'Refusing another validator genesis');
if (!VERIFY_ONLY) await mkdir(OUT, { recursive: true });
let saved: any = null;
try { saved = JSON.parse(await readFile(RECEIPT, 'utf8')); } catch (e: any) { if (e.code !== 'ENOENT') throw e; }
if (saved) assert.equal(saved.genesisHash, GENESIS, 'Existing receipt belongs to another validator');

const roles: Record<string, any> = {};
for (const [name, seed] of Object.entries({ payer: 1, upgradeAuthority: 2, governance: 3, emergency: 4, reporter: 5, feeRecipient: 6, quote: 7, user: 100, lender: 101 })) {
  roles[name] = await k.createKeyPairSignerFromPrivateKeyBytes(new Uint8Array(32).fill(seed));
}
assert.equal(roles.upgradeAuthority.address, '9hSR6S7WPtxmTojgo6GG3k4yDPecgJY292j7xrsUGWBu');
assert.equal(roles.user.address, '3wpYnGqceZ8DzN3guiTd9rrYkWTwTHCChBSuo6cvkXTG');
assert.equal(roles.lender.address, 'FR5pWwinRBn35GNhg7bsvw8Q13kRept2pm561DwZCQzT');
const config = await client.configAddress(), policy = await client.policyAddress(1n);
const market = await client.marketAddress(MINT), pool = await client.poolAddress(market, 0n);
const tokenVault = await client.vaultAddress(pool), receipt = await client.receiptAddress(pool, roles.lender.address);
const eventAuthority = await client.eventAuthority();
const programData = (await k.getProgramDerivedAddress({ programAddress: LOADER, seeds: [rawKey(client.PROGRAM)] }))[0];
assert.equal(runtime.own_programdata, programData, 'Runtime receipt has another program data address');
assert.equal(runtime.own_upgrade_authority, roles.upgradeAuthority.address, 'Runtime receipt has another upgrade authority');
const lenderAta = await client.associatedToken(roles.lender.address, MINT, client.TOKEN);
const userAta = await client.associatedToken(roles.user.address, MINT, client.TOKEN);
assert.equal(lenderAta, '5uk3cPJ5Gra4JuYTrebjXzhJrmAF8FZcq5MKHKiQQyDg');
assert.equal(userAta, 'DPMrEubpoCHgKHtEmGGZ8Xi72AHxJf29Bw9ruDYZSsYw');
async function account(address: string) {
  const { value } = await rpc.getAccountInfo(k.address(address), { encoding: 'base64', commitment: 'confirmed' }).send();
  return value ? { ...value, data: bytes(value.data[0]) } : null;
}
async function typed(address: string, decoder: any, discriminator: ArrayLike<number>, size: number) {
  const a = await account(address);
  if (!a) return null;
  assert.equal(a.owner, client.PROGRAM, `Unexpected owner ${address}`);
  assert.equal(a.executable, false);
  assert.equal(a.data.length, size, `Unexpected data size ${address}`);
  assert.deepEqual(a.data.slice(0, 8), Uint8Array.from(discriminator), `Unexpected discriminator ${address}`);
  return decoder.decode(a.data);
}
async function clock() {
  const a = await account(CLOCK);
  assert(a && a.data.length === 40);
  const v = new DataView(a.data.buffer, a.data.byteOffset, a.data.length);
  return { slot: v.getBigUint64(0, true), unixTimestamp: v.getBigInt64(32, true) };
}
async function tokenAmount(address: string, owner: string) {
  const a = await account(address);
  assert(a && a.owner === client.TOKEN && a.data.length === 165);
  assert.deepEqual(a.data.slice(0, 32), Uint8Array.from(rawKey(MINT)));
  assert.deepEqual(a.data.slice(32, 64), Uint8Array.from(rawKey(owner)));
  return new DataView(a.data.buffer, a.data.byteOffset, a.data.length).getBigUint64(64, true);
}

// Verify the loaded executables and reviewed deployment before any local writes.
const own = await account(client.PROGRAM), ownData = await account(programData);
assert(own && ownData && own.owner === LOADER && own.executable && ownData.owner === LOADER && !ownData.executable);
assert.equal(own.data.length, 36);
assert.deepEqual(own.data, concat(u32(2), rawKey(programData)));
assert.deepEqual(ownData.data.slice(0, 4), u32(3));
assert.equal(ownData.data[12], 1);
assert.deepEqual(ownData.data.slice(13, 45), Uint8Array.from(rawKey(roles.upgradeAuthority.address)));
const elf = await readFile(new URL('../../target/deploy/shortwin_simple.so', import.meta.url));
const OWN_ELF = hash(elf);
assert.equal(OWN_ELF, runtime.own_elf_sha256, 'Program binary changed since verified validator startup');
assert.deepEqual(ownData.data.slice(45, 45 + elf.length), Uint8Array.from(elf));
assert(ownData.data.slice(45 + elf.length).every((x: number) => x === 0));
// Executable bytes stay immutable during the demo. Pool/vault/token snapshots
// may change through legitimate trades, so only their startup baseline is saved.
for (const address of [client.JUPITER, client.JUPITER_DATA,
  'CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C', 'DMawCQzbgNTmbzaESc7o6pvL1KAeetY8zA7jNpzntHhU']) {
  const expected = fixture.accounts.filter((a: any) => a.address === address);
  const verified = runtime.verified_accounts.filter((a: any) => a.address === address);
  assert.equal(expected.length, 1, `Missing executable fixture ${address}`);
  assert.equal(verified.length, 1, `Missing runtime executable verification ${address}`);
  const live = await account(address);
  assert(live, `Missing local executable account ${address}`);
  for (const entry of [verified[0], { owner: live.owner, executable: live.executable, data_len: live.data.length, data_sha256: hash(live.data) }]) {
    for (const field of ['owner', 'executable', 'data_len', 'data_sha256']) assert.equal(entry[field], expected[0][field], `Changed executable ${address}: ${field}`);
  }
}
const jupiter = await account(client.JUPITER), jupiterData = await account(client.JUPITER_DATA);
assert(jupiter && jupiterData && jupiter.owner === LOADER && jupiter.executable && jupiterData.owner === LOADER && !jupiterData.executable);
assert.deepEqual(jupiter.data, concat(u32(2), rawKey(client.JUPITER_DATA)));
assert.equal(jupiterData.data.length, 2_892_269);
assert.equal(hash(jupiterData.data), JUPITER_DATA_HASH);
assert.equal(new DataView(jupiterData.data.buffer).getBigUint64(4, true), 459_479_624n);
let last = jupiterData.data.length;
while (jupiterData.data[last - 1] === 0) last--;
assert.equal(hash(jupiterData.data.slice(45, last)), JUPITER_ELF);
const mintAccount = await account(MINT);
assert(mintAccount && mintAccount.owner === client.TOKEN && mintAccount.data.length === 82 && mintAccount.data[44] === 6 && mintAccount.data[45] === 1);

async function configState() { return typed(config, client.getConfigDecoder(), client.CONFIG_DISCRIMINATOR, client.getConfigSize()); }
async function policyState() { return typed(policy, client.getPolicyDecoder(), client.POLICY_DISCRIMINATOR, client.getPolicySize()); }
async function poolState() { return typed(pool, client.getPoolGenerationDecoder(), client.POOL_GENERATION_DISCRIMINATOR, client.getPoolGenerationSize()); }
async function receiptState() { return typed(receipt, client.getLenderReceiptDecoder(), client.LENDER_RECEIPT_DISCRIMINATOR, client.getLenderReceiptSize()); }
async function validateConfig() {
  const c = await configState(), p = await policyState();
  if (!c) { assert.equal(p, null, 'Orphan policy account'); return false; }
  assert(p, 'Missing policy account');
  for (const name of ['governance', 'emergency', 'reporter', 'feeRecipient']) assert.equal(c[name], roles[name].address, `Existing config changed ${name}`);
  assert.deepEqual(c.genesisHash, Uint8Array.from(k.getBase58Encoder().encode(GENESIS)));
  assert.equal(c.currentPolicy, policy); assert.equal(c.currentPolicyVersion, 1n); assert.equal(c.reporterEpoch, 1n);
  assert.equal(c.opensPaused, false); assert.equal(c.pricesPaused, false); assert.equal(c.marketCreationFee, 10_000n);
  assert.equal(c.jupiterDeploySlot, 459_479_624n); assert.equal(c.jupiterProgramDataLen, 2_892_269);
  assert.deepEqual(c.jupiterElfHash, Uint8Array.from(Buffer.from(JUPITER_ELF, 'hex')));
  assert.equal(p.config, config); assert.equal(p.policyVersion, 1n);
  assert.deepEqual(client.getFinancialPolicyEncoder().encode(p.terms), client.getFinancialPolicyEncoder().encode(terms), 'Existing policy terms differ');
  return true;
}
let initialized = await validateConfig();
const manifest: any = {
  ...(saved ?? {}), schemaVersion: 1, status: 'preparing', scope: 'LOCAL ONLY: historical mainnet snapshot with synthetic public test balances',
  rpcUrl: RPC, genesisHash: GENESIS, program: client.PROGRAM, programData, programElfSha256: OWN_ELF,
  jupiter: client.JUPITER, jupiterProgramData: client.JUPITER_DATA, jupiterDeploySlot: '459479624',
  jupiterProgramDataLength: 2_892_269, jupiterElfSha256: JUPITER_ELF,
  roles: Object.fromEntries(Object.entries(roles).map(([name, signer]) => [name, signer.address])),
  publicTestSeeds: { payer: 1, upgradeAuthority: 2, governance: 3, emergency: 4, reporter: 5, feeRecipient: 6, quote: 7, user: 100, lender: 101 },
  config, policy, market, pool, tokenVault, lenderReceipt: receipt, eventAuthority,
  mint: MINT, mintDecimals: 6, tokenProgram: client.TOKEN, wsolMint: client.WSOL,
  lenderAta, userAta, initialCashRaw: INITIAL_CASH.toString(), policyTerms: terms,
  setupRentCapLamports: RENT_CAP.toString(), receipts: saved?.receipts ?? [],
};
async function save() { if (!VERIFY_ONLY) await writeFile(RECEIPT, stringify(manifest) + '\n'); }
await save();
async function send(label: string, instructions: any[], signers: any[] = [roles.payer]) {
  assert(!VERIFY_ONLY, `Verification requires existing setup; refusing transaction ${label}`);
  // prepareTransaction simulates an unsigned v0 transaction before any signing.
  assert.equal(await rpc.getGenesisHash().send(), GENESIS);
  const prepared = await client.prepareTransaction(RPC, roles.payer.address, instructions);
  console.log(`${label}: local simulation passed (${prepared.units} CU, ${prepared.bytes.length} bytes); signing public test identities`);
  const signed = await k.signTransactionWithSigners(signers, k.getTransactionDecoder().decode(prepared.bytes));
  const wire = Uint8Array.from(k.getTransactionEncoder().encode(signed));
  const signature = await client.submitSigned(RPC, prepared, wire);
  const item = { label, signature, units: prepared.units, feeLamports: prepared.feeLamports, bytes: wire.length, clock: await clock(), logs: prepared.logs };
  manifest.receipts.push(item);
  await writeFile(new URL(`bootstrap-${manifest.receipts.length.toString().padStart(2, '0')}-${label}.json`, OUT), stringify(item) + '\n');
  await save();
  return signature;
}
function transfer(to: string, amount: bigint) {
  return { programAddress: client.SYSTEM, accounts: [{ address: roles.payer.address, role: k.AccountRole.WRITABLE_SIGNER }, { address: k.address(to), role: k.AccountRole.WRITABLE }], data: concat(u32(2), u64(amount)) };
}
const funding = [];
for (const [name, signer] of Object.entries(roles)) {
  if (name === 'payer') continue;
  const { value } = await rpc.getBalance(signer.address, { commitment: 'confirmed' }).send();
  if (value < ROLE_BALANCE - 1_000_000_000n) funding.push(transfer(signer.address, ROLE_BALANCE - value));
}
if (funding.length && !VERIFY_ONLY) await send('fund-public-test-roles', funding);
if (!initialized) {
  await send('initialize', [client.getInitializeInstruction({
    authority: roles.upgradeAuthority, config, policy, programAccount: client.PROGRAM, programData,
    jupiter: client.JUPITER, jupiterProgramData: client.JUPITER_DATA, systemProgram: client.SYSTEM, eventAuthority, program: client.PROGRAM,
    governance: roles.governance.address, emergency: roles.emergency.address, reporter: roles.reporter.address, feeRecipient: roles.feeRecipient.address,
    genesisHash: k.getBase58Encoder().encode(GENESIS), policyVersion: 1n, terms, opensPaused: false,
    marketCreationFee: 10_000n, jupiterDeploySlot: 459_479_624n, jupiterProgramDataLen: 2_892_269,
    jupiterElfHash: Uint8Array.from(Buffer.from(JUPITER_ELF, 'hex')), maxSetupRent: RENT_CAP,
  })], [roles.payer, roles.upgradeAuthority]);
  assert(await validateConfig());
}
let marketState = await typed(market, client.getMarketDecoder(), client.MARKET_DISCRIMINATOR, client.getMarketSize());
if (!marketState) {
  assert.equal(await poolState(), null);
  await send('register-market', [client.getRegisterMarketInstruction({
    creator: roles.governance, config, policy, market, pool, mint: MINT, tokenVault, feeRecipient: roles.feeRecipient.address,
    tokenProgram: client.TOKEN, systemProgram: client.SYSTEM, eventAuthority, program: client.PROGRAM,
    maxCreationFee: 10_000n, maxSetupRent: RENT_CAP,
  })], [roles.payer, roles.governance]);
  marketState = await typed(market, client.getMarketDecoder(), client.MARKET_DISCRIMINATOR, client.getMarketSize());
}
assert(marketState && marketState.config === config && marketState.mint === MINT && marketState.tokenProgram === client.TOKEN && marketState.decimals === 6 && marketState.activeGeneration === 0n);
let p = await poolState(), r = await receiptState();
assert(p && p.market === market && p.mint === MINT && p.tokenProgram === client.TOKEN && p.generation === 0n && !p.retired);
if (!r) {
  assert.equal(p.cash, 0n); assert.equal(p.shares, 0n); assert.equal(p.principalOut, 0n); assert.equal(p.loanCount, 0);
  const before = await tokenAmount(lenderAta, roles.lender.address);
  assert(before >= INITIAL_CASH);
  await send('initial-lender-deposit', [client.getDepositInstruction({
    owner: roles.lender, market, pool, receipt, mint: MINT, ownerTokens: lenderAta, tokenVault,
    tokenProgram: client.TOKEN, systemProgram: client.SYSTEM, eventAuthority, program: client.PROGRAM,
    amount: INITIAL_CASH, minShares: INITIAL_CASH, maxReceiptRent: RENT_CAP,
  })], [roles.payer, roles.lender]);
  assert.equal(await tokenAmount(lenderAta, roles.lender.address), before - INITIAL_CASH);
  p = await poolState(); r = await receiptState();
  assert(p && r && p.cash === INITIAL_CASH && p.shares === INITIAL_CASH && r.shares === INITIAL_CASH);
} else {
  // Later demo activity may change cash; the genesis deposit must never repeat.
  assert(r.pool === pool && r.owner === roles.lender.address && r.generation === 0n && r.shares >= INITIAL_CASH, 'Existing receipt does not contain the bootstrap deposit');
  assert(p.shares >= r.shares, 'Pool shares do not cover the bootstrap lender receipt');
}
assert.equal(await tokenAmount(tokenVault, pool), p!.cash, 'Pool cash/vault mismatch');

// One real on-chain lookup table, owned by public test payer1. Include static
// route accounts and a few future loan PDAs, keeping browser v0 packets bounded.
const lookupAddresses: string[] = [];
function add(...addresses: string[]) { for (const a of addresses) { k.address(a); if (!lookupAddresses.includes(a)) lookupAddresses.push(a); } }
add(client.PROGRAM, programData, client.JUPITER, client.JUPITER_DATA, client.SYSTEM, client.TOKEN, client.TOKEN_2022, client.WSOL,
  MINT, config, policy, market, pool, tokenVault, receipt, eventAuthority, lenderAta, userAta,
  'ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL', 'ComputeBudget111111111111111111111111111111', CLOCK,
  ...Object.values(roles).map((s: any) => s.address), ...fixture.accounts.map((a: any) => a.address));
for (let nonce = 0n; nonce < 8n; nonce++) {
  const loan = await client.loanAddress(pool, roles.user.address, nonce);
  const e = await client.executionAddresses(loan);
  add(loan, e.executionAuthority, e.executionBase, e.executionWsol);
}
add(await client.exposureAddress(pool, roles.user.address));
for (const q of [1n, 10n, 100n, 1000n, 10000n]) add(await client.referenceAddress(market, policy, q * 1_000_000n));
assert(lookupAddresses.length < 256);
let table = manifest.lookupTable ? k.address(manifest.lookupTable) : null;
let tableAccount = table ? await account(table) : null;
if (!tableAccount) {
  const recentSlot = await rpc.getSlot({ commitment: 'finalized' }).send();
  const [address, bump] = await k.getProgramDerivedAddress({ programAddress: ALT_PROGRAM, seeds: [rawKey(roles.payer.address), u64(recentSlot)] });
  table = address;
  manifest.lookupTable = table;
  await save();
  await send('create-lookup-table', [{ programAddress: ALT_PROGRAM, accounts: [
    { address: table, role: k.AccountRole.WRITABLE }, { address: roles.payer.address, role: k.AccountRole.READONLY_SIGNER },
    { address: roles.payer.address, role: k.AccountRole.WRITABLE_SIGNER }, { address: client.SYSTEM, role: k.AccountRole.READONLY },
  ], data: concat(u32(0), u64(recentSlot), Uint8Array.of(bump)) }]);
  tableAccount = await account(table);
}
assert(table && tableAccount && tableAccount.owner === ALT_PROGRAM && tableAccount.data.length >= 56);
assert.equal(tableAccount.data[21], 1, 'Lookup table has no authority');
assert.deepEqual(tableAccount.data.slice(22, 54), Uint8Array.from(rawKey(roles.payer.address)), 'Wrong lookup authority');
let current = (await client.readLookupTables(RPC, [table]))[table];
const missing = lookupAddresses.filter(a => !current.includes(k.address(a)));
for (let i = 0; i < missing.length; i += 20) {
  const batch = missing.slice(i, i + 20);
  await send(`extend-lookup-table-${i / 20 + 1}`, [{ programAddress: ALT_PROGRAM, accounts: [
    { address: table, role: k.AccountRole.WRITABLE }, { address: roles.payer.address, role: k.AccountRole.READONLY_SIGNER },
    { address: roles.payer.address, role: k.AccountRole.WRITABLE_SIGNER }, { address: client.SYSTEM, role: k.AccountRole.READONLY },
  ], data: concat(u32(2), u64(BigInt(batch.length)), ...batch.map(rawKey)) }]);
}
current = (await client.readLookupTables(RPC, [table]))[table];
for (const address of lookupAddresses) assert(current.includes(k.address(address)), `ALT missing ${address}`);
const finalTable = await account(table);
assert(finalTable);
const extensionSlot = new DataView(finalTable.data.buffer, finalTable.data.byteOffset, finalTable.data.length).getBigUint64(12, true);
for (let i = 0; (await rpc.getSlot({ commitment: 'confirmed' }).send()) <= extensionSlot && i < 30; i++) await new Promise(resolve => setTimeout(resolve, 400));
assert((await rpc.getSlot({ commitment: 'confirmed' }).send()) > extensionSlot, 'ALT not yet active');
manifest.lookupTable = table;
manifest.lookupTableAddresses = current;
manifest.clock = await clock();
manifest.readback = { config: await configState(), policy: await policyState(), market: marketState, pool: await poolState(), lenderReceipt: await receiptState(),
  vaultRaw: await tokenAmount(tokenVault, pool), lenderRaw: await tokenAmount(lenderAta, roles.lender.address), userRaw: await tokenAmount(userAta, roles.user.address) };
manifest.status = 'ready';
await save();
console.log(stringify({ status: manifest.status, verificationOnly: VERIFY_ONLY, receipt: fileURLToPath(RECEIPT), rpcUrl: RPC, genesisHash: GENESIS, config, policy, market, pool, tokenVault, lookupTable: table, lookupAddressCount: current.length, initialCashRaw: INITIAL_CASH, currentCashRaw: manifest.readback.pool.cash, clock: manifest.clock }));
