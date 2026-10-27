# Reproducible incremental program bytes — CI40557150355

Source head:b7fcfc3e7437bc00d0abe34a241e13636de154c0. The workflow
checked out its PR merge tree576b529a37f9c4543ab3e73250184f22b55e3cf5;
the PR base is an ancestor of the head. The downloaded Anchor and verifier
archives matched their GitHub SHA-256 digests; all member hashes were retained.
Anchor and verifier release ELF bytes match exactly:

- Full release SHA256:392b6ac13811e599bb2b3c80ef175382ff7abb27bdcb4c2801b73e6eea82abc9
- Executable hash after trailing-zero trim:95c151e4081835772382dd48d0732c140eadca8f32c568991fd99d85c7dab7a2
- IDL SHA256:3fd8a6207f5bc25763c0f64c6d5e2bf8301c206aa6f952dc0e4c04da2099099c

The IDL exactly equals the generated-client baseline. This does not mark
the whole CI run green: its stale fuzzer catalog and committed Repay fixture
were repaired separately. It does not qualify the final proposed1.1.0
financial runtime, nor changed fuzzer/fixture inputs or mainnet operations.
