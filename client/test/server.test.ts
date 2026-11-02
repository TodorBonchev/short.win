function header(input:bigint,output:bigint,slip:number){const b=new Uint8Array(39),v=new DataView(b.buffer);b.set([187,100,250,204,49,196,175,20]);v.setBigUint64(8,input,true);v.setBigUint64(16,output,true);v.setUint16(24,slip,true);v.setUint32(30,1,true);return getBase64Decoder().decode(b);}
import {test} from 'node:test';
import assert from 'node:assert/strict';
import {address,getBase64Decoder,type AddressesByLookupTableAddress} from '@solana/kit';
import * as g from '../src/generated/index.ts';
import {PROGRAM,TOKEN,WSOL,JUPITER,configAddress,policyAddress,marketAddress,poolAddress,loanAddress,executionAddresses} from '../src/pdas.ts';
import {createBridge,type MarketState,type BridgeDependencies,type BridgeOptions} from '../src/server.ts';
const mint=address('EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v');
const origin='http://127.0.0.1:5173';
const options:BridgeOptions={rpcUrl:'https://unused.invalid',serviceSettings:'unused',serviceDatabase:'unused',frontendOrigin:origin,globalPerMinute:100,perIpPerMinute:100};
async function fixture(){
 const configKey=await configAddress(),policyKey=await policyAddress(1n),marketKey=await marketAddress(mint),poolKey=await poolAddress(marketKey,0n);
 const state={configKey,policyKey,marketKey,poolKey,mint,tokenProgram:TOKEN,clock:{slot:720n,timestamp:720n},
  config:{opensPaused:false,pricesPaused:false,reporterEpoch:1n},market:{nextLoanNonce:0n},pool:{cash:1000n},
  policy:{terms:{pricePolicyHash:new Uint8Array(32).fill(42),historyEpochSeconds:60,minimumHistoryEpochs:2,
   referenceMaxAgeSeconds:30,referenceMaxAgeSlots:100n,maxSpreadBps:1000}}} as unknown as MarketState;
 const reference={discriminator:new Uint8Array(8),version:1,bump:0,market:marketKey,policy:policyKey,quantity:100n,reporterEpoch:1n,sequence:1n,finite:true,
  sourceSlot:720n,sourceTs:720n,liveBidLower:100n,liveAskUpper:105n,historyBidLowerMax:100n,historyAskUpperMax:105n,
  acquisitionReference:105n,completedEpochs:2,historyStartTs:600n,historyEndTs:719n,evidenceHash:new Uint8Array(32).fill(1)} as g.PriceReference;
 const loanKey=await loanAddress(poolKey,PROGRAM,0n),exec=await executionAddresses(loanKey);
 const loan={market:marketKey,pool:poolKey,borrower:PROGRAM,nonce:0n,q:100n,collateral:500n,policy:policyKey,defaultAt:1000n} as g.Loan;
 const calls:{command:readonly string[],input:unknown}[]=[];let accepted=true;
 const deps:BridgeDependencies={settings:{epoch_seconds:60,history_epochs:2,reference_max_age_seconds:30,reference_max_age_slots:100,max_spread_bps:1000},
  readState:async key=>{assert.equal(key,mint);return state;},readLoan:async key=>key===loanKey?loan:null,readReference:async()=>reference,
  tables:async keys=>{assert.deepEqual(keys,[]);return {} as AddressesByLookupTableAddress;},
  service:async(command,input)=>{calls.push({command,input});
   if(command[0]==='model-hash')return '2a'.repeat(32);
   if(command[0]==='check-policy')return '{"valid":true}';
   if(command[0]==='enqueue')return JSON.stringify([{accepted}]);
   const buy=command[0]==='build-buy',amount=buy?'50':'100',minimum=buy?'100':'10';
   const build={inputMint:buy?WSOL:mint,outputMint:buy?mint:WSOL,inAmount:amount,outAmount:minimum,otherAmountThreshold:minimum,swapMode:'ExactIn',
    swapInstruction:{programId:JUPITER,data:header(BigInt(amount),BigInt(minimum),0),accounts:[
     {pubkey:exec.executionAuthority,isSigner:true,isWritable:false},{pubkey:buy?exec.executionWsol:exec.executionBase,isSigner:false,isWritable:true},
     {pubkey:buy?exec.executionBase:exec.executionWsol,isSigner:false,isWritable:true}]}};
   return JSON.stringify({status:'Ready',input_amount:amount,build,evidence_hash:'11'.repeat(32)});
  }};
 return {state,reference,loan,loanKey,exec,calls,deps,full:()=>{accepted=false;},handle:createBridge(options,deps)};
}
const request=(path:string,body:unknown)=>({method:'POST',path,body,origin,ip:'127.0.0.1'});
test('prewarm validates current policy and enqueues inactive exact-u64 identity only',async()=>{
 const f=await fixture();const result=await f.handle(request('/v1/requests',{mint,quantity:'100'}));
 assert.equal(result.status,202);assert.equal((result.body as any).status,'Warming');
 assert.equal(result.headers['access-control-allow-origin'],origin);
 const queued=f.calls.find(c=>c.command[0]==='enqueue')!.input as any[];assert.equal(queued[0].quantity,'100');assert.equal(queued[0].active_loan,false);
 assert.equal(f.calls.filter(c=>c.command[0]==='check-policy').length,1);
 await assert.doesNotReject(f.handle(request('/v1/requests',{mint,quantity:'0'})));
 assert.equal((await f.handle(request('/v1/requests',{mint,quantity:'0'}))).status,400);
 assert.equal((await f.handle(request('/v1/requests',{mint,quantity:'1001'}))).status,422);
 f.full();assert.equal((await f.handle(request('/v1/requests',{mint,quantity:'100'}))).status,429);
});
test('Unknown or stale reference never becomes Finite and cannot authorize a sale route',async()=>{
 const f=await fixture();let result=await f.handle({method:'GET',path:'/v1/reference?mint='+mint+'&quantity=100',ip:'local',origin});
 assert.equal((result.body as any).status,'Finite');
 f.reference.finite=false;result=await f.handle({method:'GET',path:'/v1/reference?mint='+mint+'&quantity=100',ip:'local',origin});
 assert.equal((result.body as any).status,'Unknown');
 // Only the three explicit scratch binding fields belong to the HTTP schema.
 const {executionAuthority,executionBase,executionWsol}=f.exec;
 const sale={mint,tokenProgram:TOKEN,side:'sell',quantity:'100',inputAmount:'100',minimumOutput:'10',loan:f.loanKey,borrower:PROGRAM,nonce:'0',executionAuthority,executionBase,executionWsol};
 assert.equal((await f.handle(request('/v1/route',sale))).status,409);assert.equal(f.calls.length,0);
 f.reference.finite=true;f.reference.sourceTs=689n;
 assert.equal(((await f.handle({method:'GET',path:'/v1/reference?mint='+mint+'&quantity=100',ip:'local',origin})).body as any).status,'Unknown');
});
test('route binds new loan nonce/scratch and returns fresh raw build plus verified table contents',async()=>{
 const f=await fixture(),{executionAuthority,executionBase,executionWsol}=f.exec;
 const body={mint,tokenProgram:TOKEN,side:'sell',quantity:'100',inputAmount:'100',minimumOutput:'10',loan:f.loanKey,borrower:PROGRAM,nonce:'0',executionAuthority,executionBase,executionWsol};
 let result=await f.handle(request('/v1/route',{...body,executionBase:PROGRAM}));assert.equal(result.status,422);assert.equal(f.calls.length,0);
 result=await f.handle(request('/v1/route',{...body,nonce:'1'}));assert.equal(result.status,409);
 result=await f.handle(request('/v1/route',body));assert.equal(result.status,200);assert.equal((result.body as any).build.inAmount,'100');
 assert.equal(f.calls.at(-1)!.command[0],'build-sell');
 result=await f.handle(request('/v1/route',{...body,side:'buy',inputAmount:'200',minimumOutput:'100'}));
 assert.equal(result.status,200);assert.equal((result.body as any).build.inAmount,'50');assert.equal(f.calls.at(-1)!.command[0],'build-buy');
 f.loan.defaultAt=720n;assert.equal((await f.handle(request('/v1/route',{...body,side:'buy',inputAmount:'200',minimumOutput:'100'}))).status,409);
});
test('CORS/body/IP limits and unsupported RPC overrides reject before provider calls',async()=>{
 const f=await fixture();assert.equal((await f.handle({...request('/v1/requests',{mint,quantity:'100'}),origin:'https://other.invalid'})).status,403);
 assert.equal((await f.handle(request('/v1/requests',{mint,quantity:'100',rpcUrl:'https://other.invalid'}))).status,400);
 assert.equal((await f.handle(request('/v1/requests',{padding:'a'.repeat(17000)}))).status,413);assert.equal(f.calls.length,0);
 const limited=createBridge({...options,perIpPerMinute:1},f.deps);assert.equal((await limited(request('/v1/requests',{mint,quantity:'100'}))).status,202);
 assert.equal((await limited(request('/v1/requests',{mint,quantity:'100'}))).status,429);
});
