#!/usr/bin/env python3
"""Read-only current Jupiter/RPC candidate capture; never signs, sends or promotes."""
import argparse
import base64
import datetime
import hashlib
import json
import os
import shutil
import struct
import subprocess
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

JUP = 'JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4'
JUP_PD = '4Ec7ZxZS6Sbdg5UGSLHbAnM7GQHp2eFd4KYWRexAipQT'
TOKEN = 'TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA'
TOKEN22 = 'TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb'
WSOL = 'So11111111111111111111111111111111111111112'
LOADER = 'BPFLoaderUpgradeab1e11111111111111111111111'
B58 = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'

def sha(raw):
    return hashlib.sha256(raw).hexdigest()

def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()

def b58(raw):
    n, out = int.from_bytes(raw, 'big'), ''
    while n:
        n, r = divmod(n, 58)
        out = B58[r] + out
    return '1' * (len(raw)-len(raw.lstrip(bytes([0])))) + out

def record(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True)+'\n')

def derive_binding(program_id, mint, borrower, pool_generation=0, nonce=0):
    """Derive public candidate addresses with the same kit PDA primitives as SDK.

    No key file is read and no signer is created. The default borrower is the
    public address of the runtime harness's fixed local seed-100 test signer.
    """
    client = Path(__file__).resolve().parents[1]/'client'
    node = os.environ.get('SHORTWIN_NODE') or shutil.which('node')
    if not node:
        candidate = Path('/root/.local/share/mise/installs/node/24.21.0/bin/node')
        if candidate.is_file():
            node = str(candidate)
    if not node:
        raise ValueError('Node runtime unavailable; set SHORTWIN_NODE to the local runtime path')
    inputs = {'program': program_id, 'mint': mint, 'borrower': borrower,
              'generation': str(pool_generation), 'nonce': str(nonce)}
    code = r"""import{address,getProgramDerivedAddress,getAddressEncoder,getUtf8Encoder,getU64Encoder}from'@solana/kit';
const i=JSON.parse(process.argv[1]),p=address(i.program),ae=getAddressEncoder(),ue=getUtf8Encoder(),u64=getU64Encoder();
async function d(s,...seeds){return(await getProgramDerivedAddress({programAddress:p,seeds:[ue.encode(s),...seeds]}))[0]};
const mint=address(i.mint),borrower=address(i.borrower),market=await d('market',ae.encode(mint)),pool=await d('pool',ae.encode(market),u64.encode(BigInt(i.generation))),loan=await d('loan',ae.encode(pool),ae.encode(borrower),u64.encode(BigInt(i.nonce)));
process.stdout.write(JSON.stringify({program_id:p,mint,borrower,pool_generation:i.generation,nonce:i.nonce,market,pool,loan,execution_authority:await d('simple_exec',ae.encode(loan)),execution_base:await d('simple_base',ae.encode(loan)),execution_wsol:await d('simple_wsol',ae.encode(loan))}));"""
    result = subprocess.run([node, '--input-type=module', '-e', code, json.dumps(inputs)],
                            cwd=client, capture_output=True, text=True)
    if result.returncode:
        raise ValueError('Public PDA derivation failed; verify canonical addresses/runtime')
    binding = json.loads(result.stdout)
    binding['derivation_script_sha256'] = sha(code.encode())
    return binding

class Capture:
    def __init__(self, output):
        self.output = output
        self.output.mkdir(parents=True, exist_ok=False)
        self.rpc_url = os.environ.get('SOLANA_RPC_URL', 'https://api.mainnet-beta.solana.com')
        self.key = os.environ.get('JUPITER_API_KEY', '')
        self.last, self.n, self.exclude_dexes = 0.0, 0, None
        self.files, self.accounts, self.account_context = [], {}, {}

    def request(self, name, url, payload=None, jupiter=False):
        # Only public parameters and selected non-secret headers are persisted.
        time.sleep(max(0, 2.2-(time.monotonic()-self.last)))
        headers = {'Content-Type': 'application/json', 'Accept': 'application/json'}
        if jupiter and self.key:
            headers['x-api-key'] = self.key
        body = json.dumps(payload, separators=(',', ':')).encode() if payload is not None else None
        started, self.last = now(), time.monotonic()
        request = urllib.request.Request(url, data=body, headers=headers)
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                raw, status = response.read(40_000_001), response.status
                safe = {k: v for k, v in response.headers.items() if k.lower() in
                        ['date', 'content-type', 'x-ratelimit-remaining', 'x-ratelimit-current', 'x-ratelimit-reset']}
        except urllib.error.HTTPError as error:
            raw, status, safe = error.read(2_000_001), error.code, {}
        except OSError as error:
            record(self.output/(name+'.receipt.json'), {'started_at': started, 'finished_at': now(),
                   'transport_error': type(error).__name__})
            raise RuntimeError('Read-only provider request failed: '+type(error).__name__) from None
        if len(raw) > 40_000_000:
            raise ValueError('response exceeds capture limit')
        path = self.output/(name+'.json')
        path.write_bytes(raw)
        receipt = {'started_at': started, 'finished_at': now(), 'status': status,
                   'length': len(raw), 'sha256': sha(raw), 'headers': safe,
                   'method': 'GET' if payload is None else payload['method'],
                   'provider': 'Jupiter API' if jupiter else
                   ('configured Solana RPC' if 'SOLANA_RPC_URL' in os.environ else 'Solana public mainnet RPC')}
        if payload is not None:
            receipt['request'] = payload
        elif jupiter:
            receipt['parameters'] = dict(urllib.parse.parse_qsl(urllib.parse.urlsplit(url).query))
        record(self.output/(name+'.receipt.json'), receipt)
        self.files.append({'file': path.name, **receipt})
        obj = json.loads(raw)
        if status != 200 or (isinstance(obj, dict) and obj.get('error')):
            raise ValueError('Provider rejected '+name+'; response retained, status '+str(status))
        return obj

    def rpc(self, name, method, params):
        return self.request(name, self.rpc_url,
                            {'jsonrpc': '2.0', 'id': 1, 'method': method, 'params': params})['result']

    def snapshot(self, keys, prefix):
        keys = sorted(set(keys)-self.accounts.keys())
        for start in range(0, len(keys), 80):
            subset = keys[start:start+80]
            self.n += 1
            response = self.rpc(prefix+'-'+str(self.n), 'getMultipleAccounts',
                                [subset, {'encoding': 'base64', 'commitment': 'confirmed'}])
            for key, value in zip(subset, response['value']):
                self.accounts[key] = value
                self.account_context[key] = response['context']['slot']

    def dependencies(self, mint):
        self.snapshot([JUP, JUP_PD, TOKEN, TOKEN22, WSOL, mint], 'dependency-accounts')
        pending = set()
        for account in self.accounts.values():
            if account and account['executable'] and account['owner'] == LOADER:
                data = base64.b64decode(account['data'][0], validate=True)
                if len(data) != 36 or int.from_bytes(data[:4], 'little') != 2:
                    raise ValueError('bad upgradeable Program state')
                pending.add(b58(data[4:36]))
        self.snapshot(pending, 'programdata-accounts')

    def quote(self, name, mint, amount, side, binding, dexes, max_accounts=32):
        params = {'inputMint': mint if side == 'sell' else WSOL,
                  'outputMint': WSOL if side == 'sell' else mint, 'amount': str(amount),
                  'taker': binding['execution_authority'],
                  'destinationTokenAccount': binding['execution_wsol'] if side == 'sell' else binding['execution_base'],
                  'wrapAndUnwrapSol': 'false', 'maxAccounts': str(max_accounts), 'transactionVersion': '0', 'slippageBps': '50'}
        if dexes:
            params['dexes'] = dexes
        if self.exclude_dexes:
            params['excludeDexes'] = self.exclude_dexes
        response = self.request(name, 'https://api.jup.ag/swap/v2/build?'+urllib.parse.urlencode(params), jupiter=True)
        swap = response.get('swapInstruction', {})
        data = base64.b64decode(swap.get('data', ''), validate=True)
        metas = swap.get('accounts', [])
        checks = {'exact_domain': (response.get('inputMint'), response.get('outputMint'),
                  response.get('swapMode'), response.get('inAmount')) ==
                  (params['inputMint'], params['outputMint'], 'ExactIn', str(amount)),
                  'jupiter_program': swap.get('programId') == JUP, 'total_metas_le48': 0 < len(metas) <= 48,
                  'service_total_metas_le40': 0 < len(metas) <= 40,
                  'route_data_le768': 39 <= len(data) <= 768,
                  'ordinary_route_v2': data[:8] == bytes([187, 100, 250, 204, 49, 196, 175, 20]),
                  'slippage50': response.get('slippageBps') == 50,
                  'output_bounds': 0 < int(response['otherAmountThreshold']) <= int(response['outAmount']),
                  'excluded_labels_absent': all(x['swapInfo']['label'] not in (self.exclude_dexes or '').split(',') for x in response['routePlan']),
                  'no_extra_signers': all(not x['isSigner'] or x['pubkey'] == binding['execution_authority'] for x in metas)}
        if len(data) >= 34:
            inp, quoted, slippage, fee, positive, steps = struct.unpack('<QQHHHI', data[8:34])
            checks.update({'header_input': inp == amount, 'header_quote': quoted == int(response['outAmount']),
                           'header_no_fees': fee == positive == 0, 'steps_le8': 0 < steps <= 8 and len(data)-34 >= steps*5,
                           'header_threshold': int(response['otherAmountThreshold']) == (quoted*(10_000-slippage)+9999)//10_000,
                           'header_slippage50': slippage == 50})
        analysis = {'side': side, 'request_amount': amount, 'expected_output': int(response['outAmount']),
                    'request_max_accounts': max_accounts,
                    'minimum_output': int(response['otherAmountThreshold']),
                    'labels': [x['swapInfo']['label'] for x in response['routePlan']],
                    'route_plan': response['routePlan'], 'total_metas': len(metas), 'route_data_bytes': len(data),
                    'checks': checks, 'envelope_pass': all(checks.values()), 'cpi_execution_qualified': False,
                    'history_qualified': False, 'blockhash_metadata': response.get('blockhashWithMetadata'),
                    'response_file': name+'.json', 'response_sha256': sha((self.output/(name+'.json')).read_bytes())}
        record(self.output/(name+'.analysis.json'), analysis)
        return response, analysis

    def adaptive_quote(self, name, mint, amount, side, binding, dexes, budgets):
        for budget in budgets:
            attempt = name+'-accounts'+str(budget)
            try:
                response, analysis = self.quote(attempt, mint, amount, side, binding, dexes, budget)
            except ValueError:
                body = self.output/(attempt+'.json')
                receipt = self.output/(attempt+'.receipt.json')
                if (body.exists() and receipt.exists() and json.loads(receipt.read_bytes()).get('status') == 400
                        and json.loads(body.read_bytes()).get('error') == 'No routes found'):
                    continue
                raise
            if analysis['envelope_pass']:
                return response, analysis
        raise ValueError('No compatible quote under the recorded account-budget fallback')

    def route_accounts(self, responses):
        keys = {JUP, JUP_PD, TOKEN, TOKEN22, WSOL}
        for response in responses:
            keys.update(x['pubkey'] for x in response['swapInstruction']['accounts'])
            keys.update(response.get('addressesByLookupTableAddress') or {})
        # Explicitly non-atomic reads; source slot retained per account.
        for key in keys:
            self.accounts.pop(key, None)
        self.snapshot(keys, 'route-accounts')
        first = responses[0]
        self.dependencies(first['inputMint'] if first['inputMint'] != WSOL else first['outputMint'])
        slot = max(self.account_context.values())
        blocktime = self.rpc('snapshot-blocktime', 'getBlockTime', [slot])
        metas, lookup_checks = [], []
        for response in responses:
            for key, vector in (response.get('addressesByLookupTableAddress') or {}).items():
                account = self.accounts.get(key)
                data = base64.b64decode(account['data'][0], validate=True) if account else b''
                addresses = [b58(data[i:i+32]) for i in range(56, len(data), 32)] if len(data) >= 56 else []
                lookup_checks.append({'key': key, 'context_slot': self.account_context.get(key),
                    'owner_valid': bool(account and account['owner'] == 'AddressLookupTab1e1111111111111111111111111'),
                    'state_valid': len(data) >= 56 and (len(data)-56) % 32 == 0 and int.from_bytes(data[:4], 'little') == 1,
                    'api_vector_matches_chain': vector == addresses,
                    'deactivation_slot': int.from_bytes(data[4:12], 'little') if len(data) >= 56 else None,
                    'last_extended_slot': int.from_bytes(data[12:20], 'little') if len(data) >= 56 else None,
                    'addresses_count': len(addresses)})
        for key, account in self.accounts.items():
            value = {'key': key, 'context_slot': self.account_context[key], 'missing': account is None}
            if account:
                data = base64.b64decode(account['data'][0], validate=True)
                value.update({k: account[k] for k in ['owner', 'lamports', 'executable', 'rentEpoch']})
                value.update({'length': len(data), 'sha256': sha(data)})
                if account['owner'] == LOADER and len(data) >= 45 and int.from_bytes(data[:4], 'little') == 3:
                    elf = data[45:].rstrip(bytes([0]))
                    value.update({'deploy_slot': int.from_bytes(data[4:12], 'little'),
                                  'programdata_length': len(data), 'elf_trimmed_length': len(elf),
                                  'elf_trimmed_sha256': sha(elf), 'elf_padded_sha256': sha(data[45:])})
                if account['executable'] and account['owner'] == LOADER and len(data) == 36:
                    value['programdata'] = b58(data[4:])
            metas.append(value)
        record(self.output/'accounts.snapshot.json', {'scope': 'candidate-only non-atomic mainnet snapshot; no promotion',
               'snapshot_slot': slot, 'snapshot_block_time': blocktime, 'accounts': self.accounts,
               'account_context_slots': self.account_context})
        record(self.output/'accounts.manifest.json', {'snapshot_slot': slot, 'snapshot_block_time': blocktime,
               'accounts': metas, 'lookup_table_checks': lookup_checks})
        return slot, blocktime

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--mint', required=True)
    parser.add_argument('--quantity', required=True, type=int)
    parser.add_argument('--binding', type=Path, help='Historical explicit binding, mutually exclusive with --program-id derivation')
    parser.add_argument('--program-id', help='Explicit candidate production program identity; derive fresh loan/scratch PDAs')
    parser.add_argument('--borrower', default='3wpYnGqceZ8DzN3guiTd9rrYkWTwTHCChBSuo6cvkXTG', help='Public fixed seed-100 runtime borrower, or explicit public borrower')
    parser.add_argument('--pool-generation', type=int, default=0)
    parser.add_argument('--loan-nonce', type=int, default=0)
    selection = parser.add_mutually_exclusive_group()
    selection.add_argument('--dexes')
    selection.add_argument('--exclude-dexes')
    parser.add_argument('--max-accounts', type=int, default=32)
    parser.add_argument('--route-budgets', help='Explicit deterministic per-quote fallback, e.g. 16,32')
    parser.add_argument('--capture-ineligible', action='store_true', help='Diagnostic quotes only; never claims mint admission')
    parser.add_argument('--buy-markup-bps', type=int, default=200)
    parser.add_argument('--max-buy-lamports', type=int, default=1_000_000_000)
    parser.add_argument('--ask-requests', type=int, default=5)
    parser.add_argument('--production-observation', action='store_true', help='Start at net bid, then cap growth at the signed production spread; never over-budget doubling')
    parser.add_argument('--max-spread-bps', type=int, default=500)
    args = parser.parse_args()
    budgets = [int(x) for x in args.route_budgets.split(',')] if args.route_budgets else [args.max_accounts]
    if bool(args.binding) == bool(args.program_id):
        raise ValueError('Choose exactly one explicit binding file or program-ID derivation')
    binding = (json.loads(args.binding.read_bytes()) if args.binding else
               derive_binding(args.program_id, args.mint, args.borrower, args.pool_generation, args.loan_nonce))
    if (not 0 < args.quantity < 2**64 or not 1 <= args.ask_requests <= 12
            or not budgets or len(budgets) > 2 or any(not 1 <= x <= 32 for x in budgets)
            or len(set(budgets)) != len(budgets) or not 0 <= args.buy_markup_bps <= 1000
            or not 0 <= args.max_spread_bps <= 500 or not 0 <= args.pool_generation < 2**64 or not 0 <= args.loan_nonce < 2**64):
        raise ValueError('bounded positive raw quantity/ask requests required')
    capture = Capture(args.output)
    record(args.output/'public-binding.json', binding)
    capture.exclude_dexes = args.exclude_dexes
    capture.dependencies(args.mint)
    mint = capture.accounts[args.mint]
    data = base64.b64decode(mint['data'][0], validate=True) if mint else b''
    eligible = bool(mint and mint['owner'] in [TOKEN, TOKEN22] and not mint['executable'] and len(data) == 82
                    and data[45] == 1 and data[46:50] == bytes(4) and args.mint != WSOL)
    record(args.output/'mint-eligibility.json', {'mint': args.mint, 'eligible': eligible,
           'owner': mint['owner'] if mint else None, 'length': len(data),
           'decimals': data[44] if len(data) == 82 else None,
           'initialized': data[45] if len(data) == 82 else None,
           'freeze_option': int.from_bytes(data[46:50], 'little') if len(data) == 82 else None,
           'context_slot': capture.account_context[args.mint], 'sha256': sha(data)})
    if not eligible and not args.capture_ineligible:
        raise ValueError('mint fails current program eligibility')
    sell, sale = capture.adaptive_quote('sell', args.mint, args.quantity, 'sell', binding, args.dexes, budgets)
    spread_cap = min(args.max_buy_lamports, sale['minimum_output']*(10_000+args.max_spread_bps)//10_000)
    initial = sale['minimum_output'] if args.production_observation else max(1, (sale['expected_output']*(10_000+args.buy_markup_bps)+9999)//10_000)
    responses, budget, best = [sell], min(initial, spread_cap), None
    for i in range(args.ask_requests):
        budget = min(budget, spread_cap)
        buy, analysis = capture.adaptive_quote('buy-'+str(i), args.mint, budget, 'buy', binding, args.dexes, budgets)
        responses.append(buy)
        if analysis['minimum_output'] >= args.quantity and analysis['envelope_pass']:
            best = analysis
            break
        if budget == spread_cap:
            break
        budget = min(spread_cap, budget*2)
    slot, blocktime = capture.route_accounts(responses)
    outcome = {'scope': 'read-only candidate; no financial CPI or history qualification', 'captured_at': now(),
               'mint': args.mint, 'quantity': args.quantity, 'binding': binding, 'dexes': args.dexes,
               'program_id': binding.get('program_id'), 'capture_tool_sha256': sha(Path(__file__).read_bytes()),
               'production_observation': args.production_observation, 'max_spread_bps': args.max_spread_bps,
               'request_max_accounts': args.max_accounts, 'buy_markup_bps': args.buy_markup_bps,
               'exclude_dexes': args.exclude_dexes,
               'request_account_budgets': budgets, 'mint_eligible_plain82_rule': eligible,
               'diagnostic_ineligible_capture': args.capture_ineligible and not eligible,
               'snapshot_slot': slot, 'snapshot_block_time': blocktime, 'sell': sale, 'buy': best,
               'whole_q_quote_available': bool(sale['envelope_pass'] and best), 'cpi_execution_qualified': False,
               'history_qualified': False, 'buy_search': 'first successful bounded ExactIn marked-up sale-output budget, doubling; not cheapest ask proof',
               'requests': capture.files}
    outcome['net_bid_lamports'] = sale['minimum_output']
    outcome['whole_q_buy_lamports'] = best['request_amount'] if best else None
    outcome['spread_bps_ceil'] = ((best['request_amount']-sale['minimum_output'])*10_000+sale['minimum_output']-1)//sale['minimum_output'] if best else None
    outcome['within_500bps_spread'] = bool(best and best['request_amount']*10_000 <= sale['minimum_output']*10_500)
    record(args.output/'qualification.json', outcome)
    print(json.dumps({k: outcome[k] for k in ['mint', 'quantity', 'snapshot_slot', 'whole_q_quote_available', 'cpi_execution_qualified']}))
    print('sell', sale['labels'], sale['total_metas'], 'buy', best['labels'] if best else None)

if __name__ == '__main__':
    main()
