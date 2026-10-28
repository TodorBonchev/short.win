/** Node-only operator. Import this subpath explicitly; the browser SDK never imports it. */
import { createHash } from "node:crypto";
import { spawn } from "node:child_process";
import { readFile, mkdir, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { DatabaseSync } from "node:sqlite";
import {
  address,
  createNoopSigner,
  createKeyPairSignerFromBytes,
  getBase58Encoder,
  getBase64Encoder,
  getTransactionDecoder,
  getTransactionEncoder,
  getSignatureFromTransaction,
  signTransaction,
  type Address,
  type Decoder,
  type Instruction,
} from "@solana/kit";
import * as g from "./generated/index.ts";
import {
  PROGRAM,
  SYSTEM,
  TOKEN,
  TOKEN_2022,
  configAddress,
  policyAddress,
  marketAddress,
  poolAddress,
  loanAddress,
  exposureAddress,
  referenceAddress,
  eventAuthority,
  executionAddresses,
  vaultAddress,
} from "./pdas.ts";
import {
  prepareTransaction,
  submitSigned,
  readLookupTables,
  type PreparedTransaction,
} from "./transactions.ts";
import { planBundle, appendRoute, type BuildResponse } from "./route.ts";

// External RPC shapes are checked before any value becomes financial authority.
type ObjectValue = Record<string, any>;
export type RpcCall = (
  method: string,
  params?: readonly unknown[],
) => Promise<any>;
export type ServiceSettings = {
  epoch_seconds: number;
  history_epochs: number;
  reference_max_age_seconds: number;
  reference_max_age_slots: number;
  max_spread_bps: number;
  [key: string]: unknown;
};
export type Row<T = unknown> = { address: Address; kind: number; data: T };
export type Snapshot = {
  slot: bigint;
  timestamp: bigint;
  config: Row<g.Config>;
  loans: Row<g.Loan>[];
  policies: Map<Address, g.Policy>;
  markets: Map<Address, g.Market>;
  pools: Map<Address, g.PoolGeneration>;
  references: Map<Address, g.PriceReference>;
  rows: Row[];
};
export type Job = {
  market: Address;
  policy: Address;
  mint: Address;
  quantity: string;
  active_loan: boolean;
};
export type Action =
  "default" | "wait-reference" | "wait" | "arm" | "clear-arm" | "force";
export type OperatorOptions = {
  rpcUrl: string;
  database: string;
  serviceDatabase: string;
  serviceSettings: string;
  evidenceDirectory: string;
  keeperAddress?: Address;
  reporterKeyfile?: string;
  keeperKeyfile?: string;
  execute?: boolean;
  tick?: boolean;
  python?: string;
  maxAccounts?: number;
  maxEventTransactions?: number;
  maxActions?: number;
  maxFeeLamports?: bigint;
  maxPassFeesLamports?: bigint;
  minBalanceLamports?: bigint;
  maxTemporaryLamports?: bigint;
  maxReferenceRentLamports?: bigint;
};
const SCRIPT = fileURLToPath(
  new URL("../../services/src/shortwin_price_service.py", import.meta.url),
);
const U64 = 0xffffffffffffffffn;
const EVENT_TAG = Uint8Array.of(228, 69, 165, 46, 81, 203, 154, 29);
export const stringify = (value: unknown) =>
  JSON.stringify(value, (_key, v) =>
    typeof v === "bigint"
      ? v.toString()
      : v instanceof Uint8Array
        ? Array.from(v)
        : v,
  );
const hash = (value: unknown) =>
  createHash("sha256")
    .update(typeof value === "string" ? value : stringify(value))
    .digest("hex");
const hex = (value: Iterable<number>) =>
  Buffer.from([...value]).toString("hex");
function integer(value: unknown, name: string): bigint {
  if (typeof value === "bigint") return value;
  if (typeof value === "number" && Number.isSafeInteger(value))
    return BigInt(value);
  if (typeof value === "string" && /^-?(0|[1-9][0-9]*)$/.test(value))
    return BigInt(value);
  throw new Error("Invalid integer " + name);
}
function u64(value: unknown, name: string): bigint {
  const n = integer(value, name);
  if (n < 0n || n > U64) throw new Error("Out of u64 " + name);
  return n;
}
function limit(
  value: number | undefined,
  fallback: number,
  max: number,
): number {
  const n = value ?? fallback;
  if (!Number.isSafeInteger(n) || n < 1 || n > max)
    throw new Error("Operator bound outside range");
  return n;
}
function bytesEqual(a: Iterable<number>, b: Iterable<number>): boolean {
  return hex(a) === hex(b);
}
export function rpcTransport(url: string): RpcCall {
  const parsed = new URL(url);
  if (!["http:", "https:"].includes(parsed.protocol))
    throw new Error("RPC must be HTTP(S)");
  return async (method, params = []) => {
    const response = await fetch(url, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: stringify({ jsonrpc: "2.0", id: 1, method, params }),
      signal: AbortSignal.timeout(15_000),
    });
    if (!response.ok || !response.body)
      throw new Error("RPC HTTP " + response.status);
    const reader = response.body.getReader();
    const chunks: Uint8Array[] = [];
    let size = 0;
    for (;;) {
      const item = await reader.read();
      if (item.done) break;
      size += item.value.length;
      if (size > 16 * 1024 * 1024) {
        await reader.cancel();
        throw new Error("RPC response exceeds 16MiB");
      }
      chunks.push(item.value);
    }
    const body = JSON.parse(Buffer.concat(chunks).toString("utf8"));
    if (body.error)
      throw new Error("RPC " + method + ": " + stringify(body.error));
    if (!("result" in body)) throw new Error("Missing RPC result");
    return body.result;
  };
}
const accountDecoders: Decoder<unknown>[] = [
  g.getConfigDecoder(),
  g.getExposureDecoder(),
  g.getLenderReceiptDecoder(),
  g.getLoanDecoder(),
  g.getMarketDecoder(),
  g.getPolicyDecoder(),
  g.getPoolGenerationDecoder(),
  g.getPriceReferenceDecoder(),
];
const accountSizes = [
  g.getConfigSize(),
  g.getExposureSize(),
  g.getLenderReceiptSize(),
  g.getLoanSize(),
  g.getMarketSize(),
  g.getPolicySize(),
  g.getPoolGenerationSize(),
  g.getPriceReferenceSize(),
];
function selected<T>(rows: Row[], kind: number): Row<T>[] {
  return rows.filter((r) => r.kind === kind) as Row<T>[];
}
export async function readSnapshot(
  rpc: RpcCall,
  maxAccounts = 2048,
): Promise<Snapshot> {
  const response = await rpc("getProgramAccounts", [
    PROGRAM,
    { encoding: "base64", commitment: "finalized", withContext: true },
  ]);
  if (
    !response?.context ||
    !Array.isArray(response.value) ||
    response.value.length > maxAccounts
  )
    throw new Error("Incomplete/oversized finalized state snapshot");
  const slot = u64(response.context.slot, "snapshot slot");
  const rows: Row[] = [];
  const seen = new Set<string>();
  for (const raw of response.value) {
    const key = address(raw.pubkey);
    if (
      seen.has(key) ||
      raw.account.owner !== PROGRAM ||
      raw.account.executable ||
      raw.account.data?.[1] !== "base64"
    )
      throw new Error("Invalid state account");
    seen.add(key);
    const bytes = getBase64Encoder().encode(raw.account.data[0]);
    const kind = g.identifyShortwinSimpleAccount(bytes);
    if (bytes.length !== accountSizes[kind])
      throw new Error("Wrong state allocation");
    const data = accountDecoders[kind]!.decode(bytes);
    if ((data as { version: number }).version !== 1)
      throw new Error("Unsupported state version");
    rows.push({ address: key, kind, data });
  }
  const configs = selected<g.Config>(rows, g.ShortwinSimpleAccount.Config);
  if (configs.length !== 1 || configs[0]!.address !== (await configAddress()))
    throw new Error("Canonical Config missing");
  const config = configs[0]!;
  const genesis = await rpc("getGenesisHash");
  if (
    typeof genesis !== "string" ||
    !bytesEqual(getBase58Encoder().encode(genesis), config.data.genesisHash)
  )
    throw new Error("RPC genesis differs from initialized Config");
  const timestamp = integer(
    await rpc("getBlockTime", [Number(slot)]),
    "finalized block time",
  );
  const head = u64(
    await rpc("getSlot", [{ commitment: "finalized" }]),
    "head slot",
  );
  if (
    head < slot ||
    head - slot > 64n ||
    timestamp <= 0n ||
    BigInt(Math.floor(Date.now() / 1000)) - timestamp > 90n ||
    timestamp - BigInt(Math.floor(Date.now() / 1000)) > 30n
  )
    throw new Error("State snapshot clock is stale");
  const policies = new Map(
    selected<g.Policy>(rows, g.ShortwinSimpleAccount.Policy).map((r) => [
      r.address,
      r.data,
    ]),
  );
  const markets = new Map(
    selected<g.Market>(rows, g.ShortwinSimpleAccount.Market).map((r) => [
      r.address,
      r.data,
    ]),
  );
  const pools = new Map(
    selected<g.PoolGeneration>(
      rows,
      g.ShortwinSimpleAccount.PoolGeneration,
    ).map((r) => [r.address, r.data]),
  );
  const references = new Map(
    selected<g.PriceReference>(
      rows,
      g.ShortwinSimpleAccount.PriceReference,
    ).map((r) => [r.address, r.data]),
  );
  if (
    !policies.has(config.data.currentPolicy) ||
    policies.get(config.data.currentPolicy)!.policyVersion !==
      config.data.currentPolicyVersion
  )
    throw new Error("Current immutable Policy missing from complete snapshot");
  for (const [key, p] of policies)
    if (
      p.config !== config.address ||
      key !== (await policyAddress(p.policyVersion))
    )
      throw new Error("Policy binding mismatch");
  for (const [key, m] of markets)
    if (
      m.config !== config.address ||
      key !== (await marketAddress(m.mint)) ||
      ![TOKEN, TOKEN_2022].includes(m.tokenProgram as typeof TOKEN)
    )
      throw new Error("Market binding mismatch");
  for (const [key, p] of pools)
    if (
      !markets.has(p.market) ||
      key !== (await poolAddress(p.market, p.generation)) ||
      p.mint !== markets.get(p.market)!.mint ||
      p.tokenProgram !== markets.get(p.market)!.tokenProgram
    )
      throw new Error("Pool binding mismatch");
  const loans = selected<g.Loan>(rows, g.ShortwinSimpleAccount.Loan);
  for (const row of loans) {
    const l = row.data;
    if (
      !policies.has(l.policy) ||
      !pools.has(l.pool) ||
      !markets.has(l.market) ||
      pools.get(l.pool)!.market !== l.market ||
      l.q <= 0n ||
      l.collateral <= 0n ||
      l.defaultAt <= l.deadline ||
      row.address !== (await loanAddress(l.pool, l.borrower, l.nonce))
    )
      throw new Error("Active loan binding mismatch");
  }
  return {
    slot,
    timestamp,
    config,
    loans,
    policies,
    markets,
    pools,
    references,
    rows,
  };
}
export function activeJobs(snapshot: Snapshot): Job[] {
  const jobs = new Map<string, Job>();
  for (const { data: l } of snapshot.loans) {
    const job = {
      market: l.market,
      policy: l.policy,
      mint: snapshot.markets.get(l.market)!.mint,
      quantity: l.q.toString(),
      active_loan: true as const,
    };
    jobs.set([job.market, job.policy, job.quantity].join(":"), job);
  }
  return [...jobs.values()].sort((a, b) =>
    stringify(a).localeCompare(stringify(b)),
  );
}
export function validateServicePolicy(
  settings: ServiceSettings,
  modelHash: string,
  policy: g.Policy,
): void {
  const p = policy.terms;
  if (
    hex(p.pricePolicyHash) !== modelHash ||
    settings.epoch_seconds !== p.historyEpochSeconds ||
    settings.history_epochs !== p.minimumHistoryEpochs ||
    settings.reference_max_age_seconds !== p.referenceMaxAgeSeconds ||
    BigInt(settings.reference_max_age_slots) !== p.referenceMaxAgeSlots ||
    settings.max_spread_bps !== p.maxSpreadBps
  )
    throw new Error("Immutable price-service model/settings drift");
}
export function referenceReady(
  r: g.PriceReference | undefined,
  l: g.Loan,
  policy: g.Policy,
  config: g.Config,
  clock: { slot: bigint; timestamp: bigint },
): boolean {
  if (
    !r ||
    config.pricesPaused ||
    !r.finite ||
    r.market !== l.market ||
    r.policy !== l.policy ||
    r.quantity !== l.q ||
    r.reporterEpoch !== config.reporterEpoch ||
    r.sequence <= 0n ||
    r.sourceSlot > clock.slot ||
    r.sourceTs > clock.timestamp
  )
    return false;
  const p = policy.terms;
  return (
    clock.slot - r.sourceSlot <= p.referenceMaxAgeSlots &&
    clock.timestamp - r.sourceTs <= BigInt(p.referenceMaxAgeSeconds) &&
    r.liveBidLower > 0n &&
    r.historyBidLowerMax > 0n &&
    r.acquisitionReference > 0n
  );
}
export function selectAction(
  l: g.Loan,
  p: g.Policy,
  c: g.Config,
  r: g.PriceReference | undefined,
  clock: { slot: bigint; timestamp: bigint },
): Action {
  if (clock.timestamp >= l.defaultAt) return "default";
  if (!referenceReady(r, l, p, c, clock)) return "wait-reference";
  if (clock.timestamp >= l.deadline) return "force";
  const unhealthy = (bid: bigint) =>
    l.collateral * 10000n < bid * BigInt(p.terms.crLiqBps);
  if (!unhealthy(r!.liveBidLower) || !unhealthy(r!.historyBidLowerMax))
    return l.crArmSequence > 0n ? "clear-arm" : "wait";
  if (l.crArmSequence === 0n || l.crArmReporterEpoch !== c.reporterEpoch)
    return "arm";
  const ts = l.crArmTimestamp + BigInt(p.terms.crConfirmationDelaySeconds),
    slot = l.crArmSlot + p.terms.crConfirmationDelaySlots;
  return r!.sequence > l.crArmSequence &&
    r!.sourceTs >= ts &&
    r!.sourceSlot >= slot &&
    clock.timestamp >= ts &&
    clock.slot >= slot
    ? "force"
    : "wait";
}
export function keeperCap(l: g.Loan, p: g.Policy, r: g.PriceReference): bigint {
  const price =
    (r.acquisitionReference * (10000n + BigInt(p.terms.buybackPremiumBps))) /
    10000n;
  const cash =
    l.collateral > p.terms.keeperCapLamports
      ? l.collateral - p.terms.keeperCapLamports
      : 0n;
  return price < cash ? price : cash;
}

/** State is authoritative; finalized successful events are retained independently.
 * No failed transaction or speculative event mutates the active account snapshot. */
export class OperatorStore {
  readonly db: DatabaseSync;
  constructor(path: string) {
    this.db = new DatabaseSync(path);
    this.db.exec(
      "PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS meta(key TEXT PRIMARY KEY,value TEXT NOT NULL); CREATE TABLE IF NOT EXISTS accounts(address TEXT PRIMARY KEY,kind INTEGER NOT NULL,slot TEXT NOT NULL,data TEXT NOT NULL); CREATE TABLE IF NOT EXISTS snapshots(slot TEXT PRIMARY KEY,hash TEXT NOT NULL,body TEXT NOT NULL); CREATE TABLE IF NOT EXISTS transactions(signature TEXT PRIMARY KEY,slot TEXT NOT NULL,status TEXT NOT NULL); CREATE TABLE IF NOT EXISTS events(signature TEXT NOT NULL,ordinal INTEGER NOT NULL,slot TEXT NOT NULL,name TEXT NOT NULL,data TEXT NOT NULL,PRIMARY KEY(signature,ordinal)); CREATE TABLE IF NOT EXISTS attempts(id TEXT PRIMARY KEY,loan TEXT NOT NULL,status TEXT NOT NULL,signature TEXT,body TEXT NOT NULL);",
    );
  }
  get(key: string): string | undefined {
    return (
      this.db.prepare("SELECT value FROM meta WHERE key=?").get(key) as
        { value: string } | undefined
    )?.value;
  }
  set(key: string, value: string): void {
    this.db
      .prepare(
        "INSERT INTO meta(key,value) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
      )
      .run(key, value);
  }
  snapshot(s: Snapshot): void {
    this.db.exec("BEGIN IMMEDIATE");
    try {
      this.db.exec("DELETE FROM accounts");
      const put = this.db.prepare("INSERT INTO accounts VALUES(?,?,?,?)");
      for (const r of s.rows)
        put.run(r.address, r.kind, s.slot.toString(), stringify(r.data));
      const body = stringify({
        slot: s.slot,
        timestamp: s.timestamp,
        rows: s.rows,
      });
      this.db
        .prepare("INSERT OR REPLACE INTO snapshots VALUES(?,?,?)")
        .run(s.slot.toString(), hash(body), body);
      this.db.exec(
        "DELETE FROM snapshots WHERE rowid NOT IN (SELECT rowid FROM snapshots ORDER BY rowid DESC LIMIT 8);COMMIT",
      );
    } catch (e) {
      this.db.exec("ROLLBACK");
      throw e;
    }
  }
  close(): void {
    this.db.close();
  }
}
const eventDecoders: Decoder<unknown>[] = [
  g.getClosedEventDecoder(),
  g.getConfigChangedEventDecoder(),
  g.getDefaultedEventDecoder(),
  g.getDepositEventDecoder(),
  g.getForcedExitEventDecoder(),
  g.getLiquidationArmedEventDecoder(),
  g.getMarginAddedEventDecoder(),
  g.getMarketRegisteredEventDecoder(),
  g.getOpenedEventDecoder(),
  g.getPolicyCreatedEventDecoder(),
  g.getPoolRolledEventDecoder(),
  g.getReferencePublishedEventDecoder(),
  g.getRepaidEventDecoder(),
  g.getSolClaimedEventDecoder(),
  g.getWithdrawEventDecoder(),
];
export function successfulEvents(
  transaction: ObjectValue,
): { ordinal: number; name: string; data: unknown }[] {
  if (!transaction?.meta || transaction.meta.err !== null) return [];
  const keys = [
    ...transaction.transaction.message.accountKeys,
    ...(transaction.meta.loadedAddresses?.writable ?? []),
    ...(transaction.meta.loadedAddresses?.readonly ?? []),
  ].map((k: any) => (typeof k === "string" ? k : k.pubkey));
  const events: { ordinal: number; name: string; data: unknown }[] = [];
  let ordinal = 0;
  for (const group of transaction.meta.innerInstructions ?? [])
    for (const inner of group.instructions ?? []) {
      if (keys[inner.programIdIndex] !== PROGRAM) continue;
      const data = getBase58Encoder().encode(inner.data);
      if (data.length < 16 || !bytesEqual(data.slice(0, 8), EVENT_TAG))
        continue;
      const payload = data.slice(8);
      const kind = g.identifyShortwinSimpleEvent(payload);
      events.push({
        ordinal: ordinal++,
        name: g.ShortwinSimpleEvent[kind]!,
        data: eventDecoders[kind]!.decode(payload),
      });
    }
  return events;
}
export async function indexEvents(
  store: OperatorStore,
  rpc: RpcCall,
  maximum = 32,
): Promise<{ caughtUp: boolean; read: number }> {
  const oldHead = store.get("event_head") ?? null;
  const scan = JSON.parse(
    store.get("event_scan") ??
      stringify({ oldHead, before: null, newHead: null }),
  ) as {
    oldHead: string | null;
    before: string | null;
    newHead: string | null;
  };
  const rows = await rpc("getSignaturesForAddress", [
    PROGRAM,
    {
      commitment: "finalized",
      limit: maximum,
      ...(scan.before ? { before: scan.before } : {}),
    },
  ]);
  if (!Array.isArray(rows) || rows.length > maximum)
    throw new Error("Oversized event page");
  if (!scan.newHead && rows.length) scan.newHead = rows[0].signature;
  const at = scan.oldHead
    ? rows.findIndex((r: any) => r.signature === scan.oldHead)
    : -1;
  const selected = at >= 0 ? rows.slice(0, at) : rows;
  const records = [];
  for (const row of selected) {
    if (
      store.db
        .prepare("SELECT signature FROM transactions WHERE signature=?")
        .get(row.signature)
    )
      continue;
    const tx = await rpc("getTransaction", [
      row.signature,
      {
        encoding: "json",
        commitment: "finalized",
        maxSupportedTransactionVersion: 0,
      },
    ]);
    if (!tx)
      throw new Error(
        "Finalized transaction evidence unavailable: " + row.signature,
      );
    records.push({
      signature: String(row.signature),
      slot: u64(row.slot, "event slot").toString(),
      status: tx.meta?.err === null ? "success" : "failed",
      events: successfulEvents(tx),
    });
  }
  const done = at >= 0 || rows.length < maximum;
  if (done && scan.oldHead && at < 0)
    throw new Error(
      "Saved event cursor missing from RPC history; archival evidence required",
    );
  store.db.exec("BEGIN IMMEDIATE");
  try {
    for (const r of records) {
      store.db
        .prepare("INSERT OR IGNORE INTO transactions VALUES(?,?,?)")
        .run(r.signature, r.slot, r.status);
      for (const e of r.events)
        store.db
          .prepare("INSERT OR IGNORE INTO events VALUES(?,?,?,?,?)")
          .run(r.signature, e.ordinal, r.slot, e.name, stringify(e.data));
    }
    if (done) {
      if (scan.newHead) store.set("event_head", scan.newHead);
      store.db.prepare("DELETE FROM meta WHERE key=?").run("event_scan");
    } else {
      scan.before = rows.at(-1).signature;
      store.set("event_scan", stringify(scan));
    }
    store.db.exec("COMMIT");
  } catch (e) {
    store.db.exec("ROLLBACK");
    throw e;
  }
  return { caughtUp: done, read: selected.length };
}
export function pythonCommand(
  python: string,
  settings: string,
  database: string,
  command: readonly string[],
  input?: unknown,
): Promise<string> {
  return new Promise((resolvePromise, reject) => {
    const child = spawn(
      python,
      [
        SCRIPT,
        "--settings",
        resolve(settings),
        "--database",
        resolve(database),
        ...command,
      ],
      { shell: false, stdio: ["pipe", "pipe", "pipe"], env: process.env },
    );
    let stdout = "",
      stderr = "";
    const timer = setTimeout(() => {
      child.kill();
      reject(new Error("Price-service command timed out"));
    }, 120_000);
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
      if (stdout.length > 4 * 1024 * 1024) {
        child.kill();
        reject(new Error("Price-service output exceeds bound"));
      }
    });
    child.stderr.on("data", (chunk) => {
      if (stderr.length < 8192) stderr += chunk;
    });
    child.on("error", (e) => {
      clearTimeout(timer);
      reject(e);
    });
    child.on("close", (code) => {
      clearTimeout(timer);
      if (code !== 0)
        reject(
          new Error(
            "Price-service " + command[0] + " failed: " + stderr.trim(),
          ),
        );
      else resolvePromise(stdout.trim());
    });
    child.stdin.end(input === undefined ? "" : stringify(input));
  });
}

export function candidateReference(
  raw: ObjectValue,
  job: Job,
  policy: g.Policy,
  config: g.Config,
  current: g.PriceReference | undefined,
  clock: { slot: bigint; timestamp: bigint },
): g.PriceReference {
  if (
    raw.market !== job.market ||
    raw.policy !== job.policy ||
    raw.model_hash !== hex(policy.terms.pricePolicyHash) ||
    !raw.args
  )
    throw new Error("Candidate reference identity/model mismatch");
  const a = raw.args;
  const n = (name: string) => u64(a[name], name);
  const ts = integer(a.source_ts, "source_ts");
  if (
    n("quantity") !== BigInt(job.quantity) ||
    n("sequence") !== (current?.sequence ?? 0n) + 1n ||
    n("source_slot") > clock.slot ||
    ts > clock.timestamp ||
    ts <= 0n ||
    clock.slot - n("source_slot") > policy.terms.referenceMaxAgeSlots ||
    clock.timestamp - ts > BigInt(policy.terms.referenceMaxAgeSeconds)
  )
    throw new Error("Candidate reference sequence/freshness mismatch");
  if (
    typeof a.finite !== "boolean" ||
    typeof a.evidence_hash !== "string" ||
    !/^[0-9a-f]{64}$/.test(a.evidence_hash)
  )
    throw new Error("Bad reference shape");
  const r: g.PriceReference = {
    discriminator: new Uint8Array(8),
    version: 1,
    bump: 0,
    market: job.market,
    policy: job.policy,
    quantity: n("quantity"),
    reporterEpoch: config.reporterEpoch,
    sequence: n("sequence"),
    finite: a.finite,
    sourceSlot: n("source_slot"),
    sourceTs: ts,
    liveBidLower: n("live_bid_lower"),
    liveAskUpper: n("live_ask_upper"),
    historyAskUpperMax: n("history_ask_upper_max"),
    historyBidLowerMax: n("history_bid_lower_max"),
    acquisitionReference: n("acquisition_reference"),
    completedEpochs: Number(a.completed_epochs),
    historyStartTs: integer(a.history_start_ts, "history_start_ts"),
    historyEndTs: integer(a.history_end_ts, "history_end_ts"),
    evidenceHash: Uint8Array.from(Buffer.from(a.evidence_hash, "hex")),
  };
  if (r.finite) {
    const p = policy.terms,
      epoch = BigInt(p.historyEpochSeconds),
      start = (ts / epoch) * epoch;
    if (
      raw.status !== "Finite" ||
      r.liveBidLower <= 0n ||
      r.liveAskUpper < r.liveBidLower ||
      r.historyBidLowerMax <= 0n ||
      r.historyAskUpperMax < r.historyBidLowerMax ||
      r.acquisitionReference <= 0n ||
      r.acquisitionReference > r.liveAskUpper ||
      r.acquisitionReference > r.historyAskUpperMax ||
      r.completedEpochs !== p.minimumHistoryEpochs ||
      r.historyStartTs !== start - epoch * BigInt(p.minimumHistoryEpochs) ||
      r.historyEndTs !== start - 1n ||
      r.evidenceHash.every((x) => x === 0) ||
      (r.liveAskUpper - r.liveBidLower) * 10000n >
        r.liveBidLower * BigInt(p.maxSpreadBps)
    )
      throw new Error("Finite candidate history/spread/domain mismatch");
  } else if (
    raw.status !== "Unknown" ||
    r.liveBidLower !== 0n ||
    r.liveAskUpper !== 0n ||
    r.historyAskUpperMax !== 0n ||
    r.historyBidLowerMax !== 0n ||
    r.acquisitionReference !== 0n ||
    r.completedEpochs !== 0 ||
    r.historyStartTs !== 0n ||
    r.historyEndTs !== 0n ||
    r.evidenceHash.some((x) => x !== 0)
  )
    throw new Error("Unknown must clear price eligibility");
  return r;
}
export async function referenceAfterPublication(
  execute: boolean,
  old: g.PriceReference | undefined,
  sequence: bigint,
  reread: () => Promise<g.PriceReference>,
): Promise<g.PriceReference | undefined> {
  if (!execute) return old;
  const current = await reread();
  if (current.sequence < sequence)
    throw new Error("Confirmed reference is behind submitted publication");
  return current;
}
export function fundingRequirement(
  prepared: { feeLamports: bigint },
  peak: bigint,
  balance: bigint,
  bounds: { maxFee: bigint; remainingFees: bigint; minBalance: bigint },
): bigint {
  if (
    prepared.feeLamports < 0n ||
    prepared.feeLamports > bounds.maxFee ||
    prepared.feeLamports > bounds.remainingFees ||
    peak < 0n ||
    balance < prepared.feeLamports + peak + bounds.minBalance
  )
    throw new Error("Known funding/fee budget is insufficient");
  return bounds.remainingFees - prepared.feeLamports;
}
async function signerFile(path: string, expected: Address) {
  const raw = await readFile(resolve(path));
  if (raw.length > 4096) throw new Error("Invalid keyfile length");
  const values = JSON.parse(raw.toString("utf8"));
  if (
    !Array.isArray(values) ||
    values.length !== 64 ||
    values.some((v) => !Number.isInteger(v) || v < 0 || v > 255)
  )
    throw new Error("Expected a 64-byte Solana keyfile");
  const signer = await createKeyPairSignerFromBytes(Uint8Array.from(values));
  if (signer.address !== expected)
    throw new Error("Operator keyfile identity mismatch");
  return signer;
}
async function currentClock(
  rpc: RpcCall,
): Promise<{ slot: bigint; timestamp: bigint }> {
  const slot = u64(await rpc("getSlot", [{ commitment: "confirmed" }]), "slot");
  const timestamp = integer(
    await rpc("getBlockTime", [Number(slot)]),
    "block time",
  );
  if (
    timestamp <= 0n ||
    BigInt(Math.floor(Date.now() / 1000)) - timestamp > 60n ||
    timestamp - BigInt(Math.floor(Date.now() / 1000)) > 30n
  )
    throw new Error("Current RPC clock unavailable/stale");
  return { slot, timestamp };
}
async function confirmedLoan(
  rpc: RpcCall,
  previous: Row<g.Loan>,
): Promise<Row<g.Loan> | undefined> {
  const response = await rpc("getAccountInfo", [
    previous.address,
    { encoding: "base64", commitment: "confirmed" },
  ]);
  const raw = response?.value;
  if (raw === null) return undefined;
  if (
    !raw ||
    raw.owner !== PROGRAM ||
    raw.executable ||
    raw.data?.[1] !== "base64"
  )
    throw new Error("Confirmed loan owner/encoding mismatch");
  const bytes = getBase64Encoder().encode(raw.data[0]);
  if (
    bytes.length !== g.getLoanSize() ||
    g.identifyShortwinSimpleAccount(bytes) !== g.ShortwinSimpleAccount.Loan
  )
    throw new Error("Confirmed loan allocation/type mismatch");
  const loan = g.getLoanDecoder().decode(bytes),
    old = previous.data;
  if (
    loan.version !== 1 ||
    loan.q !== old.q ||
    loan.borrower !== old.borrower ||
    loan.pool !== old.pool ||
    loan.market !== old.market ||
    loan.policy !== old.policy ||
    loan.nonce !== old.nonce
  )
    throw new Error("Confirmed loan immutable identity mismatch");
  return { ...previous, data: loan };
}
/** Stable bounded rotation prevents one failed route from starving later debt. */
export function roundRobin<T>(
  items: readonly T[],
  last: string | undefined,
  key: (item: T) => string,
): T[] {
  const sorted = [...items].sort((a, b) => key(a).localeCompare(key(b)));
  const at = sorted.findIndex((item) => key(item) === last);
  return at < 0
    ? sorted
    : [...sorted.slice(at + 1), ...sorted.slice(0, at + 1)];
}
export async function runOperator(
  options: OperatorOptions,
  dependencies: {
    rpc?: RpcCall;
    snapshot?: () => Promise<Snapshot>;
    service?: (command: readonly string[], input?: unknown) => Promise<string>;
    prepare?: typeof prepareTransaction;
  } = {},
): Promise<ObjectValue> {
  const maxAccounts = limit(options.maxAccounts, 2048, 10000),
    maxEvents = limit(options.maxEventTransactions, 32, 1000),
    maxActions = limit(options.maxActions, 8, 32);
  const bounds = {
    maxFee: options.maxFeeLamports ?? 25_000n,
    remainingFees: options.maxPassFeesLamports ?? 100_000n,
    minBalance: options.minBalanceLamports ?? 100_000n,
  };
  const temporaryMax = options.maxTemporaryLamports ?? 10_000_000n,
    referenceRentMax = options.maxReferenceRentLamports ?? 10_000_000n;
  if (
    bounds.maxFee < 0n ||
    bounds.remainingFees < 0n ||
    bounds.minBalance < 0n ||
    temporaryMax < 0n ||
    referenceRentMax < 0n
  )
    throw new Error("Negative operator funding bound");
  await mkdir(dirname(resolve(options.database)), { recursive: true });
  await mkdir(resolve(options.evidenceDirectory), { recursive: true });
  const store = new OperatorStore(resolve(options.database));
  const rpc = dependencies.rpc ?? rpcTransport(options.rpcUrl);
  const python = options.python ?? "python3";
  const service =
    dependencies.service ??
    ((command: readonly string[], input?: unknown) =>
      pythonCommand(
        python,
        options.serviceSettings,
        options.serviceDatabase,
        command,
        input,
      ));
  const plans: ObjectValue[] = [];
  const alerts: string[] = [];
  try {
    const snapshot = await (dependencies.snapshot
      ? dependencies.snapshot()
      : readSnapshot(rpc, maxAccounts));
    store.snapshot(snapshot);
    const indexed = await indexEvents(store, rpc, maxEvents);
    // Explicitly reconcile unknown outcomes before any potentially duplicate send.
    for (const row of store.db
      .prepare(
        "SELECT id,signature FROM attempts WHERE status IN ('submitted','unknown')",
      )
      .all() as { id: string; signature: string }[]) {
      if (!row.signature) continue;
      const status = await rpc("getSignatureStatuses", [
        [row.signature],
        { searchTransactionHistory: true },
      ]);
      const value = status?.value?.[0];
      if (value?.err !== null && value?.err !== undefined)
        store.db
          .prepare("UPDATE attempts SET status=? WHERE id=?")
          .run("failed", row.id);
      else if (
        value?.confirmationStatus === "finalized" ||
        value?.confirmationStatus === "confirmed"
      )
        store.db
          .prepare("UPDATE attempts SET status=? WHERE id=?")
          .run("confirmed", row.id);
    }
    let count = 0;
    const reservedFunds = new Map<Address, bigint>();
    const actor = options.keeperAddress ?? snapshot.config.data.reporter;
    const event = await eventAuthority();
    const dispatch = async (
      kind: string,
      loan: Address,
      instructions: Instruction[],
      payer: Address,
      keyfile: string | undefined,
      peak: bigint,
      tables: Awaited<ReturnType<typeof readLookupTables>> = {},
      permanentSpend = 0n,
    ) => {
      if (count >= maxActions) {
        alerts.push("Action capacity reached; remaining loans stay pending");
        return;
      }
      count++;
      if (
        store.db
          .prepare(
            "SELECT id FROM attempts WHERE loan=? AND status IN ('submitted','unknown')",
          )
          .get(loan)
      ) {
        alerts.push(
          "Unresolved submitted signature blocks another action for " + loan,
        );
        return;
      }
      const prepared = await (dependencies.prepare ?? prepareTransaction)(
        options.rpcUrl,
        payer,
        instructions,
        tables,
      );
      const balance = u64(
        (await rpc("getBalance", [payer, { commitment: "confirmed" }])).value,
        "payer balance",
      );
      bounds.remainingFees = fundingRequirement(
        prepared,
        peak,
        balance - (reservedFunds.get(payer) ?? 0n),
        bounds,
      );
      const id = hash({
        program: PROGRAM,
        loan,
        kind,
        slot: snapshot.slot,
        instructions,
      });
      const artifact = resolve(options.evidenceDirectory, id + ".json");
      const record = {
        id,
        kind,
        loan,
        payer,
        mode: options.execute ? "execute" : "dry-run",
        snapshotSlot: snapshot.slot,
        feeLamports: prepared.feeLamports,
        peakAdvanceLamports: peak,
        units: prepared.units,
        preparedBase64: prepared.base64,
        lastValidBlockHeight: prepared.lastValidBlockHeight,
        simulationLogs: prepared.logs,
      };
      await writeFile(artifact, stringify(record) + "\n", { mode: 0o600 });
      const plan: ObjectValue = { ...record, artifact, status: "simulated" };
      plans.push(plan);
      if (!options.execute) {
        reservedFunds.set(
          payer,
          (reservedFunds.get(payer) ?? 0n) +
            prepared.feeLamports +
            permanentSpend,
        );
        return;
      }
      if (!indexed.caughtUp)
        throw new Error(
          "Event cursor is catching up; execute remains disabled",
        );
      if (!keyfile)
        throw new Error("Explicit role keyfile required for execute");
      const signer = await signerFile(keyfile, payer);
      const signed = await signTransaction(
        [signer.keyPair],
        getTransactionDecoder().decode(prepared.bytes),
      );
      const signature = getSignatureFromTransaction(signed);
      store.db
        .prepare("INSERT OR REPLACE INTO attempts VALUES(?,?,?,?,?)")
        .run(id, loan, "submitted", signature, stringify(record));
      try {
        const result = await submitSigned(
          options.rpcUrl,
          prepared,
          Uint8Array.from(getTransactionEncoder().encode(signed)),
        );
        store.db
          .prepare("UPDATE attempts SET status=? WHERE id=?")
          .run("confirmed", id);
        plan.status = "confirmed";
        plan.signature = result;
      } catch (error) {
        store.db
          .prepare("UPDATE attempts SET status=? WHERE id=?")
          .run("unknown", id);
        throw new Error(
          "Submitted outcome requires signature reconciliation: " +
            signature +
            "; " +
            String(error),
        );
      }
    };
    const common = async (row: Row<g.Loan>) => ({
      actor: createNoopSigner(actor),
      config: snapshot.config.address,
      policy: row.data.policy,
      market: row.data.market,
      pool: row.data.pool,
      loan: row.address,
      exposure: await exposureAddress(row.data.pool, row.data.borrower),
      borrower: row.data.borrower,
      eventAuthority: event,
      program: PROGRAM,
    });
    // Default is independent of service configuration, histories, route availability and price pause.
    const passClock = snapshot.loans.length
      ? await currentClock(rpc)
      : { slot: snapshot.slot, timestamp: snapshot.timestamp };
    for (const row of roundRobin(
      snapshot.loans.filter((r) => passClock.timestamp >= r.data.defaultAt),
      store.get("default_loan_cursor"),
      (row) => row.address,
    ))
      try {
        if (count >= maxActions) {
          alerts.push(
            "Default action capacity reached; later defaults rotate into the next pass",
          );
          break;
        }
        store.set("default_loan_cursor", row.address);
        if (actor === row.data.borrower)
          throw new Error("Keeper must differ from borrower");
        await dispatch(
          "default",
          row.address,
          [g.getDefaultLoanInstruction(await common(row))],
          actor,
          options.keeperKeyfile,
          0n,
        );
      } catch (error) {
        alerts.push(row.address + ": " + String(error));
      }
    try {
      await service(["sync-loans", "-"], activeJobs(snapshot));
    } catch (error) {
      alerts.push("Active identity synchronization failed: " + String(error));
    }
    const pending = snapshot.loans.filter(
      (r) => passClock.timestamp < r.data.defaultAt,
    );
    let settings: ServiceSettings, modelHash: string;
    try {
      settings = JSON.parse(
        await readFile(resolve(options.serviceSettings), "utf8"),
      );
      modelHash = await service(["model-hash"]);
      if (!/^[0-9a-f]{64}$/.test(modelHash))
        throw new Error("Invalid service model hash");
      if (options.tick)
        await service(["tick", "--taker", snapshot.config.data.reporter]);
    } catch (error) {
      alerts.push(
        "Price service unavailable; priced actions remain pending: " +
          String(error),
      );
      return {
        program: PROGRAM,
        mode: options.execute ? "execute" : "dry-run",
        slot: snapshot.slot,
        loans: snapshot.loans.length,
        indexed,
        plans,
        alerts,
      };
    }
    const grouped = new Map<string, { job: Job; rows: Row<g.Loan>[] }>();
    for (const row of pending) {
      const l = row.data,
        id = [l.market, l.policy, l.q].join(":");
      const found = grouped.get(id);
      const job: Job = {
        market: l.market,
        policy: l.policy,
        mint: snapshot.markets.get(l.market)!.mint,
        quantity: l.q.toString(),
        active_loan: true,
      };
      grouped.set(id, { job, rows: [...(found?.rows ?? []), row] });
    }
    const queue = JSON.parse(await service(["jobs"]));
    if (
      !Array.isArray(queue) ||
      queue.length > limit(Number(settings.max_jobs), 32, 2048)
    )
      throw new Error("Queued identity capacity invalid");
    for (const raw of queue) {
      const job: Job = {
        market: address(raw.market),
        policy: address(raw.policy),
        mint: address(raw.mint),
        quantity: u64(raw.quantity, "queued quantity").toString(),
        active_loan: Boolean(raw.active_loan),
      };
      if (
        BigInt(job.quantity) <= 0n ||
        !snapshot.markets.has(job.market) ||
        snapshot.markets.get(job.market)!.mint !== job.mint ||
        !snapshot.policies.has(job.policy)
      )
        throw new Error("Queued identity is not an indexed market/policy");
      const id = [job.market, job.policy, job.quantity].join(":");
      if (grouped.has(id)) continue;
      if (job.active_loan) continue; // Only the authoritative pending-loan set gives old-policy priority.
      if (job.policy !== snapshot.config.data.currentPolicy) continue;
      grouped.set(id, { job, rows: [] });
    }
    const identity = (group: { job: Job }) =>
      [group.job.market, group.job.policy, group.job.quantity].join(":");
    const groups = [
      ...roundRobin(
        [...grouped.values()].filter((g) => g.rows.length > 0),
        store.get("active_group_cursor"),
        identity,
      ),
      ...roundRobin(
        [...grouped.values()].filter((g) => g.rows.length === 0),
        store.get("warm_group_cursor"),
        identity,
      ),
    ];
    for (const { job, rows } of groups)
      try {
        if (count >= maxActions) {
          alerts.push(
            "Action capacity reached; remaining identities rotate into the next pass",
          );
          break;
        }
        store.set(
          rows.length ? "active_group_cursor" : "warm_group_cursor",
          [job.market, job.policy, job.quantity].join(":"),
        );
        const l = {
            market: job.market,
            policy: job.policy,
            q: BigInt(job.quantity),
          },
          p = snapshot.policies.get(job.policy)!,
          market = snapshot.markets.get(job.market)!;
        validateServicePolicy(settings, modelHash, p);
        await service(["check-policy", "-"], p);
        const reference = await referenceAddress(l.market, l.policy, l.q);
        const old = snapshot.references.get(reference);
        let r = old;
        let clock = await currentClock(rpc);
        const outputRaw = await service(
          [
            "reference",
            "-",
            "--onchain-sequence",
            (old?.sequence ?? 0n).toString(),
            "--current-slot",
            clock.slot.toString(),
            "--current-ts",
            clock.timestamp.toString(),
          ],
          job,
        );
        const output = JSON.parse(outputRaw);
        const evidencePath = resolve(
          options.evidenceDirectory,
          "reference-" + hash(outputRaw) + ".json",
        );
        await writeFile(evidencePath, outputRaw + "\n", { mode: 0o600 });
        plans.push({
          kind: "reference-evidence",
          job,
          status: output.status,
          artifact: evidencePath,
        });
        const candidate = candidateReference(
          output,
          job,
          p,
          snapshot.config.data,
          old,
          clock,
        );
        const publication: Instruction[] = [];
        let peak = 0n;
        if (
          !old ||
          (candidate.sourceSlot > old.sourceSlot &&
            candidate.sourceTs >= old.sourceTs)
        ) {
          r = candidate;
          const rent = old
            ? 0n
            : u64(
                await rpc("getMinimumBalanceForRentExemption", [
                  g.getPriceReferenceSize(),
                  { commitment: "confirmed" },
                ]),
                "reference rent",
              );
          if (rent > referenceRentMax)
            throw new Error("Reference rent exceeds explicit bound");
          peak = rent;
          publication.push(
            g.getPublishReferenceInstruction({
              reporter: createNoopSigner(snapshot.config.data.reporter),
              config: snapshot.config.address,
              market: l.market,
              policy: l.policy,
              reference,
              systemProgram: SYSTEM,
              eventAuthority: event,
              program: PROGRAM,
              quantity: candidate.quantity,
              sequence: candidate.sequence,
              sourceSlot: candidate.sourceSlot,
              sourceTs: candidate.sourceTs,
              finite: candidate.finite,
              liveBidLower: candidate.liveBidLower,
              liveAskUpper: candidate.liveAskUpper,
              historyAskUpperMax: candidate.historyAskUpperMax,
              historyBidLowerMax: candidate.historyBidLowerMax,
              acquisitionReference: candidate.acquisitionReference,
              completedEpochs: candidate.completedEpochs,
              historyStartTs: candidate.historyStartTs,
              historyEndTs: candidate.historyEndTs,
              evidenceHash: candidate.evidenceHash,
              maxReferenceRent: referenceRentMax,
            }),
          );
          // Publish as its own role-signed transaction; execute then uses the current
          // on-chain reference after confirmation. Dry-run never projects publication.
          await dispatch(
            "publish-reference",
            reference,
            publication,
            snapshot.config.data.reporter,
            options.reporterKeyfile,
            peak,
            {},
            peak,
          );
          r = await referenceAfterPublication(
            Boolean(options.execute),
            old,
            candidate.sequence,
            async () => {
              const response = await rpc("getAccountInfo", [
                reference,
                {
                  encoding: "base64",
                  commitment: "confirmed",
                  minContextSlot: Number(candidate.sourceSlot),
                },
              ]);
              const value = response?.value;
              if (!value || value.owner !== PROGRAM || value.executable)
                throw new Error("Confirmed published reference unavailable");
              const bytes = getBase64Encoder().encode(value.data[0]);
              if (bytes.length !== g.getPriceReferenceSize())
                throw new Error("Invalid confirmed reference allocation");
              return g.getPriceReferenceDecoder().decode(bytes);
            },
          );
          if (options.execute) clock = await currentClock(rpc);
        }
        const cursor = "loan_cursor:" + identity({ job });
        for (const original of roundRobin(
          rows,
          store.get(cursor),
          (row) => row.address,
        )) {
          if (count >= maxActions) {
            alerts.push(
              "Loan action capacity reached; later loans rotate into the next pass",
            );
            break;
          }
          store.set(cursor, original.address);
          try {
            const row = await confirmedLoan(rpc, original);
            if (!row) {
              plans.push({
                loan: original.address,
                kind: "resolved",
                status: "observed",
              });
              continue;
            }
            clock = await currentClock(rpc);
            const loan = row.data;
            const decision = selectAction(
              loan,
              p,
              snapshot.config.data,
              r,
              clock,
            );
            if (decision === "default") {
              await dispatch(
                "default",
                row.address,
                [g.getDefaultLoanInstruction(await common(row))],
                actor,
                options.keeperKeyfile,
                0n,
              );
              continue;
            }
            if (decision === "wait" || decision === "wait-reference") {
              plans.push({
                loan: row.address,
                kind: decision,
                status: "pending",
              });
              continue;
            }
            if (actor === loan.borrower)
              throw new Error("Keeper must differ from borrower");
            const accounts = await common(row);
            if (decision === "arm" || decision === "clear-arm") {
              await dispatch(
                decision,
                row.address,
                [
                  g.getArmLiquidationInstruction({
                    ...accounts,
                    reference,
                    minimumReferenceSequence: r!.sequence,
                  }),
                ],
                actor,
                options.keeperKeyfile,
                0n,
              );
              continue;
            }
            const maximum = keeperCap(loan, p, r!);
            if (maximum <= 0n)
              throw new Error("No affordable whole-debt buyback budget");
            const exec = await executionAddresses(row.address);
            const buildRaw = await service(
              [
                "build-buy",
                "-",
                "--taker",
                exec.executionAuthority,
                "--maximum-input",
                maximum.toString(),
              ],
              job,
            );
            const build = JSON.parse(buildRaw);
            const routeEvidence = resolve(
              options.evidenceDirectory,
              "route-" + hash(buildRaw) + ".json",
            );
            await writeFile(routeEvidence, buildRaw + "\n", { mode: 0o600 });
            plans.push({
              loan: row.address,
              kind: "route-evidence",
              artifact: routeEvidence,
              status: build.status,
            });
            if (build.status !== "Ready" || build.model_hash !== modelHash)
              throw new Error("No fresh bounded whole-debt route");
            const input = u64(build.input_amount, "route input");
            if (input <= 0n || input > maximum)
              throw new Error("Route budget exceeds policy");
            const routeClock = await currentClock(rpc);
            if (routeClock.timestamp >= loan.defaultAt) {
              await dispatch(
                "default",
                row.address,
                [g.getDefaultLoanInstruction(await common(row))],
                actor,
                options.keeperKeyfile,
                0n,
              );
              continue;
            }
            if (!referenceReady(r, loan, p, snapshot.config.data, routeClock))
              throw new Error("Reference became stale during route search");
            const bundle = await planBundle(build.build as BuildResponse, {
              mint: market.mint,
              tokenProgram: market.tokenProgram,
              side: "buy",
              quantity: loan.q,
              inputAmount: input,
              minimumOutput: loan.q,
              ...exec,
            });
            const rent =
              u64(
                await rpc("getMinimumBalanceForRentExemption", [
                  165,
                  { commitment: "confirmed" },
                ]),
                "scratch rent",
              ) * 2n;
            if (rent > temporaryMax)
              throw new Error("Scratch rent exceeds explicit bound");
            const instruction = g.getForceExitInstruction({
              ...accounts,
              ...exec,
              mint: market.mint,
              tokenVault: await vaultAddress(loan.pool),
              tokenProgram: market.tokenProgram,
              reference,
              maxBuybackCost: maximum,
              temporaryFunding: temporaryMax,
              minimumReferenceSequence: r!.sequence,
              route: bundle.route,
            });
            const tables = await readLookupTables(
              options.rpcUrl,
              bundle.lookupTableKeys,
            );
            await dispatch(
              "force",
              row.address,
              [appendRoute(instruction, bundle)],
              actor,
              options.keeperKeyfile,
              rent,
              tables,
            );
          } catch (error) {
            alerts.push(original.address + ": " + String(error));
          }
        }
      } catch (error) {
        alerts.push(job.market + ": " + String(error));
      }
    return {
      program: PROGRAM,
      mode: options.execute ? "execute" : "dry-run",
      slot: snapshot.slot,
      loans: snapshot.loans.length,
      indexed,
      plans,
      alerts,
    };
  } finally {
    store.close();
  }
}
