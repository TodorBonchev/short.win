import {address,AccountRole,getBase64Encoder,type Address,type Instruction} from '@solana/kit';
import {JUPITER,WSOL,TOKEN,associatedToken} from './pdas.ts';
export type BuildResponse={inputMint:string,outputMint:string,inAmount:string,outAmount:string,otherAmountThreshold:string,swapMode:string,swapInstruction:{programId:string,accounts:{pubkey:string,isWritable:boolean,isSigner:boolean}[],data:string},addressesByLookupTableAddress?:Record<string,readonly string[]>};
export type RouteBinding={mint:Address,tokenProgram:Address,side:'sell'|'buy',quantity:bigint,inputAmount:bigint,minimumOutput:bigint,executionAuthority:Address,executionBase:Address,executionWsol:Address};
export type RouteBundle={route:{inputAmount:bigint,data:Uint8Array},remainingAccounts:{address:Address,role:AccountRole}[],lookupTableKeys:Address[],quotedOutput:bigint,minimumOutput:bigint};
function amount(x:string):bigint{if(!/^(0|[1-9][0-9]*)$/.test(x))throw new Error('Non-integer Jupiter amount');const n=BigInt(x);if(n>0xffffffffffffffffn)throw new Error('Amount exceeds u64');return n;}
/** Only the swap instruction enters the protocol CPI. Protocol-owned scratch replaces
 * the taker's endpoint ATAs; setup/cleanup/tips are never copied into a user transaction.
 * The program independently parses/binds the RouterV2 header and validates net balances.
 */
export async function planBundle(build:BuildResponse,b:RouteBinding):Promise<RouteBundle>{
 const sell=b.side==='sell';const input=sell?b.mint:WSOL,output=sell?WSOL:b.mint;
 if(build.inputMint!==input||build.outputMint!==output||build.swapMode!=='ExactIn'||amount(build.inAmount)!==b.inputAmount||b.inputAmount<=0n||b.minimumOutput<=0n||(sell&&b.inputAmount!==b.quantity))throw new Error('Jupiter quote does not match the requested whole-position trade');
 const quoted=amount(build.outAmount),minimum=amount(build.otherAmountThreshold);
 if(minimum<b.minimumOutput||quoted<minimum||(!sell&&minimum<b.quantity))throw new Error('Jupiter output does not guarantee the full required output');
 const swap=build.swapInstruction;if(swap.programId!==JUPITER||swap.accounts.length>48)throw new Error('Unsupported Jupiter route');
 const data=Uint8Array.from(getBase64Encoder().encode(swap.data));if(data.length<8||data.length>768)throw new Error('Unsupported route data length');
 const sourceAta=await associatedToken(b.executionAuthority,input,sell?b.tokenProgram:TOKEN);
 const destAta=await associatedToken(b.executionAuthority,output,sell?TOKEN:b.tokenProgram);
 const source=sell?b.executionBase:b.executionWsol,dest=sell?b.executionWsol:b.executionBase;
 const remainingAccounts=swap.accounts.map(a=>{
  const key=address(a.pubkey);
  if(a.isSigner&&key!==b.executionAuthority)throw new Error('Route asks for an unexpected signer');
  const remapped=key===sourceAta?source:key===destAta?dest:key;
  return {address:remapped,role:a.isWritable?AccountRole.WRITABLE:AccountRole.READONLY};
 });
 if(!remainingAccounts.some(a=>a.address===source)||!remainingAccounts.some(a=>a.address===dest)||!remainingAccounts.some(a=>a.address===b.executionAuthority))throw new Error('Missing route endpoints');
 return {route:{inputAmount:b.inputAmount,data},remainingAccounts,lookupTableKeys:Object.keys(build.addressesByLookupTableAddress??{}).map(address),quotedOutput:quoted,minimumOutput:minimum};
}
export function appendRoute(instruction:Instruction,bundle:RouteBundle):Instruction {
 return {...instruction,accounts:[...(instruction.accounts??[]),...bundle.remainingAccounts]};
}
/** V2 is ExactIn only. Whole-q buyback selection is a bounded off-chain search;
 * the returned minimum output must cover q, then the on-chain net credit checks it.
 */
export async function requestBuild(baseUrl:string,b:RouteBinding,slippageBps=50,apiKey?:string,send:typeof fetch=fetch):Promise<BuildResponse>{
 if(!Number.isInteger(slippageBps)||slippageBps<0||slippageBps>1000)throw new Error('Invalid slippage');
 const url=new URL('/swap/v2/build',baseUrl);
 url.search=new URLSearchParams({inputMint:b.side==='sell'?b.mint:WSOL,outputMint:b.side==='sell'?WSOL:b.mint,amount:b.inputAmount.toString(),taker:b.executionAuthority,destinationTokenAccount:b.side==='sell'?b.executionWsol:b.executionBase,wrapAndUnwrapSol:'false',maxAccounts:'32',slippageBps:String(slippageBps)}).toString();
 const response=await send(url,{headers:apiKey?{'x-api-key':apiKey}:{},signal:AbortSignal.timeout(10000)});
 if(!response.ok)throw new Error('Jupiter quote unavailable: '+response.status);
 const build=await response.json() as BuildResponse;await planBundle(build,b);return build;
}
