import { address, getAddressEncoder, getProgramDerivedAddress, getU64Encoder, getUtf8Encoder, type Address } from '@solana/kit';
import { SHORTWIN_SIMPLE_PROGRAM_ADDRESS } from './generated/index.ts';
export const PROGRAM = SHORTWIN_SIMPLE_PROGRAM_ADDRESS;
export const SYSTEM = address('11111111111111111111111111111111');
export const TOKEN = address('TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA');
export const TOKEN_2022 = address('TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb');
export const WSOL = address('So11111111111111111111111111111111111111112');
export const JUPITER = address('JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4');
export const JUPITER_DATA = address('4Ec7ZxZS6Sbdg5UGSLHbAnM7GQHp2eFd4KYWRexAipQT');
const utf8=getUtf8Encoder(), key=getAddressEncoder(), u64=getU64Encoder();
export async function derive(seed:string,...parts:readonly (Address|bigint)[]):Promise<Address> {
 return (await getProgramDerivedAddress({programAddress:PROGRAM,seeds:[utf8.encode(seed),...parts.map(p=>typeof p==='bigint'?u64.encode(p):key.encode(p))]}))[0];
}
export const configAddress=()=>derive('config');
export const policyAddress=async(version:bigint)=>derive('policy',await configAddress(),version);
export const marketAddress=(mint:Address)=>derive('market',mint);
export const poolAddress=(market:Address,generation:bigint)=>derive('pool',market,generation);
export const vaultAddress=(pool:Address)=>derive('token_vault',pool);
export const receiptAddress=(pool:Address,owner:Address)=>derive('lender',pool,owner);
export const loanAddress=(pool:Address,borrower:Address,nonce:bigint)=>derive('loan',pool,borrower,nonce);
export const exposureAddress=(pool:Address,borrower:Address)=>derive('exposure',pool,borrower);
export const referenceAddress=(market:Address,policy:Address,q:bigint)=>derive('reference',market,policy,q);
export const eventAuthority=()=>derive('__event_authority');
export async function executionAddresses(loan:Address) {return {
 executionAuthority:await derive('simple_exec',loan),executionBase:await derive('simple_base',loan),executionWsol:await derive('simple_wsol',loan),
 wsolMint:WSOL,wsolTokenProgram:TOKEN,jupiter:JUPITER,jupiterProgramData:JUPITER_DATA,systemProgram:SYSTEM,
};}
export async function associatedToken(owner:Address,mint:Address,tokenProgram:Address):Promise<Address> {
 return (await getProgramDerivedAddress({programAddress:address('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL'),seeds:[key.encode(owner),key.encode(tokenProgram),key.encode(mint)]}))[0];
}
