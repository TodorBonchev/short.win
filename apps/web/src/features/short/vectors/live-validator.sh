#!/usr/bin/env bash
# OPTIONAL: a private solana-test-validator for `src/features/short/live.validator.test.ts` — the
# TEST-ONLY test-venue build of short.win (scripts/build-test-venue.sh), the mock venue
# (target/deploy/mock_cpamm.so, `anchor build`) and the market of `program-vectors.json` case
# `spl-open-0` loaded as accounts (config, venue guard, oracle cache, market, vaults, mint, mock pool).
# The ledger lives in apps/web/.e2e/short-live (gitignored; never /tmp). Ports 38899 / 38900 / 39900
# by default, apart from a developer validator (8899) and the E2E one (18899).
#
#   apps/web/src/features/short/vectors/live-validator.sh &
#   SHORTWIN_LIVE_RPC=http://127.0.0.1:38899 pnpm --filter web exec vitest run src/features/short/live.validator.test.ts
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../../../.." && pwd)
dir=${SHORTWIN_LIVE_DIR:-$root/apps/web/.e2e/short-live}
port=${SHORTWIN_LIVE_PORT:-38899}
bin=${SOLANA_BIN:-$HOME/.local/share/solana/install/active_release/bin}
so=${SHORTWIN_TEST_VENUE_SO:-$root/target/test-venue/deploy/shortwin.so}
mock=${MOCK_CPAMM_SO:-$root/target/deploy/mock_cpamm.so}

mkdir -p "$dir/accounts"
python3 - "$root/apps/web/src/features/short/vectors/program-vectors.json" "$dir/accounts" <<'PY'
import base64, json, sys
cases = json.load(open(sys.argv[1]))["cases"]
c = next(x for x in cases if x["name"] == "spl-open-0")
for key, acc in c["accounts"].items():
    if key == c["borrower"]:
        continue
    data = base64.b64decode(acc["data"])
    json.dump(
        {"pubkey": key, "account": {"lamports": int(acc["lamports"]), "data": [acc["data"], "base64"],
                                    "owner": acc["owner"], "executable": False, "rentEpoch": 0, "space": len(data)}},
        open(f"{sys.argv[2]}/{key}.json", "w"),
    )
PY

"$bin/solana-keygen" new --no-bip39-passphrase -s --force -o "$dir/upgrade-authority.json" >/dev/null
accounts=()
for f in "$dir"/accounts/*.json; do
    accounts+=(--account "$(basename "$f" .json)" "$f")
done
exec "$bin/solana-test-validator" --reset --quiet --ledger "$dir/ledger" --bind-address 127.0.0.1 \
    --rpc-port "$port" --faucet-port $((port + 1001)) --gossip-port $((port - 899)) \
    --dynamic-port-range "$((port - 889))-$((port - 699))" \
    --upgradeable-program 5yvyAddWb8RRGh3pPiFS5eV33eZYeHVmAAfwAFmqVpKK "$so" \
    "$("$bin/solana-keygen" pubkey "$dir/upgrade-authority.json")" \
    --bpf-program BPAZFPkmVtYEWEZHCzFUns7oybh1VWfAvmu3ofM2mQi7 "$mock" \
    "${accounts[@]}"
