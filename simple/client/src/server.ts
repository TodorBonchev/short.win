/** Optional Node HTTP bridge. This module contains no wallet or signing path. */
import {createServer,type IncomingMessage,type ServerResponse} from 'node:http';
import {readFile} from 'node:fs/promises';
import {address,getBase58Encoder,getBase64Encoder,type Address,type Decoder,type AddressesByLookupTableAddress} from '@solana/kit';
import * as g from './generated/index.ts';
import {PROGRAM,TOKEN,TOKEN_2022,WSOL,configAddress,policyAddress,marketAddress,poolAddress,referenceAddress,loanAddress,executionAddresses} from './pdas.ts';
import {planBundle,type BuildResponse} from './route.ts';
import {readLookupTables} from './transactions.ts';
import {pythonCommand,rpcTransport,validateServicePolicy,stringify,type RpcCall,type ServiceSettings} from './operator.ts';

export type BridgeOptions={rpcUrl:string,serviceSettings:string,serviceDatabase:string,frontendOrigin:string,
 python?:string,host?:string,port?:number,globalPerMinute?:number,perIpPerMinute?:number,maxPeers?:number};
export type MarketState={config:g.Config,policy:g.Policy,market:g.Market,pool:g.PoolGeneration,
 configKey:Address,policyKey:Address,marketKey:Address,poolKey:Address,mint:Address,tokenProgram:Address,
 clock:{slot:bigint,timestamp:bigint}};
export type BridgeDependencies={readState:(mint:Address)=>Promise<MarketState>,readLoan:(loan:Address)=>Promise<g.Loan|null>,
 readReference:(market:Address,policy:Address,q:bigint)=>Promise<g.PriceReference|null>,
 service:(command:readonly string[],input?:unknown)=>Promise<string>,tables:(keys:readonly Address[])=>Promise<AddressesByLookupTableAddress>,
 settings:ServiceSettings,now?:()=>number};
export type BridgeRequest={method:string,path:string,origin?:string,ip:string,body?:unknown};
export type BridgeResponse={status:number,body:unknown,headers:Record<string,string>};
const U64=0xffffffffffffffffn;
class Refusal extends Error {status:number;constructor(status:number,message:string){super(message);this.status=status;}}
function object(value:unknown):Record<string,unknown>{if(!value||typeof value!=='object'||Array.isArray(value))throw new Refusal(400,'JSON object required');return value as Record<string,unknown>;}
function fields(body:Record<string,unknown>,allowed:readonly string[]):void{if(Object.keys(body).some(key=>!allowed.includes(key)))throw new Refusal(400,'Unsupported request field');}
function key(value:unknown):Address{if(typeof value!=='string')throw new Refusal(400,'Public key required');try{return address(value);}catch{throw new Refusal(400,'Invalid public key');}}
function integer(value:unknown,name:string,positive=true):bigint{if(typeof value!=='string'||!/^(0|[1-9][0-9]*)$/.test(value))throw new Refusal(400,name+' must be an unsigned decimal string');const n=BigInt(value);if(n>U64||(positive&&n===0n))throw new Refusal(400,name+' outside u64 bounds');return n;}
function bounded(value:number|undefined,fallback:number,max:number):number{const n=value??fallback;if(!Number.isInteger(n)||n<1||n>max)throw new Error('Invalid bridge bound');return n;}

async function account<T>(rpc:RpcCall,key:Address,decoder:Decoder<T>,size:number,kind:number):Promise<T|null>{
 const response=await rpc('getAccountInfo',[key,{encoding:'base64',commitment:'confirmed'}]);const value=response?.value;if(value===null)return null;
 if(!value||value.owner!==PROGRAM||value.executable||value.data?.[1]!=='base64')throw new Refusal(503,'Program account identity unavailable');
 const bytes=getBase64Encoder().encode(value.data[0]);if(bytes.length!==size||g.identifyShortwinSimpleAccount(bytes)!==kind)throw new Refusal(503,'Program account type mismatch');
 const decoded=decoder.decode(bytes);if((decoded as {version:number}).version!==1)throw new Refusal(503,'Unsupported program version');return decoded;
}
export function liveDependencies(options:BridgeOptions,settings:ServiceSettings):BridgeDependencies {
 const rpc=rpcTransport(options.rpcUrl);
 const read=<T>(key:Address,decoder:Decoder<T>,size:number,kind:number)=>account(rpc,key,decoder,size,kind);
 return {settings,service:(command,input)=>pythonCommand(options.python??'python3',options.serviceSettings,options.serviceDatabase,command,input),
  tables:keys=>readLookupTables(options.rpcUrl,keys),
  readLoan:key=>read(key,g.getLoanDecoder(),g.getLoanSize(),g.ShortwinSimpleAccount.Loan),
  readReference:async(market,policy,q)=>read(await referenceAddress(market,policy,q),g.getPriceReferenceDecoder(),g.getPriceReferenceSize(),g.ShortwinSimpleAccount.PriceReference),
  readState:async(mint)=>{
   const configKey=await configAddress(),marketKey=await marketAddress(mint);
   const config=await read(configKey,g.getConfigDecoder(),g.getConfigSize(),g.ShortwinSimpleAccount.Config);
   const market=await read(marketKey,g.getMarketDecoder(),g.getMarketSize(),g.ShortwinSimpleAccount.Market);
   if(!config||!market)throw new Refusal(409,'Register this eligible mint on-chain before requesting history');
   const genesis=await rpc('getGenesisHash');if(typeof genesis!=='string'||Buffer.from(getBase58Encoder().encode(genesis)).toString('hex')!==Buffer.from(config.genesisHash).toString('hex'))throw new Refusal(503,'RPC chain differs from initialized Config');
   const policyKey=await policyAddress(config.currentPolicyVersion),poolKey=await poolAddress(marketKey,market.activeGeneration);
   if(config.currentPolicy!==policyKey||market.config!==configKey||market.mint!==mint)throw new Refusal(503,'Canonical market/config binding mismatch');
   const policy=await read(policyKey,g.getPolicyDecoder(),g.getPolicySize(),g.ShortwinSimpleAccount.Policy);
   const pool=await read(poolKey,g.getPoolGenerationDecoder(),g.getPoolGenerationSize(),g.ShortwinSimpleAccount.PoolGeneration);
   if(!policy||!pool||policy.config!==configKey||pool.market!==marketKey||pool.mint!==mint||pool.tokenProgram!==market.tokenProgram||pool.retired)throw new Refusal(503,'Canonical policy/pool binding mismatch');
   const result=await rpc('getAccountInfo',[mint,{encoding:'base64',commitment:'confirmed'}]);const m=result?.value;
   if(!m||m.executable||![TOKEN,TOKEN_2022].includes(m.owner)||m.owner!==market.tokenProgram||m.data?.[1]!=='base64'||mint===WSOL)throw new Refusal(422,'Mint is outside supported plain SPL/Token-2022 scope');
   const data=getBase64Encoder().encode(m.data[0]);if(data.length!==82||data[45]!==1||data.slice(46,50).some(x=>x!==0)||data[44]!==market.decimals)throw new Refusal(422,'Mint is outside supported plain SPL/Token-2022 scope');
   const slot=BigInt(await rpc('getSlot',[{commitment:'confirmed'}]));const timestamp=BigInt(await rpc('getBlockTime',[Number(slot)]));
   const now=BigInt(Math.floor(Date.now()/1000));if(slot<=0n||timestamp<=0n||timestamp>now+5n||now-timestamp>60n)throw new Refusal(503,'RPC clock unavailable or stale');
   return {config,policy,market,pool,configKey,policyKey,marketKey,poolKey,mint,tokenProgram:market.tokenProgram,clock:{slot,timestamp}};
  }};
}
function finite(reference:g.PriceReference|null,state:MarketState,q:bigint):boolean {
 if(!reference||state.config.pricesPaused||!reference.finite||reference.market!==state.marketKey||reference.policy!==state.policyKey||reference.quantity!==q||reference.reporterEpoch!==state.config.reporterEpoch||reference.sequence<=0n)return false;
 const r=reference,p=state.policy.terms,c=state.clock;if(r.sourceSlot<=0n||r.sourceTs<=0n||r.sourceSlot>c.slot||r.sourceTs>c.timestamp||c.slot-r.sourceSlot>p.referenceMaxAgeSlots||c.timestamp-r.sourceTs>BigInt(p.referenceMaxAgeSeconds))return false;
 const epoch=BigInt(p.historyEpochSeconds),start=r.sourceTs/epoch*epoch;
 return r.liveBidLower>0n&&r.liveAskUpper>=r.liveBidLower&&r.historyBidLowerMax>0n&&r.historyAskUpperMax>=r.historyBidLowerMax&&r.acquisitionReference>0n&&r.acquisitionReference<=r.liveAskUpper&&r.acquisitionReference<=r.historyAskUpperMax&&r.completedEpochs===p.minimumHistoryEpochs&&r.historyStartTs===start-epoch*BigInt(p.minimumHistoryEpochs)&&r.historyEndTs===start-1n&&r.evidenceHash.some(x=>x!==0)&&(r.liveAskUpper-r.liveBidLower)*10000n<=r.liveBidLower*BigInt(p.maxSpreadBps);
}

/** Per-process request limits complement the provider's shared SQLite rate gate. */
export function createBridge(options:BridgeOptions,deps:BridgeDependencies):(request:BridgeRequest)=>Promise<BridgeResponse>{
 const origin=new URL(options.frontendOrigin).origin;if(origin!==options.frontendOrigin)throw new Error('Frontend origin must contain only scheme and host');
 const globalLimit=bounded(options.globalPerMinute,20,1000),perIp=bounded(options.perIpPerMinute,5,100),maxPeers=bounded(options.maxPeers,1024,10000);
 const peers=new Map<string,{window:number,count:number}>();let window=-1,globalCount=0;const now=deps.now??Date.now;
 const response=(status:number,body:unknown,headers:Record<string,string>={})=>({status,body,headers:{'content-type':'application/json','cache-control':'no-store',...headers}});
 return async request=>{
  const headers:Record<string,string>={};
  try{
   if(request.origin){if(request.origin!==origin)throw new Refusal(403,'Origin is not allowed');headers['access-control-allow-origin']=origin;headers.vary='Origin';}
   if(request.method==='OPTIONS')return response(204,null,{...headers,'access-control-allow-methods':'GET, POST, OPTIONS','access-control-allow-headers':'content-type'});
   const current=Math.floor(now()/60000);if(current!==window){window=current;globalCount=0;peers.clear();}
   if(!peers.has(request.ip)&&peers.size>=maxPeers)throw new Refusal(429,'Peer capacity reached');
   const peer=peers.get(request.ip)??{window:current,count:0};peers.set(request.ip,peer);if(++globalCount>globalLimit||++peer.count>perIp)throw new Refusal(429,'Request budget exceeded');
   if(request.body!==undefined&&Buffer.byteLength(stringify(request.body))>16384)throw new Refusal(413,'Request body exceeds 16KiB');
   const url=new URL(request.path,'http://localhost');
   if(request.method==='GET'&&url.pathname==='/v1/reference'){
    if([...url.searchParams.keys()].some(name=>!['mint','quantity','q'].includes(name))||(url.searchParams.has('quantity')&&url.searchParams.has('q')))throw new Refusal(400,'Unsupported or ambiguous query parameter');
    const mint=key(url.searchParams.get('mint')),q=integer(url.searchParams.get('quantity')??url.searchParams.get('q'),'quantity');const state=await deps.readState(mint);
    const ref=await deps.readReference(state.marketKey,state.policyKey,q);const status=finite(ref,state,q)?'Finite':ref?'Unknown':'Warming';
    return response(200,{status,mint,quantity:q.toString(),reference:ref,opensPaused:state.config.opensPaused,pricesPaused:state.config.pricesPaused},headers);
   }
   if(request.method!=='POST')throw new Refusal(404,'Endpoint not found');
   const body=object(request.body);
   if(url.pathname==='/v1/requests'){
    fields(body,['mint','quantity']);const mint=key(body.mint),q=integer(body.quantity,'quantity');const state=await deps.readState(mint);
    if(state.config.opensPaused||state.config.pricesPaused)throw new Refusal(409,'New opens or price use is currently paused');
    if(q>state.pool.cash)throw new Refusal(422,'Quantity exceeds available tracked principal');
    const model=(await deps.service(['model-hash'])).trim();validateServicePolicy(deps.settings,model,state.policy);await deps.service(['check-policy','-'],state.policy);
    const job={market:state.marketKey,policy:state.policyKey,mint,quantity:q.toString(),active_loan:false};const result=JSON.parse(await deps.service(['enqueue','-'],[job]));
    if(!Array.isArray(result)||result.length!==1||result[0]?.accepted!==true)throw new Refusal(429,'Exact-quantity history queue is full');
    return response(202,{status:'Warming',mint,quantity:q.toString(),market:state.marketKey,policy:state.policyKey},headers);
   }
   if(url.pathname==='/v1/route'){
    fields(body,['mint','tokenProgram','side','quantity','inputAmount','minimumOutput','executionAuthority','executionBase','executionWsol','loan','borrower','nonce']);
    const mint=key(body.mint),tokenProgram=key(body.tokenProgram),loanKey=key(body.loan),q=integer(body.quantity,'quantity'),input=integer(body.inputAmount,'inputAmount'),minimum=integer(body.minimumOutput,'minimumOutput');
    if(body.side!=='sell'&&body.side!=='buy')throw new Refusal(400,'side must be sell or buy');
    const state=await deps.readState(mint);if(tokenProgram!==state.tokenProgram)throw new Refusal(422,'Token program mismatch');const exec=await executionAddresses(loanKey);
    if(key(body.executionAuthority)!==exec.executionAuthority||key(body.executionBase)!==exec.executionBase||key(body.executionWsol)!==exec.executionWsol)throw new Refusal(422,'Loan scratch identity mismatch');
    let policyKey=state.policyKey;
    if(body.side==='sell'){
     const borrower=key(body.borrower),nonce=integer(body.nonce,'nonce',false);
     if(state.config.opensPaused||state.config.pricesPaused||nonce!==state.market.nextLoanNonce||loanKey!==await loanAddress(state.poolKey,borrower,nonce)||q!==input||q>state.pool.cash)throw new Refusal(409,'Current opening context changed or unavailable');
     const ref=await deps.readReference(state.marketKey,state.policyKey,q);if(!finite(ref,state,q))throw new Refusal(409,'Exact-quantity reference is not ready');
    }else{
     const loan=await deps.readLoan(loanKey);if(!loan||loan.market!==state.marketKey||loan.pool!==state.poolKey||loan.q!==q||loanKey!==await loanAddress(loan.pool,loan.borrower,loan.nonce)||input>loan.collateral||minimum<q||state.clock.timestamp>=loan.defaultAt)throw new Refusal(409,'Whole-debt repayment context changed or unavailable');policyKey=loan.policy;
    }
    const job={market:state.marketKey,policy:policyKey,mint,quantity:q.toString(),active_loan:body.side==='buy'};
    const command=body.side==='sell'?['build-sell','-','--taker',exec.executionAuthority,'--minimum-output',minimum.toString()]:['build-buy','-','--taker',exec.executionAuthority,'--maximum-input',input.toString()];
    const built=JSON.parse(await deps.service(command,job));if(built.status!=='Ready')throw new Refusal(503,'Fresh route unavailable');const actual=integer(built.input_amount,'route input');if(actual>input||(body.side==='sell'&&actual!==q))throw new Refusal(503,'Provider route exceeds requested input');
    const bundle=await planBundle(built.build as BuildResponse,{mint,tokenProgram,side:body.side,quantity:q,inputAmount:actual,minimumOutput:minimum,...exec});
    const tables=await deps.tables(bundle.lookupTableKeys);return response(200,{status:'Ready',build:built.build,bundle,lookupTables:tables,loan:loanKey,evidenceHash:built.evidence_hash},headers);
   }
   throw new Refusal(404,'Endpoint not found');
  }catch(error){return response(error instanceof Refusal?error.status:503,{status:'Unavailable',error:error instanceof Refusal?error.message:'Upstream evidence or route unavailable'},headers);}
 };
}

export async function bridgeServer(options:BridgeOptions,deps?:BridgeDependencies){
 const settings=deps?.settings??JSON.parse(await readFile(options.serviceSettings,'utf8')) as ServiceSettings;
 const handle=createBridge(options,deps??liveDependencies(options,settings));
 const server=createServer(async(request:IncomingMessage,response:ServerResponse)=>{
  try{let bytes=0;const chunks:Buffer[]=[];for await(const raw of request){const chunk=Buffer.from(raw);bytes+=chunk.length;if(bytes>16384)throw new Refusal(413,'Request body exceeds 16KiB');chunks.push(chunk);}
   const body=bytes?JSON.parse(Buffer.concat(chunks).toString('utf8')):undefined;
   const result=await handle({method:request.method??'',path:request.url??'/',origin:request.headers.origin,ip:request.socket.remoteAddress??'unknown',body});response.writeHead(result.status,result.headers);response.end(result.status===204?'':stringify(result.body));
  }catch(error){response.writeHead(error instanceof Refusal?error.status:400,{'content-type':'application/json','cache-control':'no-store'});response.end(stringify({status:'Unavailable',error:'Invalid or oversized request body'}));}
 });
 server.requestTimeout=15_000;server.headersTimeout=10_000;server.keepAliveTimeout=5_000;server.maxHeadersCount=32;server.maxConnections=64;
 return server;
}
