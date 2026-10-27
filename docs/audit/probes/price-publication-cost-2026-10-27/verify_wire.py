"""Independent shortvec/v0 structural reader; no Solana libraries, keys or network."""
from pathlib import Path
import hashlib, json
root=Path(__file__).resolve().parent
results=json.loads((root/'results.json').read_text())
checks=[]
for case in results['packets']:
 data=(root/case['file']).read_bytes(); offset=0
 def take(n):
  global offset
  assert offset+n<=len(data)
  value=data[offset:offset+n];offset+=n
  return value
 def shortvec():
  value=shift=0
  for _ in range(3):
   byte=take(1)[0];value|=(byte&127)<<shift
   if byte<128:return value
   shift+=7
  raise AssertionError('shortvec exceeds 3 bytes')
 signatures=shortvec(); assert signatures==1
 assert take(64)==bytes(64)
 assert take(1)==bytes([0x80])
 header=take(3);assert header[0]==signatures
 keys=shortvec();take(keys*32);take(32)
 instructions=shortvec();sizes=[];account_indices=[]
 for _ in range(instructions):
  program_index=take(1)[0];assert program_index<keys
  accounts=take(shortvec());account_indices.extend(accounts)
  payload=take(shortvec());sizes.append(len(payload))
  if len(payload)==457:
   assert payload[:8]==bytes([83,130,3,116,85,225,131,88])
   assert payload[-25]==1 # Finite discriminated-union variant.
 lookups=shortvec();assert lookups==case['address_lookup_tables']
 loaded=0;lookup_records=[]
 for _ in range(lookups):
  table=take(32);writable=list(take(shortvec()));readonly=list(take(shortvec()))
  loaded+=len(writable)+len(readonly)
  assert table==bytes([13])*32
  assert sorted(writable+readonly)==[0,1,2,3]
  assert len(writable)==2 and len(readonly)==2
  lookup_records.append({'table_bytes_hex':table.hex(),'writable_indices':writable,'readonly_indices':readonly})
 assert loaded==case['loaded_account_count']
 assert all(account<keys+loaded for account in account_indices)
 assert offset==len(data)==case['serialized_packet_bytes']
 assert keys==case['static_account_keys']
 assert instructions==case['total_instructions']
 assert sizes==case['instruction_data_bytes']
 assert hashlib.sha256(data).hexdigest()==case['sha256']
 checks.append({'file':case['file'],'bytes':len(data),'required_signatures':signatures,'static_keys':keys,
  'instructions':instructions,'instruction_data_bytes':sizes,'address_lookups':lookups,'loaded_accounts':loaded,
  'lookup_records':lookup_records,'wire_structure_valid':True})
for case in results['thirty_day_base_fee_lower_bounds']:
 transactions=int(case['transactions']);lamports=transactions*int(case['base_lamports_per_signature'])
 assert lamports==int(case['base_fee_lamports'])
 assert int(case['publication_updates_per_feed'])==30*24*60*60//2
 assert int(case['total_reports'])==2*int(case['publication_updates_per_feed'])
output={'schema':'shortwin-price-publication-wire-check/v1','checks':checks,'base_fee_arithmetic_checked':True}
(root/'wire-check.json').write_text(json.dumps(output,indent=2)+'\n')
print(json.dumps(output,indent=2))
