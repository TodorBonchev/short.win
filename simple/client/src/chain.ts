import {createSolanaRpc,getBase64Encoder,getBase58Encoder,type Address,type Decoder} from '@solana/kit';
import * as g from './generated/index.ts';
import {PROGRAM,TOKEN,TOKEN_2022,WSOL} from './pdas.ts';
export async function readAccount<T>(url:string,key:Address,decoder:Decoder<T>,kind:number):Promise<T|null> {
 const {value}=await createSolanaRpc(url).getAccountInfo(key,{encoding:'base64',commitment:'confirmed'}).send();
 if(value===null)return null;
 if(value.owner!==PROGRAM||value.executable)throw new Error('Unexpected program account owner');
 const bytes=getBase64Encoder().encode(value.data[0]);
 if(g.identifyShortwinSimpleAccount(bytes)!==kind)throw new Error('Wrong account type');
 return decoder.decode(bytes);
}
export async function listAccounts(url:string) {
 const rows=await createSolanaRpc(url).getProgramAccounts(PROGRAM,{encoding:'base64',commitment:'confirmed'}).send();
 return rows.map(({pubkey,account})=>{
  if(account.executable)throw new Error('Executable in state scan');
  const bytes=getBase64Encoder().encode(account.data[0]);const kind=g.identifyShortwinSimpleAccount(bytes);
  const decoder=[g.getConfigDecoder(),g.getExposureDecoder(),g.getLenderReceiptDecoder(),g.getLoanDecoder(),g.getMarketDecoder(),g.getPolicyDecoder(),g.getPoolGenerationDecoder(),g.getPriceReferenceDecoder()][kind]!;
  return {address:pubkey,kind,data:decoder.decode(bytes)};
 });
}
export async function readMint(url:string,mint:Address) {
 const {value}=await createSolanaRpc(url).getAccountInfo(mint,{encoding:'base64',commitment:'confirmed'}).send();
 if(value===null)throw new Error('Mint does not exist');
 const bytes=getBase64Encoder().encode(value.data[0]);
 return validateMint(mint,value.owner,value.executable,Uint8Array.from(bytes));
}

/** Mirror the program's metadata-only mint envelope; metadata strings never name
 * financial identities. The program independently verifies current mint bytes. */
export function validateMint(mint:Address,owner:Address,executable:boolean,bytes:Uint8Array){
 const reject=()=>{throw new Error('Mint is outside supported SPL/metadata-only Token-2022 scope');};
 if(!(owner===TOKEN||owner===TOKEN_2022)||executable||mint===WSOL||bytes.length<82||bytes.length>4096||bytes.length===355||(owner===TOKEN&&bytes.length!==82))reject();
 const view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength);
 if(![0,1].includes(view.getUint32(0,true))||bytes[45]!==1||view.getUint32(46,true)!==0)reject();
 if(bytes.length>82){
  if(bytes.length<166||bytes.slice(82,165).some(x=>x!==0)||bytes[165]!==1)reject();
  let at=166;const seen=new Set<number>(),mintBytes=getBase58Encoder().encode(mint);
  while(at<bytes.length){
   if(bytes.slice(at).every(x=>x===0))break;
   if(bytes.length-at<4)reject();
   const kind=view.getUint16(at,true),length=view.getUint16(at+2,true),end=at+4+length;at+=4;
   if(end>bytes.length||seen.has(kind)||![18,19].includes(kind))reject();seen.add(kind);
   if(kind===18){if(length!==64)reject();}
   else {
    if(length<80||!mintBytes.every((x,i)=>bytes[at+32+i]===x))reject();
    let cursor=at+64;
    const u32=()=>{if(cursor+4>end)reject();const n=view.getUint32(cursor,true);cursor+=4;return n;};
    const string=()=>{const n=u32();if(cursor+n>end)reject();new TextDecoder('utf-8',{fatal:true}).decode(bytes.subarray(cursor,cursor+n));cursor+=n;};
    for(let i=0;i<3;i++)string();
    const pairs=u32();if(pairs>Math.floor((end-cursor)/8))reject();
    for(let i=0;i<pairs;i++){string();string();}
    if(cursor!==end)reject();
   }
   at=end;
  }
 }
 return {tokenProgram:owner,decimals:bytes[44]!};
}
