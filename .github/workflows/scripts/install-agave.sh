#!/usr/bin/env bash
# Install the exact Agave release used by CI without executing or extracting mutable downloads.
set -euo pipefail

: "${AGAVE_VERSION:?AGAVE_VERSION is required}"
: "${AGAVE_INSTALL_INIT_SHA256:?AGAVE_INSTALL_INIT_SHA256 is required}"
: "${AGAVE_RELEASE_SHA256:?AGAVE_RELEASE_SHA256 is required}"
: "${RUNNER_TEMP:?RUNNER_TEMP is required}"

if [[ $(uname -s) != Linux || $(uname -m) != x86_64 ]]; then
  echo "error: pinned Agave assets require Linux x86_64" >&2
  exit 1
fi

base="https://github.com/anza-xyz/agave/releases/download/v${AGAVE_VERSION}"
installer="$RUNNER_TEMP/agave-install-init-${AGAVE_VERSION}"
archive="$RUNNER_TEMP/solana-release-${AGAVE_VERSION}.tar.bz2"
curl_args=(--fail --silent --show-error --location --retry 3 --retry-all-errors \
  --proto '=https' --proto-redir '=https')

curl "${curl_args[@]}" --output "$installer" \
  "$base/agave-install-init-x86_64-unknown-linux-gnu"
curl "${curl_args[@]}" --output "$archive" \
  "$base/solana-release-x86_64-unknown-linux-gnu.tar.bz2"
printf '%s  %s\n' "$AGAVE_INSTALL_INIT_SHA256" "$installer" | sha256sum --check --strict
printf '%s  %s\n' "$AGAVE_RELEASE_SHA256" "$archive" | sha256sum --check --strict

install_root="$HOME/.local/share/solana/install"
release_dir="$install_root/releases/$AGAVE_VERSION"
if [[ -e "$release_dir" ]]; then
  echo "error: refusing to replace unexpected Agave release directory: $release_dir" >&2
  exit 1
fi

stage=$(mktemp -d "$RUNNER_TEMP/agave-release.XXXXXX")
tar -xjf "$archive" -C "$stage"
[[ -x "$stage/solana-release/bin/agave-install" ]] || {
  echo "error: pinned Agave archive lacks solana-release/bin/agave-install" >&2
  exit 1
}
version_file="$stage/solana-release/version.yml"
grep -Fxq "channel: v$AGAVE_VERSION" "$version_file"
grep -Fxq "target: x86_64-unknown-linux-gnu" "$version_file"
mkdir -p "$install_root/releases"
mv "$stage" "$release_dir"

chmod u+x "$installer"
"$installer" --no-modify-path "$AGAVE_VERSION"
