# Candidate oracle SDK review — 2026-10-27

This is an independent AI-assisted engineering review of the selected architecture's
candidate dependency, not an external-firm audit or a protocol finding disposition.
Signed Jupiter-derived feeds remain selected; Switchboard and its exact deployment,
trust settings and integration remain unqualified for production.

## Actual native evidence

- Initial manual [run40476628349](https://github.com/TodorBonchev/short.win/actions/runs/40476628349)
  failed only in our two ambiguous borrow lifetimes (E0106). This was a prototype
  error, not proof of SDK incompatibility. Artifact, source, lock and280 active
  registry archives were independently verified and retained.
- Repaired manual [run40478037582](https://github.com/TodorBonchev/short.win/actions/runs/40478037582)
  on c70aa259c7c68ac6eeebe37b8c9f9f61b2499ff8 passed11 Rust tests, two runner
  fault tests, fmt and strict Clippy. The exact independently verified281-package
  standalone lock was reused with --locked; production locks stayed unchanged.
- This establishes native type/component compatibility with published
  switchboard-on-demand0.13.0, features no-entrypoint+solana-v3, actual bridge
  solana-program4.0.0, Anchor1.2.0, AccountInfo3.1.1, Clock3.1.1,
  Instruction3.4.1 and Pubkey4.2.1. It does not establish safe runtime verification.

The [retained receipts](probes/jupiter-redesign-2026-10-27/native-sdk/) contain exact
GitHub artifact/source/registry digests and independent reviewer attribution. The
published SDK archive SHA256 is bc7a36809592252cc73f9ec01623b89628654a2fba00dbf5c339c286eb040d40.

## Source-level qualification blockers

**JUP-SDK-01 — slot-hash slice extent.** Both implementations of
quote_verifier.rs::find_slothash_in_sysvar transmute &slothash_data[8..] directly
to &[SlotHash]. SlotHash is repr(C), size40, alignment8. A normal512-entry payload
has20480 bytes; the transmute preserves20480 as the new element count instead of512,
so the resulting slice nominally describes819200 bytes. The backing extent required
for that slice has not been established. Input-format bounds cannot repair this
construction. The exact source and independent reasoning are retained in the
source-review receipt. No SBF exploit, live crash or loss was demonstrated.

**JUP-SDK-02 — original instruction-buffer alignment.** The SDK instruction helper
extracts a slice at its original sysvar byte offset. Its Ed25519 parser directly
dereferences repr(C) u16 offset records, requiring alignment2. An odd-sized preceding
instruction can place the Ed25519 instruction data at an odd byte offset. A securely
loaded, explicitly aligned local copy could address this condition, but not SDK-01.
The public-key/hash comparison helpers use read_unaligned; no unnecessary8-byte
alignment restriction is imposed on their wire fields.

Both are production qualification blockers for this candidate verifier. They are
source-level findings, not claims of tested exploitability or official advisory IDs.
Do not silently promote the ABI pass, patch a registry cache, relax checks or replace
this with an unreviewed custom signer/verifier. A reviewed upstream/local repair or
separately reviewed safe consumer needs its own source pin, regressions and SBF proof.

## Safe follow-up and current behavior

The dependency-free safe envelope prevalidator bounds the full offset table, local
signature/key/shared-message regions, explicit transaction indices, complete packed
feed records, exact suffix/version, distinct signing keys/queue indices/feed IDs and
signed sample counts. It returns FORMAT ONLY. The runtime preflight uses checked
sysvar loads and requires a preceding Ed25519 instruction, then returns unit.

The experimental read_verified_raw_output entry point now unconditionally returns
ProviderVerifierQuarantined after mapping/account checks. It has no authenticated
value path and calls no SDK verify method. Synthetic financial-policy helpers and
owned-parser comparisons exist only in tests. No production program/service links
this standalone experiment. Native verification of these later changes needs its
own manual receipt; the earlier11-test pass does not cover them.

Authentic managed-update bytes, live queue/authority keys, quorum independence,
source freshness, fixed-q search/lower-reference policy, SBF verification, resource
costs and funded operation remain open. Valid-attestation successes and SBF builds
in this experiment remain0. No feed, signature authority, payment or transaction
was created by this work.
