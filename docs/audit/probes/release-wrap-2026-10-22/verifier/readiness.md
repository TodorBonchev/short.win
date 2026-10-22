# Official Docker verifier availability

Checked 2026-10-22 against `/home/todor_dev/Projects/short.win-release-devnet`, source HEAD `bf166672efee578b6a2a1835e0dd53729c548671`. Pending tracked edits were documentation only (`TODO.md`, `docs/AUDIT.md`, `docs/audit/README.md`); deployment/release docs were also untracked. No program input was edited by this task.

**Blocked before build:** official Docker CLI is available, but this user cannot access the official daemon socket. Both ordinary daemon/version and exact cached-image inspection exited 1 with `permission denied while trying to connect to the docker API at unix:///var/run/docker.sock`. Socket mode is `srw-rw---- root docker`; current user's groups are `todor_dev chrome-remote-desktop wheel`, without `docker`. Root then explicitly authorized one bounded noninteractive sudo probe of version and exact image inspection. Both also exited 1 with `sudo: a password is required`; no privileged Docker operation ran. Daemon running state and cached image availability therefore remain unestablished, not disproved.

No image pull, `scripts/verifiable-build.sh` execution, Docker restart/configuration change, new install, group change, shim/local mode or alternative image execution was attempted. No password was requested or read; no privileged operation succeeded. No signing, key/`.env` reads, deployment, faucet, stopped fixtures or excluded server review/tests occurred. This follows root's stop condition when official Docker is unavailable to the session; no further sudo attempt or fallback follows.

## Exact attempted commands

```sh
NO_DNA=1 timeout 20s /usr/bin/docker version
NO_DNA=1 timeout 20s /usr/bin/docker image inspect \
  'solanafoundation/solana-verifiable-build@sha256:2e0b78f44ee76612e9260c7c988570c5e14de6fbd93e0ab07115ec7054473b4f' \
  --format '{{json .RepoDigests}} {{.Id}} {{.Size}}'
NO_DNA=1 timeout 10s solana-verify --version
NO_DNA=1 timeout 10s solana-verify get-executable-hash \
  target/program-readiness-2026-10-22/deploy/shortwin.so
NO_DNA=1 timeout 20s sudo -n /usr/bin/docker version
NO_DNA=1 timeout 20s sudo -n /usr/bin/docker image inspect \
  'solanafoundation/solana-verifiable-build@sha256:2e0b78f44ee76612e9260c7c988570c5e14de6fbd93e0ab07115ec7054473b4f' \
  --format '{{json .RepoDigests}} {{.Id}} {{.Size}}'
```

Complete outputs are retained as `docker-version.log`, `docker-pinned-image.log`, `solana-verify-version.log`, `candidate-executable-hash.log`, `docker-sudo-version.log` and `docker-sudo-pinned-image.log`.

## Pins and observed identities

* Official Docker CLI `/usr/bin/docker`: version 29.9.1, API 1.56, Git commit `dfc520dcc2`, Linux/amd64; file SHA-256 `566a4e83c1f148720e9628fbee98afcbe00867f0bfec819786f900c40f90d41a`.
* Installed `/home/todor_dev/.cargo/bin/solana-verify`: reports version 0.5.2; file SHA-256 `8494a8feb2e3de0d9e4fa089dd1c208d5aa51a2db5c9ae062964fbbb61ee5ab9`. Version/file identity is observed; this task has not independently established its full installation/source provenance.
* Existing repository `scripts/verifiable-build.sh` Git blob `ad389adae804481c685407cb71457fa451e75ddd` pins solana-verify 0.5.2, image repository `solanafoundation/solana-verifiable-build`, digest `sha256:2e0b78f44ee76612e9260c7c988570c5e14de6fbd93e0ab07115ec7054473b4f`, platform-tools v1.57 and SBF v3. Root manifest has `[workspace.metadata.cli] solana = "4.1.2"`.
* Script default is the real-Docker path; `--out DIR` retains hashes/build logs/output; `--compare DIR` demands byte identity with `DIR/shortwin.so`. Default builds use checkout `target/deploy` and perform a postbuild ownership repair, so final build coordination must account for that output; `--out` alone does not isolate all compiler output. `--local` is explicitly excluded for this task.
* Existing fresh candidate release ELF at `target/program-readiness-2026-10-22/deploy/shortwin.so`: 1,644,024 bytes; SHA-256 `d8d08784b3b3081d5fe1cf7aaff319e7e9e063733c308bd2e9e7ed8981bee182`; executable hash from installed solana-verify `8531f4958aaff2c5050d478ec8752a7b947e70224da3d0a08e27dedfa5107112`.
* Earlier retained canonical release ELF SHA-256 remains `d6f4e092dd15639c502535d68bcdb6c428c423b66d49d08f4f89e9409af538bc`. Program reviewer attributes current difference to one Anchor error-origin line-number byte following the article rustdoc line shift. It is not byte-identical and cannot inherit that canonical artifact's verifier/fuzz identity.

There is **no new independently reproduced verifier ELF**, no matched pinned-image receipt from this session, and no exact installed Anchor-source provenance closure. Old pinned CI verifier evidence remains attributable only to its recorded source/artifacts. A daemon socket and locally installed verifier version do not close the candidate's pending standard pinned verification gate.
