import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,rm,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {DatabaseSync} from 'node:sqlite';
import {spawn,execFile} from 'node:child_process';
import {once} from 'node:events';
import {promisify} from 'node:util';
import {OperatorStore,runOperator,pythonCommand,type Snapshot,type Job,type RpcCall} from '../src/operator.ts';
import {parseQueueState,queueReconciliation,publicationBarriers,acquirePublisherLock,fulfillmentKey} from '../src/monitoring.ts';
import {PROGRAM,TOKEN,referenceAddress} from '../src/pdas.ts';
import * as g from '../src/generated/index.ts';
const H='7d17ec71-b15b-4a08-bd8c-bdd2f81b3baf';
const historyFields={history_instance_id:H,operator_binding:null};
const job:Job={market:PROGRAM,policy:TOKEN,mint:PROGRAM,quantity:'10',active_loan:false};
const row=(generation=0)=>({...job,generation,publication_requested_at:1});
const state=()=>parseQueueState({...historyFields,schema:1,revision:'2',jobs:[row()]});
function snapshot(slot=100n):Snapshot{return {slot,timestamp:BigInt(Math.floor(Date.now()/1000)),config:{address:PROGRAM,kind:0,data:{reporter:PROGRAM,reporterEpoch:1n,currentPolicy:TOKEN,pricesPaused:false} as g.Config},loans:[],policies:new Map([[TOKEN,{terms:{historyEpochSeconds:60,minimumHistoryEpochs:2,referenceMaxAgeSeconds:30,referenceMaxAgeSlots:100n,maxSpreadBps:1000}} as g.Policy]]),markets:new Map([[PROGRAM,{mint:PROGRAM,tokenProgram:TOKEN} as g.Market]]),pools:new Map(),references:new Map(),rows:[]};}
function attempt(store:OperatorStore,reference:string){store.db.prepare('INSERT INTO attempts VALUES(?,?,?,?,?)').run('p1',reference,'confirmed','synthetic-signature',JSON.stringify({kind:'publish-reference'}));}
test('queue state parsing refuses duplicate, ambiguous and unsafe generation data',()=>{
 assert.equal(state().jobs[0]!.generation,0n);
 for(const jobs of [[row(),row()],[{...row(),active_loan:'false'}],[{...row(),generation:Number.MAX_SAFE_INTEGER+1}]])assert.throws(()=>parseQueueState({...historyFields,schema:1,revision:'2',jobs}));
});
test('confirmed or post-snapshot publication holds capacity until finalized state includes it',async()=>{
 const store=new OperatorStore(':memory:'),s=snapshot(),key=await referenceAddress(job.market,job.policy,10n);attempt(store,key);
 try{
  let value:any={confirmationStatus:'confirmed',slot:110,err:null};const rpc:RpcCall=async()=>({value:[value]});
  assert.equal((await publicationBarriers(store,rpc,s,state())).length,1);
  value={confirmationStatus:'finalized',slot:110,err:null};assert.equal((await publicationBarriers(store,rpc,s,state())).length,1);
  s.slot=120n;s.references.set(key,{market:job.market,policy:job.policy,quantity:10n} as g.PriceReference);
  assert.deepEqual(await publicationBarriers(store,rpc,s,state()),[]);
  assert.equal(store.db.prepare('SELECT status FROM attempts').get()!.status,'finalized');
 }finally{store.close();}
});
test('missing/malformed finalized error fields never become failed-publication proof',async()=>{
 for(const err of [undefined,false,0,[]]){
  const store=new OperatorStore(':memory:'),key=await referenceAddress(job.market,job.policy,10n);attempt(store,key);
  try{const value:any={confirmationStatus:'finalized',slot:90};if(err!==undefined)value.err=err;await assert.rejects(publicationBarriers(store,async()=>({value:[value]}),snapshot(),state()),/Malformed/);assert.equal(store.get('attempt-finalized-result:p1'),undefined);}
  finally{store.close();}
 }
});
test('finalized success absent from complete snapshot blocks retirement; exact failed proof can release',async()=>{
 const store=new OperatorStore(':memory:'),key=await referenceAddress(job.market,job.policy,10n);attempt(store,key);
 try{
  await assert.rejects(publicationBarriers(store,async()=>({value:[{confirmationStatus:'finalized',slot:90,err:null}]}),snapshot(),state()),/absent/);
  store.db.prepare('DELETE FROM meta WHERE key=?').run('attempt-finalized-result:p1');
  const error={InstructionError:[0,{Custom:1}]};assert.deepEqual(await publicationBarriers(store,async()=>({value:[{confirmationStatus:'finalized',slot:90,err:error}]}),snapshot(),state()),[]);
  assert.equal(store.db.prepare('SELECT status FROM attempts').get()!.status,'finalized-failed');
 }finally{store.close();}
});
test('lifecycle envelope binds complete bank clock and generation-specific fulfillment',async()=>{
 const store=new OperatorStore(':memory:'),s=snapshot(),st=parseQueueState({...historyFields,schema:1,revision:'7',jobs:[row(1)]});
 try{
  store.set(fulfillmentKey(job,0n),'99');let result=await queueReconciliation(s,st,store,async()=>{throw new Error('unexpected');});
  assert.equal(result.clock_slot,result.context_slot);assert.equal(result.clock_ts,s.timestamp.toString());assert.deepEqual(result.fulfilled_requests,[]);
  store.set(fulfillmentKey(job,1n),'100');result=await queueReconciliation(s,st,store,async()=>{throw new Error('unexpected');});assert.equal(result.fulfilled_requests[0]!.generation,'1');
 }finally{store.close();}
});
test('publisher OS lock excludes another process and releases after holder exit',{timeout:15000},async()=>{
 const dir=await mkdtemp(join(tmpdir(),'shortwin-publisher-')),db=join(dir,'history.sqlite');
 const module=new URL('../src/monitoring.ts',import.meta.url).href;
 const child=spawn(process.execPath,['--input-type=module','-e',`const {acquirePublisherLock}=await import(${JSON.stringify(module)});await acquirePublisherLock(${JSON.stringify(db)});console.log('LOCKED');setInterval(()=>{},1000);`],{stdio:['ignore','pipe','pipe']});
 try{
  const [data]=await Promise.race([once(child.stdout,'data'),once(child,'exit').then(()=>{throw new Error('lock holder exited early');})]);assert.match(String(data),/LOCKED/);
  await assert.rejects(acquirePublisherLock(db),/publisher lock/);
  const exited=once(child,'exit');child.kill('SIGTERM');await exited;
  const lock=await acquirePublisherLock(db);lock.close();
 }finally{if(child.exitCode===null&&child.signalCode===null)child.kill('SIGKILL');await rm(dir,{recursive:true,force:true});}
});
test('operator database cannot silently adopt another program or an unbound existing snapshot',()=>{
 const store=new OperatorStore(':memory:');try{store.bindProgram(PROGRAM);assert.throws(()=>store.bindProgram(TOKEN),/another program/);}finally{store.close();}
});
test('real Python queue frees an expired inactive identity and admits another without API calls',{timeout:15000},async()=>{
 const dir=await mkdtemp(join(tmpdir(),'shortwin-reuse-'));try{
  const settings=join(dir,'settings.json'),db=join(dir,'history.sqlite'),operator=join(dir,'operator.sqlite');
  const settingsObject={epoch_seconds:60,history_epochs:2,max_sample_gap_seconds:30,reference_max_age_seconds:30,reference_max_age_slots:100,slippage_bps:50,max_spread_bps:1000,max_buy_lamports:10000,max_ask_requests:4,max_jobs:1,jobs_per_tick:1,request_timeout_seconds:1,request_interval_ms:1000};await writeFile(settings,JSON.stringify(settingsObject));
  const script=new URL('../../services/src/shortwin_price_service.py',import.meta.url).pathname;
  const code=`import importlib.util,sys,json,time
from pathlib import Path
spec=importlib.util.spec_from_file_location('p',sys.argv[1]);p=importlib.util.module_from_spec(spec);sys.modules['p']=p;spec.loader.exec_module(p)
h=p.History(sys.argv[3],p.Settings(**json.loads(Path(sys.argv[2]).read_text())));h.enqueue(p.Job(**json.loads(sys.argv[4])),int(time.time())-400);h.close()`;
  await promisify(execFile)('python3',['-c',code,script,settings,db,JSON.stringify(job)]);
  const service=(command:readonly string[],input?:unknown)=>pythonCommand('python3',settings,db,command,input);
  const s=snapshot(),model=(await service(['model-hash'])).trim();s.policies.get(TOKEN)!.terms.pricePolicyHash=Uint8Array.from(Buffer.from(model,'hex'));
  const rpc:RpcCall=async method=>{if(method==='getSignaturesForAddress')return [];if(method==='getSlot')return 210;if(method==='getBlockTime')return Number(s.timestamp);throw new Error(method);};
  const result=await runOperator({rpcUrl:'https://unused.invalid',database:operator,serviceDatabase:db,serviceSettings:settings,evidenceDirectory:join(dir,'evidence')},{snapshot:async()=>s,rpc,service});assert.deepEqual(result.alerts,[]);
  assert.equal(JSON.parse(await service(['queue-state'])).jobs.length,0);
  const admitted=JSON.parse(await service(['enqueue','-'],[{...job,quantity:'11'}]));assert.equal(admitted[0].accepted,true);
  assert.equal(JSON.parse(await service(['queue-state'])).jobs[0].quantity,'11');
 }finally{await rm(dir,{recursive:true,force:true});}
});

function makeHistory(path:string,id=H,binding?:unknown){const db=new DatabaseSync(path);try{db.exec('CREATE TABLE metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL)');db.prepare('INSERT INTO metadata VALUES(?,?)').run('history_instance_id',id);if(binding)db.prepare('INSERT INTO metadata VALUES(?,?)').run('operator_binding',JSON.stringify(binding));}finally{db.close();}}
test('publisher pairing preserves allowances and rejects operator switches and history replacement',async()=>{
 const dir=await mkdtemp(join(tmpdir(),'shortwin-pairing-')),history=join(dir,'history.sqlite'),operator=join(dir,'operator.sqlite');
 makeHistory(history);let store=new OperatorStore(operator),lock=await acquirePublisherLock(history);
 try{
  store.bindProgram(PROGRAM);store.reservePublicationFees(1,7n,10n);store.reserveReference(PROGRAM,1);
  const bound=await lock.bind(store,PROGRAM);assert.equal(bound.historyInstanceId,H);lock.close();store.close();
  store=new OperatorStore(operator);store.bindProgram(PROGRAM);lock=await acquirePublisherLock(history);
  assert.deepEqual(await lock.bind(store,PROGRAM),bound);assert.equal(store.publicationFees(1),7n);assert.throws(()=>store.reserveReference(TOKEN,1),/allowance/);lock.close();store.close();
  const other=new OperatorStore(join(dir,'other.sqlite'));other.bindProgram(PROGRAM);lock=await acquirePublisherLock(history);
  try{await assert.rejects(lock.bind(other,PROGRAM),/pairing changed/);}finally{other.close();lock.close();}
  await rm(history);makeHistory(history,'17d8ab59-751e-4078-bd93-9bb234102ed1');store=new OperatorStore(operator);store.bindProgram(PROGRAM);lock=await acquirePublisherLock(history);
  await assert.rejects(lock.bind(store,PROGRAM),/missing or replaced/);
 }finally{lock.close();store.close();await rm(dir,{recursive:true,force:true});}
});
test('history mirror rejects a replacement operator even if the lock anchor is lost',async()=>{
 const dir=await mkdtemp(join(tmpdir(),'shortwin-mirror-')),history=join(dir,'history.sqlite');let store=new OperatorStore(join(dir,'old.sqlite'));store.bindProgram(PROGRAM);
 const expected={schema:1,program_id:PROGRAM,operator_store_id:store.instanceId(),operator_database:store.path};makeHistory(history,H,expected);store.close();store=new OperatorStore(join(dir,'new.sqlite'));store.bindProgram(PROGRAM);
 const lock=await acquirePublisherLock(history);try{await assert.rejects(lock.bind(store,PROGRAM),/another operator/);}finally{lock.close();store.close();await rm(dir,{recursive:true,force:true});}
});
test('fresh default-only local pairing can bind its first history later without resetting store identity',async()=>{
 const dir=await mkdtemp(join(tmpdir(),'shortwin-initial-pair-')),history=join(dir,'history.sqlite'),store=new OperatorStore(join(dir,'operator.sqlite'));store.bindProgram(PROGRAM);
 const lock=await acquirePublisherLock(history);try{const first=await lock.bind(store,PROGRAM);assert.equal(first.historyInstanceId,null);makeHistory(history);const bound=await lock.bind(store,PROGRAM);assert.equal(bound.historyInstanceId,H);assert.deepEqual(bound.operatorBinding,first.operatorBinding);}finally{lock.close();store.close();await rm(dir,{recursive:true,force:true});}
});


test('competing histories cannot both claim the same operator store',{timeout:15000},async()=>{
 const dir=await mkdtemp(join(tmpdir(),'shortwin-pair-race-')),operator=join(dir,'operator.sqlite');
 const store=new OperatorStore(operator);store.bindProgram(PROGRAM);store.close();
 const module=new URL('../src/monitoring.ts',import.meta.url).href,ops=new URL('../src/operator.ts',import.meta.url).href;
 const run=async(label:string,peer:string)=>{
  // Hold each process at the old vulnerable metadata-read boundary. With the
  // write transaction, the rejected peer signals completion and releases the winner.
  const source=`const fs=await import('node:fs');const dir=${JSON.stringify(dir)},label=${JSON.stringify(label)},peer=${JSON.stringify(peer)};let store,lock;try{const {OperatorStore}=await import(${JSON.stringify(ops)});const {acquirePublisherLock}=await import(${JSON.stringify(module)});store=new OperatorStore(${JSON.stringify(operator)});const get=store.get.bind(store);store.get=key=>{const value=get(key);if(key==='publisher-binding'){fs.writeFileSync(dir+'/'+label+'.read','ready');const deadline=Date.now()+5000;while(!fs.existsSync(dir+'/'+peer+'.read')&&!fs.existsSync(dir+'/'+peer+'.done')){if(Date.now()>deadline)throw new Error('pairing test barrier timeout');Atomics.wait(new Int32Array(new SharedArrayBuffer(4)),0,0,10);}}return value;};lock=await acquirePublisherLock(dir+'/history-'+label+'.sqlite');await lock.bind(store,${JSON.stringify(PROGRAM)});console.log('BOUND');}catch(e){console.log('REJECTED '+String(e));}finally{lock?.close();store?.close();fs.writeFileSync(dir+'/'+label+'.done','done');}`;
  return (await promisify(execFile)(process.execPath,['--input-type=module','-e',source])).stdout;
 };
 try{const results=await Promise.all([run('a','b'),run('b','a')]);assert.equal(results.filter(v=>v.trim()==='BOUND').length,1);assert.equal(results.filter(v=>v.includes('REJECTED')).length,1);assert.ok(results.every(v=>!v.includes('barrier timeout')));}finally{await rm(dir,{recursive:true,force:true});}
});
