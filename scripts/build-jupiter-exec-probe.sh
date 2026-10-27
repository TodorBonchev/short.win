#!/usr/bin/env bash
# TEST-ONLY standalone SBF probe; never changes target/deploy or the Anchor program list.
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
probe_target=${SHORTWIN_JUPITER_PROBE_TARGET:-$root/target/jupiter-exec-probe}
mkdir -p "$probe_target/deploy"
CARGO_TARGET_DIR="$probe_target/sbf" cargo build-sbf \
  --manifest-path "$root/tests/jupiter_exec_probe/Cargo.toml" \
  --tools-version v1.57 --arch v3 --jobs "${CARGO_BUILD_JOBS:-2}" \
  --sbf-out-dir "$probe_target/deploy" -- --locked
python3 - "$root" "$probe_target" <<'PY'
from pathlib import Path
import hashlib,json,subprocess,sys
root=Path(sys.argv[1]);out=Path(sys.argv[2]);so=out/'deploy/shortwin_jupiter_exec_probe.so'
data=so.read_bytes();assert data[:4]==b'\x7fELF' and int.from_bytes(data[48:52],'little')==3
sources=['tests/jupiter_exec_probe/Cargo.toml','tests/jupiter_exec_probe/Cargo.lock','tests/jupiter_exec_probe/src/lib.rs','tests/jupiter_exec_probe/src/wire.rs','crates/shortwin-jupiter-route/src/lib.rs','crates/shortwin-jupiter-route/src/isolation.rs','scripts/materialize-jupiter-probe.py','tests/jupiter_exec_probe/fixtures/jupiter-2026-10-27/identity.json','tests/jupiter_exec_probe/fixtures/jupiter-2026-10-27/getMultipleAccounts.json.gz']
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
record={'scope':'TEST-ONLY hostile SBF scratch CPI proof; not Jupiter execution or financial authorization','git_head':subprocess.check_output(['git','-C',str(root),'rev-parse','HEAD'],text=True).strip(),'working_tree_status':subprocess.check_output(['git','-C',str(root),'status','--porcelain'],text=True),'platform_tools':'v1.57','sbpf_arch':'v3','artifact':str(so),'artifact_sha256':sha(so),'artifact_bytes':len(data),'source_sha256':{p:sha(root/p)for p in sources}}
(out/'build-identity.json').write_text(json.dumps(record,indent=2)+'\n');print(json.dumps(record,indent=2))
PY

python3 "$root/scripts/materialize-jupiter-probe.py" "$probe_target/jupiter-candidate"
