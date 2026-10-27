/** Read-only packet/cost illustration. No keys, signing, RPC, submission or account creation. */
import { createRequire } from 'node:module';
import { readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import assert from 'node:assert/strict';
import { getPublishPriceReportInstruction } from '../../clients/js/src/generated/instructions/publishPriceReport.ts';

const require = createRequire(new URL('../../clients/js/package.json', import.meta.url));
const kit = require('@solana/kit');
const root = fileURLToPath(new URL('../../', import.meta.url));
const out = fileURLToPath(new URL('./', import.meta.url));
const hash = (bytes: Uint8Array) => createHash('sha256').update(bytes).digest('hex');
const bytes = (n: number) => new Uint8Array(32).fill(n);
const fakeAddress = (n: number) => kit.getAddressDecoder().decode(bytes(n));
const addresses = {
  program: fakeAddress(1), publisherFeePayer: fakeAddress(2), config: fakeAddress(3),
  market: fakeAddress(4), reports: [fakeAddress(5), fakeAddress(6)], syntheticNonexistentLookupTable: fakeAddress(13), baseMint: fakeAddress(7),
  baseTokenProgram: fakeAddress(8), quoteMint: fakeAddress(9), quoteTokenProgram: fakeAddress(10),
};
const signer = kit.createNoopSigner(addresses.publisherFeePayer); // Address-only object; no signing method is invoked.
const domain = {
  genesisHash: bytes(12), program: addresses.program, config: addresses.config, market: addresses.market,
  baseMint: addresses.baseMint, baseTokenProgram: addresses.baseTokenProgram,
  quoteMint: addresses.quoteMint, quoteTokenProgram: addresses.quoteTokenProgram,
  baseDecimals: 6, quoteDecimals: 9,
};
const reports = [1n, 4n].map((multiple, index) => getPublishPriceReportInstruction({
  publisher: signer, config: addresses.config, market: addresses.market, priceReport: addresses.reports[index],
  version: 1, domain, quantity: multiple * 1_000_000n,
  model: { version: 1, epoch: 1n, policyHash: bytes(31) }, configurationEpoch: 1n,
  publisherArg: addresses.publisherFeePayer, sequence: 1n,
  startSlot: 1000n, startTs: 2_000_000_000n, endSlot: 1002n, endTs: 2_000_000_001n,
  observationDigest: bytes(40 + index),
  status: { __kind: 'Finite', lower: multiple * 100_000_000n, central: multiple * 105_000_000n, upper: multiple * 110_000_000n },
}, { programAddress: addresses.program }));

const packetLimit = 1232;
const cases = [];
const variants = [
  { count: 1, withComputeBudget: false, withAlt: false },
  { count: 1, withComputeBudget: true, withAlt: false },
  { count: 2, withComputeBudget: false, withAlt: false },
  { count: 2, withComputeBudget: true, withAlt: false },
  { count: 2, withComputeBudget: true, withAlt: true },
];
for (const { count, withComputeBudget, withAlt } of variants) {
  let message = kit.createTransactionMessage({ version: 0 });
  message = kit.setTransactionMessageFeePayer(addresses.publisherFeePayer, message);
  message = kit.setTransactionMessageLifetimeUsingBlockhash({
    blockhash: kit.blockhash(fakeAddress(11)), lastValidBlockHeight: 2000n,
  }, message);
  if (withComputeBudget) {
    // Byte-size illustrations only: neither this CU limit nor this CU price is measured/approved.
    message = kit.setTransactionMessageComputeUnitLimit(200_000, message);
    message = kit.setTransactionMessageComputeUnitPrice(1_000n, message);
  }
  message = kit.appendTransactionMessageInstructions(reports.slice(0, count), message);
  if (withAlt) {
    // Pure in-memory map for a NON-EXISTENT synthetic ALT; no lookup-account fetch/create occurs.
    message = kit.compressTransactionMessageUsingAddressLookupTables(message, {
      [addresses.syntheticNonexistentLookupTable]: [addresses.config, addresses.market, ...addresses.reports],
    });
  }
  const compiledMessage = kit.compileTransactionMessage(message);
  assert.ok(compiledMessage.staticAccounts.includes(addresses.publisherFeePayer));
  assert.ok(compiledMessage.staticAccounts.includes(addresses.program));
  if (withComputeBudget) assert.ok(compiledMessage.staticAccounts.includes('ComputeBudget111111111111111111111111111111'));
  if (withAlt) {
    assert.equal(compiledMessage.staticAccounts.length, 3);
    assert.equal(compiledMessage.addressTableLookups.length, 1);
  }
  const transaction = kit.compileTransaction(message);
  assert.equal(Object.keys(transaction.signatures).length, 1);
  assert.ok(Object.values(transaction.signatures).every(signature => signature === null));
  const encoded = new Uint8Array(kit.getTransactionEncoder().encode(transaction));
  assert.equal(encoded[0], 1); // Shortvec signature count.
  assert.ok(encoded.slice(1, 65).every(value => value === 0)); // Unsigned placeholder, never signed.
  assert.equal(encoded[65], 0x80); // v0 message version prefix.
  const filename = `v0-${count}-finite-${withComputeBudget ? 'with' : 'without'}-compute-budget${withAlt ? '-synthetic-alt' : ''}.unsigned.bin`;
  writeFileSync(out + filename, encoded);
  cases.push({
    finite_reports: count, compute_budget_instructions: withComputeBudget ? 2 : 0,
    illustrative_compute_unit_limit: withComputeBudget ? 200_000 : null,
    illustrative_micro_lamports_per_compute_unit: withComputeBudget ? '1000' : null,
    compute_settings_approved_or_measured: false,
    required_transaction_signatures: Object.keys(transaction.signatures).length,
    signature_values_all_null: true, zero_signature_placeholders: 1,
    message_version: 0, address_lookup_tables: withAlt ? 1 : 0,
    synthetic_alt_account_exists_or_fetched: false,
    loaded_account_count: withAlt ? 4 : 0,
    synthetic_alt_contents: withAlt ? [addresses.config, addresses.market, ...addresses.reports] : [],
    publisher_and_program_ids_remain_static: true,
    static_account_keys: compiledMessage.staticAccounts.length,
    total_instructions: message.instructions.length,
    finite_instruction_data_bytes: reports.slice(0, count).map(ix => ix.data.length),
    instruction_data_bytes: message.instructions.map(ix => ix.data.length),
    serialized_packet_bytes: encoded.length, packet_limit_bytes: packetLimit,
    fits_legacy_v0_packet_limit: encoded.length <= packetLimit,
    remaining_bytes: packetLimit - encoded.length,
    file: filename, sha256: hash(encoded),
  });
}

const monthSeconds = 30n * 24n * 60n * 60n;
const updatesPerFeed = monthSeconds / 2n;
const baseFee = 5000n;
const costCase = (name: string, transactions: bigint, reportsPerTransaction: number, feasible: string) => ({
  name, feeds: 2, cadence_seconds_per_feed: 2, days: 30, seconds: monthSeconds.toString(),
  publication_updates_per_feed: updatesPerFeed.toString(), total_reports: (updatesPerFeed * 2n).toString(),
  reports_per_transaction: reportsPerTransaction, transactions: transactions.toString(),
  required_signatures_per_transaction: 1, base_lamports_per_signature: baseFee.toString(),
  base_fee_lamports: (transactions * baseFee).toString(),
  base_fee_sol: `${transactions * baseFee / 1_000_000_000n}.${(transactions * baseFee % 1_000_000_000n).toString().padStart(9, '0')}`,
  feasibility: feasible,
});
const sourcePaths = [
  'clients/js/src/generated/instructions/publishPriceReport.ts',
  'clients/js/src/generated/types/priceReportDomainArgs.ts',
  'clients/js/src/generated/types/priceReportModelArgs.ts',
  'clients/js/src/generated/types/priceReportStatusArgs.ts',
  'clients/js/package.json', 'pnpm-lock.yaml',
];
const results = {
  schema: 'shortwin-price-publication-cost/v1', measured_at_utc: new Date().toISOString(),
  source_head: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(),
  node_version: process.version,
  kit_version: JSON.parse(readFileSync(root + 'clients/js/node_modules/@solana/kit/package.json', 'utf8')).version,
  sources: sourcePaths.map(path => ({ path, sha256: hash(readFileSync(root + path)) })),
  official_sources: [
    { url: 'https://solana.com/docs/core/fees/fee-structure', fact: 'Current base fee 5000 lamports per transaction signature; failed transactions are charged; optional priority fees separate.' },
    { url: 'https://solana.com/docs/core/transactions', fact: 'Legacy/v0 maximum packet size 1232 bytes; each signature 64 bytes.' },
  ],
  assumptions: [
    'Synthetic addresses and blockhash; no real keys, signing, RPC, sending, simulation, account creation or program execution.',
    'Two distinct already-initialized exact-q report accounts for one market; publisher is fee payer and sole transaction signer.',
    'Generated current Finite publish_price_report instruction; no Ed25519 verification precompile instructions, no nonce or account initialization. Four baseline packets have no ALT; a fifth uses an in-memory NON-EXISTENT synthetic ALT containing config/market/both report addresses, while signer and both program IDs remain static.',
    'All signatures remain null; standard serialized unsigned transactions contain 64-byte zero signature placeholders.',
    'ComputeBudget limit 200000 and price 1000 micro-lamports/CU illustrate packet overhead only; no CU sufficiency, priority-fee budget, settings approval or landing guarantee.',
    '30 consecutive 24-hour days and exactly one successful publication per feed every 2 seconds; no extra signatures, failed attempts, resends requiring new fees, account rent, priority fees, initialization, RPC/API credits, indexer/keeper transactions or market creation included.',
    'A byte-size fit does not prove timely observation, simulation, execution, source-window freshness, matching epochs, simultaneous report availability, confirmation or finality. The synthetic ALT neither exists nor has an approved creation/funding/authority/extension/deactivation lifecycle.',
  ],
  synthetic_addresses: addresses, finite_instruction_data_bytes: reports[0].data.length,
  packets: cases,
  thirty_day_base_fee_lower_bounds: [
    costCase('Separate one-report transactions, two feeds', updatesPerFeed * 2n, 1, 'Each measured one-report packet fits with or without the illustrative ComputeBudget instructions; runtime/landing unqualified.'),
    costCase('One two-report transaction each 2 seconds', updatesPerFeed, 2, 'Current no-ALT payload fits by 6 bytes only without ComputeBudget; adding normal limit+price exceeds 1232. A non-existent synthetic four-address ALT makes the two-report + both-CU packet 1188 bytes, but no ALT account/lifecycle, runtime or landing is qualified. This is not an approved batching or spending plan.'),
  ],
};
writeFileSync(out + 'results.json', JSON.stringify(results, null, 2) + '\n');
console.log(JSON.stringify({ source_head: results.source_head, kit_version: results.kit_version, packets: cases.map(({ finite_reports, compute_budget_instructions, address_lookup_tables, serialized_packet_bytes, remaining_bytes }) => ({ finite_reports, compute_budget_instructions, address_lookup_tables, serialized_packet_bytes, remaining_bytes })), thirty_day_base_fee_lower_bounds: results.thirty_day_base_fee_lower_bounds.map(({ name, transactions, base_fee_sol }) => ({ name, transactions, base_fee_sol })) }, null, 2));
