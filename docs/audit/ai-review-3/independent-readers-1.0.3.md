# Independent reader and recovery fix review: 1.0.3

Review date: 2026-10-19. Reviewer: independent reader/recovery review agent, separate
from the implementation author and the accounting and candidate-choice reviewers.

Reviewed candidate: `9b910602ed076a6d4359c496960b3ffc0e7c6af8`.
Baseline: `b815269224ba0f5d946f49de7e113220c4067041`.
The review checkout initially contained evidence descendant `b4abcaa6`; integration
advanced HEAD to `708746eb0085016c37f4d73e2aea02031bd6efc9` during the review.
A candidate-to-integrated-HEAD diff of Rust sources under services/common,
services/keeper, services/keeper-litesvm, programs/shortwin and the shortwin crates
was empty. Packaging metadata changes do not change the reviewed reader logic.
No applicable AGENTS.md was present in the checkout.

This review covers packet question 3 (AI3-L03/L04), question 4 (AI3-L01), and the
batching regression affected by earlier rejection of unsupported Market layouts.
It does not accept the separate economic or venue risks listed in the packet.

## Decision

| Item | Decision | Basis |
|---|---|---|
| Q3 / AI3-L03: typed version checks | **Accept** for the current supported layouts. | All production typed GlobalConfig, Market, VenueExtras and VenueGuard decode sites in the keeper were traced; the version is checked before the payload enters the corresponding typed consumer. Existing valid versions remain readable, including list-only markets and VenueExtras with a publication counter unrelated to the layout version. |
| Q3 / AI3-L04: ranking retry | **Accept**. | Queue removal and publication happen after every RPC group has succeeded. First-group and later-group failure tests retain the entire queue; retries do not wait for discovery and include pool 34 in an intact three-account snapshot. |
| Q4 / AI3-L01: truncated ALT metadata | **Accept**. | Every input shorter than the 56-byte metadata boundary is rejected; a complete empty table and populated address arrays still parse. An independently executed old-parser negative control fails at four bytes. |
| Crank batching regression | **Accept**. | A supported-version Market with a corrupted PDA bump reaches the actual on-chain ConstraintSeeds failure, and the two healthy cranks still land. The test no longer depends on an unsupported layout being admitted to the keeper view. |

No new blocker or regression was found in this scope. These are fix-review
decisions, not a claim that the separate 24-hour fuzz gate or overall release
policy is complete.

## Code assessment

* [ALT parser](../../../services/common/src/tx.rs#L294): owner and initialized-state
  tag checks still precede body parsing. Replacing the absent-body fallback with
  an error means lengths 4 through 55 cannot masquerade as an empty table. Length
  56 still yields an empty body, and subsequent bytes must divide into complete
  32-byte addresses. This is the requested truncation fix; it does not add a new
  semantic metadata decoder.
* [GlobalConfig and Market reads](../../../services/keeper/src/view.rs#L265):
  GlobalConfig version 1 is checked before configuration interpretation.
  Market version 1 is checked before venue dispatch and before insertion into
  either the regular or list-only view. An unsupported market is skipped with a
  version-specific reason without losing supported markets in the same read.
* [VenueExtras read](../../../services/keeper/src/view.rs#L380): layout version 2 is
  checked before the boxed extras is published. Current/previous set publication
  versions are separate u32 counters; the regression deliberately preserves
  `u32::MAX` in a valid current set while accepting the account. Unsupported
  extras become unavailable without deleting the market view.
* Both VenueGuard reads are covered:
  [candidate latch](../../../services/keeper/src/keeper.rs#L3119) and
  [upgrade watcher](../../../services/keeper/src/keeper.rs#L3693) check layout
  version 1. The watcher does not interpret an unsupported guard. The candidate
  path retains its existing unreadable-guard behavior (no off-chain latch
  verdict; transaction simulation remains authoritative), rather than consuming
  unknown fields. Restoration of a supported guard was independently tested
  against a pending real fork upgrade.
* [Ranking](../../../services/keeper/src/keeper.rs#L3240) keeps `scored` local.
  An RPC error or incomplete result returns before `finish_rank_batch`, leaving
  both the queued prefix and prior published candidates intact. `next_rank_batch`
  clones an immutable prefix; only `finish_rank_batch` drains it. Each group has
  at most 33 candidates times three keys, so RpcChain's 100-key splitting cannot
  separate a pool/vault/mint triple. Complete groups earlier in a failed batch
  are deliberately read again on retry. A short-result response is rejected by
  the explicit length check, though the focused executed fault injection uses
  RPC errors rather than a malformed short result.

## Executed evidence

All commands below ran under WSL from `/root/short.win-audit-review`, using Rust
1.98.1 and the shared build cache only as a cache. Cargo reported review-checkout
source paths for rebuilt local crates. The host workspace lockfile was not
rewritten. Before LiteSVM execution, the CI artifacts downloaded from run
39144291500 were independently hashed:

| Artifact | SHA-256 |
|---|---|
| release shortwin.so | `b88aa520c24301522b45871e73622a4d498603dc03a8f34ff8bbb557d7fa0647` |
| test-venue shortwin.so | `994ad12c468f465937af21080ec3c48678bb1f197ab143ab44f91f98ca56132b` |
| mock_cpamm.so | `4cf1700761782aece1a4e3463ce0b2b9f2afaed2ecf3d0a6362f115faf9c7ab9` |

The older existing ELFs in `/root/short.win/target` had different release and
test-venue hashes and were not used as program artifacts for these runs.

```sh
export CARGO_TARGET_DIR=/root/short.win/services/target
export SHORTWIN_SO=/root/short.win-audit-review/target/ci-39144291500/deploy/shortwin.so
export SHORTWIN_TEST_VENUE_SO=/root/short.win-audit-review/target/ci-39144291500/test-venue/deploy/shortwin.so
export MOCK_CPAMM_SO=/root/short.win-audit-review/target/ci-39144291500/deploy/mock_cpamm.so

cargo test --manifest-path services/Cargo.toml -p shortwin-services-common --lib lookup_table --locked
cargo test --manifest-path services/Cargo.toml -p shortwin-keeper-litesvm --test view_versions --locked
cargo test --manifest-path services/Cargo.toml -p shortwin-keeper-litesvm --test scenarios batching:: --locked
```

Results: **3/3**, **4/4**, and **4/4** passed, respectively. The batching run
reported the named failed-crank isolation test as passing, including assertions
for no general errors, precisely two healthy markets cranked, and precisely one
`crank_failed` with the Anchor ConstraintSeeds code.

The [separate keeper reviewer](independent-keeper-1.0.3.md) executed the full
`pool_select::` scenario group
against the same rehashed artifacts: **15/15 passed**, including
`ranking_rpc_failure_retries_every_pool_with_coherent_groups`. I independently
reviewed that scenario: its 0/40-junk-pool cases inject an error in the first or
second group, compare the complete queue to its pre-failure value, prohibit
partial score/candidate publication, and check every retried three-account group
and the retained discovery timestamp. This run is attributed to that reviewer,
rather than presented as a second execution by this reader reviewer.

## Independent probes and negative control

A temporary crate at `/tmp/shortwin-reader-probe` called the candidate's public
APIs. Its lockfile was seeded from `services/Cargo.lock`; comparing every package
name, version and checksum found no added/different dependency except the probe
package itself. It was run offline:

```sh
cargo test --manifest-path /tmp/shortwin-reader-probe/Cargo.toml --offline -- --nocapture
```

**2/2 passed.** The ALT probe exhausts lengths 0 through 184 with initialized tags,
then checks a four-address roundtrip and wrong owner/tag rejection. The guard
probe tests unsupported versions 0, 2 and 65535 on otherwise valid fork guard
accounts after a Raydium upgrade. No flag is attempted from the unsupported
guard; restoring layout version 1 flags the pending upgrade on the next slot.
This extends the committed typed-version suite to the upgrade watcher.

The exact preceding parser function was extracted from the baseline with
`git show b815269224ba0f5d946f49de7e113220c4067041:services/common/src/tx.rs`,
renamed to `old_parse_lookup_table`, and included in the temporary crate without
changing its body. The same ALT assertions were then switched to it:

```sh
ALT_NEGATIVE_CONTROL=1 cargo test --manifest-path /tmp/shortwin-reader-probe/Cargo.toml --offline independent_alt_size_matrix_and_populated_roundtrip -- --nocapture
```

Expected failure reproduced: **0 passed, 1 failed; exit 1**. At length 4,
the old parser returned success where rejection was required:
`left: true; right: false`. Candidate parsing had passed the entire matrix.
The temporary probe has a harmless unused-import warning; this does not affect
the tested parser or watcher.

For reproduction, create the following temporary manifest, copy the service
lockfile, extract the old function as described above into `src/old_parser.rs`,
and place the following source in `src/lib.rs`. No probe source was added to
production or committed test code.

```toml
[package]
name = "independent-reader-probe"
version = "0.0.0"
edition = "2024"

[dependencies]
shortwin = { path = "/root/short.win-audit-review/programs/shortwin", features = ["no-entrypoint"] }
shortwin-services-common = { path = "/root/short.win-audit-review/services/common" }
shortwin-keeper = { path = "/root/short.win-audit-review/services/keeper" }
shortwin-keeper-litesvm = { path = "/root/short.win-audit-review/services/keeper-litesvm" }
shortwin-litesvm-tests = { path = "/root/short.win-audit-review/tests/litesvm" }
anchor-lang = "=1.2.0"
anyhow = "1"
solana-message = "4.2.4"
```

```rust
use anchor_lang::prelude::Pubkey;
use shortwin_services_common::tx::{ALT_META_SIZE, ALT_PROGRAM_ID};
use shortwin_keeper::{KeeperConfig, pda};
use shortwin_keeper_litesvm::Harness;
use shortwin_litesvm_tests::{Env, fork};
use shortwin::constants::{VENUE_GUARD_VERSION, GUARD_RAYDIUM_V4};
use anyhow::{bail, Context};
use solana_message::AddressLookupTableAccount;

include!("old_parser.rs");

#[test]
fn independent_alt_size_matrix_and_populated_roundtrip() {
    let key = Pubkey::new_unique();
    let negative = std::env::var_os("ALT_NEGATIVE_CONTROL").is_some();
    let parse = if negative { old_parse_lookup_table } else { shortwin_services_common::tx::parse_lookup_table };
    for len in 0..=184 {
        let mut data = vec![0u8; len];
        if len >= 4 { data[..4].copy_from_slice(&1u32.to_le_bytes()); }
        let expected = len >= ALT_META_SIZE && (len - ALT_META_SIZE) % 32 == 0;
        assert_eq!(parse(key, &ALT_PROGRAM_ID, &data).is_ok(), expected, "length {len}");
    }
    let addresses: Vec<_> = (0..4).map(|_| Pubkey::new_unique()).collect();
    let mut data = vec![0u8; ALT_META_SIZE];
    data[..4].copy_from_slice(&1u32.to_le_bytes());
    for address in &addresses { data.extend_from_slice(address.as_ref()); }
    assert_eq!(parse(key, &ALT_PROGRAM_ID, &data).unwrap().addresses, addresses);
    assert!(parse(key, &Pubkey::new_unique(), &data).is_err());
    data[..4].copy_from_slice(&2u32.to_le_bytes());
    assert!(parse(key, &ALT_PROGRAM_ID, &data).is_err());
}

#[test]
fn independent_guard_unsupported_versions_then_recovery_flags_pending_upgrade() {
    for version in [0, VENUE_GUARD_VERSION + 1, u16::MAX] {
        let mut h = Harness::new(Env::ray_fork(), KeeperConfig::default());
        let initial = h.slot().clone();
        assert!(initial.errors.is_empty(), "{:?}", initial.errors);
        let expected_slot = fork::FIXTURE_GUARD_SLOTS[usize::from(GUARD_RAYDIUM_V4)] + 777;
        h.with_env(|env| {
            fork::set_deploy_slot(&mut env.svm, fork::RaydiumProgram::AmmV4, expected_slot);
            let key = pda::venue_guard();
            let mut account = env.svm.get_account(&key).unwrap();
            account.data[8..10].copy_from_slice(&version.to_le_bytes());
            env.svm.set_account(key, account).unwrap();
        });
        let unsupported = h.slot().clone();
        assert!(unsupported.flagged.is_empty(), "{unsupported:?}");
        assert!(unsupported.errors.iter().any(|e| e.contains("VenueGuard not found")), "{unsupported:?}");
        h.with_env(|env| {
            let key = pda::venue_guard();
            let mut account = env.svm.get_account(&key).unwrap();
            account.data[8..10].copy_from_slice(&VENUE_GUARD_VERSION.to_le_bytes());
            env.svm.set_account(key, account).unwrap();
        });
        let restored = h.slot().clone();
        assert!(restored.errors.is_empty(), "{restored:?}");
        assert_eq!(restored.flagged, vec![(GUARD_RAYDIUM_V4, expected_slot)], "{restored:?}");
    }
}

```
