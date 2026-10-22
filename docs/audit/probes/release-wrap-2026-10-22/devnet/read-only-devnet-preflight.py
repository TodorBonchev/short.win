#!/usr/bin/env python3
"""Bounded public devnet metadata lookup; no signers, account writes or credential reads."""
import base64,datetime,hashlib,json,struct,urllib.request,urllib.error
from pathlib import Path
OUT=Path(__file__).parent
RPC='https://api.devnet.solana.com'
ALPH='123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'
def b58decode(s):
 n=0
 for c in s:n=n*58+ALPH.index(c)
 return b'\0'*(len(s)-len(s.lstrip('1')))+(n.to_bytes((n.bit_length()+7)//8,'big') if n else b'')
def b58encode(b):
 n=int.from_bytes(b,'big');s=''
 while n:n,r=divmod(n,58);s=ALPH[r]+s
 return '1'*(len(b)-len(b.lstrip(b'\0')))+s
def on_curve(b):
 p=2**255-19;y=int.from_bytes(b,'little')&((1<<255)-1)
 if y>=p:return False
 d=(-121665*pow(121666,p-2,p))%p;yy=y*y%p;den=(d*yy+1)%p
 if den==0:return False
 x2=(yy-1)*pow(den,p-2,p)%p
 return x2==0 or pow(x2,(p-1)//2,p)==1
def pda(program,seeds):
 for bump in range(255,-1,-1):
  b=hashlib.sha256(b''.join(seeds)+bytes([bump])+b58decode(program)+b'ProgramDerivedAddress').digest()
  if not on_curve(b):return b58encode(b)
 raise ValueError('no PDA')
LOADER='BPFLoaderUpgradeab1e11111111111111111111111'
programs={
 'shortwin':'5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK',
 'pump':'6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P',
 'pump_swap':'pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA',
 'pump_fees':'pfeeUxB6jkeY1Hxd7CsFCAjcbHA9rWtchMGdZ6VojVZ',
 'raydium_v4_canonical':'675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8',
 'raydium_cpmm_canonical':'CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C',
 'jupiter':'JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4',
 'squads':'SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf',
 'raydium_v4_official_devnet':'DRaya7Kj3aMWQSy19kSjvmuwq9docCHofyP9kanQGaav',
 'raydium_cpmm_official_devnet':'DRaycpLY18LhpbydsBWbVJtxpNv9oXPgjRSfpF2bWpYb',
 'token':'TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA',
 'token_2022':'TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb',
 'associated_token':'ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL',
 'memo':'MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr',
 'pyth_receiver':'rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ'}
accounts=dict(programs)
for name,key in list(programs.items())[:10]:accounts[name+'_program_data']=pda(LOADER,[b58decode(key)])
accounts.update({
 'pump_global':'4wTV1YmiEkRvAtNtsSGPtUrqRYQMe5SKy2uB4Jjaxnjf',
 'pump_fee_config':'8Wf5TiAheLUqBrKXeYg2JtAFFMWtKdG2BSFgqUcPVwTt',
 'pump_swap_global':'ADyA8hdefvWN2dbGGWFotbzWxrAvLW83WG6QCVXvJKqw',
 'pump_swap_fee_config':'5PHirr8joyTMp9JMm6nW7hNDVyEYdkzDqazxPD7RaTjx',
 'pyth_sol_usd':'7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE',
 'wsol':'So11111111111111111111111111111111111111112',
 'wsol_2022':'9pan9bMn5HatX4EJdBwg9VgCa7Uz5HL8N1m5D3NdXejP',
 'squads_program_config':pda(programs['squads'],[b'multisig',b'program_config']),
 'shortwin_config':pda(programs['shortwin'],[b'config']),
 'shortwin_venue_guard':pda(programs['shortwin'],[b'venue_guard']),
 'shortwin_oracle_cache':pda(programs['shortwin'],[b'sol_usd'])})
# Compare source-pinned ProgramData derivation: public addresses are validated before RPC.
expected_pd=['B5MvUwXdiW1NMM6QFFD3ssPKBujD4zMohncbM73Z2BQu','6naEzKeUuFh1Jeeu51NXQgr5qkXgXtc9WKNct4xynVJc','75Uu23mqWBb8LM8vDppqC1mQAnCcBuLXhVaDezVMQLRw','A7ZG7ByDi8DpzT9Ab7CiXhvgYTJQmaDPJkMDoPitaCQV','DMawCQzbgNTmbzaESc7o6pvL1KAeetY8zA7jNpzntHhU','4Ec7ZxZS6Sbdg5UGSLHbAnM7GQHp2eFd4KYWRexAipQT']
for name,want in zip(['pump','pump_swap','pump_fees','raydium_v4_canonical','raydium_cpmm_canonical','jupiter'],expected_pd):
 assert accounts[name+'_program_data']==want,(name,accounts[name+'_program_data'],want)
for key in accounts.values():assert len(b58decode(key))==32 and b58encode(b58decode(key))==key,key
metadata={'rpc':RPC,'started_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'requests':[],'public_accounts':accounts}
def request(method,params,label):
 start=datetime.datetime.now(datetime.timezone.utc).isoformat();payload={'jsonrpc':'2.0','id':label,'method':method,'params':params}
 result={'request':payload,'started_at':start}
 try:
  req=urllib.request.Request(RPC,data=json.dumps(payload).encode(),headers={'Content-Type':'application/json','User-Agent':'shortwin-readonly-devnet-preflight/1'})
  with urllib.request.urlopen(req,timeout=30) as response:
   raw=response.read(2_000_000);result['http_status']=response.status
  result['response']=json.loads(raw)
 except (urllib.error.URLError,TimeoutError,ValueError) as e:result['error']=str(e)
 result['finished_at']=datetime.datetime.now(datetime.timezone.utc).isoformat();metadata['requests'].append(result)
 (OUT/'devnet-rpc-raw.json').write_text(json.dumps(metadata,indent=2)+'\n')
 return result.get('response',{}).get('result')
genesis=request('getGenesisHash',[],'genesis')
slot=request('getSlot',[{'commitment':'confirmed'}],'slot')
version=request('getVersion',[],'version')
result=request('getMultipleAccounts',[list(accounts.values()),{'encoding':'base64','commitment':'confirmed','dataSlice':{'offset':0,'length':128}}],'public_metadata')
summary={'genesis_hash':genesis,'slot':slot,'version':version,'metadata_context':None,'accounts':[]}
if isinstance(result,dict) and isinstance(result.get('value'),list):
 summary['metadata_context']=result.get('context')
 assert len(result['value'])==len(accounts)
 for (name,key),account in zip(accounts.items(),result['value']):
  row={'name':name,'address':key,'exists':account is not None}
  if account is not None:
   assert isinstance(account,dict) and isinstance(account.get('owner'),str) and len(b58decode(account['owner']))==32
   data=base64.b64decode(account['data'][0],validate=True)
   row.update({k:account.get(k) for k in ['owner','lamports','executable','space']});row['returned_data_bytes']=len(data);row['prefix_hex']=data.hex()
   if account['owner']==LOADER and len(data)>=4:
    tag=int.from_bytes(data[:4],'little');row['loader_tag']=tag
    if tag==2 and len(data)>=36:row['program_data_pointer']=b58encode(data[4:36])
    if tag==3 and len(data)>=13:
     row['deploy_slot']=int.from_bytes(data[4:12],'little');row['upgrade_authority_option']=data[12]
     if data[12]==1 and len(data)>=45:row['upgrade_authority_public_key']=b58encode(data[13:45])
   if name=='squads_program_config' and len(data)>=80:
    row['multisig_creation_fee_lamports']=int.from_bytes(data[40:48],'little');row['treasury_public_key']=b58encode(data[48:80])
  summary['accounts'].append(row)
(OUT/'devnet-rpc-summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps({'genesis_hash':genesis,'slot':slot,'context':summary['metadata_context'],'accounts':[{'name':r['name'],'address':r['address'],'exists':r['exists'],'owner':r.get('owner'),'executable':r.get('executable'),'space':r.get('space'),'deploy_slot':r.get('deploy_slot'),'upgrade_authority_public_key':r.get('upgrade_authority_public_key')} for r in summary['accounts']],'errors':[r.get('error') or r.get('response',{}).get('error') for r in metadata['requests'] if r.get('error') or r.get('response',{}).get('error')]},indent=2))
