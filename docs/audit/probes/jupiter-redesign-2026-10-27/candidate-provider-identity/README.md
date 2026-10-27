# Unadmitted candidate-provider identity snapshot

Read-only public RPC observations for the Switchboard candidate on 2026-10-27.
The two observations have different finalized context slots and are not an atomic
provider snapshot. Request commitment/timestamps are recorded operator evidence;
they are not cryptographically proved by JSON responses. The independent receipt
checks consistency, source layout and byte hashes, with these limits disclosed.

`programdata-response.json.gz` preserves the exact response bytes after gzip
decoding; its decoded SHA256 is
`a2de441cb71aedf4e4462483e6be4feb42c6fc1f45b1c58c23acb0f650a05039`.
The signing-key count is a layout observation, not active quorum or independent
operators. Program identities/hashes do not establish reviewed source provenance.
These observations do not promote an ELF to managed test fixtures, approve a
provider/authority/feed, or qualify safe verification. No transaction was sent.
