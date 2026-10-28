function header(input:bigint,output:bigint,slip:number){const b=new Uint8Array(39),v=new DataView(b.buffer);b.set([187,100,250,204,49,196,175,20]);v.setBigUint64(8,input,true);v.setBigUint64(16,output,true);v.setUint16(24,slip,true);v.setUint32(30,1,true);return getBase64Decoder().decode(b);}
import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {address,AccountRole,getBase64Decoder,getTransactionEncoder,createTransactionMessage,setTransactionMessageFeePayerSigner,setTransactionMessageLifetimeUsingBlockhash,compileTransaction,createNoopSigner,type Blockhash} from '@solana/kit';
import {getPoolGenerationDecoder,getOpenPositionInstructionDataDecoder,getOpenPositionInstructionDataEncoder,getClosePositionInstructionDataDecoder,SHORTWIN_SIMPLE_PROGRAM_ADDRESS,associatedToken,executionAddresses,loanAddress,poolAddress,marketAddress,WSOL,TOKEN,JUPITER,planBundle,appendRoute,assertSameMessage,requestBuild,type BuildResponse} from '../src/index.ts';
const wire=JSON.parse(readFileSync(new URL('./fixtures/rust-wire.json',import.meta.url),'utf8')) as Record<string,string>;
test('generated client decodes exact Rust account and instruction bytes without precision loss',()=>{
 assert.equal(wire.program,SHORTWIN_SIMPLE_PROGRAM_ADDRESS);
 const pool=getPoolGenerationDecoder().decode(Buffer.from(wire.pool!,'hex'));
 assert.equal(pool.cash,9007199254740993n);assert.equal(pool.shares,(1n<<80n)+7n);assert.deepEqual(pool.solIndex,[1n,2n,3n,4n]);
 const bytes=Buffer.from(wire.open!,'hex');const open=getOpenPositionInstructionDataDecoder().decode(bytes);
 assert.equal(open.nonce,9007199254740995n);assert.equal(open.q,17n);assert.equal(open.route.inputAmount,17n);assert.equal(open.route.setupLamports,3123456n);
 assert.deepEqual(Buffer.from(getOpenPositionInstructionDataEncoder().encode(open)),bytes);
 const close=getClosePositionInstructionDataDecoder().decode(Buffer.from(wire.close!,'hex'));assert.equal(close.maxBuybackCost,101n);assert.equal(close.route.setupLamports,4567890n);
});
const mint=address('EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v');
const owner=SHORTWIN_SIMPLE_PROGRAM_ADDRESS;
async function caseRoute(){
 const loan=await loanAddress(await poolAddress(await marketAddress(mint),0n),owner,0n);const e=await executionAddresses(loan);
 const source=await associatedToken(e.executionAuthority,mint,TOKEN),dest=await associatedToken(e.executionAuthority,WSOL,TOKEN);
 const build:BuildResponse={inputMint:mint,outputMint:WSOL,inAmount:'17',outAmount:'100',otherAmountThreshold:'95',swapMode:'ExactIn',slippageBps:500,routePlan:[{swapInfo:{label:'fixture'}}],swapInstruction:{programId:JUPITER,data:header(17n,100n,500),accounts:[{pubkey:e.executionAuthority,isWritable:false,isSigner:true},{pubkey:source,isWritable:true,isSigner:false},{pubkey:dest,isWritable:true,isSigner:false}]}};
 const binding={...e,mint,tokenProgram:TOKEN,side:'sell' as const,quantity:17n,inputAmount:17n,minimumOutput:90n};return{e,build,binding};
}
test('route uses loan-local scratch, with no outer PDA signature or API setup instructions',async()=>{
 const {e,build,binding}=await caseRoute();const bundle=await planBundle(build,binding);
 assert.deepEqual(bundle.remainingAccounts,[{address:e.executionAuthority,role:AccountRole.READONLY},{address:e.executionBase,role:AccountRole.WRITABLE},{address:e.executionWsol,role:AccountRole.WRITABLE}]);
 const ix=appendRoute({programAddress:owner,data:new Uint8Array([42])},bundle);assert.equal(ix.programAddress,owner);assert.equal(ix.accounts?.length,3);
});
test('route rejects wrong mode, quantity, threshold, and unexpected signer',async()=>{
 const {build,binding}=await caseRoute();
 await assert.rejects(planBundle({...build,swapMode:'ExactOut'},binding));
 await assert.rejects(planBundle({...build,inAmount:'18'},binding));
 await assert.rejects(planBundle({...build,otherAmountThreshold:'89'},binding));
 await assert.rejects(planBundle({...build,inAmount:'1.7'},binding));
 const bad=structuredClone(build);bad.swapInstruction.accounts.push({pubkey:owner,isWritable:false,isSigner:true});await assert.rejects(planBundle(bad,binding));
});
test('buy minimum output must cover whole token debt',async()=>{
 const {build,binding}=await caseRoute();const buy={...build,inputMint:WSOL,outputMint:mint,inAmount:'17',outAmount:'18',otherAmountThreshold:'16'};
 await assert.rejects(planBundle(buy,{...binding,side:'buy',minimumOutput:17n}));
});
test('wallet signatures may change but reviewed message bytes may not',()=>{
 const m=setTransactionMessageLifetimeUsingBlockhash({blockhash:'11111111111111111111111111111111' as Blockhash,lastValidBlockHeight:1n},setTransactionMessageFeePayerSigner(createNoopSigner(owner),createTransactionMessage({version:0})));
 const tx=compileTransaction(m),bytes=Uint8Array.from(getTransactionEncoder().encode(tx));
 const signed=bytes.slice();signed.fill(1,1,65);assert.doesNotThrow(()=>assertSameMessage(bytes,signed));
 const changed=signed.slice();changed[changed.length-1]^=1;assert.throws(()=>assertSameMessage(bytes,changed));
});

test('Jupiter request leaves envelope room and still refuses over-budget responses',async()=>{
 const {build,binding}=await caseRoute();let routeBudget='';
 const send=(async(input:RequestInfo|URL)=>{routeBudget=new URL(String(input)).searchParams.get('maxAccounts')??'';return new Response(JSON.stringify(build),{status:200});}) as typeof fetch;
 await requestBuild('https://api.jup.ag',binding,500,undefined,send);assert.equal(routeBudget,'16');
 const oversized=structuredClone(build);oversized.swapInstruction.accounts=Array.from({length:55},()=>build.swapInstruction.accounts[0]!);
 await assert.rejects(planBundle(oversized,binding),/Unsupported Jupiter route/);
});

test('managed request tries16 then32 for no-route or unsupported shape and stops on rate limits',async()=>{
 const {build,binding}=await caseRoute();const budgets:string[]=[];
 const send=(async(input:RequestInfo|URL)=>{const budget=new URL(String(input)).searchParams.get('maxAccounts')!;budgets.push(budget);return budget==='16'?new Response('{}',{status:400}):new Response(JSON.stringify(build),{status:200});}) as typeof fetch;
 await requestBuild('https://api.jup.ag',binding,500,undefined,send);assert.deepEqual(budgets,['16','32']);
 let calls=0;await assert.rejects(requestBuild('https://api.jup.ag',binding,500,undefined,(async()=>{calls++;return new Response('{}',{status:429});}) as typeof fetch));assert.equal(calls,1);
});

test('native setup allowance is separately signed and marks only loan-local authority writable',async()=>{
 const {build,binding,e}=await caseRoute();const route=await planBundle(build,{...binding,setupLamports:5_000_000n});
 assert.equal(route.route.setupLamports,5_000_000n);assert.equal(route.remainingAccounts.find(a=>a.address===e.executionAuthority)?.role,AccountRole.WRITABLE);
 await assert.rejects(planBundle(build,{...binding,setupLamports:5_000_001n}),/setup allowance/);
});

test('direct requests reject provider-widened slippage and excluded venue responses',async()=>{
 const {build,binding}=await caseRoute();
 for(const changed of [{...build,slippageBps:1000},{...build,routePlan:[{swapInfo:{label:'Scorch'}}]}]){
  await assert.rejects(requestBuild('https://api.jup.ag',binding,500,undefined,(async()=>new Response(JSON.stringify(changed),{status:200})) as typeof fetch),/profile/);
 }
});
