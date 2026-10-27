#!/usr/bin/env python3
"""Materialize only the pinned research snapshot to ignored probe output; no RPC or signing."""
from pathlib import Path
import base64,gzip,hashlib,json,sys,zlib
root=Path(__file__).resolve().parents[1]
source=root/'tests/jupiter_exec_probe/fixtures/jupiter-2026-10-27'
out=Path(sys.argv[1])if len(sys.argv)>1 else root/'target/jupiter-exec-probe/jupiter-candidate'
sha=lambda b:hashlib.sha256(b).hexdigest()
raw=gzip.decompress((source/'getMultipleAccounts.json.gz').read_bytes())
assert sha(raw)=='c611ab92f91fd6bf4ce415054a1e72cad3a3abe1bef5c2bb22f1e574d7a3c3ff'
rpc=json.loads(raw);assert rpc['result']['context']['slot']==461997164
values=rpc['result']['value'];assert len(values)==3
program,pd,idl=[base64.b64decode(v['data'][0],validate=True)for v in values]
assert sha(program)=='77969e6c064ab31f68250951941ad20690dfc23e97870e63006ccdad39c8a47e'
assert len(pd)==2892269 and sha(pd)=='21e28c59e3b0a5536e48d31a1ca7346514a2bc61d70dc1388c920d7e3be35c1b'
assert int.from_bytes(program[:4],'little')==2 and int.from_bytes(pd[:4],'little')==3
assert int.from_bytes(pd[4:12],'little')==459479624
elf=pd[45:].rstrip(bytes([0]));assert len(elf)==2036377 and sha(elf)=='5e31d320f5d88ae93c7983ce82a1497067abd86ff540b35073abaeeb25ef397b'
loader='BPFLoaderUpgradeab1e11111111111111111111111'
assert values[0]['owner']==loader and values[1]['owner']==loader and values[0]['executable']and not values[1]['executable']
size=int.from_bytes(idl[40:44],'little');decoded=zlib.decompress(idl[44:44+size])
assert sha(decoded)=='1c70d6c96e01648411ff1579a35853d966407a4dff5020a5d33e92e6923c9e67'
assert decoded==(source/'jupiter-onchain-idl.json').read_bytes()
out.mkdir(parents=True,exist_ok=True)
for name,data in [('program.bin',program),('programdata.bin',pd),('jupiter.so',elf)]: (out/name).write_bytes(data)
record={'scope':'candidate-only frozen runtime input; not a live route or production qualification','context_slot':461997164,'deploy_slot':459479624,'raw_rpc_sha256':sha(raw),'elf_sha256':sha(elf),'accounts':[]}
for name,key,value in [('program','JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4',values[0]),('programdata','4Ec7ZxZS6Sbdg5UGSLHbAnM7GQHp2eFd4KYWRexAipQT',values[1])]:
 record['accounts'].append({'name':name,'key':key,**{k:value[k]for k in ['owner','lamports','executable','rentEpoch']}})
(out/'accounts.json').write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps(record,indent=2))
