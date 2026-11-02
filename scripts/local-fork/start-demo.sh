#!/usr/bin/env bash
# Start the loopback demo stack, or reuse verified foreground/service instances.
set -euo pipefail
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
output="$root/target/local-fork"
apps_only=false
case "${1:-}" in
  "") ;;
  --apps-only) apps_only=true ;;
  -h|--help)
    echo "Usage: $0 [--apps-only]"
    echo "Default: start/resume the validator, bootstrap as needed, then start bridge/web."
    echo "--apps-only: require existing verified bootstrap; start/reuse only bridge/web."
    exit 0 ;;
  *) echo "Unknown argument: $1" >&2; exit 2 ;;
esac
if (( $# > 1 )); then echo "Expected at most one argument" >&2; exit 2; fi
for executable in systemd-run systemctl python3 node flock ss; do
  command -v "$executable" >/dev/null || { echo "Required command missing: $executable" >&2; exit 1; }
done
node="$(command -v node)"
mkdir -p -- "$output"
exec 8>"$output/demo-start.lock"
flock -n 8 || { echo "Another demo startup is in progress" >&2; exit 1; }

if ! "$apps_only"; then
  bash "$root/scripts/local-fork/start-validator.sh"
fi
if [[ -f "$output/bootstrap.json" ]] || "$apps_only"; then
  "$node" "$root/scripts/local-fork/bootstrap.mts" --verify-only
else
  "$node" "$root/scripts/local-fork/bootstrap.mts"
fi

listener_state() {
  python3 - "$1" <<'PY'
import subprocess, sys
port = int(sys.argv[1])
lines = subprocess.check_output(["ss", "-H", "-ltn", f"sport = :{port}"], text=True).splitlines()
if any(line.split()[3] != f"127.0.0.1:{port}" for line in lines):
    raise SystemExit(f"Port {port} has a listener outside 127.0.0.1; leaving it untouched")
print("occupied" if lines else "free")
PY
}

verify_component() {
  python3 - "$root" "$1" <<'PY'
import json, sys, urllib.request
from pathlib import Path
root, component = Path(sys.argv[1]), sys.argv[2]
setup = json.loads((root / "target/local-fork/bootstrap.json").read_text())
class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise ValueError("Local demo HTTP redirects are forbidden")
opener = urllib.request.build_opener(NoRedirect)
def get(url):
    with opener.open(url, timeout=3) as response:
        return response.read().decode()
if component == "bridge":
    info = json.loads(get("http://127.0.0.1:8787/demo/info"))
    expected = {"mode": "local-fork", "testFunds": True, "syntheticHistory": True,
                "ready": True, "genesis": setup["genesisHash"], "mint": setup["mint"],
                "program": setup["program"], "wallet": setup["roles"]["user"]}
    if any(info.get(key) != value for key, value in expected.items()):
        raise SystemExit("Existing bridge does not match this verified local demo")
else:
    html = get("http://127.0.0.1:5173")
    if "short.win" not in html or "/src/main.ts" not in html:
        raise SystemExit("Port 5173 does not serve the expected short.win Vite app")
    source = get("http://127.0.0.1:5173/src/main.ts")
    if not source.startswith("import.meta.env = "):
        raise SystemExit("Cannot verify the existing Vite demo environment")
    environment, _ = json.JSONDecoder().raw_decode(source.split("=", 1)[1].lstrip())
    expected = {"VITE_ENABLE_LOCAL_DEMO": "true", "VITE_CLUSTER": "localnet",
                "VITE_RPC_URL": "http://127.0.0.1:8899",
                "VITE_SERVICE_URL": "http://127.0.0.1:8787", "VITE_DEMO_MINT": setup["mint"]}
    if any(environment.get(key) != value for key, value in expected.items()):
        raise SystemExit("Existing web app is not configured for this loopback demo")
PY
}

wait_component() {
  local role="$1" deadline=$((SECONDS + 45))
  while (( SECONDS < deadline )); do
    if verify_component "$role" >/dev/null 2>&1; then return 0; fi
    sleep 1
  done
  verify_component "$role"
}

for role in bridge web; do
  unit="shortwin-local-fork-$role.service"
  port=8787
  if [[ "$role" == web ]]; then port=5173; fi
  occupied="$(listener_state "$port")"
  if [[ "$occupied" == occupied ]]; then
    verify_component "$role"
    if systemctl --user is-active --quiet "$unit"; then
      echo "Reusing active $unit on 127.0.0.1:$port."
    else
      echo "Reusing verified foreground $role on 127.0.0.1:$port; no duplicate service created."
      echo "Stop its original terminal/session with Ctrl-C when finished."
    fi
    continue
  fi
  if systemctl --user is-active --quiet "$unit"; then
    wait_component "$role"
    listener_state "$port" >/dev/null
    echo "Existing $unit is ready."
    continue
  fi
  systemctl --user reset-failed "$unit" >/dev/null 2>&1 || true
  common=(--user --collect --unit "$unit" --property=Restart=no
    --property=MemoryMax=1G --property=KillSignal=SIGINT --property=TimeoutStopSec=20)
  if [[ "$role" == bridge ]]; then
    systemd-run "${common[@]}" --description='Shortwin loopback Jupiter demo bridge' \
      --working-directory="$root" \
      --property="StandardOutput=append:$output/server.log" \
      --property="StandardError=append:$output/server.log" \
      --setenv=NODE_OPTIONS=--max-old-space-size=512 \
      "$node" "$root/scripts/local-fork/server.mts"
  else
    test -f "$root/web/node_modules/vite/bin/vite.js" || {
      echo "Install the workspace dependencies before starting Vite." >&2; exit 1;
    }
    systemd-run "${common[@]}" --description='Shortwin loopback demo web app' \
      --working-directory="$root/web" \
      --property="StandardOutput=append:$output/web.log" \
      --property="StandardError=append:$output/web.log" \
      --setenv=NODE_OPTIONS=--max-old-space-size=768 \
      --setenv=VITE_ENABLE_LOCAL_DEMO=true --setenv=VITE_CLUSTER=localnet \
      --setenv=VITE_RPC_URL=http://127.0.0.1:8899 \
      --setenv=VITE_SERVICE_URL=http://127.0.0.1:8787 \
      --setenv=VITE_DEMO_MINT=Dz9mQ9NzkBcCsuGPFJ3r1bS4wgqKMHBPiVuniW8Mbonk \
      "$node" "$root/web/node_modules/vite/bin/vite.js" --host 127.0.0.1 --port 5173 --strictPort
  fi
  wait_component "$role"
  listener_state "$port" >/dev/null
  systemctl --user show "$unit" -p ExecStart -p Environment -p MemoryMax -p InvocationID -p MainPID >"$output/$role-command.txt"
done
echo "Local demo ready: http://127.0.0.1:5173"
