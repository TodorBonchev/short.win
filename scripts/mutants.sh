#!/usr/bin/env bash
# Mutation testing of the program's open and coverage paths (M7-SEC-6): each mutant below is a
# one-line change the M7 security review used to show an oracle was missing. The script builds the
# ORACLE once from this checkout — the LiteSVM test binaries and the fuzzer, whose hooks (I1–I19)
# recompute from `shortwin-math` and the harness's own venue views — and then, per mutant, only the
# PROGRAM from a private copy with the change applied (the test-venue build for the mock oracles,
# the release build for the fork ones). It expects the named tests AND a short fuzz run of the
# named profile to FAIL against the mutant build: a mutant every check survives means an oracle is
# missing again.
#
#   scripts/mutants.sh [--fuzz 3m] [--work DIR] [--keep] [mutant ...]     (default: every mutant)
#
# Environment: SHORTWIN_SO / SHORTWIN_TEST_VENUE_SO / MOCK_CPAMM_SO / COMPLETE_CURVE_SO for the
# unmutated builds the fork tests also load (default: target/deploy, target/test-venue/deploy);
# MUTANTS_JOBS (4), MUTANTS_SEED (20261001).
# Exit status: 0 when every selected mutant is killed by its tests and by the fuzzer, 1 otherwise.
# Pinned toolchain as everywhere (anchor 1.2.0, platform-tools v1.57, SBPF v3; docs/DEVELOPMENT.md).
set -uo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
fuzz=3m
work=${MUTANTS_WORK:-$root/target/mutants}
keep=0
sel=()
while [ $# -gt 0 ]; do
  case "$1" in
    --fuzz) fuzz=$2; shift 2 ;;
    --work) work=$2; shift 2 ;;
    --keep) keep=1; shift ;;
    -h|--help) sed -n '2,19p' "$0"; exit 0 ;;
    *) sel+=("$1"); shift ;;
  esac
done

# add NAME BUILD FILE SUBSTITUTION TESTS PROFILE — BUILD: tv (test-venue) or rel (release); the
# perl substitution runs with -0 (whole file); TESTS: the litesvm test targets that must fail.
names=(); builds=(); files=(); substs=(); testsets=(); profiles=()
add() { names+=("$1"); builds+=("$2"); files+=("$3"); substs+=("$4"); testsets+=("$5"); profiles+=("$6"); }
add d4-without-ema tv programs/shortwin/src/instructions/open_short.rs \
  's/risk::margin_d4\(s, n_open, b_ema, tp\.cr_open_bps\)/risk::margin_d4(s, n_open, None, tp.cr_open_bps)/' \
  "invariants_m2_hooks caps_boundary_it" mock
add fee-size-cap-doubled tv programs/shortwin/src/engine/caps.rs \
  's/caps::fee_size_cap\(x0, inp\.phi_ppm, params\.fee_size_cap_x100\)/caps::fee_size_cap(x0, inp.phi_ppm, params.fee_size_cap_x100.saturating_mul(2))/' \
  "caps_boundary_it" mock
add wallet-cap-removed tv programs/shortwin/src/engine/caps.rs \
  's/caps::wallet_ok\(inp\.wallet_q, q, oi_cap, params\.wallet_cap_bps\)\.math\(\)\?/true || caps::wallet_ok(inp.wallet_q, q, oi_cap, params.wallet_cap_bps).math()?/' \
  "caps_boundary_it" mock
add tier-c-as-b tv programs/shortwin/src/engine/tier.rs \
  's/\.map\(\|i\| tier::assign\(&i\)\)/.map(|i| match tier::assign(&i) { Tier::C => Tier::B, t => t })/' \
  "tier_transition_it" mock
add coverage-clear-immediate tv programs/shortwin/src/engine/guard.rs \
  's/&& now >= clear_delay_ends,/\&\& now >= clear_delay_ends.min(0),/' \
  "m7_sec_it coverage_pause_it" mock
add curve-caps-live rel programs/shortwin/src/engine/caps.rs \
  's/if p_ema <= c\.p_spot \{/if true {/' \
  "m5_sec_curve_it" curve
add pump-caps-live-fee rel programs/shortwin/src/instructions/open_short.rs \
  's/phi_ppm: state\.cap_fee_ppm\(\),\n(\s+)tokens_free/phi_ppm: state.fee_ppm,\n$1tokens_free/' \
  "m7_sec_it" pumpswap

tools=${PLATFORM_TOOLS:-v1.57}
arch=${SBPF_ARCH:-v3}
mkdir -p "$work"
clean_so=${SHORTWIN_SO:-$root/target/deploy/shortwin.so}
clean_tv=${SHORTWIN_TEST_VENUE_SO:-$root/target/test-venue/deploy/shortwin.so}
mock_so=${MOCK_CPAMM_SO:-$root/target/deploy/mock_cpamm.so}
curve_so=${COMPLETE_CURVE_SO:-$root/target/deploy/complete_curve.so}

# The oracle, once: every test binary the mutants name, and the fuzzer.
oracle="$work/oracle"
all_tests=()
for tests in "${testsets[@]}"; do
  for x in $tests; do
    [[ " ${all_tests[*]} " =~ " --test $x " ]] || all_tests+=(--test "$x")
  done
done
echo "=== building the oracle (tests and fuzzer) in $oracle"
# Host builds without the deploy profile's fat LTO (the oracle needs speed, not link time; the
# program builds below keep the pinned profile).
host_env=(CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 CARGO_TARGET_DIR="$oracle")
(cd "$root" && env "${host_env[@]}" cargo test --release --locked -p shortwin-litesvm-tests \
  "${all_tests[@]}" --no-run >"$work/oracle-build.log" 2>&1) || { echo "error: the oracle tests do not build (see $work/oracle-build.log)" >&2; exit 1; }
(cd "$root" && env "${host_env[@]}" cargo build --release --locked -p shortwin-fuzz \
  >>"$work/oracle-build.log" 2>&1) \
  || { echo "error: the fuzzer does not build (see $work/oracle-build.log)" >&2; exit 1; }

fail=0
summary=()
for i in "${!names[@]}"; do
  name=${names[$i]} build=${builds[$i]} file=${files[$i]} subst=${substs[$i]}
  tests=${testsets[$i]} profile=${profiles[$i]}
  if [ ${#sel[@]} -gt 0 ] && [[ ! " ${sel[*]} " =~ " $name " ]]; then
    continue
  fi
  echo "=== mutant $name ($file)"
  dir="$work/$name"
  rm -rf "$dir/repo"
  mkdir -p "$dir"
  rsync -a --exclude target --exclude node_modules --exclude .git --exclude keys "$root/" "$dir/repo/"
  before=$(sha256sum "$dir/repo/$file" | cut -d' ' -f1)
  perl -0pi -e "$subst" "$dir/repo/$file"
  if [ "$(sha256sum "$dir/repo/$file" | cut -d' ' -f1)" = "$before" ]; then
    echo "error: the substitution of $name no longer applies to $file" >&2
    summary+=("$name: STALE (the mutation no longer applies)")
    fail=1
    continue
  fi
  t="$dir/target"
  (
    cd "$dir/repo" || exit 1
    if [ "$build" = tv ]; then
      SHORTWIN_TV_TARGET="$t/tv" scripts/build-test-venue.sh >"$dir/build.log" 2>&1
    else
      CARGO_TARGET_DIR="$t/sbf" anchor build --tools-version "$tools" --arch "$arch" --ignore-keys \
        --no-idl -p shortwin >"$dir/build.log" 2>&1
    fi
  ) || { echo "error: $name does not build (see $dir/build.log)" >&2; summary+=("$name: BUILD FAILED"); fail=1; continue; }
  if [ "$build" = tv ]; then
    so_rel="$clean_so"; so_tv="$t/tv/deploy/shortwin.so"
  else
    so_rel="$t/sbf/deploy/shortwin.so"; so_tv="$clean_tv"
  fi
  args=()
  for x in $tests; do args+=(--test "$x"); done
  (cd "$root" && env "${host_env[@]}" SHORTWIN_SO="$so_rel" SHORTWIN_TEST_VENUE_SO="$so_tv" \
    MOCK_CPAMM_SO="$mock_so" COMPLETE_CURVE_SO="$curve_so" \
    cargo test --release --locked -p shortwin-litesvm-tests "${args[@]}" >"$dir/tests.log" 2>&1)
  tests_rc=$?
  (cd "$root" && SHORTWIN_SO="$so_rel" SHORTWIN_TEST_VENUE_SO="$so_tv" MOCK_CPAMM_SO="$mock_so" \
    COMPLETE_CURVE_SO="$curve_so" "$oracle/release/shortwin-fuzz" run --profile "$profile" \
    --duration "$fuzz" --jobs "${MUTANTS_JOBS:-4}" --seed "${MUTANTS_SEED:-20261001}" \
    --out "$dir/fuzz" >"$dir/fuzz.log" 2>&1)
  fuzz_rc=$?
  failures=$(grep -oE 'failures [0-9]+' "$dir/fuzz.log" | tail -1 | awk '{print $2}')
  if [ $tests_rc -ne 0 ]; then t_state=killed; else t_state=SURVIVED; fi
  if [ "${failures:-0}" != 0 ] || [ $fuzz_rc -ne 0 ]; then
    f_state="killed (${failures:-?} failures)"
  else
    f_state=SURVIVED
  fi
  echo "$name: tests $t_state, fuzz ($profile, $fuzz) $f_state"
  summary+=("$name: tests $t_state, fuzz ($profile, $fuzz) $f_state")
  if [ "$t_state" = SURVIVED ] || [ "$f_state" = SURVIVED ]; then
    fail=1
  fi
  [ "$keep" = 1 ] || rm -rf "$t" "$dir/repo"
done
echo "=== mutation summary"
printf '%s\n' "${summary[@]}"
exit $fail
