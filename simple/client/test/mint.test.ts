import test from 'node:test';
import assert from 'node:assert/strict';
import {getBase58Encoder,address} from '@solana/kit';
import {validateMint} from '../src/chain.ts';
import {TOKEN,TOKEN_2022} from '../src/pdas.ts';
const mint=address('EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v');
function metadata(){
 const data=new Uint8Array(318),view=new DataView(data.buffer);data[44]=6;data[45]=1;data[165]=1;
 view.setUint16(166,18,true);view.setUint16(168,64,true);data.set(getBase58Encoder().encode(mint),202);
 view.setUint16(234,19,true);view.setUint16(236,80,true);data.set(getBase58Encoder().encode(mint),270);
 return data;
}
test('mint discovery and HTTP admission accept metadata-only Token2022 and reject changed token behavior',()=>{
 const data=metadata();assert.deepEqual(validateMint(mint,TOKEN_2022,false,data),{tokenProgram:TOKEN_2022,decimals:6});
 assert.throws(()=>validateMint(mint,TOKEN,false,data));
 for(const [offset,value] of [[0,2],[45,0],[46,1],[82,1],[165,2],[166,1],[168,63],[270,1],[302,255]] as const){const changed=data.slice();changed[offset]=value;assert.throws(()=>validateMint(mint,TOKEN_2022,false,changed),String(offset));}
 const duplicate=new Uint8Array(data.length+68);duplicate.set(data);duplicate.set(data.subarray(166,234),data.length);assert.throws(()=>validateMint(mint,TOKEN_2022,false,duplicate));
 assert.throws(()=>validateMint(mint,TOKEN_2022,false,new Uint8Array(4097)));
 assert.throws(()=>validateMint(mint,TOKEN_2022,true,data));
 const plain=data.slice(0,82);assert.equal(validateMint(mint,TOKEN,false,plain).decimals,6);
});
