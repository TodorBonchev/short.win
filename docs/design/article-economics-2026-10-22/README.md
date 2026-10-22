# Article economics: current math observations

This small host harness calls actual production `shortwin-math` APIs. It reproduces two claims discussed in the article review:

- One Tier D position keeps its opening coefficients while its actual rent follows utilization from 80% to 90% across two days.
- Expiry rewards, under the current schedule and an explicit 0.01 SOL fixed-payment what-if, remain funded by borrower remainder and conserve the position vault across full, low and zero remaining equity.

The checked [fixture](src/main.rs) is identical to the original successful research harness (SHA256 `f39389ebdb93b43cb42361647f55d5c6f5d30e2922112217e532022b165eea08`). It uses production functions rather than copied equations. [evidence.json](evidence.json) records exact source/tool/fixture hashes, commands, exits and observed cases. [build.log](build.log) and [run.log](run.log) retain the complete successful portable-harness output.

From this directory, with Rust **1.98.1 already installed**, reproduce without downloading dependencies or placing build output in the repository:

```bash
math_check_dir="$(mktemp -d /tmp/shortwin-article-math.XXXXXX)"
mkdir "$math_check_dir/cargo-home"
math_cargo="$(rustup which --toolchain 1.98.1-x86_64-unknown-linux-gnu cargo)"
math_rustc="$(rustup which --toolchain 1.98.1-x86_64-unknown-linux-gnu rustc)"
timeout --signal=TERM --kill-after=5s 120s env -i \
  PATH=/usr/bin:/bin CARGO_HOME="$math_check_dir/cargo-home" \
  RUSTC="$math_rustc" CARGO_TARGET_DIR="$math_check_dir/target" \
  CARGO_BUILD_JOBS=2 CARGO_NET_OFFLINE=true \
  "$math_cargo" build --offline --locked --jobs 2 --manifest-path Cargo.toml
timeout --signal=TERM --kill-after=5s 120s env -i PATH=/usr/bin:/bin \
  "$math_check_dir/target/debug/article-current-math-observation"
```

This directory has its own `[workspace]` and portable path dependency. The repository workspace members and root lockfile are untouched. No target binaries or caches are retained here. Remove the temporary directory when finished.

The observations use constant EMA price, a single copied math position, and declared waterfall inputs. They establish host-library behavior and math value-copy independence; they do **not** execute program account snapshots, governance updates, SBF, venue CPIs, deployment state, congestion or live keeper economics. The higher expiry payment is a sensitivity example, not a selected default or recommendation. Root documentation/comment edits during the review are separate from these checks.
