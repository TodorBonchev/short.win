LEAD | program: shortwin-venue-state | function: read_base | bug_class: hardening-mint-type-unchecked | group_key: shortwin-venue-state | read_base | hardening-mint-type-unchecked
file: crates/shortwin-venue-state/src/mint.rs:169
title: The base mint reader omits account type checks.
code_smells: The reader accepts every length above 81 bytes and does not inspect the Token-2022 account type.
description: A future caller that skips mint identity checks can accept token-account bytes as mint supply; current callers use constrained keys.
source_argument: read_base checks only owner in {TOKEN_PROGRAM, TOKEN_2022_PROGRAM}, data.len() >= 82, and base_of. In contrast, mint::check rejects legacy lengths other than 82, length 355, nonzero extension padding, and account type other than Mint. The program's authoritative check_mint_safety uses StateWithExtensions<Mint>::unpack, which performs the structural checks. A 166-byte buffer with valid base COption tags, byte 45 = 1 and byte 165 = 2 passes read_base but fails the strict type check. This is a parser counterexample, not an attacker-created, Token-2022-owned account.
callers: programs/shortwin/src/venues/pump_amm.rs:419 lp_mint_supply calls read_base without an additional length or type check; venues/mod.rs:1091 mint_supply does the same. Raydium's lp_supply separately requires exactly 82 bytes. PumpSwap ties the LP key to pool.lp_mint, and the named mint is constrained elsewhere.
verification_gap: This pass found no way for an outsider to replace an accepted named mint or canonical LP mint with the counterexample. Current key constraints prevent the proposed substitution.
minimal_fix: Add mint account length, padding, and type checks to read_base without applying the extension allowlist or freeze-authority policy to LP mints.

LEAD | program: shortwin | function: parse | bug_class: hardening-vault-authority-unchecked | group_key: shortwin | parse | hardening-vault-authority-unchecked
file: programs/shortwin/src/venues/raydium_v4.rs:146; programs/shortwin/src/venues/raydium_cpmm.rs:169
title: The Raydium state readers omit vault authority checks.
code_smells: Both parsers validate vault keys, token-program owners, and mints but omit the token authorities that their candidate verifiers check.
description: A future venue change can leave the readers to report an executable pool with invalid vault authorities; this pass found no current authority-change path.
source_argument: raydium_v4::parse accepts coin and pc when token_of succeeds and their mints match the pool. raydium_cpmm::parse does the same for v0 and v1. Their verify functions additionally require t.owner == the constant venue authority. read_state subsequently calls parse and state_of without that requirement. PumpSwap's vault_of performs the authority comparison on each read. These are separate loaders with different validation strength.
verification_gap: The recorded vault keys were checked at adoption. This pass did not prove that the deployed venues expose a way to change those vault authorities. A venue CPI would also reject an invalid authority rather than authorize token theft.
minimal_fix: Require both decoded vault token authorities to equal the corresponding Raydium authority inside each parse function.

LEAD | program: shortwin | function: math_tiers | bug_class: unbounded-tier-allocation | group_key: shortwin | math_tiers | unbounded-tier-allocation
file: programs/shortwin/src/venues/pump_curve.rs:404; programs/shortwin/src/venues/pump_amm.rs:608
title: The adapters copy each fee tier into the program heap.
code_smells: The shared fee reader bounds tier vectors by account length, while both adapters collect every tier into a Vec without a local count limit.
description: A fee-config expansion can exhaust the program heap before a reader returns no route; this pass did not prove a reachable failing tier count.
source_argument: pump::tiers_at checks offset arithmetic and slice bounds but no tier-count ceiling. pump_curve::pricing_inputs collects the curve tiers and uses Rc::from(math_tiers(pfc.fee_tiers)) for the pool tiers. pump_amm::read_state also collects its whole tier vector. open_short reads venue state before its prologue and again after its sell. The linked solana-program-entrypoint 3.1.1 defaults to HEAP_LENGTH = 32 * 1024, and BumpAllocator::dealloc is a no-op. Dropped vectors therefore do not restore heap space during the instruction. The documented 4097-byte FeeConfig allocation can structurally contain 100 forty-byte tiers plus both vector lengths and trailing fees, but that structural capacity does not prove the venue's setter permits that count.
verification_gap: No test or build ran. This pass did not establish the deployed fee-config setter's maximum tier count, the maximum allocation trace of an instruction, or a count that exhausts the heap. The current fixture schedules were not assessed as failing.
minimal_fix: Stream selection through the shared FeeTiers reader where possible and establish a measured bound for schedules retained by curve risk quotes.

LEAD | program: shortwin | function: elf_fingerprint_of | bug_class: hardening-truncated-elf-digest | group_key: shortwin | elf_fingerprint_of | hardening-truncated-elf-digest
file: programs/shortwin/src/state/venue_guard.rs:48
title: The upgrade latch stores only twelve bytes of each hash.
code_smells: The guard stores 12 of 32 SHA-256 bytes and compares only that prefix after an extension.
description: A venue authority can bypass the latch with paired ELF hashes that share twelve bytes; this pass found no such collision.
source_argument: GUARD_ELF_LEN = 12; elf_fingerprint_of copies only that prefix; VenueGuard.guard_elf holds four 12-byte arrays. flag_venue_upgraded::is_upgrade treats a changed ProgramData length as unchanged code when the new trimmed hash has the stored prefix. Thus an authority that controls two candidate ELFs can search a 96-bit collision before the first review, then combine the second upgrade with a later extension. A chosen pair has the generic birthday scale of 2^48 hashes, whereas replacing a fixed reviewed ELF has the much larger 2^96 second-preimage scale. Neither is an actual collision proof.
verification_gap: No colliding executable pair, practical work estimate, or deployed bypass was demonstrated. This is a hardening lead only. The source identifies AIR2-07 as an unresolved full-digest migration; that disclosure does not establish closure.
minimal_fix: Store and compare the full SHA-256 digest through an explicit account-layout migration.

LEAD | program: shortwin | function: exit_risk_cap_bps | bug_class: live-supply-risk-fee | group_key: shortwin | exit_risk_cap_bps | live-supply-risk-fee
file: programs/shortwin/src/venues/mod.rs:822; programs/shortwin/src/venues/pump_amm.rs:702
title: A supply burn can raise the PumpSwap risk fee.
code_smells: PumpSwap resolves both its live and EMA fee tiers from the current mint supply; the EMA price does not fix the supply input.
description: A holder can burn tokens before liquidation and raise its risk fee; this pass did not prove a profitable liquidation.
source_argument: pump_amm::read_state sets PammFeeSchedule.base_supply to its supplied live mint supply. It resolves fee_ema using quote = floor(P_ema * base / WAD) and that same schedule. PoolVenue.fee_ppm_at_ema feeds exit_risk_cap_bps, which limits risk quotes to min(hard_cap_bps, ceil(fee_ppm_at_ema / 100)). A burn lowers supply and market cap without changing P_ema, so a decreasing fee schedule can raise this cap. The source's exit_risk_cap_bps comment explicitly retains the burn residual as KI-72. The exposure is current for multi-tier PumpSwap schedules; the description does not assert an unexamined size or profit bound.
verification_gap: This pass did not construct or execute a concrete healthy position, burn, liquidation, and net-profit trace. The existing disclosure and its referenced test were located but were not treated as an independent reproduced proof.
minimal_fix: Resolve the risk-view fee from a supply value fixed before the current observation interval while execution quotes retain the live supply.
known_status: Source-disclosed KI-72; retained as a current source-supported lead, not closed by disclosure.

LEAD | program: shortwin | function: pricing_inputs | bug_class: live-supply-synthetic-risk-fee | group_key: shortwin | pricing_inputs | live-supply-synthetic-risk-fee
file: programs/shortwin/src/venues/pump_curve.rs:470
title: A burn can raise the curve's synthetic risk fee.
code_smells: The live curve's synthetic fee reads the current mint supply, and curve risk quotes retain it under the position's hard cap.
description: A holder can raise the synthetic risk fee after a burn under multiple tiers; this pass did not prove a liquidation.
source_argument: pricing_inputs resolves synthetic_fees with fees::pump_curve_at_supply(live.vt, live.vq, s, &schedule), where s is the live mint supply. CurvePricingInputs::curve_pricing forwards that value into CurvePricing.synthetic. Quoter::capped caps the resulting pricing at the position's cap but does not fix the market-cap supply. Quoter::buy_live and buy_xy use the synthetic route past rT. A supply burn can therefore change the fee used on the live and hypothetical risk paths in the same transaction under a multi-tier curve schedule. This differs from PumpSwap's exit cap and is not closed by its EMA cap.
verification_gap: The code documents a single flat curve tier in the pinned venue configuration, so this effect requires a fee-schedule change. This pass did not construct a liquidation at a tier boundary or measure a profitable burn.
minimal_fix: Resolve synthetic risk fees from a supply value fixed before the current observation interval while execution pricing follows the live supply.
known_status: Source-disclosed KI-77(a); conditional on a multi-tier curve fee configuration, not claimed as an active flat-schedule exploit.

LEAD | program: shortwin | function: pricing_inputs | bug_class: synthetic-fee-gate-basis-mismatch | group_key: shortwin | pricing_inputs | synthetic-fee-gate-basis-mismatch
file: programs/shortwin/src/venues/pump_curve.rs:454; programs/shortwin/src/venues/mod.rs:796
title: The curve fee gates use a different supply basis.
code_smells: The ordinary curve fee uses the fixed 10^15 supply, while an exit past rT resolves its synthetic fee from the live supply.
description: The curve fee gates can admit a lower fee than a synthetic exit pays; this requires a tier boundary between the two supply bases.
source_argument: pricing_inputs first obtains fees through fees::pump_curve, whose documented supply basis is PUMP_CURVE_MCAP_SUPPLY = 10^15, and separately obtains synthetic_fees through pump_curve_at_supply. read_state writes st.fee_ppm from inputs.fees; cap_fee_ppm compares that ordinary fee with the ordinary fee at P_ema. These fee-gate inputs do not select the synthetic fee when the position's buyback crosses rT. Thus a burned supply and a multi-tier schedule can produce ordinary fee phi below synthetic fee phi_s; the route/open fee checks use phi while the synthetic exit uses phi_s. The source explicitly documents this retained distinction as KI-77(b).
verification_gap: This pass did not construct a current on-chain multi-tier curve schedule or a complete open/exit trace. The pinned flat curve schedule removes the difference today. The reported numerical jump in the old disclosure was not independently verified here.
minimal_fix: Make fee gates account for the fee of each buyback route that an admitted position can require, while retaining conservative execution quotes.
known_status: Source-disclosed KI-77(b); retained separately from the burn's effect on risk fees because changing the supply snapshot alone does not align the gates.

LEAD | program: shortwin | function: read_state_boxed_for_exit | bug_class: curve-risk-fee-at-spot | group_key: shortwin | read_state_boxed_for_exit | curve-risk-fee-at-spot
file: programs/shortwin/src/venues/mod.rs:1078; programs/shortwin/src/venues/pump_curve.rs:454
title: Curve exit risk quotes retain the fee resolved at spot.
code_smells: read_state_boxed_for_exit enables ema_fee only for PumpSwap; ordinary curve risk quotes reuse fees resolved from the live virtual reserves.
description: A keeper can raise the curve risk fee with a sale under multiple tiers; this pass did not prove a liquidation.
source_argument: read_state_boxed_for_exit sets ema_fee = kind == VenueKind::PumpSwap. pump_curve::pricing_inputs always resolves its ordinary fee from live.vt and live.vq; without ema_fee it leaves fee_ppm_at_ema at u32::MAX. exit_risk_cap_bps has only the PumpSwap branch. Curve Quoter::buy_xy reprojects cached reserves but retains c.pricing's already-resolved fee, so an earlier sale can move the fee tier used for both live and hypothetical risk quotes. A fixed supply prevents a burn from changing this ordinary fee basis, but does not prevent a sale from changing its reserve ratio. This candidate concerns the ordinary curve fee and is separate from KI-77's synthetic live-supply fee.
verification_gap: The pinned curve schedule is flat. This pass did not establish a full liquidation trace under a venue-admin update to multiple tiers or prove the absence of every downstream assessment guard.
minimal_fix: Resolve and cap ordinary curve risk fees at the stored EMA market cap, separately from the synthetic execution fee.
