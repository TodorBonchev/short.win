import {address,AccountRole,getBase64Encoder,type Address,type Instruction} from '@solana/kit';
import {JUPITER,WSOL,TOKEN,associatedToken} from './pdas.ts';
export type BuildResponse={inputMint:string,outputMint:string,inAmount:string,outAmount:string,otherAmountThreshold:string,swapMode:string,slippageBps?:number,routePlan?:{swapInfo?:{label?:string}}[],swapInstruction:{programId:string,accounts:{pubkey:string,isWritable:boolean,isSigner:boolean}[],data:string},addressesByLookupTableAddress?:Record<string,readonly string[]>};
export type RouteBinding={mint:Address,tokenProgram:Address,side:'sell'|'buy',quantity:bigint,inputAmount:bigint,minimumOutput:bigint,executionAuthority:Address,executionBase:Address,executionWsol:Address,setupLamports?:bigint};
export type RouteBundle={route:{inputAmount:bigint,setupLamports:bigint,data:Uint8Array},remainingAccounts:{address:Address,role:AccountRole}[],lookupTableKeys:Address[],quotedOutput:bigint,minimumOutput:bigint};
function amount(x:string):bigint{if(!/^(0|[1-9][0-9]*)$/.test(x))throw new Error('Non-integer Jupiter amount');const n=BigInt(x);if(n>0xffffffffffffffffn)throw new Error('Amount exceeds u64');return n;}
/** Only the swap instruction enters the protocol CPI. Protocol-owned scratch replaces
 * the taker's endpoint ATAs; setup/cleanup/tips are never copied into a user transaction.
 * The program independently parses/binds the RouterV2 header and validates net balances.
 */
export async function planBundle(build:BuildResponse,b:RouteBinding):Promise<RouteBundle>{
 const setupLamports=b.setupLamports??0n;
 if(setupLamports<0n||setupLamports>5_000_000n)throw new Error('Native route setup allowance exceeds cap');
 const sell=b.side==='sell';const input=sell?b.mint:WSOL,output=sell?WSOL:b.mint;
 if(build.inputMint!==input||build.outputMint!==output||build.swapMode!=='ExactIn'||amount(build.inAmount)!==b.inputAmount||b.inputAmount<=0n||b.minimumOutput<=0n||(sell&&b.inputAmount!==b.quantity))throw new Error('Jupiter quote does not match the requested whole-position trade');
 const quoted=amount(build.outAmount),minimum=amount(build.otherAmountThreshold);
 if(minimum<b.minimumOutput||quoted<minimum||(!sell&&minimum<b.quantity))throw new Error('Jupiter output does not guarantee the full required output');
 const swap=build.swapInstruction;if(swap.programId!==JUPITER||swap.accounts.length>40)throw new Error('Unsupported Jupiter route');
 const data=Uint8Array.from(getBase64Encoder().encode(swap.data));if(data.length<8||data.length>768)throw new Error('Unsupported route data length');
 const view=new DataView(data.buffer,data.byteOffset,data.byteLength);
 const discriminator=[187,100,250,204,49,196,175,20];
 if(data.length<39||!discriminator.every((v,i)=>data[i]===v)||view.getBigUint64(8,true)!==b.inputAmount||view.getBigUint64(16,true)!==quoted||view.getUint16(24,true)>1000||view.getUint16(26,true)!==0||view.getUint16(28,true)!==0)throw new Error('Unsupported or mismatched ordinary RouteV2 header');
 const slip=BigInt(view.getUint16(24,true)),steps=view.getUint32(30,true);
 if(steps<1||steps>8||data.length-34<steps*5||minimum!==(quoted*(10000n-slip)+9999n)/10000n)throw new Error('Route step/slippage envelope mismatch');
 const sourceAta=await associatedToken(b.executionAuthority,input,sell?b.tokenProgram:TOKEN);
 const destAta=await associatedToken(b.executionAuthority,output,sell?TOKEN:b.tokenProgram);
 const source=sell?b.executionBase:b.executionWsol,dest=sell?b.executionWsol:b.executionBase;
 const remainingAccounts=swap.accounts.map(a=>{
  const key=address(a.pubkey);
  if(a.isSigner&&key!==b.executionAuthority)throw new Error('Route asks for an unexpected signer');
  const remapped=key===sourceAta?source:key===destAta?dest:key;
  return {address:remapped,role:(a.isWritable||(key===b.executionAuthority&&setupLamports>0n))?AccountRole.WRITABLE:AccountRole.READONLY};
 });
 if(!remainingAccounts.some(a=>a.address===source)||!remainingAccounts.some(a=>a.address===dest)||!remainingAccounts.some(a=>a.address===b.executionAuthority))throw new Error('Missing route endpoints');
 return {route:{inputAmount:b.inputAmount,setupLamports,data},remainingAccounts,lookupTableKeys:Object.keys(build.addressesByLookupTableAddress??{}).map(address),quotedOutput:quoted,minimumOutput:minimum};
}
export function appendRoute(instruction:Instruction,bundle:RouteBundle):Instruction {
 return {...instruction,accounts:[...(instruction.accounts??[]),...bundle.remainingAccounts]};
}
/** V2 is ExactIn only. Whole-q buyback selection is a bounded off-chain search;
 * the returned minimum output must cover q, then the on-chain net credit checks it.
 */
export async function requestBuild(baseUrl:string,b:RouteBinding,slippageBps=50,apiKey?:string,send:typeof fetch=fetch):Promise<BuildResponse>{
 if(!Number.isInteger(slippageBps)||slippageBps<0||slippageBps>1000)throw new Error('Invalid slippage');
 let last:unknown;
 for(const accountBudget of [16,32]){
  if(accountBudget===32)await new Promise(resolve=>setTimeout(resolve,apiKey?1000:2000));
  const url=new URL('/swap/v2/build',baseUrl);
  url.search=new URLSearchParams({inputMint:b.side==='sell'?b.mint:WSOL,outputMint:b.side==='sell'?WSOL:b.mint,amount:b.inputAmount.toString(),taker:b.executionAuthority,destinationTokenAccount:b.side==='sell'?b.executionWsol:b.executionBase,wrapAndUnwrapSol:'false',maxAccounts:String(accountBudget),transactionVersion:'0',excludeDexes:'Scorch',slippageBps:String(slippageBps)}).toString();
  const response=await send(url,{headers:apiKey?{'x-api-key':apiKey}:{},signal:AbortSignal.timeout(10000)});
  if(response.status===400){last=new Error('Jupiter has no route for this request budget');continue;}
  if(!response.ok)throw new Error('Jupiter quote unavailable: '+response.status);
  try{const build=await response.json() as BuildResponse;
   if(build.slippageBps!==slippageBps||!Array.isArray(build.routePlan)||!build.routePlan.length||build.routePlan.some(step=>!step||typeof step!=='object'||(step.swapInfo!==undefined&&(!step.swapInfo||typeof step.swapInfo!=='object'))||step.swapInfo?.label==='Scorch'))throw new Error('Jupiter response violates requested route profile');
   const data=getBase64Encoder().encode(build.swapInstruction.data);if(data.length<26||new DataView(data.buffer,data.byteOffset,data.byteLength).getUint16(24,true)!==slippageBps)throw new Error('Jupiter header slippage differs from requested slippage');
   await planBundle(build,b);return build;}catch(error){last=error;}
 }
 throw last??new Error('No supported bounded Jupiter route');
}
