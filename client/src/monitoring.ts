/** Node-only finalized queue lifecycle. No signing or public retirement API. */
import { DatabaseSync } from "node:sqlite";
import { mkdir, realpath, stat } from "node:fs/promises";
import { dirname, resolve, basename } from "node:path";
import { address, type Address } from "@solana/kit";
import { referenceAddress } from "./pdas.ts";
import type { Snapshot, Job, RpcCall, OperatorStore } from "./operator.ts";
const U64=0xffffffffffffffffn;
const validTransactionError=(err:unknown)=>err===null||(typeof err==='string'&&err.length>0)||(typeof err==='object'&&err!==null&&!Array.isArray(err)&&Object.keys(err).length>0);
function number(value:unknown,name:string):bigint {
 if(typeof value==='number'&&!Number.isSafeInteger(value))throw new Error('Unsafe '+name);
 if(typeof value!=='string'&&typeof value!=='number'&&typeof value!=='bigint')throw new Error('Invalid '+name);
 if(!/^(0|[1-9][0-9]*)$/.test(String(value)))throw new Error('Invalid '+name);
 const n=BigInt(value);if(n<0n||n>U64)throw new Error('Invalid '+name);return n;
}
export type QueuedJob={job:Job,generation:bigint,requestedAt:bigint};
export type OperatorBinding={schema:1,program_id:Address,operator_store_id:string,operator_database:string};
export type QueueState={revision:string,jobs:QueuedJob[],historyInstanceId:string,operatorBinding:OperatorBinding|null};
const UUID=/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
function uuid(value:unknown):string {
 if(typeof value!=="string"||!UUID.test(value)||value==="00000000-0000-0000-0000-000000000000")throw new Error("Invalid persistent database UUID");
 return value;
}
export function parseOperatorBinding(value:any):OperatorBinding {
 if(!value||value.schema!==1||Object.keys(value).sort().join(',')!=="operator_database,operator_store_id,program_id,schema"||typeof value.operator_database!=="string"||!value.operator_database.startsWith('/')||resolve(value.operator_database)!==value.operator_database)throw new Error("Malformed persistent operator binding");
 return {schema:1,program_id:address(value.program_id),operator_store_id:uuid(value.operator_store_id),operator_database:value.operator_database};
}
const equal=(a:unknown,b:unknown)=>JSON.stringify(a)===JSON.stringify(b);
async function canonicalPath(path:string):Promise<string> {
 const resolved=resolve(path);await mkdir(dirname(resolved),{recursive:true});
 try{return await realpath(resolved);}catch(error){if((error as NodeJS.ErrnoException).code!=="ENOENT")throw error;return resolve(await realpath(dirname(resolved)),basename(resolved));}
}
type PublisherAnchor={schema:1,history_database:string,history_instance_id:string|null,operator_binding:OperatorBinding};
function anchor(value:string):PublisherAnchor {
 const a=JSON.parse(value);
 if(!a||a.schema!==1||Object.keys(a).sort().join(',')!=="history_database,history_instance_id,operator_binding,schema"||typeof a.history_database!=="string")throw new Error("Malformed persistent publisher anchor");
 return {schema:1,history_database:a.history_database,history_instance_id:a.history_instance_id===null?null:uuid(a.history_instance_id),operator_binding:parseOperatorBinding(a.operator_binding)};
}
async function historyIdentity(path:string):Promise<{id:string,binding:OperatorBinding|null}|undefined> {
 try{await stat(path);}catch(error){if((error as NodeJS.ErrnoException).code==='ENOENT')return undefined;throw error;}
 const db=new DatabaseSync(path,{readOnly:true});
 try{
  const get=(key:string)=>(db.prepare('SELECT value FROM metadata WHERE key=?').get(key) as {value:string}|undefined)?.value;
  const id=uuid(get('history_instance_id')), binding=get('operator_binding');
  return {id,binding:binding===undefined?null:parseOperatorBinding(JSON.parse(binding))};
 }finally{db.close();}
}
export const identity=(job:Job)=>[job.market,job.policy,job.quantity].join(':');
export const fulfillmentKey=(job:Job,generation:bigint)=>'fulfilled-request:'+identity(job)+':generation:'+generation;
export function parseQueueState(raw:unknown):QueueState {
 const a=raw as any;
 if(!a||a.schema!==1||!Array.isArray(a.jobs)||a.jobs.length>4096)throw new Error('Invalid complete queue state');
 const seen=new Set<string>();
 return {historyInstanceId:uuid(a.history_instance_id),operatorBinding:a.operator_binding===null?null:parseOperatorBinding(a.operator_binding),revision:number(a.revision,'queue revision').toString(),jobs:a.jobs.map((item:any)=>{
  if(typeof item.active_loan!=='boolean')throw new Error('Invalid debt priority');
  const job:Job={market:address(item.market),policy:address(item.policy),mint:address(item.mint),quantity:number(item.quantity,'queued quantity').toString(),active_loan:item.active_loan};
  if(job.quantity==='0'||seen.has(identity(job)))throw new Error('Duplicate/zero queue identity');seen.add(identity(job));
  return {job,generation:number(item.generation,'generation'),requestedAt:number(item.publication_requested_at,'publication request')};
 })};
}
/** SQLite's OS lock releases on process exit; PID markers or TTLs cannot steal it.
 * Every publisher using a history DB must use this lock and its persistent store. */
export async function acquirePublisherLock(serviceDatabase:string):Promise<{
 close:()=>void;
 bind:(store:OperatorStore,program:Address)=>Promise<{historyInstanceId:string|null,operatorBinding:OperatorBinding}>;
}> {
 const canonical=await canonicalPath(serviceDatabase);
 const db=new DatabaseSync(canonical+'.publisher-lock.sqlite');
 try{db.exec('PRAGMA busy_timeout=0; CREATE TABLE IF NOT EXISTS publisher_lock(id INTEGER PRIMARY KEY); BEGIN EXCLUSIVE;');}
 catch{db.close();throw new Error('Another operator holds the exclusive publisher lock');}
 let identities:DatabaseSync;
 try{identities=new DatabaseSync(canonical+'.publisher-identity.sqlite');identities.exec('PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS identity(key TEXT PRIMARY KEY,value TEXT NOT NULL);');}
 catch(error){db.exec('ROLLBACK');db.close();throw error;}
 let closed=false;
 return {
  close:()=>{if(!closed){closed=true;try{identities.close();}finally{try{db.exec('ROLLBACK');}finally{db.close();}}}},
  bind:async(store,program)=>{
   if(closed)throw new Error('Publisher lock is not held');
   const operatorPath=await canonicalPath(store.path), history=await historyIdentity(canonical);
   // Different history paths have different OS locks, so the shared operator
   // store's pairing must be reread and claimed inside its own write transaction.
   store.db.exec('BEGIN IMMEDIATE');
   try{
    if(store.get('program-id')!==program)throw new Error('Operator program identity changed');
    const expected:OperatorBinding={schema:1,program_id:program,operator_store_id:store.instanceId(),operator_database:operatorPath};
    const raw=(identities.prepare("SELECT value FROM identity WHERE key='publisher'").get() as {value:string}|undefined)?.value;
    const old=raw===undefined?undefined:anchor(raw),mirror=store.get('publisher-binding');
    const local=mirror===undefined?undefined:anchor(mirror);
    for(const prior of [old,local])if(prior&&(prior.history_database!==canonical||!equal(prior.operator_binding,expected)))throw new Error('Publisher history/operator database pairing changed; reviewed recovery required');
    if(history?.binding&&!equal(history.binding,expected))throw new Error('History is bound to another operator database');
    const known=[old?.history_instance_id,local?.history_instance_id].filter((v):v is string=>v!==null&&v!==undefined);
    if(known.some(id=>id!==history?.id))throw new Error('History database missing or replaced; reviewed recovery required');
    const next:PublisherAnchor={schema:1,history_database:canonical,history_instance_id:history?.id??null,operator_binding:expected};
    const encoded=JSON.stringify(next);
    // The companion persists independently while the publisher OS lock stays held.
    if(raw!==encoded)identities.prepare("INSERT INTO identity VALUES('publisher',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value").run(encoded);
    if(mirror!==encoded)store.set('publisher-binding',encoded);
    store.db.exec('COMMIT');
    return {historyInstanceId:next.history_instance_id,operatorBinding:expected};
   }catch(error){store.db.exec('ROLLBACK');throw error;}
  },
 };
}
/** Unknown, confirmed-but-unfinalized and post-snapshot publications protect slots.
 * Successful finalized publications must actually exist in the complete snapshot. */
export async function publicationBarriers(store:OperatorStore,rpc:RpcCall,snapshot:Snapshot,state:QueueState):Promise<Job[]> {
 const byReference=new Map<Address,Job>();
 for(const {job}of state.jobs)byReference.set(await referenceAddress(job.market,job.policy,BigInt(job.quantity)),job);
 const blocked=new Map<string,Job>();
 const rows=store.db.prepare('SELECT id,loan,status,signature,body FROM attempts').all() as {id:string,loan:Address,status:string,signature:string|null,body:string}[];
 for(const row of rows){
  const body=JSON.parse(row.body);if(!body||typeof body.kind!=='string')throw new Error('Unidentifiable attempt blocks queue retirement');if(body.kind!=='publish-reference')continue;
  const job=byReference.get(row.loan);
  // A retired identity cannot have unresolved publication authority; fail closed.
  if(!job){if(!['finalized','finalized-failed'].includes(row.status))throw new Error('Unmapped pending publication blocks queue retirement');continue;}
  let finalSlot:bigint|undefined, failed=false;
  const cached=store.get('attempt-finalized-result:'+row.id);
  if(cached!==undefined){
   const proof=JSON.parse(cached);
   if(!proof||!Object.hasOwn(proof,'err')||!validTransactionError(proof.err))throw new Error('Malformed cached finality proof');
   finalSlot=number(proof.slot,'cached final inclusion slot');if(finalSlot===0n)throw new Error('Invalid cached final inclusion slot');failed=proof.err!==null;
   if(row.status!==(failed?'finalized-failed':'finalized'))throw new Error('Cached publication finality status mismatch');
  }else if(row.signature){
   const response=await rpc('getSignatureStatuses',[[row.signature],{searchTransactionHistory:true}]);const value=response?.value?.[0];
   if(value?.confirmationStatus==='finalized'){
    if(!Object.hasOwn(value,'err')||!validTransactionError(value.err))throw new Error('Malformed finalized publication error status');
    finalSlot=number(value.slot,'finalized inclusion slot');if(finalSlot===0n)throw new Error('Invalid finalized publication slot');
    failed=value.err!==null;
    store.db.exec('BEGIN IMMEDIATE');
    try{
     store.db.prepare('UPDATE attempts SET status=? WHERE id=?').run(failed?'finalized-failed':'finalized',row.id);
     store.set('attempt-finalized-result:'+row.id,JSON.stringify({slot:finalSlot.toString(),err:value.err}));
     store.db.exec('COMMIT');
    }catch(error){store.db.exec('ROLLBACK');throw error;}
   }
  }
  if(failed)continue;
  if(finalSlot===undefined||finalSlot>snapshot.slot){blocked.set(identity(job),job);continue;}
  if(!snapshot.references.has(row.loan))throw new Error('Finalized publication absent from complete state snapshot');
 }
 return [...blocked.values()];
}
export async function queueReconciliation(snapshot:Snapshot,state:QueueState,store:OperatorStore,rpc:RpcCall){
 const pending=await publicationBarriers(store,rpc,snapshot,state);
 const active=new Map<string,Job>();
 for(const {data:l}of snapshot.loans){
  const market=snapshot.markets.get(l.market);if(!market)throw new Error('Active market missing');
  const job:Job={market:l.market,policy:l.policy,mint:market.mint,quantity:l.q.toString(),active_loan:true};active.set(identity(job),job);
 }
 for(const {job}of state.jobs){
  if(snapshot.markets.get(job.market)?.mint!==job.mint||!snapshot.policies.has(job.policy))throw new Error('Queue not covered by finalized market/policy snapshot');
 }
 const references=[];
 for(const [key,r]of snapshot.references){
  const market=snapshot.markets.get(r.market),policy=snapshot.policies.get(r.policy);
  if(!market||!policy||key!==await referenceAddress(r.market,r.policy,r.quantity)||r.quantity<=0n)throw new Error('Finalized reference binding mismatch');
  if(!r.finite)continue;
  references.push({job:{market:r.market,policy:r.policy,mint:market.mint,quantity:r.quantity.toString(),active_loan:false},source_slot:r.sourceSlot.toString(),source_ts:r.sourceTs.toString(),max_age_seconds:policy.terms.referenceMaxAgeSeconds,max_age_slots:policy.terms.referenceMaxAgeSlots.toString()});
 }
 const fulfilled_requests=state.jobs.flatMap(({job,generation})=>{
  const value=store.get(fulfillmentKey(job,generation));return value===undefined?[]:[{job,generation:generation.toString(),requested_at:number(value,'fulfilled request').toString()}];
 });
 return {schema:1,complete:true,commitment:'finalized',publisher_lock:true,context_slot:snapshot.slot.toString(),clock_slot:snapshot.slot.toString(),clock_ts:snapshot.timestamp.toString(),expected_queue_revision:state.revision,active_jobs:[...active.values()],references,pending_publications:pending,fulfilled_requests};
}
