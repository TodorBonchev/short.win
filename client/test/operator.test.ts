import test from "node:test";
import { DatabaseSync } from "node:sqlite";
import assert from "node:assert/strict";
import { mkdtemp, writeFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { getBase58Decoder } from "@solana/kit";
import * as g from "../src/generated/index.ts";
import { PROGRAM, TOKEN, WSOL } from "../src/pdas.ts";
import {
  OperatorStore,
  activeJobs,
  indexEvents,
  successfulEvents,
  selectAction,
  publicationNeeded,
  keeperCap,
  validateServicePolicy,
  fundingRequirement,
  referenceAfterPublication,
  runOperator,
  candidateReference,
  roundRobin,
  type ServiceSettings,
  type Snapshot,
  type RpcCall,
} from "../src/operator.ts";
const HISTORY="7d17ec71-b15b-4a08-bd8c-bdd2f81b3baf";
function mockHistory(path:string){const db=new DatabaseSync(path);db.exec("CREATE TABLE metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL)");db.prepare("INSERT INTO metadata VALUES('history_instance_id',?)").run(HISTORY);db.close();}
const policy = {
  terms: {
    pricePolicyHash: Uint8Array.from(Buffer.from("ab".repeat(32), "hex")),
    historyEpochSeconds: 60,
    minimumHistoryEpochs: 2,
    referenceMaxAgeSeconds: 30,
    referenceMaxAgeSlots: 100n,
    maxSpreadBps: 1000,
    crLiqBps: 15000,
    crConfirmationDelaySeconds: 5,
    crConfirmationDelaySlots: 5n,
    buybackPremiumBps: 500,
    keeperCapLamports: 10n,
  },
} as unknown as g.Policy;
const config = {
  pricesPaused: false,
  reporterEpoch: 1n,
  currentPolicy: TOKEN,
} as unknown as g.Config;
const loan = {
  market: PROGRAM,
  pool: PROGRAM,
  borrower: WSOL,
  policy: TOKEN,
  q: 10n,
  collateral: 200n,
  deadline: 200n,
  defaultAt: 300n,
  crArmSequence: 0n,
  crArmTimestamp: 0n,
  crArmSlot: 0n,
  crArmReporterEpoch: 0n,
} as unknown as g.Loan;
const ref = {
  market: PROGRAM,
  policy: TOKEN,
  quantity: 10n,
  reporterEpoch: 1n,
  sequence: 5n,
  finite: true,
  sourceSlot: 100n,
  sourceTs: 100n,
  liveBidLower: 100n,
  historyBidLowerMax: 100n,
  acquisitionReference: 100n,
} as unknown as g.PriceReference;
const clock = { slot: 100n, timestamp: 100n };
const settings: ServiceSettings = {
  epoch_seconds: 60,
  history_epochs: 2,
  reference_max_age_seconds: 30,
  reference_max_age_slots: 100,
  max_spread_bps: 1000,
  max_jobs: 2,
};
test("default boundary wins without price, pause, history or route; expiry requires fresh feed", () => {
  assert.equal(
    selectAction(loan, policy, { ...config, pricesPaused: true }, undefined, {
      slot: 999n,
      timestamp: 300n,
    }),
    "default",
  );
  assert.equal(
    selectAction(loan, policy, config, undefined, {
      slot: 100n,
      timestamp: 200n,
    }),
    "wait-reference",
  );
  assert.equal(
    selectAction(
      loan,
      policy,
      config,
      { ...ref, sourceTs: 200n },
      { slot: 100n, timestamp: 200n },
    ),
    "force",
  );
  assert.equal(
    selectAction(loan, policy, config, ref, { slot: 201n, timestamp: 100n }),
    "wait-reference",
  );
});
test("CR actions bind both bad bids, strict sequence, source delays and reporter epoch", () => {
  assert.equal(selectAction(loan, policy, config, ref, clock), "wait");
  const bad = { ...ref, liveBidLower: 200n, historyBidLowerMax: 200n };
  assert.equal(selectAction(loan, policy, config, bad, clock), "arm");
  const armed = {
    ...loan,
    crArmSequence: 4n,
    crArmTimestamp: 95n,
    crArmSlot: 95n,
    crArmReporterEpoch: 1n,
  };
  assert.equal(selectAction(armed, policy, config, bad, clock), "force");
  assert.equal(
    selectAction(armed, policy, config, { ...bad, sourceTs: 99n }, clock),
    "wait",
  );
  assert.equal(
    selectAction(armed, policy, config, { ...bad, sourceSlot: 99n }, clock),
    "wait",
  );
  assert.equal(
    selectAction(
      armed,
      policy,
      { ...config, reporterEpoch: 2n },
      { ...bad, reporterEpoch: 2n },
      clock,
    ),
    "arm",
  );
  assert.equal(selectAction(armed, policy, config, ref, clock), "clear-arm");
  assert.equal(
    keeperCap(loan, policy, { ...ref, acquisitionReference: 1000n }),
    190n,
  );
});
test("service model drift and fee/funding caps fail before signing", () => {
  validateServicePolicy(settings, "ab".repeat(32), policy);
  assert.throws(
    () =>
      validateServicePolicy(
        { ...settings, epoch_seconds: 61 },
        "ab".repeat(32),
        policy,
      ),
    /drift/,
  );
  assert.throws(
    () => validateServicePolicy(settings, "cd".repeat(32), policy),
    /drift/,
  );
  assert.equal(
    fundingRequirement({ feeLamports: 5n }, 10n, 100n, {
      maxFee: 10n,
      remainingFees: 20n,
      minBalance: 20n,
    }),
    15n,
  );
  for (const [fee, peak, balance] of [
    [11n, 0n, 100n],
    [5n, 10n, 34n],
    [21n, 0n, 100n],
  ])
    assert.throws(
      () =>
        fundingRequirement({ feeLamports: fee }, peak!, balance!, {
          maxFee: 10n,
          remainingFees: 20n,
          minBalance: 20n,
        }),
      /funding\/fee/,
    );
});
test("confirmed publication continues to a priced action; dry-run never assumes publication", async () => {
  const expired = { ...loan, deadline: 100n };
  const fresh = { ...ref, sequence: 6n };
  let reads = 0;
  const actual = await referenceAfterPublication(
    true,
    undefined,
    6n,
    async () => {
      reads++;
      return fresh;
    },
  );
  assert.equal(reads, 1);
  assert.equal(selectAction(expired, policy, config, actual, clock), "force");
  assert.equal(
    await referenceAfterPublication(false, undefined, 6n, async () => {
      throw new Error("must not reread");
    }),
    undefined,
  );
  await assert.rejects(
    referenceAfterPublication(true, ref, 6n, async () => ref),
    /behind/,
  );
});
function transaction(failed = false) {
  const payload = g
    .getMarginAddedEventEncoder()
    .encode({ loan: PROGRAM, borrower: WSOL, amount: 1n, collateral: 201n });
  const bytes = Uint8Array.from([
    228,
    69,
    165,
    46,
    81,
    203,
    154,
    29,
    ...payload,
  ]);
  return {
    transaction: { message: { accountKeys: [PROGRAM] } },
    meta: {
      err: failed ? { InstructionError: [0, "failure"] } : null,
      innerInstructions: [
        {
          instructions: [
            { programIdIndex: 0, data: getBase58Decoder().decode(bytes) },
          ],
        },
      ],
    },
  };
}
test("finalized event pagination is resumable and idempotent; failed tx is never projected", async () => {
  const store = new OperatorStore(":memory:");
  try {
    let phase = 0;
    const rpc: RpcCall = async (method, params) => {
      if (method === "getSignaturesForAddress") {
        const page =
          phase++ < 3
            ? [{ signature: ["s3", "s2", "s1"][phase - 1]!, slot: 100 }]
            : [];
        return page;
      }
      if (method === "getTransaction") return transaction(params?.[0] === "s2");
      throw new Error(method);
    };
    for (let i = 0; i < 3; i++)
      assert.equal((await indexEvents(store, rpc, 1)).caughtUp, false);
    assert.equal((await indexEvents(store, rpc, 1)).caughtUp, true);
    assert.equal(store.get("event_head"), "s3");
    assert.equal(
      (
        store.db.prepare("SELECT count(*) AS n FROM events").get() as {
          n: number;
        }
      ).n,
      2,
    );
    assert.equal(successfulEvents(transaction(true)).length, 0);
    const repeat: RpcCall = async (method) =>
      method === "getSignaturesForAddress"
        ? [{ signature: "s3", slot: 100 }]
        : Promise.reject(new Error("no reread"));
    assert.equal((await indexEvents(store, repeat, 2)).caughtUp, true);
    assert.equal(
      (
        store.db.prepare("SELECT count(*) AS n FROM events").get() as {
          n: number;
        }
      ).n,
      2,
    );
  } finally {
    store.close();
  }
});
test("empty authoritative loan snapshot reconciles capacity and processes queued warm work", async () => {
  const dir = await mkdtemp(join(tmpdir(), "shortwin-operator-"));
  try {
    const settingsPath = join(dir, "settings.json");
    await writeFile(settingsPath, JSON.stringify(settings));
    mockHistory(join(dir,"prices.sqlite"));
    const calls: { command: readonly string[]; input: unknown }[] = [];
    const snapshot = {
      slot: 100n,
      timestamp: 100n,
      config: { address: PROGRAM, kind: 0, data: config },
      loans: [],
      policies: new Map(),
      markets: new Map(),
      pools: new Map(),
      references: new Map(),
      rows: [],
    } as Snapshot;
    assert.deepEqual(activeJobs(snapshot), []);
    const result = await runOperator(
      {
        rpcUrl: "https://invalid.test",
        database: join(dir, "operator.sqlite"),
        serviceDatabase: join(dir, "prices.sqlite"),
        serviceSettings: settingsPath,
        evidenceDirectory: join(dir, "evidence"),
      },
      {
        snapshot: async () => snapshot,
        rpc: async (method) => {
          if (method === "getSignaturesForAddress") return [];
          throw new Error("No live RPC " + method);
        },
        service: async (command, input) => {
          calls.push({ command, input });
          if (command[0] === "model-hash") return "ab".repeat(32);
          if (command[0] === "queue-state") return JSON.stringify({schema:1,revision:"0",jobs:[],history_instance_id:HISTORY,operator_binding:null});
          if (command[0] === "bind-operator") return JSON.stringify({schema:1,history_instance_id:HISTORY,operator_binding:input});
          if (command[0] === "jobs") return "[]";
          return "{}";
        },
      },
    );
    assert.equal(result.loans, 0);
    assert.deepEqual(
      (calls.find((c) => c.command[0] === "reconcile-queue")?.input as any)?.active_jobs,
      [],
    );
    assert.ok(
      calls.some((c) => c.command[0] === "jobs"),
      "no-loan pass must still enumerate prewarm jobs",
    );
    assert.deepEqual(result.alerts, []);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("post-tick confirmed clock accepts newer sources than initial finalized snapshot and rejects actual future", () => {
  const sourceSlot = 210n,
    sourceTs = 1000n;
  const raw = {
    market: PROGRAM,
    policy: TOKEN,
    model_hash: "ab".repeat(32),
    status: "Finite",
    args: {
      quantity: "10",
      sequence: "1",
      source_slot: sourceSlot.toString(),
      source_ts: Number(sourceTs),
      finite: true,
      live_bid_lower: "100",
      live_ask_upper: "100",
      history_ask_upper_max: "100",
      history_bid_lower_max: "100",
      acquisition_reference: "100",
      completed_epochs: 2,
      history_start_ts: 840,
      history_end_ts: 959,
      evidence_hash: "01".repeat(32),
    },
  };
  const job = {
    market: PROGRAM,
    policy: TOKEN,
    mint: WSOL,
    quantity: "10",
    active_loan: false,
  };
  assert.throws(
    () =>
      candidateReference(raw, job, policy, config, undefined, {
        slot: 200n,
        timestamp: 990n,
      }),
    /freshness/,
  );
  assert.equal(
    candidateReference(raw, job, policy, config, undefined, {
      slot: 211n,
      timestamp: 1001n,
    }).sourceSlot,
    210n,
  );
  assert.throws(
    () =>
      candidateReference(raw, job, policy, config, undefined, {
        slot: 209n,
        timestamp: 1001n,
      }),
    /freshness/,
  );
});

test("an inactive warmed quantity publishes before the first loan without reading a keyfile", async () => {
  const dir = await mkdtemp(join(tmpdir(), "shortwin-warm-"));
  try {
    const path = join(dir, "settings.json");
    await writeFile(path, JSON.stringify(settings));
    const now = BigInt(Math.floor(Date.now() / 1000)),
      epoch = (now / 60n) * 60n;
    const cfg = { ...config, reporter: WSOL };
    const job = {
      market: PROGRAM,
      policy: TOKEN,
      mint: PROGRAM,
      quantity: "10",
      active_loan: false,
      publication_requested_at: Number(now),
      generation: 0,
    };
    const snapshot = {
      slot: 200n,
      timestamp: now - 1n,
      config: { address: PROGRAM, kind: 0, data: cfg },
      loans: [],
      policies: new Map([[TOKEN, policy]]),
      markets: new Map([
        [PROGRAM, { mint: PROGRAM, tokenProgram: TOKEN } as g.Market],
      ]),
      pools: new Map(),
      references: new Map(),
      rows: [],
    } as Snapshot;
    mockHistory(join(dir,"prices.sqlite"));
    let publications = 0;
    const result = await runOperator(
      {
        rpcUrl: "https://invalid.test",
        database: join(dir, "operator.sqlite"),
        serviceDatabase: join(dir, "prices.sqlite"),
        serviceSettings: path,
        evidenceDirectory: join(dir, "evidence"),
        reporterKeyfile: "/never-read-in-dry-run",
      },
      {
        snapshot: async () => snapshot,
        rpc: async (method) => {
          if (method === "getSignaturesForAddress") return [];
          if (method === "getSlot") return 210;
          if (method === "getBlockTime") return Number(now);
          if (method === "getMinimumBalanceForRentExemption") return 1000000;
          if (method === "getBalance") return { value: 100000000 };
          throw new Error("No live RPC " + method);
        },
        service: async (command, input) => {
          if (command[0] === "model-hash") return "ab".repeat(32);
          if (command[0] === "queue-state") return JSON.stringify({schema:1,revision:"0",jobs:[job],history_instance_id:HISTORY,operator_binding:null});
          if (command[0] === "bind-operator") return JSON.stringify({schema:1,history_instance_id:HISTORY,operator_binding:input});
          if (command[0] === "jobs") return JSON.stringify([job]);
          if (command[0] === "reference")
            return JSON.stringify({
              market: PROGRAM,
              policy: TOKEN,
              model_hash: "ab".repeat(32),
              generation: 0,
              status: "Finite",
              args: {
                quantity: "10",
                sequence: "1",
                source_slot: "210",
                source_ts: Number(now),
                finite: true,
                live_bid_lower: "100",
                live_ask_upper: "100",
                history_ask_upper_max: "100",
                history_bid_lower_max: "100",
                acquisition_reference: "100",
                completed_epochs: 2,
                history_start_ts: Number(epoch - 120n),
                history_end_ts: Number(epoch - 1n),
                evidence_hash: "01".repeat(32),
              },
            });
          return "{}";
        },
        prepare: async (_url, _payer, instructions) => {
          assert.equal(
            g.identifyShortwinSimpleInstruction(instructions[0]!.data!),
            g.ShortwinSimpleInstruction.PublishReference,
          );
          publications++;
          return {
            bytes: new Uint8Array(),
            base64: "",
            lastValidBlockHeight: 1n,
            units: 100n,
            feeLamports: 5n,
            logs: [],
          };
        },
      },
    );
    assert.equal(publications, 1);
    assert.deepEqual(result.alerts, []);
    assert.ok(
      result.plans.some(
        (p: { kind: string }) => p.kind === "publish-reference",
      ),
    );
    assert.equal(result.loans, 0);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("bounded rotation advances beyond a persistently failing first identity", () => {
  const jobs = ["c", "a", "b"];
  let last: string | undefined;
  const seen: string[] = [];
  for (let pass = 0; pass < 6; pass++) {
    const selected = roundRobin(jobs, last, (v) => v)[0]!;
    seen.push(selected);
    last = selected;
  }
  assert.deepEqual(seen, ["a", "b", "c", "a", "b", "c"]);
  assert.deepEqual(
    roundRobin(["a", "c"], "b", (v) => v),
    ["a", "c"],
  );
});

test("healthy private history stays off chain; requests, adverse prices and maturity publish on demand", () => {
  const good = ref;
  const bad = {...ref, liveBidLower:200n, historyBidLowerMax:200n};
  assert.equal(publicationNeeded(good,[loan],policy,config,clock,0n,0n),false);
  assert.equal(publicationNeeded(bad,[loan],policy,config,clock,0n,0n),true);
  assert.equal(publicationNeeded(good,[{...loan,crArmSequence:4n}],policy,config,clock,0n,0n),true);
  assert.equal(publicationNeeded(good,[{...loan,deadline:100n}],policy,config,clock,0n,0n),true);
  assert.equal(publicationNeeded(good,[],policy,config,clock,100n,0n),true);
  assert.equal(publicationNeeded(good,[],policy,config,clock,100n,100n),false);
  assert.equal(publicationNeeded(good,[],policy,config,{slot:1000n,timestamp:1000n},100n,0n),false);
  assert.equal(publicationNeeded({...good,finite:false},[loan],policy,config,clock,100n,0n),false);
  assert.equal(publicationNeeded(good,[{...loan,defaultAt:100n}],policy,config,clock,0n,0n),false);
});

test("reference allocation reservations survive unknown outcomes, restarts and duplicate submissions", async () => {
  const dir=await mkdtemp(join(tmpdir(),"shortwin-allocations-"));
  const file=join(dir,"operator.sqlite");
  try {
    let store=new OperatorStore(file);
    store.reserveReference(PROGRAM,1);
    store.close();
    store=new OperatorStore(file);
    store.reserveReference(PROGRAM,1);
    assert.throws(()=>store.reserveReference(TOKEN,1),/allowance exhausted/);
    assert.equal(store.get("reference-allocation:"+TOKEN),undefined);
    store.close();
  } finally {await rm(dir,{recursive:true,force:true});}
});

test("UTC-day publication fee reservations survive failures and never consume default action budget", () => {
 const store=new OperatorStore(":memory:");
 try {
  store.reservePublicationFees(12,5n,10n);store.reservePublicationFees(12,5n,10n);
  assert.throws(()=>store.reservePublicationFees(12,1n,10n),/fee allowance/);
  assert.equal(store.publicationFees(12),10n);
  store.reservePublicationFees(13,1n,10n);assert.equal(store.publicationFees(13),1n);
  assert.throws(()=>store.reservePublicationFees(13,-1n,10n),/fee allowance/);
  store.checkReferenceAllowance(PROGRAM,1,new Set());
  assert.throws(()=>store.checkReferenceAllowance(TOKEN,1,new Set([PROGRAM])),/allowance/);
  assert.equal(store.get("reference-allocation:"+PROGRAM),undefined);
 } finally {store.close();}
});

test("fresh local pairing permits deterministic default when price settings/service are unavailable", async()=>{
 const dir=await mkdtemp(join(tmpdir(),"shortwin-default-unavailable-"));
 try{
  const now=Math.floor(Date.now()/1000);
  const s={slot:400n,timestamp:BigInt(now),config:{address:PROGRAM,kind:0,data:{...config,reporter:PROGRAM,pricesPaused:true}},loans:[{address:TOKEN,kind:3,data:loan}],policies:new Map([[TOKEN,policy]]),markets:new Map([[PROGRAM,{mint:PROGRAM,tokenProgram:TOKEN} as g.Market]]),pools:new Map(),references:new Map(),rows:[]} as Snapshot;
  let defaults=0;
  const result=await runOperator({rpcUrl:"https://unused.invalid",database:join(dir,"operator.sqlite"),serviceDatabase:join(dir,"history.sqlite"),serviceSettings:join(dir,"missing-settings.json"),evidenceDirectory:join(dir,"evidence")},{snapshot:async()=>s,service:async()=>{throw new Error("price unavailable");},rpc:async method=>{if(method==="getSignaturesForAddress")return [];if(method==="getSlot")return 400;if(method==="getBlockTime")return now;if(method==="getBalance")return {value:1000000};throw new Error(method);},prepare:async(_url,_payer,instructions)=>{assert.equal(g.identifyShortwinSimpleInstruction(instructions[0]!.data!),g.ShortwinSimpleInstruction.DefaultLoan);defaults++;return {bytes:new Uint8Array(),base64:"",lastValidBlockHeight:1n,units:100n,feeLamports:5n,logs:[]};}});
  assert.equal(defaults,1);assert.ok(result.plans.some((v:{kind:string})=>v.kind==="default"));assert.ok(result.alerts.some((v:string)=>v.includes("price unavailable")));
 }finally{await rm(dir,{recursive:true,force:true});}
});
