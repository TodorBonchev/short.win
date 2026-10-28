#!/usr/bin/env node
import {bridgeServer,type BridgeOptions} from '../src/server.ts';
if(process.argv.includes('--help')){
 console.log(`Unsigned quote/history HTTP bridge (default 127.0.0.1:8787).
Required: --settings FILE --service-database FILE
RPC: --rpc-url URL or SOLANA_RPC_URL
Optional: --frontend-origin ORIGIN --host HOST --port PORT --python PATH
JUPITER_API_KEY stays in the server environment; keyless spacing must be >=2000ms.
POST /v1/requests warms exact-q history. GET /v1/reference reports current on-chain readiness.
POST /v1/route returns a checked fresh whole-position route. No HTTP signing endpoint exists.`);
 process.exit(0);
}
const value=(name:string,fallback?:string)=>{const at=process.argv.indexOf(name);const result=at<0?fallback:process.argv[at+1];if(result===undefined||result.startsWith('--'))throw new Error('Missing '+name);return result;};
const options:BridgeOptions={rpcUrl:value('--rpc-url',process.env.SOLANA_RPC_URL),serviceSettings:value('--settings'),serviceDatabase:value('--service-database'),
 frontendOrigin:value('--frontend-origin','http://127.0.0.1:5173'),host:value('--host','127.0.0.1'),port:Number(value('--port','8787')),
 python:value('--python','python3')};
if(!Number.isInteger(options.port)||options.port!<1||options.port!>65535)throw new Error('Port outside range');
const server=await bridgeServer(options);
server.listen(options.port,options.host,()=>console.log('Unsigned quote/history bridge listening on '+options.host+':'+options.port));
