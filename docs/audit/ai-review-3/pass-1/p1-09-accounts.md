Pass 1 — specialist 9: account validation
Frozen source: b815269224ba0f5d946f49de7e113220c4067041
Auditor: rust-auditor 6c41e1d46386de7fd9b1fa48c847e876ac8b2676
Result: 0 FINDINGs; 3 LEADs, all hardening-only.
Method: static source review. No build, test, or runtime reproduction. No audited repository writes.
Coverage and all twelve no-signer dispositions: p1-09-coverage.txt.
These leads do not assert a present exploit. No finding requires a fix diff.

LEAD | program: shortwin | function: deposit | bug_class: hardening-pool-vault-fields-unchecked | group_key: shortwin | deposit | hardening-pool-vault-fields-unchecked
title: Pool vault loads rely on creation-time mint, authority, and program checks
file: programs/shortwin/src/instructions/deposit.rs:94
locations: programs/shortwin/src/instructions/withdraw.rs:127; programs/shortwin/src/instructions/open_short.rs:167; programs/shortwin/src/instructions/close_short.rs:102; programs/shortwin/src/instructions/liquidate.rs:140; programs/shortwin/src/instructions/expire.rs:101
boundary: A caller supplies the pool_vault InterfaceAccount to an instruction that reads its balance or transfers its tokens.
code_smells: The constraints bind pool_vault to the market PDA and stored key, but omit token::mint, token::authority, and token::token_program on that account. InterfaceAccount accepts either token program. The creation path establishes the missing relationships.
proof: deposit.rs:94-99 checks mut and seeds=[POOL_VAULT_SEED, market.key()] with the stored bump. Its mint uses mint::token_program at :85 and owner_token_account uses token::mint and token::token_program at :90-91. withdraw.rs:127-132 repeats the weaker pool_vault constraint. create_market.rs:281-349 creates the canonical PDA under the chosen token program and initializes its mint and market authority. Anchor SPL 1.2.0 token_interface.rs:15,31-35 permits both supported owners.
current_blocker: The current program creates this canonical vault itself. The caller cannot replace it with another key or change its mint. Token CPIs also validate the operating token program and authority. No current account substitution or balance-credit exploit was proved.
description: A future vault importer could let callers credit the wrong asset because loaders do not check the vault mint, authority, and token program.
fix: Add token::mint, token::authority, and token::token_program constraints to each pool_vault InterfaceAccount.

LEAD | program: shortwin | function: parse | bug_class: hardening-vault-authority-unchecked | group_key: shortwin | parse | hardening-vault-authority-unchecked
title: Raydium read loaders omit vault-authority checks present at adoption
file: programs/shortwin/src/venues/raydium_v4.rs:146
locations: programs/shortwin/src/venues/raydium_cpmm.rs:169
boundary: ReadSets for an already adopted Raydium pool feed permissionless price observations and trade decisions.
code_smells: Both parse functions validate pool owner/layout, vault keys, token program owners, and mints, but omit each token account's authority. Both verify functions check that authority at adoption.
proof: raydium_v4.rs:160-164 loads the two token accounts and compares their mints; :304-310 additionally requires t.owner == RAYDIUM_V4_AUTHORITY during verify. raydium_cpmm.rs:185-190 loads both vaults and compares their mints; :393-403 additionally requires t.owner == RAYDIUM_CPMM_AUTHORITY during verify. read_state uses parse at raydium_v4.rs:214-221 and raydium_cpmm.rs:289-299.
current_blocker: Adoption validates the authorities, stored keys bind the later ReadSet, and outside callers cannot change the venue-controlled token authorities. No current attacker-controlled authority transition was proved.
description: A future venue authority change could let callers refresh price state from vaults that the configured swap authority cannot spend.
fix: Apply the adoption-time vault-authority checks in each shared parse function.

LEAD | program: shortwin | function: init_uva | bug_class: hardening-existing-uva-validation | group_key: shortwin | init_uva | hardening-existing-uva-validation
title: Existing Pump user-volume accounts bypass trade-time decoding at listing
file: programs/shortwin/src/venues/pump_curve.rs:1047
locations: programs/shortwin/src/venues/pump_amm.rs:1161
boundary: create_market calls each venue's init_uva helper for its derived trader user-volume accumulator.
code_smells: The already-existing account branch returns success after an owner check and nonempty-data check. The trading path decodes the discriminator and compares the stored user with the trader.
proof: pump_curve.rs:1041 pins the key, but :1047-1048 returns Ok for owner==PUMP_PROGRAM_ID and nonempty data. pump_amm.rs:1155 pins the key, but :1161-1162 has the same early return. Compare pump_curve.rs:766-780 and pump_amm.rs:469-478, which decode the accumulator and validate its user. The raw decoder checks owner and discriminator at crates/shortwin-venue-state/src/pump.rs:765-773; the PumpSwap wrapper checks its own owner at pump_amm.rs:497-500.
current_blocker: The caller supplies a derived venue PDA and cannot assign arbitrary bytes to a Pump-owned account there. The creator signs create_market; these helpers are not externally dispatched instructions. No current unauthorized write or missing-signer path was proved.
description: A future venue layout change could let a caller list a market with an existing UVA that later fails the stricter trade checks.
fix: Decode each existing UVA and compare its user with the trader before returning success.
