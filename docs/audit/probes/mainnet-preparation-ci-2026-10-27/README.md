# Initial checkpoint CI and lockfile repair

Checkpoint `07a34d2ad8f9726184a5a534ee7cf3de948e6115` passed both focused
Jupiter test-probe runs (push40556146369 and PR40556152856). The push
artifact's GitHub SHA-256 digest and all eight member hashes were verified;
its identity binds the checked source to that commit. This is test-only
functional execution evidence, not completion of the provider-stopped
review scope or a production adapter qualification.

Initial ordinary PR CI40556153080 failed `deny` because a benchmark's
locked path dependencies omitted the new local report crate. Five bench
locks were repaired with only that source-less package/edge added. All
external resolved package objects stayed unchanged. Locked offline metadata
passes all twelve active manifests; the unchanged historical AIR2 packet
still lacks its archived sibling math path. M1 all-targets check passes.
The corrected source requires a new ordinary CI run; no old result is
reported as a full green gate.
