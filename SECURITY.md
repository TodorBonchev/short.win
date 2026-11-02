# Security policy

## Scope

This repository contains the Jupiter Router V2 pooled implementation with public program ID `BCPiRMTMoLNY5EHm2RYxoxJChgyrttkFX2Qe2Rnbe3jh`. Software identity does not establish a deployed mainnet instance. Reports about the lending program, pooled accounting, generated client, operator, history service and wallet interface are welcome.

Third-party Jupiter, DEX, SPL Token and Token-2022 programs are maintained by their respective authors. Interoperability fixtures are test inputs, not deployed short.win components.

## Private reporting

Contact **security@short.win** in English. Do not publish a vulnerability through an issue, pull request or discussion. No encryption key is published; request an encrypted channel in an initial message without sensitive details when needed.

Include the affected component and exact commit/program hash, impact and funds or liveness properties at risk, a local reproducer, and whether exploitation is known. An actual-SBF reproducer in `tests/runtime` is useful. Never test against funds or accounts you do not own.

Acknowledgement, triage and coordinated disclosure are best effort. We keep the reporter informed, arrange disclosure after remediation, and credit the reporter unless they request otherwise.

## Safe harbour

We will not pursue or support legal action against research done in good faith under this policy: research that avoids privacy violations, the loss or destruction of data and the interruption or degradation of the service; that never moves or puts at risk funds that are not your own (reproduce on a local validator or a LiteSVM fork, never against other users on mainnet); that does not exploit an issue beyond what is needed to show it; and that reports the issue to us promptly and keeps it confidential until a fix is deployed.

## Audit and rewards status

No external security audit or funded bug bounty is represented as active, and no reward is guaranteed. The intended initial phase uses borrowing caps before an external audit and funded bounty; deposits and market count remain uncapped. This policy does not establish deployment approval, operating readiness, an audit booking or a bounty commitment. [Operations](docs/OPERATIONS.md) and [policy proposals](config/README.md) describe the concrete commissioning work.
