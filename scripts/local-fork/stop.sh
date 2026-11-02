#!/usr/bin/env bash
# Stop exactly the three demo units; preserve the ledger and unrelated work.
set -euo pipefail
for role in web bridge validator; do
  unit="shortwin-local-fork-$role.service"
  if [[ "$(systemctl --user show "$unit" -p LoadState --value 2>/dev/null || true)" != "not-found" ]]; then
    systemctl --user stop "$unit"
  fi
done
echo "Local demo services stopped. The ledger and local positions are preserved."
