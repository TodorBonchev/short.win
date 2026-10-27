# Candidate-only Jupiter execution identity

This is retained research input for the isolated Jupiter SBF probe. It is **not**
a promoted `tests/fixtures` dependency baseline, a source-provenance proof, a live
route snapshot or a mainnet compatibility/admission approval. The pinned program's
source build is not reproduced here; runtime-byte identity only is established.

Original finalized read: 2026-10-27T08:08:21Z to08:08:22Z, public mainnet RPC,
context slot461997164, Jupiter deploy slot459479624. The exact decoded raw RPC SHA256
is `c611ab92f91fd6bf4ce415054a1e72cad3a3abe1bef5c2bb22f1e574d7a3c3ff`.
The gzip is deterministic packaging of those bytes; verify both its packaged checksum
and the fixed decoded anchor. The previous identity record is retained verbatim.

The account list is program JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4,
ProgramData4Ec7ZxZS6Sbdg5UGSLHbAnM7GQHp2eFd4KYWRexAipQT and its on-chain IDL.
Reconstruct the ELF after the verified45-byte upgradeable ProgramData header;
retain its original zero padding for loading. Identity's trimmed ELF hash removes
trailing zero padding; do not substitute a newly downloaded binary under this pin.

A successful actual routed test must independently identify its DEX accounts,
DEX/token ELF inputs, bank features, amount/rent deltas and resource use. Neither
this archive nor a malformed-instruction rejection meets that gate. Production
promotion requires compatibility review and a new final-runtime qualification.
