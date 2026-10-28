#!/usr/bin/env python3
"""Archive public capture bytes losslessly, with a hash index; no network or promotion.

Large raw RPC bodies are retained only for explicitly selected replay captures.
The derived accounts.snapshot.json is reconstructible from raw responses and
receipts, so retaining a duplicate is unnecessary. Existing different evidence
is never overwritten. index.json is regenerated when additional captures arrive.
"""
import argparse
import gzip
import hashlib
import json
from pathlib import Path

def sha(data):
    return hashlib.sha256(data).hexdigest()

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--capture', action='append', default=[])
    args = parser.parse_args()
    source, output = args.source.resolve(), args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    index = {'schema': 1, 'scope': 'Candidate evidence, never fixture promotion or live funded-operation approval',
             'full_rpc_captures': args.capture, 'files': [], 'captures': []}
    old = json.loads((output/'index.json').read_bytes()) if (output/'index.json').exists() else {}
    # Remove only previously generated large archives no longer selected. This
    # is bounded by the owned index; unrelated files and review records survive.
    for item in old.get('files', []):
        name = Path(item['file'])
        if item.get('gzip') and len(name.parts) > 1 and name.parts[0] not in args.capture:
            path = (output/name).resolve()
            if output not in path.parents:
                raise ValueError('archive index path escapes output')
            if path.exists() and sha(path.read_bytes()) == item['stored_sha256']:
                path.unlink()
    def archive(path):
        rel = path.relative_to(source)
        raw = path.read_bytes()
        if path.name == 'accounts.snapshot.json':
            return
        compressed = len(raw) > 500_000
        if compressed and (len(rel.parts) < 2 or rel.parts[0] not in args.capture):
            return
        stored = gzip.compress(raw, compresslevel=9, mtime=0) if compressed else raw
        dest = output/(str(rel)+('.gz' if compressed else ''))
        dest.parent.mkdir(parents=True, exist_ok=True)
        if dest.exists() and dest.read_bytes() != stored:
            raise ValueError('refusing to overwrite changed evidence: '+str(rel))
        dest.write_bytes(stored)
        index['files'].append({'file': str(dest.relative_to(output)), 'source_file': str(rel),
            'raw_sha256': sha(raw), 'raw_bytes': len(raw), 'stored_sha256': sha(stored),
            'stored_bytes': len(stored), 'gzip': compressed})
    for path in sorted(source.iterdir()):
        if path.is_file() and path.suffix in ['.json', '.md', '.txt']:
            archive(path)
        elif path.is_dir():
            if (path/'qualification.json').exists():
                q = json.loads((path/'qualification.json').read_bytes())
                index['captures'].append({k: q.get(k) for k in ['captured_at', 'mint', 'quantity',
                    'request_account_budgets', 'request_max_accounts', 'dexes', 'exclude_dexes',
                    'snapshot_slot', 'snapshot_block_time', 'whole_q_quote_available',
                    'net_bid_lamports', 'whole_q_buy_lamports', 'spread_bps_ceil', 'within_500bps_spread',
                    'mint_eligible_plain82_rule', 'diagnostic_ineligible_capture']} | {'name': path.name})
            for child in sorted(path.iterdir()):
                if child.is_file():
                    archive(child)
    (output/'index.json').write_text(json.dumps(index, indent=2, sort_keys=True)+'\n')
    print(json.dumps({'files': len(index['files']), 'raw_bytes': sum(x['raw_bytes'] for x in index['files']),
                      'stored_bytes': sum(x['stored_bytes'] for x in index['files']),
                      'index_sha256': sha((output/'index.json').read_bytes())}))

if __name__ == '__main__':
    main()
