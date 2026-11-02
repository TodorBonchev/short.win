#!/usr/bin/env bash
# Start/resume only the isolated local demo validator. Never configure a public RPC.
set -euo pipefail

root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
output="$root/target/local-fork"
unit="shortwin-local-fork-validator.service"
reset=false
case "${1:-}" in
  "") ;;
  --reset-local-state) reset=true ;;
  -h|--help)
    echo "Usage: $0 [--reset-local-state]"
    echo "Default: resume the existing ledger and preserve local loans/deposits."
    echo "--reset-local-state archives local state and requires bootstrap again."
    exit 0 ;;
  *) echo "Unknown argument: $1" >&2; exit 2 ;;
esac
if (( $# > 1 )); then echo "Expected at most one argument" >&2; exit 2; fi

mkdir -p -- "$output"
exec 9>"$output/start.lock"
flock -n 9 || { echo "Another local validator startup is in progress" >&2; exit 1; }

if "$reset"; then
  echo "WARNING: reset discards the active local demo state; archived loans, deposits and balances require a new bootstrap." >&2
  for role in web bridge validator; do
    if systemctl --user is-active --quiet "shortwin-local-fork-$role.service"; then
      echo "Stop the local demo first: bash scripts/local-fork/stop.sh" >&2
      exit 1
    fi
  done
  archive="$output/resets/$(date -u +%Y%m%dT%H%M%SZ)-$$"
  mkdir -p -- "$archive"
  for item in "$output"/*; do
    case "${item##*/}" in
      accounts|bind-loopback.c|bind-loopback.so|fixtures.receipt.json|resets|start.lock) ;;
      *) mv -- "$item" "$archive/" ;;
    esac
  done
  echo "Previous local state archived under $archive"
fi

if systemctl --user is-active --quiet "$unit"; then
  echo "Local validator already running; verifying it without restarting."
  python3 "$root/scripts/local-fork/materialize-fixtures.py" --verify-local-rpc
  systemctl --user show "$unit" -p ExecStart -p Environment -p MemoryMax -p InvocationID -p MainPID >"$output/runtime-command.txt"
  exit 0
fi

for command in systemd-run systemctl python3 cc flock solana-test-validator; do
  command -v "$command" >/dev/null || { echo "Required command missing: $command" >&2; exit 1; }
done
validator="$(command -v solana-test-validator)"
test -f "$root/target/deploy/shortwin_simple.so" || {
  echo "Build target/deploy/shortwin_simple.so before starting the local fork." >&2
  exit 1
}

# Refuse to connect to an unrelated process occupying the local demo ports.
python3 - <<'PY'
import socket
for port in (8899, 8900, 9900):
    with socket.socket() as connection:
        connection.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        try:
            connection.bind(("127.0.0.1", port))
        except OSError as error:
            raise SystemExit(f"Local demo port {port} is already occupied: {error}")
PY

python3 "$root/scripts/local-fork/materialize-fixtures.py" --loopback-shim
options=(
  --ledger "$output/ledger"
  --bind-address 127.0.0.1 --rpc-port 8899
  --mint AKnL4NNf3DGWZJS6cPknBuEGnVsV4A4m5tgebLHaRSZ9
  --account-dir "$output/accounts"
  --upgradeable-program BCPiRMTMoLNY5EHm2RYxoxJChgyrttkFX2Qe2Rnbe3jh
  "$root/target/deploy/shortwin_simple.so" 9hSR6S7WPtxmTojgo6GG3k4yDPecgJY292j7xrsUGWBu
  --dynamic-port-range 10000-10099 --limit-ledger-size 10000 --log
)
if [[ ! -f "$output/ledger/genesis.bin" ]]; then
  options+=(--warp-slot 461997165)
  echo "Creating a fresh local snapshot ledger."
else
  echo "Resuming the existing local ledger; balances and loans are retained."
fi

systemctl --user reset-failed "$unit" >/dev/null 2>&1 || true
systemd-run --user --collect --unit "$unit" \
  --description='Shortwin localhost historical mainnet snapshot fork' \
  --working-directory="$root" \
  --property=MemoryMax=6G --property=Restart=no \
  --property=KillSignal=SIGINT --property=TimeoutStopSec=30 \
  --property="StandardOutput=append:$output/service.log" \
  --property="StandardError=append:$output/service.log" \
  --setenv=NO_DNA=1 --setenv=RAYON_NUM_THREADS=4 --setenv=SOLANA_RAYON_THREADS=4 \
  --setenv=RUST_LOG=warn --setenv="LD_PRELOAD=$output/bind-loopback.so" \
  "$validator" "${options[@]}"

python3 - <<'PY'
import json, time, urllib.request
deadline = time.monotonic() + 60
while True:
    try:
        request = urllib.request.Request("http://127.0.0.1:8899",
            data=b'{"jsonrpc":"2.0","id":1,"method":"getHealth"}',
            headers={"Content-Type": "application/json"})
        with urllib.request.urlopen(request, timeout=2) as response:
            if json.load(response).get("result") == "ok":
                break
    except Exception:
        pass
    if time.monotonic() >= deadline:
        raise SystemExit("Local validator did not become healthy; see target/local-fork/service.log")
    time.sleep(1)
PY
python3 "$root/scripts/local-fork/materialize-fixtures.py" --verify-local-rpc
systemctl --user show "$unit" -p ExecStart -p Environment -p MemoryMax -p InvocationID -p MainPID >"$output/runtime-command.txt"
echo "Local validator ready: http://127.0.0.1:8899 (WebSocket ws://127.0.0.1:8900)"
