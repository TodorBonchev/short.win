import {createSolanaRpc,getBase64Encoder,type Address,type Decoder} from '@solana/kit';
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
 if(!(value.owner===TOKEN||value.owner===TOKEN_2022)||value.executable||bytes.length!==82||bytes[45]!==1||bytes.slice(46,50).some(x=>x!==0)||mint===WSOL)throw new Error('This mint is outside the plain SPL/Token-2022 scope');
 return {tokenProgram:value.owner,decimals:bytes[44]!};
}
