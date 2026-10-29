#!/usr/bin/env python3
"""Read-only current Pump metadata-mint inventory witnesses, never admission/signing.

Jupiter may quote a virtual curve whose real tokens were fully repurchased. A
candidate for the runtime harness must have actual SOL reserves and enough
circulating tokens outside the curve to fund its explicit synthetic 10*q LP.
The program's complete metadata parser and actual CPI remain separate gates.
"""
import argparse
import base64
import json
import os
from pathlib import Path
import shutil
import subprocess
from importlib.util import spec_from_file_location, module_from_spec

ROOT = Path(__file__).resolve().parents[2]
spec = spec_from_file_location('capture', Path(__file__).with_name('qualify-current-routes.py'))
capture = module_from_spec(spec)
spec.loader.exec_module(capture)
PUMP = '6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P'

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--target-lamports', type=int, default=130_000_000)
    parser.add_argument('--minimum-real-sol', type=int, default=2_000_000_000)
    parser.add_argument('--discovery', choices=['recent','toptraded/5m','toptrending/5m'], default='recent')
    parser.add_argument('--mints-json', type=Path, help='Previously observed public mint identifiers; all account state is fetched anew')
    args = parser.parse_args()
    if not 100_000_000 <= args.target_lamports <= 1_000_000_000:
        raise ValueError('bounded diagnostic target must be 0.1–1 SOL')
    c = capture.Capture(args.output)
    if args.mints_json:
        values = json.loads(args.mints_json.read_bytes())
        if not isinstance(values, list) or not 1 <= len(values) <= 100 or any(not isinstance(v, str) for v in values):
            raise ValueError('bounded public mint identifier list required')
        capture.record(c.output/'public-discovery-identifiers.json', {'file_sha256':capture.sha(args.mints_json.read_bytes()),'mints':values})
        selected = [{'id':v} for v in values]
    else:
        tokens = c.request('discovery-tokens', 'https://api.jup.ag/tokens/v2/'+args.discovery+'?limit=100', jupiter=True)
        selected = [t for t in tokens if t.get('launchpad') == 'pump.fun' and t.get('tokenProgram') == capture.TOKEN22]
    node = os.environ.get('SHORTWIN_NODE') or shutil.which('node') or '/root/.local/share/mise/installs/node/24.21.0/bin/node'
    code = r"""import{address,getProgramDerivedAddress,getAddressEncoder,getUtf8Encoder}from'@solana/kit';
const m=JSON.parse(process.argv[1]),ae=getAddressEncoder(),ue=getUtf8Encoder(),rows=[];
for(const mint of m){const[curve]=await getProgramDerivedAddress({programAddress:address('6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P'),seeds:[ue.encode('bonding-curve'),ae.encode(address(mint))]});
const[vault]=await getProgramDerivedAddress({programAddress:address('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL'),seeds:[ae.encode(curve),ae.encode(address('TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb')),ae.encode(address(mint))]});rows.push({mint,curve,vault});}process.stdout.write(JSON.stringify(rows));"""
    result = subprocess.run([node, '--input-type=module', '-e', code, json.dumps([t['id'] for t in selected])],
                            cwd=ROOT/'simple/client', capture_output=True, text=True)
    if result.returncode:
        raise ValueError('public Pump PDA derivation failed')
    bindings = json.loads(result.stdout)
    capture.record(c.output/'derived-public-addresses.json', {'script_sha256': capture.sha(code.encode()), 'bindings': bindings})
    # Each coin's three accounts are read from one bank, never across a chunk.
    for start in range(0, len(bindings), 25):
        c.snapshot([row[k] for row in bindings[start:start+25] for k in ['mint','curve','vault']], 'mint-curve-vault')
    rows = []
    for row in bindings:
        mint, curve, vault = [c.accounts[row[k]] for k in ['mint', 'curve', 'vault']]
        if not mint or not curve or not vault:
            continue
        m, b, v = [base64.b64decode(a['data'][0], validate=True) for a in [mint, curve, vault]]
        if (curve['owner'] != PUMP or len(b) < 115 or b[:8] != bytes([23,183,248,55,96,216,172,96])
                or mint['owner'] != capture.TOKEN22 or len(m) < 82 or len(v) < 165
                or vault['owner'] != capture.TOKEN22 or capture.b58(v[:32]) != row['mint']
                or capture.b58(v[32:64]) != row['curve']):
            continue
        supply, inventory = int.from_bytes(m[36:44], 'little'), int.from_bytes(v[64:72], 'little')
        virtual_tokens, virtual_quote = int.from_bytes(b[8:16], 'little'), int.from_bytes(b[16:24], 'little')
        real_tokens, real_quote = int.from_bytes(b[24:32], 'little'), int.from_bytes(b[32:40], 'little')
        quantity = (args.target_lamports*virtual_tokens+virtual_quote-1)//virtual_quote if virtual_quote else 0
        extensions, at, valid_tlv = [], 166, len(m) >= 166 and not any(m[82:165]) and m[165] == 1
        while valid_tlv and at < len(m):
            if not any(m[at:]):
                break
            if len(m)-at < 4:
                valid_tlv = False
                break
            kind, length = int.from_bytes(m[at:at+2], 'little'), int.from_bytes(m[at+2:at+4], 'little')
            end = at+4+length
            if end > len(m) or kind not in [18,19] or kind in extensions:
                valid_tlv = False
                break
            extensions.append(kind)
            at = end
        native = b[83:115] == bytes(32)
        contexts = {key:c.account_context[row[key]] for key in ['mint','curve','vault']}
        same_bank = len(set(contexts.values())) == 1
        feasible = (same_bank and b[48] == 0 and native and m[45] == 1 and m[46:50] == bytes(4) and valid_tlv
                    and real_quote >= args.minimum_real_sol and 0 < quantity < 2**64
                    and inventory <= supply and 10*quantity <= supply-inventory)
        rows.append({**row, 'context_slot': c.account_context[row['curve']], 'source_contexts':contexts,
          'single_bank_inventory_witness':same_bank, 'complete': b[48],
          'native_sol_quote': native, 'mint_length': len(m), 'mint_sha256': capture.sha(m),
          'curve_sha256': capture.sha(b), 'vault_sha256': capture.sha(v), 'mint_supply': supply,
          'curve_vault_tokens': inventory, 'circulating_outside_curve': supply-inventory,
          'real_tokens': real_tokens, 'real_quote_raw': real_quote, 'virtual_tokens': virtual_tokens,
          'virtual_quote_raw': virtual_quote, 'metadata_extension_types': extensions,
          'metadata_tlv_bounded_only': valid_tlv, 'mayhem_flag': b[81], 'cashback_flag': b[82],
          'suggested_quantity_raw': quantity, 'ten_q_tokens': 10*quantity,
          'target_lamports_proxy': args.target_lamports, 'physical_candidate_only': feasible})
    rows.sort(key=lambda row: (row['physical_candidate_only'], row['real_quote_raw']), reverse=True)
    capture.record(c.output/'physical-candidates.json', {'scope': 'Physical inventory witness only, not oracle price/admission/CPI qualification',
                   'checked_at': capture.now(), 'rows': rows, 'requests': c.files})
    print(json.dumps([row for row in rows if row['physical_candidate_only']], indent=2))

if __name__ == '__main__':
    main()
