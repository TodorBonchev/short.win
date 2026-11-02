#!/usr/bin/env node
/** Local snapshot demo: real Jupiter simulations and explicitly synthetic history.
 * Public test reporter only; no keyfiles, remote RPC, or production price claims.
 */
import { createRequire } from 'node:module';
import { readFile, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { createServer } from 'node:http';
import * as sw from '../../client/src/index.ts';
import { createBridge, type BridgeDependencies, type MarketState } from '../../client/src/server.ts';
import { quote, build } from './routes.mts';

const require = createRequire(new URL('../../client/package.json', import.meta.url));
const k = require('@solana/kit');
const ROOT = new URL('../../target/local-fork/', import.meta.url);
const setup = JSON.parse(await readFile(new URL('bootstrap.json', ROOT), 'utf8'));
const RPC = 'http://127.0.0.1:8899';
const ORIGIN = 'http://127.0.0.1:5173';
const client = k.createSolanaRpc(RPC);
if (setup.rpcUrl !== RPC || setup.status !== 'ready' || await client.getGenesisHash().send() !== setup.genesisHash)
  throw new Error('Local demo bootstrap/genesis mismatch');
const signer = await k.createKeyPairSignerFromPrivateKeyBytes(new Uint8Array(32).fill(5));
if (signer.address !== setup.roles.reporter) throw new Error('Public test reporter mismatch');
const payer = await k.createKeyPairSignerFromPrivateKeyBytes(new Uint8Array(32).fill(1));
const tables = { [setup.lookupTable]: setup.lookupTableAddresses };
const string = (value: unknown) => JSON.stringify(value, (_key, v) => typeof v === 'bigint' ? v.toString() : v);
const hash = (value: string) => createHash('sha256').update(value).digest();
const settings = { epoch_seconds: 60, history_epochs: 2, reference_max_age_seconds: 60,
  reference_max_age_slots: 1000, max_spread_bps: 1000 };
const requested = new Set<string>(['100000000']);
let publishing: Promise<void> = Promise.resolve();
const activity: unknown[] = [];

async function clock() {
  const { value } = await client.getAccountInfo(k.address('SysvarC1ock11111111111111111111111111111111'), { encoding: 'base64', commitment: 'confirmed' }).send();
  if (!value) throw new Error('Local Clock missing');
  const data = Buffer.from(value.data[0], 'base64');
  return { slot: data.readBigUInt64LE(0), timestamp: data.readBigInt64LE(32) };
}
async function state(mint: any): Promise<MarketState> {
  if (mint !== setup.mint || await client.getGenesisHash().send() !== setup.genesisHash)
    throw new Error('The local demo supports only its pinned snapshot market');
  const config = await sw.readAccount(RPC, k.address(setup.config), sw.getConfigDecoder(), sw.ShortwinSimpleAccount.Config);
  const policy = await sw.readAccount(RPC, k.address(setup.policy), sw.getPolicyDecoder(), sw.ShortwinSimpleAccount.Policy);
  const market = await sw.readAccount(RPC, k.address(setup.market), sw.getMarketDecoder(), sw.ShortwinSimpleAccount.Market);
  const pool = await sw.readAccount(RPC, k.address(setup.pool), sw.getPoolGenerationDecoder(), sw.ShortwinSimpleAccount.PoolGeneration);
  if (!config || !policy || !market || !pool || market.config !== setup.config || market.mint !== mint ||
      pool.market !== setup.market || pool.mint !== mint || pool.tokenProgram !== sw.TOKEN || pool.retired ||
      config.currentPolicy !== setup.policy || policy.config !== setup.config)
    throw new Error('Local market identity mismatch');
  return { config, policy, market, pool, configKey: k.address(setup.config), policyKey: k.address(setup.policy),
    marketKey: k.address(setup.market), poolKey: k.address(setup.pool), mint, tokenProgram: sw.TOKEN, clock: await clock() };
}
async function endpoints(authority: any) {
  return { base: await sw.associatedToken(authority, k.address(setup.mint), sw.TOKEN),
    wsol: await sw.associatedToken(authority, sw.WSOL, sw.TOKEN) };
}
async function buyInput(q: bigint, maximum: bigint, authority: any, base: any, wsol: any): Promise<bigint> {
  if (maximum < 1000n || maximum > 25_000_000_000n) throw new Error('Local buy budget is outside demo bounds');
  const target = (q * 10000n + 9949n) / 9950n;
  if (await quote('buy', maximum, authority, base, wsol, tables) < target) throw new Error('Buy budget cannot repay the whole token debt');
  let lo = 0n, hi = maximum;
  // Each candidate is actually executed by Jupiter inside an unsigned simulation.
  for (let n = 0; n < 32 && hi - lo > 1n; n++) {
    const mid = (lo + hi) / 2n;
    let output = 0n;
    try { output = await quote('buy', mid, authority, base, wsol, tables); } catch (error) {
      if (mid >= 1000n) throw error;
    }
    if (output >= target) hi = mid; else lo = mid;
  }
  return hi;
}
async function sendReference(q: bigint) {
  const s = await state(k.address(setup.mint));
  const loan = await sw.loanAddress(s.poolKey, k.address(setup.roles.user), s.market.nextLoanNonce);
  const exec = await sw.executionAddresses(loan);
  const e = await endpoints(exec.executionAuthority);
  const sell = await build('sell', q, exec.executionAuthority, e.base, e.wsol, tables);
  const bid = BigInt(sell.otherAmountThreshold);
  const budget = BigInt(sell.outAmount) * 115n / 100n + 1000n;
  const ask = await buyInput(q, budget, exec.executionAuthority, e.base, e.wsol);
  const now = await clock();
  const key = await sw.referenceAddress(s.marketKey, s.policyKey, q);
  const previous = await sw.readAccount(RPC, key, sw.getPriceReferenceDecoder(), sw.ShortwinSimpleAccount.PriceReference);
  if (previous && now.timestamp <= previous.sourceTs) return;
  const epoch = now.timestamp / 60n * 60n;
  const ix = await sw.getPublishReferenceInstructionAsync({ reporter: signer, config: s.configKey,
    market: s.marketKey, policy: s.policyKey, reference: key, eventAuthority: k.address(setup.eventAuthority), program: sw.PROGRAM,
    quantity: q, sequence: previous ? previous.sequence + 1n : 1n, sourceSlot: now.slot, sourceTs: now.timestamp,
    finite: true, liveBidLower: bid, liveAskUpper: ask, historyAskUpperMax: ask, historyBidLowerMax: bid,
    acquisitionReference: ask, completedEpochs: 2, historyStartTs: epoch - 120n, historyEndTs: epoch - 1n,
    evidenceHash: hash(string({ mode: 'synthetic-local-history', genesis: setup.genesisHash, q, bid, ask, ...now })),
    maxReferenceRent: 100_000_000n });
  const { value: lifetime } = await client.getLatestBlockhash({ commitment: 'confirmed' }).send();
  let message = k.createTransactionMessage({ version: 0 });
  message = k.setTransactionMessageFeePayerSigner(payer, message);
  message = k.setTransactionMessageLifetimeUsingBlockhash(lifetime, message);
  message = k.setTransactionMessageComputeUnitLimit(250000, message);
  message = k.appendTransactionMessageInstructions([ix], message);
  message = k.compressTransactionMessageUsingAddressLookupTables(message, tables);
  const probe = k.compileTransaction(message);
  const { value: simulation } = await client.simulateTransaction(k.getBase64EncodedWireTransaction(probe), { encoding: 'base64', sigVerify: false, commitment: 'confirmed' }).send();
  if (simulation.err !== null) throw new Error('Local reference simulation failed: ' + string(simulation.err));
  const tx = await k.signTransactionMessageWithSigners(message);
  const signature = k.getSignatureFromTransaction(tx);
  await client.sendTransaction(k.getBase64EncodedWireTransaction(tx), { encoding: 'base64', skipPreflight: false, maxRetries: 3 }).send();
  for (let n = 0; n < 100; n++) {
    const status = (await client.getSignatureStatuses([signature]).send()).value[0];
    if (status?.err) throw new Error('Local reference failed');
    if (status?.confirmationStatus === 'confirmed' || status?.confirmationStatus === 'finalized') break;
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  activity.push({ quantity: q.toString(), signature, bid: bid.toString(), ask: ask.toString(), sourceSlot: now.slot.toString(), sourceTs: now.timestamp.toString() });
  if (activity.length > 20) activity.shift();
  await writeFile(new URL('reference-publications.json', ROOT), string(activity) + '\n');
}
async function publish(q: bigint) {
  if (q < 1_000_000n || q > 400_000_000n) throw new Error('The local demonstration supports 1–400 whole tokens per short');
  requested.add(q.toString());
  const next = publishing.then(() => sendReference(q));
  publishing = next.catch(error => console.error('Local reference:', error.message));
  await next;
}
const deps: BridgeDependencies = {
  settings,
  readState: state,
  readLoan: key => sw.readAccount(RPC, key, sw.getLoanDecoder(), sw.ShortwinSimpleAccount.Loan),
  readReference: async (market, policy, q) => sw.readAccount(RPC, await sw.referenceAddress(market, policy, q), sw.getPriceReferenceDecoder(), sw.ShortwinSimpleAccount.PriceReference),
  tables: async keys => { const output: Record<string, any> = {}; for (const key of keys) { if (key !== setup.lookupTable) throw new Error('Unknown local lookup table'); output[key] = setup.lookupTableAddresses; } return output; },
  service: async (command, input: any) => {
    if (command[0] === 'model-hash') return '2a'.repeat(32);
    if (command[0] === 'check-policy') return '{}';
    if (command[0] === 'enqueue') { for (const job of input) await publish(BigInt(job.quantity)); return string(input.map(() => ({ accepted: true }))); }
    if (command[0] !== 'build-sell' && command[0] !== 'build-buy') throw new Error('Unsupported local fixture command');
    if (input.mint !== setup.mint) throw new Error('Only the local snapshot mint is supported');
    const authority = k.address(command[command.indexOf('--taker') + 1]);
    const e = await endpoints(authority), q = BigInt(input.quantity);
    const side = command[0] === 'build-sell' ? 'sell' : 'buy';
    const amount = side === 'sell' ? q : await buyInput(q, BigInt(command[command.indexOf('--maximum-input') + 1]), authority, e.base, e.wsol);
    const result = await build(side, amount, authority, e.base, e.wsol, tables);
    result.addressesByLookupTableAddress = tables;
    return string({ status: 'Ready', input_amount: amount.toString(), build: result, evidence_hash: hash(string(result)).toString('hex') });
  },
};
// The local Clock is authoritative; production's host-wallclock assumptions are not changed.
const handle = createBridge({ rpcUrl: RPC, serviceSettings: '', serviceDatabase: '', frontendOrigin: ORIGIN, globalPerMinute: 500, perIpPerMinute: 100 }, deps);
const server = createServer(async (request, response) => {
  const headers: Record<string, string> = { 'content-type': 'application/json', 'cache-control': 'no-store' };
  if (request.headers.origin === ORIGIN) { headers['access-control-allow-origin'] = ORIGIN; headers.vary = 'Origin'; }
  try {
    if (request.headers.origin && request.headers.origin !== ORIGIN) { response.writeHead(403, headers); response.end(string({ error: 'Origin is not allowed' })); return; }
    if (request.method === 'GET' && request.url === '/demo/info') {
      response.writeHead(200, headers); response.end(string({ mode: 'local-fork', testFunds: true, syntheticHistory: true,
        genesis: setup.genesisHash, mint: setup.mint, program: setup.program, snapshot: '2026-10-27 Jupiter / historical CPMM',
        wallet: setup.roles.user, lookupTable: setup.lookupTable, ready: true })); return;
    }
    let bytes = 0; const chunks: Buffer[] = [];
    for await (const chunk of request) { bytes += chunk.length; if (bytes > 16384) throw new Error('Request too large'); chunks.push(Buffer.from(chunk)); }
    const body = bytes ? JSON.parse(Buffer.concat(chunks).toString('utf8')) : undefined;
    const result = await handle({ method: request.method ?? '', path: request.url ?? '/', origin: request.headers.origin,
      ip: request.socket.remoteAddress ?? '', body });
    response.writeHead(result.status, { ...headers, ...result.headers }); response.end(result.status === 204 ? '' : string(result.body));
  } catch (error) { response.writeHead(503, headers); response.end(string({ status: 'Unavailable', error: error instanceof Error ? error.message : 'Local demo unavailable' })); }
});
server.maxConnections = 32; server.requestTimeout = 45000; server.headersTimeout = 10000;
server.listen(8787, '127.0.0.1', () => console.log('Local Jupiter demo bridge: http://127.0.0.1:8787'));
await publish(100_000_000n);
setInterval(() => { for (const q of requested) void publish(BigInt(q)).catch(error => console.error('Refresh:', error.message)); }, 15000);
