#!/usr/bin/env python3
import base64,datetime,json,struct,urllib.error,urllib.request
from pathlib import Path
out=Path(__file__).parent
rpc='https://api.devnet.solana.com'
record={'rpc':rpc,'started_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'requests':[]}
def call(payload,label):
 row={'request':payload,'started_at':datetime.datetime.now(datetime.timezone.utc).isoformat()}
 try:
  req=urllib.request.Request(rpc,data=json.dumps(payload).encode(),headers={'Content-Type':'application/json','User-Agent':'shortwin-readonly-devnet-preflight/1'})
  with urllib.request.urlopen(req,timeout=30) as resp:
   row['http_status']=resp.status;row['response']=json.loads(resp.read(1_000_000))
 except (urllib.error.URLError,TimeoutError,ValueError) as err:row['error']=str(err)
 row['finished_at']=datetime.datetime.now(datetime.timezone.utc).isoformat();record['requests'].append(row)
 (out/'devnet-cost-oracle-raw.json').write_text(json.dumps(record,indent=2)+'\n')
 return row.get('response')
addresses=['7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE','SysvarC1ock11111111111111111111111111111111']
r=call({'jsonrpc':'2.0','id':'oracle_clock','method':'getMultipleAccounts','params':[addresses,{'encoding':'base64','commitment':'confirmed','dataSlice':{'offset':0,'length':256}}]},'oracle_clock')
summary={}
if r and isinstance(r.get('result'),dict):
 summary['oracle_context']=r['result']['context'];pv,cv=r['result']['value']
 assert pv['owner']=='rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ' and not pv['executable'] and pv['space']==134
 pd=base64.b64decode(pv['data'][0],validate=True);cd=base64.b64decode(cv['data'][0],validate=True)
 assert len(pd)==134 and pd[:8]==bytes.fromhex('22f123639d7ef4cd') and pd[40] in [0,1] and len(cd)==40
 m=41 if pd[40]==1 else 42;feed=pd[m:m+32].hex();price,conf,expo,publish,prev,ema,emaconf,posted=struct.unpack('<qQiqqqQQ',pd[m+32:m+92]);clock_slot,epoch_start,epoch,leader,now=struct.unpack('<QqQQq',cd)
 summary['oracle']={'verification':'Full' if pd[40]==1 else 'Partial','feed_id':feed,'price':price,'conf':conf,'exponent':expo,'publish_time':publish,'posted_slot':posted,'clock_unix_timestamp':now,'clock_slot':clock_slot,'age_s':now-publish,'full':pd[40]==1,'expected_feed':feed=='ef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d','confidence_bps_numerator':conf*10000,'positive_price':price>0,'note':'Schema-matched read; apply exact program parameter limits at transaction time.'}
sizes=[0,36,1644069,1644061,1448,304,6512,45,3104,2200,165,82]
r=call([{'jsonrpc':'2.0','id':'rent_'+str(n),'method':'getMinimumBalanceForRentExemption','params':[n,{'commitment':'confirmed'}]} for n in sizes],'rents')
if isinstance(r,list):
 summary['rent_lamports_by_size']={str(item['id'])[5:]:item.get('result') for item in r};summary['rent_errors']=[item for item in r if 'error' in item]
else:summary['rent_batch_error']=r
(out/'devnet-cost-oracle-summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps(summary,indent=2))
